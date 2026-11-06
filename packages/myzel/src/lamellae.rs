use glam::{Vec2, Vec3};
use nodarium_utils::{curve, geometry::orthogonal_to, Point};
use std::collections::HashMap;
use std::f32::consts::{PI, TAU};

use crate::mesh::{pack_rgba, Mesh};
use crate::noise::value_noise;
use crate::pattern::Pattern;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LamellaForm {
    /// Thin plates with flat sides and a rounded edge, like the gills of a field mushroom
    Plates,
    /// Blunt folds as wide as they are deep, like the false gills of a chanterelle
    Ridges,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LamellaeParams {
    pub form: LamellaForm,
    /// How far the gills reach down from the underside of the cap where they are deepest
    pub depth: f32,
    /// Share of the depth from the stipe (0) to the margin (1)
    pub depth_curve: Vec<f32>,
    /// Thickness of a gill where it meets the cap
    pub thickness: f32,
    /// Gap between the stipe and the inner ends of the gills, along the underside
    pub free_distance: f32,
    /// How far the gills that reach the stipe run down along it
    pub decurrent: f32,
    /// 0 keeps the gill edges straight, 1 lets them wave sideways by half a gill spacing
    pub edge_wave: f32,
    /// How much depth and thickness vary between neighbouring gills and along them
    pub irregularity: f32,
    /// Share of the gills that end close to a neighbour which join it instead: as a fork
    /// where a short gill meets a long one, as a cross vein where two run side by side
    pub forking: f32,
    pub seed: u32,
}

impl Default for LamellaeParams {
    fn default() -> Self {
        LamellaeParams {
            form: LamellaForm::Plates,
            depth: 0.006,
            depth_curve: vec![0.0, 0.3, 0.35, 1.0, 0.7, 0.85, 1.0, 0.2],
            thickness: 0.0005,
            free_distance: 0.001,
            decurrent: 0.0,
            edge_wave: 0.3,
            irregularity: 0.4,
            forking: 0.2,
            seed: 0,
        }
    }
}

/// Points every cap ray is resampled to along its underside
const PROFILE_SAMPLES: usize = 64;
/// Neighbouring rays that drift further apart than their angles keep them, by more than this
/// many times the sum of their flesh half thicknesses, leave a notch in the cap. No gill hangs
/// there.
const NOTCH: f32 = 1.0;
const NOTCH_SLACK: f32 = 0.05;
/// How deep, as a share of the flesh under the ray, a gill starts inside the cap, so the two
/// surfaces overlap instead of leaving a seam
const EMBED: f32 = 0.6;
/// A gill that starts in a gap of the pattern slopes up to full depth over this many spacings
const FREE_END_TAPER: f32 = 1.2;
/// The inner end of a gill that stops short of the stipe rounds off over this many spacings
const INNER_END_TAPER: f32 = 0.5;
/// Lines of the pattern shorter than this many spacings that end in a gap are noise, not gills
const MIN_SPUR: f32 = 0.6;
const MIN_ISLAND: f32 = 1.0;
/// A line that ends this many spacings or closer to another one can join it
const JOIN_REACH: f32 = 0.9;
/// A stripe that ends this many spacings or closer inside the margin runs on to it
const MARGIN_REACH: f32 = 1.5;
/// Passes of smoothing along a gill, at its points
const SMOOTHING: usize = 6;
/// Gill points along the pattern, per spacing
const SAMPLES_PER_SPACING: f32 = 4.0;
/// How much the irregularity varies depth and thickness, up or down
const DEPTH_VARIATION: f32 = 0.45;
const THICKNESS_VARIATION: f32 = 0.5;
/// Wavelength of the variation between gills and of the edge waves, in spacings
const VARIATION_WAVELENGTH: f32 = 2.5;
const WAVE_WAVELENGTH: f32 = 1.8;
/// A plate narrows to this share of its thickness at the edge before it rounds off
const PLATE_EDGE_SHARE: f32 = 0.6;
/// Heights of the cross section, as a share of the depth, on each side of the gill
const SECTION: [f32; 7] = [0.0, 0.2, 0.45, 0.7, 0.85, 0.94, 0.985];

/// The underside of one cap ray: from where it leaves the stipe to the margin, as distance from
/// the axis, height over the hinge, angle off the ray's own angle and flesh half thickness
#[derive(Clone)]
struct Profile {
    angle: f32,
    samples: Vec<[f32; 4]>,
    length: f32,
}

/// The underside of a cap, read from its rays, so gills follow the cap however it is shaped,
/// opened or bent
#[derive(Clone)]
pub struct CapUnderside {
    pub centre: Vec3,
    pub axis: Vec3,
    side: Vec3,
    around: Vec3,
    pub stipe_radius: f32,
    /// The stipe the cap grows on
    pub id: i32,
    profiles: Vec<Profile>,
    /// The points of the stipe from its top down, as position and radius
    stipe: Vec<(Vec3, f32)>,
}

/// A place on the underside
#[derive(Clone, Copy, Debug)]
pub struct UndersidePoint {
    pub position: Vec3,
    /// Pointing away from the flesh, the way the gills hang
    pub down: Vec3,
    /// Half the thickness of the flesh above it
    pub half: f32,
    /// Distance from the stipe along the underside
    pub distance: f32,
    /// False in a notch of the cap, where its rays drift apart and leave a gap
    pub covered: bool,
}

fn wrap_angle(a: f32) -> f32 {
    (a + PI).rem_euclid(TAU) - PI
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    if edge1 <= edge0 {
        return if x < edge0 { 0.0 } else { 1.0 };
    }
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl CapUnderside {
    /// The underside of the rays that start inside the stipe, `rays` are their points as
    /// x, y, z, radius. The others start out on the cap and add nothing to its shape.
    pub fn new(hinge: &Point, rays: &[&[f32]]) -> Option<CapUnderside> {
        let axis = hinge.tangent.try_normalize()?;
        let centre = hinge.position;
        let side = orthogonal_to(axis);
        let around = axis.cross(side);
        let stipe_radius = hinge.scale.abs();
        let cylindrical = |p: Vec3| {
            let d = p - centre;
            let h = d.dot(axis);
            let q = d - axis * h;
            (q.length(), h, q.dot(around).atan2(q.dot(side)))
        };

        let mut profiles: Vec<Profile> = Vec::new();
        for ray in rays {
            let n = ray.len() / 4;
            if n < 3 {
                continue;
            }
            let c: Vec<(f32, f32, f32)> =
                (0..n).map(|i| cylindrical(Vec3::new(ray[i * 4], ray[i * 4 + 1], ray[i * 4 + 2]))).collect();
            if c[0].0 >= stipe_radius {
                continue;
            }
            // the root crosses the axis, the ray runs out from the point closest to it
            let first = (0..n).take_while(|&i| c[i].0 < stipe_radius).min_by(|&a, &b| c[a].0.total_cmp(&c[b].0)).unwrap_or(0);
            let under: Vec<[f32; 4]> = (first..n)
                .map(|i| {
                    let (a, b) = (c[i.saturating_sub(1).max(first)], c[(i + 1).min(n - 1)]);
                    let tangent = Vec2::new(b.0 - a.0, b.1 - a.1).try_normalize().unwrap_or(Vec2::X);
                    let half = ray[i * 4 + 3].abs();
                    let q = Vec2::new(c[i].0, c[i].1) - tangent.perp() * half;
                    [q.x, q.y, c[i].2, half]
                })
                .collect();
            let Some(k) = under.iter().position(|u| u[0] >= stipe_radius) else { continue };
            if k == 0 || k + 2 > under.len() {
                continue;
            }
            // where the underside leaves the stipe
            let (a, b) = (under[k - 1], under[k]);
            let t = ((stipe_radius - a[0]) / (b[0] - a[0])).clamp(0.0, 1.0);
            let mut start = [0.0; 4];
            for j in 0..4 {
                start[j] = a[j] + (b[j] - a[j]) * t;
            }
            let angle = start[2];
            let line: Vec<[f32; 4]> = std::iter::once(start)
                .chain(under[k..].iter().copied())
                .map(|u| [u[0], u[1], wrap_angle(u[2] - angle), u[3]])
                .collect();
            let position = |u: &[f32; 4]| {
                let dir = Vec2::from_angle(angle + u[2]);
                (side * dir.x + around * dir.y) * u[0] + axis * u[1]
            };
            let mut along = vec![0.0f32; line.len()];
            for i in 1..line.len() {
                along[i] = along[i - 1] + position(&line[i]).distance(position(&line[i - 1]));
            }
            let length = along[line.len() - 1];
            if length <= 0.0 {
                continue;
            }
            let mut j = 0;
            let samples = (0..PROFILE_SAMPLES)
                .map(|s| {
                    let target = length * s as f32 / (PROFILE_SAMPLES - 1) as f32;
                    while j + 2 < line.len() && along[j + 1] < target {
                        j += 1;
                    }
                    let span = along[j + 1] - along[j];
                    let t = if span > 0.0 { ((target - along[j]) / span).clamp(0.0, 1.0) } else { 0.0 };
                    let mut out = [0.0; 4];
                    for k in 0..4 {
                        out[k] = line[j][k] + (line[j + 1][k] - line[j][k]) * t;
                    }
                    out
                })
                .collect();
            profiles.push(Profile { angle: angle.rem_euclid(TAU), samples, length });
        }
        if profiles.is_empty() {
            return None;
        }
        profiles.sort_by(|a, b| a.angle.total_cmp(&b.angle));
        Some(CapUnderside { centre, axis, side, around, stipe_radius, id: hinge.parent_id, profiles, stipe: Vec::new() })
    }

    /// The stipe as its points x, y, z, radius from the bottom up, decurrent gills run down it
    pub fn with_stipe(mut self, points: &[f32]) -> Self {
        self.stipe = points.chunks_exact(4).rev().map(|p| (Vec3::new(p[0], p[1], p[2]), p[3].abs())).collect();
        self
    }

    /// The surface of the stipe `distance` below its top on the side `towards` points at, and
    /// the way out of it. Without a stipe it runs straight down from the hinge.
    fn down_the_stipe(&self, distance: f32, towards: Vec3) -> (Vec3, Vec3) {
        let outward = |axis: Vec3| (towards - axis * towards.dot(axis)).try_normalize().unwrap_or(self.side);
        let mut walked = 0.0;
        for w in self.stipe.windows(2) {
            let ((a, ra), (b, rb)) = (w[0], w[1]);
            let step = a.distance(b);
            if walked + step >= distance && step > 0.0 {
                let t = (distance - walked) / step;
                let out = outward((a - b) / step);
                return (a.lerp(b, t) + out * (ra + (rb - ra) * t), out);
            }
            walked += step;
        }
        let out = outward(self.axis);
        (self.centre + out * self.stipe_radius - self.axis * distance, out)
    }

    /// Distance from the axis, height, angle offset, half flesh and length of the underside at
    /// `f` from the stipe (0) to the margin (1) and `angle` around the axis
    fn profile_at(&self, f: f32, angle: f32) -> ([f32; 4], f32) {
        let angle = angle.rem_euclid(TAU);
        let n = self.profiles.len();
        let next = self.profiles.iter().position(|p| p.angle > angle).unwrap_or(0);
        let previous = (next + n - 1) % n;
        let (a, b) = (&self.profiles[previous], &self.profiles[next]);
        let gap = (b.angle - a.angle).rem_euclid(TAU);
        let w = if n == 1 || gap <= 0.0 { 0.0 } else { (angle - a.angle).rem_euclid(TAU) / gap };
        let at = |p: &Profile| {
            let x = f.clamp(0.0, 1.0) * (PROFILE_SAMPLES - 1) as f32;
            let i = (x.floor() as usize).min(PROFILE_SAMPLES - 2);
            let t = x - i as f32;
            let mut out = [0.0; 4];
            for k in 0..4 {
                out[k] = p.samples[i][k] + (p.samples[i + 1][k] - p.samples[i][k]) * t;
            }
            out
        };
        let (pa, pb) = (at(a), at(b));
        let mut out = [0.0; 4];
        for k in 0..4 {
            out[k] = pa[k] + (pb[k] - pa[k]) * w;
        }
        (out, a.length + (b.length - a.length) * w)
    }

    /// Whether the flesh closes between the two rays on either side of `angle` at `f`. Where
    /// the cap is torn or folded into lobes they drift further apart than their angles keep
    /// them, and leave a notch.
    fn covered(&self, f: f32, angle: f32) -> bool {
        let n = self.profiles.len();
        if n < 2 {
            return true;
        }
        let angle = angle.rem_euclid(TAU);
        let next = self.profiles.iter().position(|p| p.angle > angle).unwrap_or(0);
        let previous = (next + n - 1) % n;
        let (a, b) = (&self.profiles[previous], &self.profiles[next]);
        let i = ((f.clamp(0.0, 1.0) * (PROFILE_SAMPLES - 1) as f32).round() as usize).min(PROFILE_SAMPLES - 1);
        let point = |p: &Profile| {
            let s = p.samples[i];
            let dir = Vec2::from_angle(p.angle + s[2]);
            (self.side * dir.x + self.around * dir.y) * s[0] + self.axis * s[1]
        };
        let (sa, sb) = (a.samples[i], b.samples[i]);
        let gap = (b.angle - a.angle).rem_euclid(TAU);
        let apart = 2.0 * 0.5 * (sa[0] + sb[0]) * (0.5 * gap).sin();
        point(a).distance(point(b)) <= (1.0 + NOTCH_SLACK) * apart + NOTCH * (sa[3] + sb[3])
    }

    fn position(&self, f: f32, angle: f32) -> Vec3 {
        let (p, _) = self.profile_at(f, angle);
        let dir = Vec2::from_angle(angle + p[2]);
        self.centre + (self.side * dir.x + self.around * dir.y) * p[0] + self.axis * p[1]
    }

    /// The underside at `f` from the stipe (0) to the margin (1) and `angle` around the axis
    pub fn at(&self, f: f32, angle: f32) -> UndersidePoint {
        let (p, length) = self.profile_at(f, angle);
        let df = 0.5 / (PROFILE_SAMPLES - 1) as f32;
        let (f0, f1) = ((f - df).max(0.0), (f + df).min(1.0));
        let along = self.position(f1, angle) - self.position(f0, angle);
        let da = 0.01;
        let sideways = self.position(f, angle + da) - self.position(f, angle - da);
        let up = along.cross(sideways).try_normalize().unwrap_or(self.axis);
        UndersidePoint {
            position: self.position(f, angle),
            down: -up,
            half: p[3],
            distance: f.clamp(0.0, 1.0) * length,
            covered: self.covered(f, angle),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LineEnd {
    /// The stripe ends in a gap of the pattern
    Free,
    /// The stripe forks or joins another one here
    Joined,
}

/// A line along the middle of a stripe of the pattern, in the plane of the pattern
#[derive(Clone, Debug, PartialEq)]
pub struct PatternLine {
    pub points: Vec<Vec2>,
    pub ends: [LineEnd; 2],
}

const NEIGHBOURS: [(isize, isize); 8] = [(0, -1), (1, -1), (1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1)];

/// Thins the stripes of the pattern to lines one cell wide (Zhang-Suen)
fn thin(on: &mut [bool], n: usize) {
    let at = |on: &[bool], x: usize, y: usize, k: usize| {
        let (dx, dy) = NEIGHBOURS[k];
        on[(y as isize + dy) as usize * n + (x as isize + dx) as usize]
    };
    loop {
        let mut changed = false;
        for pass in 0..2 {
            let mut remove = Vec::new();
            for y in 1..n - 1 {
                for x in 1..n - 1 {
                    if !on[y * n + x] {
                        continue;
                    }
                    let p: [bool; 8] = std::array::from_fn(|k| at(on, x, y, k));
                    let count = p.iter().filter(|&&v| v).count();
                    let transitions = (0..8).filter(|&k| !p[k] && p[(k + 1) % 8]).count();
                    let (n_, e, s, w) = (p[0], p[2], p[4], p[6]);
                    let keep = if pass == 0 { (n_ && e && s) || (e && s && w) } else { (n_ && e && w) || (n_ && s && w) };
                    if (2..=6).contains(&count) && transitions == 1 && !keep {
                        remove.push(y * n + x);
                    }
                }
            }
            changed |= !remove.is_empty();
            for i in remove {
                on[i] = false;
            }
        }
        if !changed {
            break;
        }
    }
}

/// The lines along the middle of the stripes, split where they fork or join
pub fn pattern_lines(pattern: &Pattern) -> Vec<PatternLine> {
    if pattern.is_empty() {
        return Vec::new();
    }
    let n = pattern.size;
    let mut on: Vec<bool> = pattern.values.iter().map(|&v| v > 0.5).collect();
    for i in 0..n {
        for j in [i, (n - 1) * n + i, i * n, i * n + n - 1] {
            on[j] = false;
        }
    }
    thin(&mut on, n);

    // neighbours by m-adjacency: a diagonal neighbour only counts when no cell next to both of
    // them is on, so a staircase doesn't read as a fork
    let neighbours = |i: usize| -> Vec<usize> {
        let (x, y) = ((i % n) as isize, (i / n) as isize);
        let cell = |dx: isize, dy: isize| -> Option<usize> {
            let (xx, yy) = (x + dx, y + dy);
            (xx >= 0 && yy >= 0 && xx < n as isize && yy < n as isize && on[yy as usize * n + xx as usize])
                .then(|| yy as usize * n + xx as usize)
        };
        NEIGHBOURS
            .iter()
            .filter_map(|&(dx, dy)| {
                let j = cell(dx, dy)?;
                let diagonal = dx != 0 && dy != 0;
                (!diagonal || (cell(dx, 0).is_none() && cell(0, dy).is_none())).then_some(j)
            })
            .collect()
    };
    let cells: Vec<usize> = (0..n * n).filter(|&i| on[i]).collect();
    let adjacent: HashMap<usize, Vec<usize>> = cells.iter().map(|&i| (i, neighbours(i))).collect();
    let degree = |i: usize| adjacent[&i].len();

    // forks that span a few cells are one node, at their centre
    let mut node_of: HashMap<usize, usize> = HashMap::new();
    let mut nodes: Vec<Vec<usize>> = Vec::new();
    for &i in &cells {
        if degree(i) < 3 || node_of.contains_key(&i) {
            continue;
        }
        let mut stack = vec![i];
        let mut members = Vec::new();
        node_of.insert(i, nodes.len());
        while let Some(c) = stack.pop() {
            members.push(c);
            for &j in &adjacent[&c] {
                if degree(j) >= 3 && !node_of.contains_key(&j) {
                    node_of.insert(j, nodes.len());
                    stack.push(j);
                }
            }
        }
        nodes.push(members);
    }
    let cell_centre = |i: usize| -> Vec2 {
        let cell = 2.0 * pattern.extent / n as f32;
        Vec2::new((i % n) as f32 + 0.5, (i / n) as f32 + 0.5) * cell - Vec2::splat(pattern.extent)
    };
    let node_centre: Vec<Vec2> =
        nodes.iter().map(|m| m.iter().map(|&i| cell_centre(i)).sum::<Vec2>() / m.len() as f32).collect();

    let is_stop = |i: usize| degree(i) != 2;
    let mut walked: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
    let edge = |a: usize, b: usize| (a.min(b), a.max(b));
    let mut lines: Vec<(Vec<usize>, [LineEnd; 2])> = Vec::new();
    let end_of = |i: usize| if degree(i) >= 3 { LineEnd::Joined } else { LineEnd::Free };
    let walk = |from: usize, to: usize, walked: &mut std::collections::HashSet<(usize, usize)>| -> Vec<usize> {
        let mut line = vec![from];
        let (mut previous, mut current) = (from, to);
        walked.insert(edge(from, to));
        while !is_stop(current) && current != from {
            line.push(current);
            let Some(&next) = adjacent[&current].iter().find(|&&j| j != previous) else { break };
            if !walked.insert(edge(current, next)) {
                break;
            }
            previous = current;
            current = next;
        }
        line.push(current);
        line
    };
    for &i in &cells {
        if !is_stop(i) {
            continue;
        }
        for &j in &adjacent[&i] {
            if walked.contains(&edge(i, j)) {
                continue;
            }
            let line = walk(i, j, &mut walked);
            let (a, b) = (line[0], line[line.len() - 1]);
            let same_node = matches!((node_of.get(&a), node_of.get(&b)), (Some(x), Some(y)) if x == y);
            if same_node && line.len() <= 3 {
                continue;
            }
            lines.push((line, [end_of(a), end_of(b)]));
        }
    }
    // rings without forks
    for &i in &cells {
        if is_stop(i) || adjacent[&i].iter().all(|&j| walked.contains(&edge(i, j))) {
            continue;
        }
        let line = walk(i, adjacent[&i][0], &mut walked);
        lines.push((line, [LineEnd::Joined, LineEnd::Joined]));
    }

    let length = |p: &[Vec2]| p.windows(2).map(|w| w[0].distance(w[1])).sum::<f32>();
    lines
        .into_iter()
        .filter_map(|(cells, ends)| {
            let mut points: Vec<Vec2> = cells.iter().map(|&i| cell_centre(i)).collect();
            for (k, &end) in [0, cells.len() - 1].iter().enumerate() {
                if let Some(&node) = node_of.get(&cells[end]) {
                    let last = if k == 0 { 0 } else { points.len() - 1 };
                    points[last] = node_centre[node];
                }
            }
            let l = length(&points);
            let free = ends.iter().filter(|&&e| e == LineEnd::Free).count();
            let keep = match free {
                2 => l >= MIN_ISLAND * pattern.spacing,
                1 => l >= MIN_SPUR * pattern.spacing,
                _ => points.len() >= 2,
            };
            keep.then(|| PatternLine { points: smooth(&points), ends })
        })
        .collect()
}

/// Leads `share` of the free ends that come close to another line on to it, so the two fork or
/// join there
fn join_ends(lines: &mut [PatternLine], reach: f32, share: f32, seed: u32) {
    if share <= 0.0 {
        return;
    }
    let all: Vec<Vec<Vec2>> = lines.iter().map(|l| l.points.clone()).collect();
    for (k, line) in lines.iter_mut().enumerate() {
        let n = line.points.len();
        if n < 2 {
            continue;
        }
        for e in 0..2 {
            if line.ends[e] != LineEnd::Free || hash01((k * 2 + e) as u32, seed ^ 0x6e35) >= share {
                continue;
            }
            let (end, back) = if e == 0 { (line.points[0], line.points[4.min(n - 1)]) } else { (line.points[n - 1], line.points[n.saturating_sub(5)]) };
            let ahead = (end - back).try_normalize().unwrap_or(Vec2::ZERO);
            let mut best: Option<(f32, Vec2)> = None;
            for (j, other) in all.iter().enumerate() {
                if j == k {
                    continue;
                }
                for &q in other {
                    let d = q.distance(end);
                    if d < reach && (q - end).dot(ahead) > 0.0 && best.is_none_or(|b| d < b.0) {
                        best = Some((d, q));
                    }
                }
            }
            if let Some((_, q)) = best {
                if e == 0 {
                    line.points.insert(0, q);
                } else {
                    line.points.push(q);
                }
                line.ends[e] = LineEnd::Joined;
            }
        }
    }
}

/// One pass of a [1 2 1] filter, the ends stay where they are
fn smooth_once(points: &[Vec2]) -> Vec<Vec2> {
    let mut p = points.to_vec();
    for i in 1..p.len().saturating_sub(1) {
        p[i] = 0.25 * points[i - 1] + 0.5 * points[i] + 0.25 * points[i + 1];
    }
    p
}

/// Rounds off the steps of the cell grid
fn smooth(points: &[Vec2]) -> Vec<Vec2> {
    (0..4).fold(points.to_vec(), |p, _| smooth_once(&p))
}

/// Points along a polyline `step` apart, the last one at its end
fn resample(points: &[Vec2], step: f32) -> Vec<Vec2> {
    let length: f32 = points.windows(2).map(|w| w[0].distance(w[1])).sum();
    let count = ((length / step).ceil() as usize).max(1) + 1;
    let mut out = Vec::with_capacity(count);
    let (mut j, mut walked) = (0, 0.0);
    for k in 0..count {
        let target = length * k as f32 / (count - 1) as f32;
        while j + 2 < points.len() && walked + points[j].distance(points[j + 1]) < target {
            walked += points[j].distance(points[j + 1]);
            j += 1;
        }
        let span = points[j].distance(points[j + 1]);
        let t = if span > 0.0 { ((target - walked) / span).clamp(0.0, 1.0) } else { 0.0 };
        out.push(points[j].lerp(points[j + 1], t));
    }
    out
}

/// How an end of a gill is shaped
#[derive(Clone, Copy, Debug, PartialEq)]
enum GillEnd {
    /// Slopes up into the cap, where a short gill starts in a gap
    Free,
    /// Cut square, where it meets another gill or the margin
    Square,
    /// Rounded off short of the stipe
    Inner,
    /// Runs on down the stipe
    Decurrent,
}

/// Where a gill hangs from the cap, one point per step along it
struct GillPoint {
    /// From the stipe (0) to the margin (1)
    f: f32,
    /// Around the axis, before the cap's rays turn it
    angle: f32,
    under: UndersidePoint,
    /// Where it lies in the plane of the pattern, for the noise
    plan: Vec2,
}

struct Gill {
    points: Vec<GillPoint>,
    ends: [GillEnd; 2],
}

/// A hash of `i` in [0, 1]
fn hash01(i: u32, seed: u32) -> f32 {
    let mut h = i.wrapping_mul(0x8da6_b343) ^ seed.wrapping_mul(0x9e37_79b9);
    h ^= h >> 16;
    h = h.wrapping_mul(0x7feb_352d);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846c_a68b);
    h ^= h >> 16;
    (h >> 8) as f32 / (1u32 << 24) as f32
}

/// The gills of a pattern line under one cap: the runs of it that lie between the stipe (after
/// the free distance) and the margin
fn place_line(
    line: &PatternLine,
    cap: &CapUnderside,
    pattern: &Pattern,
    turn: f32,
    params: &LamellaeParams,
) -> Vec<Gill> {
    let width = (pattern.radius - pattern.inner).max(f32::EPSILON);
    // stripes stop a little short of the edge of the pattern, gills that end there run on to
    // the margin
    let mut line_points = line.points.clone();
    for e in 0..2 {
        let end = if e == 0 { line_points[0] } else { line_points[line_points.len() - 1] };
        let short = pattern.radius - end.length();
        if line.ends[e] == LineEnd::Free && short > 0.0 && short < MARGIN_REACH * pattern.spacing {
            let out = end.try_normalize().unwrap_or(Vec2::X) * (pattern.radius + 0.5 * pattern.spacing);
            if e == 0 {
                line_points.insert(0, out);
            } else {
                line_points.push(out);
            }
        }
    }
    let mut points = resample(&line_points, pattern.spacing / SAMPLES_PER_SPACING);
    // the stripes waver on the scale of the grid, a gill runs smoother
    for _ in 0..SMOOTHING {
        points = smooth_once(&points);
    }
    let locate = |p: Vec2| -> (f32, f32) { ((p.length() - pattern.inner) / width, p.y.atan2(p.x) + turn) };
    // -1 inside the free distance or in a notch, 0 on the cap, 1 past the margin
    let zone = |p: Vec2| -> i32 {
        let (f, angle) = locate(p);
        if f > 1.0 {
            return 1;
        }
        let under = cap.at(f.max(0.0), angle);
        if f < 0.0 || under.distance < params.free_distance || !under.covered {
            -1
        } else {
            0
        }
    };
    // the last point from `a`, on the cap, towards `b`, off it
    let boundary = |a: Vec2, b: Vec2| -> Vec2 {
        let (mut lo, mut hi) = (0.0f32, 1.0f32);
        for _ in 0..12 {
            let mid = 0.5 * (lo + hi);
            if zone(a.lerp(b, mid)) == 0 {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        a.lerp(b, lo)
    };
    let point = |p: Vec2| {
        let (f, angle) = locate(p);
        let f = f.clamp(0.0, 1.0);
        GillPoint { f, angle, under: cap.at(f, angle), plan: p }
    };
    let cut_end = |outside: i32| if outside > 0 { GillEnd::Square } else { GillEnd::Inner };
    let line_end = |e: LineEnd| if e == LineEnd::Joined { GillEnd::Square } else { GillEnd::Free };

    let zones: Vec<i32> = points.iter().map(|&p| zone(p)).collect();
    let mut gills = Vec::new();
    let mut i = 0;
    while i < points.len() {
        if zones[i] != 0 {
            i += 1;
            continue;
        }
        let start = i;
        while i + 1 < points.len() && zones[i + 1] == 0 {
            i += 1;
        }
        let end = i;
        let mut run: Vec<GillPoint> = Vec::new();
        let mut ends = [line_end(line.ends[0]), line_end(line.ends[1])];
        if start > 0 {
            run.push(point(boundary(points[start], points[start - 1])));
            ends[0] = cut_end(zones[start - 1]);
        }
        run.extend(points[start..=end].iter().map(|&p| point(p)));
        if end + 1 < points.len() {
            run.push(point(boundary(points[end], points[end + 1])));
            ends[1] = cut_end(zones[end + 1]);
        }
        i = end + 1;
        if run.len() < 2 {
            continue;
        }
        // a gill that reaches the stipe runs on down it
        for (k, end) in ends.iter_mut().enumerate() {
            let at = if k == 0 { &run[0] } else { &run[run.len() - 1] };
            let near_stipe = at.under.distance < pattern.spacing.max(params.free_distance + 0.5 * pattern.spacing);
            if params.decurrent > 0.0 && params.free_distance <= 0.0 && near_stipe && *end != GillEnd::Square {
                *end = GillEnd::Decurrent;
            }
        }
        gills.push(Gill { points: run, ends });
    }
    gills
}

/// Extends a gill that reaches the stipe by `length`: in along the underside to the stipe, then
/// down the stipe
fn run_down(gill: &mut Gill, cap: &CapUnderside, length: f32) {
    for k in 0..2 {
        if gill.ends[k] != GillEnd::Decurrent {
            continue;
        }
        let end = if k == 0 { &gill.points[0] } else { &gill.points[gill.points.len() - 1] };
        let (f, angle, distance) = (end.f, end.angle, end.under.distance);
        let direction = end.plan.try_normalize().unwrap_or(Vec2::X);
        let plan = |moved: f32| direction * (end.plan.length() - moved);
        let mut added: Vec<GillPoint> = Vec::new();
        let steps_in = (f * PROFILE_SAMPLES as f32).ceil() as usize;
        for s in 1..=steps_in {
            let f = f * (1.0 - s as f32 / steps_in as f32);
            let under = cap.at(f, angle);
            added.push(GillPoint { f, angle, plan: plan(distance - under.distance), under });
        }
        let top = cap.at(0.0, angle);
        let radial = top.position - cap.centre;
        let towards = radial - cap.axis * radial.dot(cap.axis);
        let count = ((length / (0.25 * top.half.max(1e-4))).ceil() as usize).clamp(2, 32);
        for s in 1..=count {
            let d = length * s as f32 / count as f32;
            let (position, out) = cap.down_the_stipe(d, towards);
            let under = UndersidePoint { position, down: out, half: top.half, distance: -d, covered: true };
            added.push(GillPoint { f: 0.0, angle, plan: plan(distance + d), under });
        }
        if k == 0 {
            added.reverse();
            added.append(&mut gill.points);
            gill.points = added;
        } else {
            gill.points.append(&mut added);
        }
    }
}

/// Half the thickness of the cross section at `u` from the cap (0) to the edge (1)
fn half_width(form: LamellaForm, u: f32) -> f32 {
    match form {
        LamellaForm::Plates => {
            let narrowing = 1.0 - (1.0 - PLATE_EDGE_SHARE) * u;
            narrowing * (1.0 - u.powi(8)).max(0.0).powf(0.125)
        }
        LamellaForm::Ridges => (1.0 - u * u).max(0.0).sqrt(),
    }
}

struct MeshBuilder {
    positions: Vec<Vec3>,
    faces: Vec<u32>,
}

impl MeshBuilder {
    fn add_gill(&mut self, gill: &Gill, pattern: &Pattern, params: &LamellaeParams, salt: u32) {
        let pts = &gill.points;
        let n = pts.len();
        if n < 2 {
            return;
        }
        let spacing = pattern.spacing;
        let mut along = vec![0.0f32; n];
        for i in 1..n {
            along[i] = along[i - 1] + pts[i].under.position.distance(pts[i - 1].under.position);
        }
        let total = along[n - 1];
        let taper_of = |end: GillEnd, d: f32| match end {
            GillEnd::Free => smoothstep(0.0, FREE_END_TAPER * spacing, d),
            GillEnd::Inner => smoothstep(0.0, INNER_END_TAPER * spacing, d),
            GillEnd::Square => 1.0,
            GillEnd::Decurrent => smoothstep(0.0, params.decurrent, d),
        };
        let noise = |p: Vec2, wavelength: f32, salt2: u32| {
            value_noise(Vec3::new(p.x / wavelength, p.y / wavelength, 0.5), params.seed ^ salt ^ salt2)
        };
        let ring = SECTION.len() * 2 + 1;
        let first = self.positions.len() as u32;
        for (i, p) in pts.iter().enumerate() {
            let tangent = (pts[(i + 1).min(n - 1)].under.position - pts[i.saturating_sub(1)].under.position)
                .try_normalize()
                .unwrap_or(Vec3::X);
            let down = (p.under.down - tangent * p.under.down.dot(tangent)).try_normalize().unwrap_or(p.under.down);
            let lateral = tangent.cross(down);
            let taper = taper_of(gill.ends[0], along[i]) * taper_of(gill.ends[1], total - along[i]);
            let variation = |salt2: u32, amount: f32| {
                (1.0 + params.irregularity * amount * noise(p.plan, VARIATION_WAVELENGTH * spacing, salt2)).max(0.1)
            };
            let depth_share = if p.under.distance < 0.0 { 1.0 } else { curve(&params.depth_curve, p.f).max(0.0) };
            let depth = params.depth * depth_share * variation(0x3d1, DEPTH_VARIATION) * taper;
            let half = 0.5 * params.thickness * variation(0x7c2, THICKNESS_VARIATION) * (0.3 + 0.7 * taper.sqrt());
            let half = match params.form {
                LamellaForm::Plates => half,
                LamellaForm::Ridges => half.max(0.5 * depth),
            };
            let wave = params.edge_wave * 0.5 * spacing * noise(p.plan, WAVE_WAVELENGTH * spacing, 0x1a5);
            let embed = EMBED * p.under.half;
            let vertex = |u: f32, side: f32| {
                let height = if u <= 0.0 { -embed } else { u * depth };
                let width = side * half * half_width(params.form, u);
                p.under.position + down * height + lateral * (width + wave * u * u)
            };
            for &u in &SECTION {
                self.positions.push(vertex(u, -1.0));
            }
            self.positions.push(vertex(1.0, 0.0));
            for &u in SECTION.iter().rev() {
                self.positions.push(vertex(u, 1.0));
            }
        }
        let r = ring as u32;
        for i in 0..n as u32 - 1 {
            for j in 0..r - 1 {
                let a = first + i * r + j;
                let (b, c, d) = (a + 1, a + r + 1, a + r);
                self.faces.extend([a, b, c, a, c, d]);
            }
        }
        // close the ends, each with vertices of its own so its flat normal doesn't bend the sides
        for (i, outward) in [(0u32, false), (n as u32 - 1, true)] {
            let start = self.positions.len() as u32;
            let section: Vec<Vec3> = (0..r).map(|j| self.positions[(first + i * r + j) as usize]).collect();
            let centre = section.iter().copied().sum::<Vec3>() / r as f32;
            self.positions.extend(section);
            self.positions.push(centre);
            let c = start + r;
            for j in 0..r - 1 {
                if outward {
                    self.faces.extend([c, start + j, start + j + 1]);
                } else {
                    self.faces.extend([c, start + j + 1, start + j]);
                }
            }
        }
    }

    fn normals(&self) -> Vec<f32> {
        let mut normals = vec![Vec3::ZERO; self.positions.len()];
        for f in self.faces.chunks_exact(3) {
            let [a, b, c] = [f[0] as usize, f[1] as usize, f[2] as usize];
            let n = (self.positions[b] - self.positions[a]).cross(self.positions[c] - self.positions[a]);
            for v in [a, b, c] {
                normals[v] += n;
            }
        }
        normals.iter().flat_map(|n| n.try_normalize().unwrap_or(Vec3::Y).to_array()).collect()
    }
}

/// Where the gills hang under every cap, each with a salt for its noise
fn place_gills(caps: &[CapUnderside], pattern: &Pattern, params: &LamellaeParams) -> Vec<(u32, Gill)> {
    let mut lines = pattern_lines(pattern);
    join_ends(&mut lines, JOIN_REACH * pattern.spacing, params.forking, params.seed);
    let mut gills = Vec::new();
    for cap in caps {
        // every cap turns the pattern its own way, so neighbouring mushrooms differ
        let turn = TAU * hash01(cap.id as u32, params.seed ^ 0x5bd1);
        for (k, line) in lines.iter().enumerate() {
            for mut gill in place_line(line, cap, pattern, turn, params) {
                run_down(&mut gill, cap, params.decurrent);
                gills.push((k as u32 ^ (cap.id as u32).rotate_left(16), gill));
            }
        }
    }
    gills
}

/// Gills under every cap: the stripes of the pattern, stretched so its inner radius lies where
/// the underside leaves the stipe and its radius on the margin, hang from the underside as
/// plates or ridges. They are added to `base` in a material group of their own.
pub fn lamellae(caps: &[CapUnderside], pattern: &Pattern, params: &LamellaeParams, base: Mesh) -> Mesh {
    let mut builder = MeshBuilder { positions: Vec::new(), faces: Vec::new() };
    for (salt, gill) in place_gills(caps, pattern, params) {
        builder.add_gill(&gill, pattern, params, salt);
    }

    let mut mesh = base;
    if builder.positions.is_empty() {
        return mesh;
    }
    let group = mesh.groups.iter().max().map_or(0, |g| g + 1);
    let offset = mesh.vertex_count() as u32;
    let normals = builder.normals();
    let count = builder.positions.len();
    mesh.faces.extend(builder.faces.iter().map(|i| i + offset));
    mesh.positions.extend(builder.positions.iter().flat_map(|p| p.to_array()));
    mesh.normals.extend(normals);
    if !mesh.colors.is_empty() {
        mesh.colors.extend(std::iter::repeat_n(pack_rgba(0.85, 0.78, 0.72, 1.0), count));
    }
    mesh.groups.extend(std::iter::repeat_n(group, count));
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pattern::{reaction_diffusion, ReactionDiffusionParams};

    /// Writes the pattern with its lines over it when `PATTERN_DUMP` names a folder
    fn dump_lines(pattern: &Pattern, lines: &[PatternLine], name: &str) {
        let Ok(dir) = std::env::var("PATTERN_DUMP") else { return };
        let n = pattern.size;
        let scale = 3;
        let m = n * scale;
        let mut rgb = vec![0u8; m * m * 3];
        for y in 0..m {
            for x in 0..m {
                let v = (pattern.values[(y / scale) * n + x / scale] * 120.0) as u8;
                rgb[(y * m + x) * 3..(y * m + x) * 3 + 3].copy_from_slice(&[v, v, v]);
            }
        }
        let cell = 2.0 * pattern.extent / n as f32;
        for (k, line) in lines.iter().enumerate() {
            let color = [255, 230, (k * 97 % 200) as u8];
            for w in line.points.windows(2) {
                for s in 0..20 {
                    let p = w[0].lerp(w[1], s as f32 / 20.0);
                    let g = (p + Vec2::splat(pattern.extent)) / cell * scale as f32;
                    let (x, y) = (g.x as usize, g.y as usize);
                    if x < m && y < m {
                        rgb[(y * m + x) * 3..(y * m + x) * 3 + 3].copy_from_slice(&color);
                    }
                }
            }
            for (end, p) in line.ends.iter().zip([line.points[0], line.points[line.points.len() - 1]]) {
                let g = (p + Vec2::splat(pattern.extent)) / cell * scale as f32;
                let c = if *end == LineEnd::Free { [255, 60, 60] } else { [60, 255, 60] };
                for dy in 0..3 {
                    for dx in 0..3 {
                        let (x, y) = (g.x as usize + dx, g.y as usize + dy);
                        if x < m && y < m {
                            rgb[(y * m + x) * 3..(y * m + x) * 3 + 3].copy_from_slice(&c);
                        }
                    }
                }
            }
        }
        let mut bytes = format!("P6\n{} {}\n255\n", m, m).into_bytes();
        bytes.extend(rgb);
        std::fs::write(format!("{}/{}.ppm", dir, name), bytes).unwrap();
    }

    const STIPE: f32 = 0.004;
    const RADIUS: f32 = 0.025;

    /// Height of the middle of the flesh and its half thickness at `x` from the axis: a dome
    /// that drops 4 mm to the margin
    fn dome(x: f32) -> (f32, f32) {
        let a = x / RADIUS;
        (0.002 - 0.004 * a * a, 0.002 * (1.0 - 0.7 * a))
    }

    /// A cap of `rays` rays around an upright stipe with its top at the origin, like the cap
    /// node makes: each starts below the top on the far side of the axis, crosses it and runs
    /// out to the margin. With `tear` that ray and the next turn apart towards the margin.
    fn cap_rays(rays: usize, tear: Option<usize>) -> (Point, Vec<Vec<f32>>) {
        let hinge = Point {
            position: Vec3::ZERO,
            direction: Vec3::X,
            tangent: Vec3::Y,
            scale: STIPE,
            parent_id: 5,
            alpha: 1.0,
            depth: 1,
            index: 0,
        };
        let paths = (0..rays)
            .map(|k| {
                let angle = TAU * k as f32 / rays as f32;
                let turn = match tear {
                    Some(t) if t == k => -0.3,
                    Some(t) if t + 1 == k => 0.3,
                    _ => 0.0,
                };
                let at = |x: f32| {
                    let a = angle + turn * (x / RADIUS).powi(2);
                    Vec3::new(a.cos(), 0.0, a.sin()) * x
                };
                let mut points = vec![(-0.5 * STIPE, -0.6 * STIPE, 0.001), (-0.2 * STIPE, 0.0, 0.0015)];
                points.extend((0..40).map(|i| {
                    let x = RADIUS * i as f32 / 39.0;
                    let (h, half) = dome(x);
                    (x, h, half)
                }));
                points.iter().flat_map(|&(x, h, r)| (at(x) + Vec3::Y * h).extend(r).to_array()).collect()
            })
            .collect();
        (hinge, paths)
    }

    fn cap(rays: usize, tear: Option<usize>) -> CapUnderside {
        let (hinge, paths) = cap_rays(rays, tear);
        let refs: Vec<&[f32]> = paths.iter().map(|p| p.as_slice()).collect();
        CapUnderside::new(&hinge, &refs).unwrap()
    }

    fn pattern() -> Pattern {
        reaction_diffusion(&ReactionDiffusionParams { steps: 2000, ..Default::default() })
    }

    fn positions(mesh: &Mesh) -> Vec<Vec3> {
        mesh.positions.chunks_exact(3).map(Vec3::from_slice).collect()
    }

    #[test]
    fn the_underside_is_the_lower_side_of_the_flesh() {
        let cap = cap(24, None);
        assert!((cap.stipe_radius - STIPE).abs() < 1e-7);
        for k in 0..50 {
            let (f, angle) = (k as f32 / 49.0, 0.37 * k as f32);
            let u = cap.at(f, angle);
            let x = Vec2::new(u.position.x, u.position.z).length();
            let (h, half) = dome(x);
            // the dome is gentle, the flesh below its middle is about half a thickness down
            assert!((u.position.y - (h - half)).abs() < 2e-4, "underside at {} for a flesh from {}", u.position.y, h - half);
            assert!(u.down.y < -0.8, "gills hang down, not {:?}", u.down);
            assert!(u.covered);
        }
        assert!((Vec2::new(cap.at(0.0, 1.0).position.x, cap.at(0.0, 1.0).position.z).length() - STIPE).abs() < 1e-4);
        let margin = cap.at(1.0, 2.0).position;
        assert!((Vec2::new(margin.x, margin.z).length() - RADIUS).abs() < 3e-4);
    }

    #[test]
    fn gills_hang_under_the_cap_inside_the_margin_and_free_of_the_stipe() {
        let params = LamellaeParams { free_distance: 0.002, ..Default::default() };
        let mesh = lamellae(&[cap(24, None)], &pattern(), &params, Mesh::default());
        assert!(mesh.face_count() > 1000, "{} faces", mesh.face_count());
        let deepest = params.depth * (1.0 + params.irregularity * DEPTH_VARIATION);
        for p in positions(&mesh) {
            let x = Vec2::new(p.x, p.z).length();
            let (h, half) = dome(x.min(RADIUS));
            assert!(x <= RADIUS + params.thickness, "gill {} mm from the axis, the margin is at {}", x * 1e3, RADIUS * 1e3);
            assert!(x >= STIPE + params.free_distance - params.thickness, "gill {} mm from the axis", x * 1e3);
            assert!(p.y <= h + 1e-4, "gill reaches {} mm above the middle of the flesh", (p.y - h) * 1e3);
            assert!(p.y >= h - half - deepest - 1e-4, "gill {} mm below the flesh", (h - half - p.y) * 1e3);
        }
    }

    #[test]
    fn depth_follows_the_input_and_its_curve() {
        let cap = cap(24, None);
        let pattern = pattern();
        // how far a gill reaches below the underside next to the stipe, in the middle and at
        // the margin
        let reach = |params: &LamellaeParams| {
            let mut bands = [0.0f32; 3];
            for p in positions(&lamellae(&[cap.clone()], &pattern, params, Mesh::default())) {
                let x = Vec2::new(p.x, p.z).length().min(RADIUS);
                let (h, half) = dome(x);
                let a = (x - STIPE) / (RADIUS - STIPE);
                let band = if a < 0.12 { 0 } else if (0.45..0.55).contains(&a) { 1 } else if a > 0.9 { 2 } else { continue };
                bands[band] = bands[band].max(h - half - p.y);
            }
            bands
        };
        let even = LamellaeParams { irregularity: 0.0, edge_wave: 0.0, depth_curve: vec![0.0, 1.0, 1.0, 1.0], ..Default::default() };
        let bands = reach(&even);
        // the inner ends round off short of the stipe
        assert!(bands[0] > 0.75 * even.depth, "{:?}", bands);
        for band in &bands[1..] {
            assert!((band - even.depth).abs() < 0.1 * even.depth, "gills {} mm deep, asked for {}", band * 1e3, even.depth * 1e3);
        }
        let deeper = reach(&LamellaeParams { depth: 0.009, ..even.clone() });
        assert!((deeper[1] - 0.009).abs() < 0.1 * 0.009);
        // low folds at the stipe, deepest in the middle, flat at the margin
        let profile = reach(&LamellaeParams { depth_curve: vec![0.0, 0.2, 0.5, 1.0, 1.0, 0.1], ..even.clone() });
        assert!(profile[0] < 0.6 * profile[1] && profile[2] < 0.6 * profile[1], "{:?}", profile);
    }

    #[test]
    fn gills_keep_the_spacing_of_the_pattern() {
        let pattern = pattern();
        let cap = cap(24, None);
        let gills = place_gills(&[cap.clone()], &pattern, &LamellaeParams { free_distance: 0.0, ..Default::default() });
        // gills that cross the circle at `f` from the stipe to the margin
        let crossing = |f: f32| {
            gills.iter().filter(|(_, g)| g.points.windows(2).any(|w| (w[0].f - f) * (w[1].f - f) <= 0.0)).count()
        };
        for f in [0.3, 0.6, 0.9] {
            let r = pattern.inner + f * (pattern.radius - pattern.inner);
            let stripes = crate::pattern::tests::stripes_at(&pattern, r) as f32;
            let gills = crossing(f) as f32;
            assert!((gills - stripes).abs() <= 0.15 * stripes, "{} gills and {} stripes at {}", gills, stripes, f);
        }
        assert!(crossing(0.9) as f32 > 1.8 * crossing(0.1) as f32);
    }

    #[test]
    fn no_gills_hang_in_a_notch_of_the_cap() {
        let cap = cap(24, Some(4));
        // rays 4 and 5 turn apart and open a notch towards the margin, between them only
        let open = |f: f32| (0..720).filter(|&k| !cap.at(f, TAU * k as f32 / 720.0).covered).count();
        assert_eq!(open(0.2), 0);
        assert!(open(0.9) > 0 && open(0.9) <= 32, "{} of 720 directions open", open(0.9));
        let gills = place_gills(&[cap.clone()], &pattern(), &LamellaeParams::default());
        for (_, gill) in &gills {
            for p in &gill.points {
                assert!(p.under.covered);
            }
        }
    }

    #[test]
    fn decurrent_gills_run_down_the_stipe() {
        let on_the_stipe = |p: &Vec3| Vec2::new(p.x, p.z).length() < 1.5 * STIPE && p.y < -0.004;
        let free = lamellae(&[cap(24, None)], &pattern(), &LamellaeParams::default(), Mesh::default());
        assert!(!positions(&free).iter().any(on_the_stipe));
        let params = LamellaeParams { free_distance: 0.0, decurrent: 0.008, ..Default::default() };
        let mut stipe = Vec::new();
        for i in 0..10 {
            stipe.extend([0.0, -0.05 + 0.05 * i as f32 / 9.0, 0.0, STIPE * (0.6 + 0.4 * i as f32 / 9.0)]);
        }
        let cap = cap(24, None).with_stipe(&stipe);
        let mesh = lamellae(&[cap], &pattern(), &params, Mesh::default());
        let below: Vec<Vec3> = positions(&mesh).into_iter().filter(on_the_stipe).collect();
        assert!(!below.is_empty());
        for p in below {
            assert!(p.y > -0.0085, "{} below the stipe top", -p.y);
            // on the stipe, which narrows downwards, their upper edge sunk into it
            let stipe_radius = STIPE * (0.6 + 0.4 * (p.y + 0.05) / 0.05);
            let x = Vec2::new(p.x, p.z).length();
            assert!(x > 0.6 * stipe_radius && x < stipe_radius + params.depth, "{} from a stipe of {}", x, stipe_radius);
        }
    }

    #[test]
    fn forking_joins_short_gills_to_their_neighbours() {
        let pattern = pattern();
        let joined = |forking: f32| {
            let mut lines = pattern_lines(&pattern);
            join_ends(&mut lines, JOIN_REACH * pattern.spacing, forking, 0);
            lines.iter().flat_map(|l| l.ends).filter(|&e| e == LineEnd::Joined).count()
        };
        let (none, some, all) = (joined(0.0), joined(0.5), joined(1.0));
        assert!(none < some && some < all, "{} {} {} joined ends", none, some, all);
    }

    #[test]
    fn gills_join_the_mesh_in_a_group_of_their_own() {
        let base = Mesh {
            faces: vec![0, 1, 2],
            positions: vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            normals: vec![0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0],
            colors: Vec::new(),
            groups: vec![0, 2, 1],
            tissue_groups: false,
        };
        let params = LamellaeParams::default();
        let pattern = pattern();
        let mesh = lamellae(&[cap(24, None)], &pattern, &params, base.clone());
        assert_eq!(&mesh.faces[..3], &base.faces[..]);
        assert_eq!(&mesh.positions[..9], &base.positions[..]);
        assert!(mesh.groups[3..].iter().all(|&g| g == 3));
        assert_eq!(mesh, Mesh::decode(&mesh.encode()));
        assert_eq!(mesh, lamellae(&[cap(24, None)], &pattern, &params, base.clone()));
        assert_ne!(mesh, lamellae(&[cap(24, None)], &pattern, &LamellaeParams { seed: 1, ..params.clone() }, base.clone()));
        // the sides of a gill face away from it
        let gills = lamellae(&[cap(24, None)], &pattern, &LamellaeParams { form: LamellaForm::Ridges, ..params.clone() }, Mesh::default());
        let down = gills.normals.chunks_exact(3).filter(|n| n[1] < -0.9).count();
        let up = gills.normals.chunks_exact(3).filter(|n| n[1] > 0.9).count();
        assert!(down > 10 * up, "{} normals down, {} up", down, up);

        assert_eq!(lamellae(&[cap(24, None)], &Pattern::default(), &params, base.clone()), base);
        assert_eq!(lamellae(&[], &pattern, &params, base.clone()), base);
    }

    #[test]
    fn lines_follow_the_stripes() {
        let pattern = reaction_diffusion(&ReactionDiffusionParams::default());
        let lines = pattern_lines(&pattern);
        dump_lines(&pattern, &lines, "lines");
        for line in &lines {
            for p in &line.points {
                assert!(pattern.sample(*p) > 0.3, "line point at {:?} on {}", p, pattern.sample(*p));
            }
        }
    }
}
