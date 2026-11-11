//! Puts a pattern on the top of every cap of a mesh, as a relief and a color.
//!
//! A vertex is on the top of a cap when it lies over the rays of the cap: in the plane through
//! the axis at its angle, the place of the rays on either side nearest to it is where it is on
//! the cap, and it is on the top when it lies on the side of the flesh that faces away from the
//! gills. Where the surface turns over the margin the pattern fades out. Of the vertices there
//! only those in the material group most of the top is in count, so the stipe, the gills, the
//! veil and the underside of the cap stay as they are. The faces on the top are split until they
//! are about as fine as the pattern, and their neighbours so the surface stays closed.
//!
//! The pattern is read at the angle of the vertex around the axis and at its distance along the
//! rays from the axis, stretched to the length of every ray like the gills of `lamellae`. So a
//! spot of the pattern stays on the same spot of the tissue while the cap opens, the rays bend
//! but keep their length.
//!
//! The relief moves a vertex along its normal by `relief` times the value of the pattern, the
//! color goes over the color of the vertex by `opacity` times the value. Both are read along the
//! rays, alpha 0 at the axis to 1 at the margin, and for every stipe.

use glam::{Vec2, Vec3};
use myzel_core::{linear_rgba, opaque, pack_linear, read_color, AlongRay, Cap, CapFrame, Mesh, NO_COLOR};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{geometry::wrap_path, split_args, Ctx, Pattern};
use std::collections::HashMap;
use std::f32::consts::TAU;

nodarium_definition_file!("src/definition.json");

/// A vertex further than this share of the cap length from the flesh around the rays is not on
/// the cap. The strands of the cap spread around the rays, the surface follows them.
const SHELL: f32 = 0.25;
/// Where the surface faces away from the top, the cosine of how far, the pattern fades out
const SIDE_FROM: f32 = 0.1;
const SIDE_TO: f32 = 0.5;
/// The faces on a top are split until no edge is longer than this many rings of the pattern
const DETAIL: f32 = 2.0;
/// Faces are split at most this often, and never into more than this many times the vertices
const MAX_SPLITS: usize = 2;
const MAX_GROWTH: usize = 4;

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn even(_: f32) -> f32 {
    1.0
}

/// The place of a ray nearest to a point, in the plane of the ray
struct Nearest {
    /// Distance along the ray from the axis
    s: f32,
    point: Vec2,
    /// The side of the flesh that faces away from the gills
    normal: Vec2,
    radius: f32,
}

/// A ray in the plane through the axis, from where it crosses the axis on
struct Line {
    angle: f32,
    plane: Vec<Vec2>,
    /// Distance along the ray from the axis, negative before it crosses the axis
    s: Vec<f32>,
    /// Half the flesh thickness
    radius: Vec<f32>,
    /// From the axis to the margin
    length: f32,
    /// The pattern is stretched to the length of every ray: the length of the cap over this one
    stretch: f32,
}

impl Line {
    /// The place nearest to `p`, `None` when that is where a ray that starts out on the cap
    /// starts: `p` lies further in than it reaches
    fn nearest(&self, p: Vec2) -> Option<Nearest> {
        let mut best: Option<(f32, Nearest, bool)> = None;
        for i in 0..self.plane.len().saturating_sub(1) {
            if self.s[i + 1] <= 0.0 {
                continue;
            }
            let (a, b) = (self.plane[i], self.plane[i + 1]);
            let ab = b - a;
            let t = if ab.length_squared() > 0.0 { ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
            let point = a + ab * t;
            let d = p.distance_squared(point);
            if best.as_ref().is_some_and(|(e, _, _)| *e <= d) {
                continue;
            }
            let lerp = |x: &[f32]| x[i] + (x[i + 1] - x[i]) * t;
            let normal = ab.try_normalize().map_or(Vec2::Y, |t| t.perp());
            let at_start = i == 0 && t == 0.0 && self.s[0] > 0.0;
            best = Some((d, Nearest { s: lerp(&self.s).max(0.0), point, normal, radius: lerp(&self.radius).abs() }, at_start));
        }
        best.filter(|b| !b.2).map(|b| b.1)
    }

}

/// Where a vertex is on the top of a cap
struct OnTop {
    angle: f32,
    /// Distance from the axis on the pattern
    s: f32,
    /// 0 at the axis to 1 at the margin
    alpha: f32,
    /// How much the vertex faces up, 1 on the top, 0 past the margin
    weight: f32,
    /// How far it is from the middle of the flesh
    distance: f32,
}

/// The top of one cap, its rays and the relief and opacity read for its stipe
struct Top {
    frame: CapFrame,
    /// Sorted by angle
    lines: Vec<Line>,
    /// The corners of the rays in the plane through the axis, and how far around them a vertex
    /// may be
    low: Vec2,
    high: Vec2,
    shell: f32,
    relief: AlongRay,
    opacity: AlongRay,
}

impl Top {
    fn new(cap: &Cap, items: &[&[i32]], relief: AlongRay, opacity: AlongRay) -> Option<Top> {
        let lines: Vec<Line> = cap
            .rays
            .iter()
            .filter(|ray| ray.length > 0.0)
            .map(|ray| {
                let path = wrap_path(items[ray.path]);
                let s: Vec<f32> = ray.to_end.iter().map(|d| ray.length - d).collect();
                Line {
                    angle: ray.angle,
                    plane: ray.plane.clone(),
                    s,
                    radius: (0..ray.plane.len()).map(|i| path.points[i * 4 + 3]).collect(),
                    length: ray.length,
                    stretch: cap.length / ray.length,
                }
            })
            .collect();
        if lines.is_empty() || cap.length <= 0.0 {
            return None;
        }
        let (mut low, mut high) = (Vec2::splat(f32::INFINITY), Vec2::splat(f32::NEG_INFINITY));
        let mut thickest = 0.0f32;
        for line in &lines {
            for (p, r) in line.plane.iter().zip(&line.radius) {
                low = low.min(*p);
                high = high.max(*p);
                thickest = thickest.max(r.abs());
            }
        }
        let shell = 2.0 * thickest + SHELL * cap.length;
        Some(Top { frame: cap.frame, lines, low: low.max(Vec2::new(0.0, f32::NEG_INFINITY)), high, shell, relief, opacity })
    }

    fn locate(&self, p: Vec3) -> Option<OnTop> {
        let f = &self.frame;
        let q = p - f.hinge;
        let up = q.dot(f.axis);
        let place = Vec2::new((q - f.axis * up).length(), up);
        if place.cmplt(self.low - self.shell).any() || place.cmpgt(self.high + self.shell).any() {
            return None;
        }
        let angle = f.angle(p);
        let n = self.lines.len();
        let k = self.lines.partition_point(|l| l.angle <= angle);
        // how far out the vertex is on the cap, measured on the nearest ray that leaves the
        // stipe. Measured across the plane, a ray that starts near the margin of a closed cap can
        // pass close to its top.
        let out = (1..=n)
            .map(|j| &self.lines[(k + n - j) % n])
            .find(|l| l.s[0] <= 0.0)
            .and_then(|l| l.nearest(place))
            .map_or(f32::INFINITY, |near| near.s);
        // the nearest rays on either side that reach this far in
        let reaching = |i: usize| {
            let line = &self.lines[i];
            if line.s[0] > out {
                return None;
            }
            line.nearest(place).map(|near| (line, near))
        };
        let (a, na) = (1..=n).find_map(|j| reaching((k + n - j) % n))?;
        let (b, nb) = (0..n).find_map(|j| reaching((k + j) % n))?;
        let span = (b.angle - a.angle).rem_euclid(TAU);
        let t = if span > 0.0 { (angle - a.angle).rem_euclid(TAU) / span } else { 0.0 };
        let point = na.point.lerp(nb.point, t);
        let normal = na.normal.lerp(nb.normal, t).try_normalize().unwrap_or(na.normal);
        let radius = na.radius + (nb.radius - na.radius) * t;
        let offset = place - point;
        let distance = offset.length();
        if distance > radius + self.shell {
            return None;
        }
        let facing = if distance > 0.0 { offset.dot(normal) / distance } else { 1.0 };
        let weight = smoothstep(SIDE_FROM, SIDE_TO, facing);
        if weight <= 0.0 {
            return None;
        }
        let lerp = |x: f32, y: f32| x + (y - x) * t;
        Some(OnTop {
            angle,
            s: lerp(na.s * a.stretch, nb.s * b.stretch),
            alpha: lerp(na.s / a.length, nb.s / b.length).clamp(0.0, 1.0),
            weight,
            distance,
        })
    }
}

/// `color` over the color `under` of a vertex, covering `cover` of it, in linear light
fn over(under: u32, color: [f32; 4], cover: f32) -> u32 {
    let [r, g, b, a] = linear_rgba(under);
    let alpha = cover + a * (1.0 - cover);
    if alpha <= 0.0 {
        return under;
    }
    let mix = |c: f32, u: f32| (c * cover + u * a * (1.0 - cover)) / alpha;
    pack_linear([mix(color[0], r), mix(color[1], g), mix(color[2], b), alpha])
}

/// New normals for the vertices of the faces that have a moved vertex
fn renormal(mesh: &mut Mesh, moved: &[bool]) {
    let n = mesh.vertex_count();
    let position = |positions: &[f32], i: u32| Vec3::from_slice(&positions[i as usize * 3..i as usize * 3 + 3]);
    let mut touched = vec![false; n];
    for f in mesh.faces.chunks_exact(3) {
        if f.iter().any(|&i| moved[i as usize]) {
            f.iter().for_each(|&i| touched[i as usize] = true);
        }
    }
    let mut normals = vec![Vec3::ZERO; n];
    for f in mesh.faces.chunks_exact(3) {
        if !f.iter().any(|&i| touched[i as usize]) {
            continue;
        }
        let [a, b, c] = [f[0], f[1], f[2]].map(|i| position(&mesh.positions, i));
        let area = (b - a).cross(c - a);
        f.iter().for_each(|&i| normals[i as usize] += area);
    }
    for v in (0..n).filter(|&v| touched[v]) {
        if let Some(normal) = normals[v].try_normalize() {
            mesh.normals[v * 3..v * 3 + 3].copy_from_slice(&normal.to_array());
        }
    }
}

/// Where every vertex from `from` on is on a top, and on which
fn locate_from(mesh: &Mesh, tops: &[Top], located: &mut Vec<Option<(usize, OnTop)>>) {
    for v in located.len()..mesh.vertex_count() {
        let p = Vec3::from_slice(&mesh.positions[v * 3..v * 3 + 3]);
        located.push(
            tops.iter()
                .enumerate()
                .filter_map(|(k, t)| t.locate(p).map(|on| (k, on)))
                .min_by(|a, b| a.1.distance.total_cmp(&b.1.distance)),
        );
    }
}

/// The group of every top: the one most of the top is in, so gills or a stipe that come close
/// to the top don't get the pattern
fn top_groups(mesh: &Mesh, located: &[Option<(usize, OnTop)>]) -> HashMap<usize, u32> {
    let mut weights: HashMap<(usize, u32), f32> = HashMap::new();
    for (v, on) in located.iter().enumerate() {
        if let Some((k, on)) = on {
            *weights.entry((*k, mesh.groups[v])).or_default() += on.weight;
        }
    }
    let mut groups: HashMap<usize, (u32, f32)> = HashMap::new();
    for (&(k, group), &weight) in &weights {
        let best = groups.entry(k).or_insert((group, weight));
        if weight > best.1 || (weight == best.1 && group < best.0) {
            *best = (group, weight);
        }
    }
    groups.into_iter().map(|(k, (group, _))| (k, group)).collect()
}

/// Adds a vertex in the middle of the edge from `a` to `b`
fn add_middle(mesh: &mut Mesh, a: u32, b: u32) -> u32 {
    let (a, b) = (a as usize, b as usize);
    let at = |x: &[f32], i: usize| Vec3::from_slice(&x[i * 3..i * 3 + 3]);
    let position = at(&mesh.positions, a).lerp(at(&mesh.positions, b), 0.5);
    let normal = (at(&mesh.normals, a) + at(&mesh.normals, b)).try_normalize().unwrap_or(at(&mesh.normals, a));
    mesh.positions.extend(position.to_array());
    mesh.normals.extend(normal.to_array());
    if !mesh.colors.is_empty() {
        let (ca, cb) = (linear_rgba(mesh.colors[a]), linear_rgba(mesh.colors[b]));
        mesh.colors.push(pack_linear([0, 1, 2, 3].map(|i| 0.5 * (ca[i] + cb[i]))));
    }
    mesh.groups.push(mesh.groups[a]);
    (mesh.vertex_count() - 1) as u32
}

/// Splits the marked faces into four, and their neighbours into two or three where they share a
/// split edge, so the surface stays closed
fn refine(mesh: &mut Mesh, marked: &[bool]) {
    let key = |a: u32, b: u32| (a.min(b), a.max(b));
    let faces = std::mem::take(&mut mesh.faces);
    let mut middles: HashMap<(u32, u32), u32> = HashMap::new();
    for t in faces.chunks_exact(3).zip(marked).filter(|(_, &m)| m).map(|(t, _)| t) {
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            if !middles.contains_key(&key(a, b)) {
                let m = add_middle(mesh, a, b);
                middles.insert(key(a, b), m);
            }
        }
    }
    let mut out = Vec::with_capacity(faces.len() * 2);
    for t in faces.chunks_exact(3) {
        let [a, b, c] = [t[0], t[1], t[2]];
        let m = |x: u32, y: u32| middles.get(&key(x, y)).copied();
        match (m(a, b), m(b, c), m(c, a)) {
            (None, None, None) => out.extend([a, b, c]),
            (Some(ab), Some(bc), Some(ca)) => out.extend([a, ab, ca, ab, b, bc, ca, bc, c, ab, bc, ca]),
            (Some(ab), None, None) => out.extend([a, ab, c, ab, b, c]),
            (None, Some(bc), None) => out.extend([b, bc, a, bc, c, a]),
            (None, None, Some(ca)) => out.extend([c, ca, b, ca, a, b]),
            (Some(ab), Some(bc), None) => out.extend([ab, b, bc, a, ab, bc, a, bc, c]),
            (None, Some(bc), Some(ca)) => out.extend([bc, c, ca, b, bc, ca, b, ca, a]),
            (Some(ab), None, Some(ca)) => out.extend([ca, a, ab, c, ca, ab, c, ab, b]),
        }
    }
    mesh.faces = out;
}

/// Puts the pattern on the tops, returns whether anything changed
fn paint(mesh: &mut Mesh, tops: &[Top], pattern: &Pattern, color: [u8; 3]) -> bool {
    if !pattern.rings.iter().flatten().any(|&v| v > 0.0) {
        return false;
    }
    let mut located = Vec::with_capacity(mesh.vertex_count());
    locate_from(mesh, tops, &mut located);
    let groups = top_groups(mesh, &located);
    // the faces on the tops are split until they are about as fine as the pattern
    let detail = DETAIL * (pattern.outer - pattern.inner) / pattern.rings.len().saturating_sub(1).max(1) as f32;
    let limit = MAX_GROWTH * mesh.vertex_count();
    for _ in 0..MAX_SPLITS {
        let on_top = |v: u32| located[v as usize].as_ref().is_some_and(|(k, _)| groups.get(k) == Some(&mesh.groups[v as usize]));
        let position = |v: u32| Vec3::from_slice(&mesh.positions[v as usize * 3..v as usize * 3 + 3]);
        let marked: Vec<bool> = mesh
            .faces
            .chunks_exact(3)
            .map(|t| {
                t.iter().any(|&v| on_top(v))
                    && [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])].iter().any(|&(a, b)| position(a).distance(position(b)) > detail)
            })
            .collect();
        let count = marked.iter().filter(|&&m| m).count();
        if count == 0 || mesh.vertex_count() + 3 * count > limit {
            break;
        }
        refine(mesh, &marked);
        locate_from(mesh, tops, &mut located);
    }

    let n = mesh.vertex_count();
    let color = linear_rgba(opaque(color));
    let mut moved = vec![false; n];
    let mut painted = false;
    for (v, on) in located.into_iter().enumerate() {
        let Some((k, on)) = on else {
            continue;
        };
        if groups.get(&k) != Some(&mesh.groups[v]) {
            continue;
        }
        let (top, p) = (&tops[k], Vec3::from_slice(&mesh.positions[v * 3..v * 3 + 3]));
        let value = pattern.sample(on.angle, on.s).unwrap_or(0.0).clamp(0.0, 1.0) * on.weight;
        if value <= 0.0 {
            continue;
        }
        let lift = top.relief.at(on.alpha) * value;
        if lift != 0.0 {
            let normal = Vec3::from_slice(&mesh.normals[v * 3..v * 3 + 3]);
            mesh.positions[v * 3..v * 3 + 3].copy_from_slice(&(p + normal * lift).to_array());
            moved[v] = true;
        }
        let cover = (top.opacity.at(on.alpha) * value).clamp(0.0, 1.0);
        if cover > 0.0 {
            if mesh.colors.is_empty() {
                mesh.colors = vec![NO_COLOR; n];
            }
            mesh.colors[v] = over(mesh.colors[v], color, cover);
            painted = true;
        }
    }
    let any_moved = moved.iter().any(|&m| m);
    if any_moved {
        renormal(mesh, &moved);
    }
    any_moved || painted
}

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    // an input nothing is connected to is not a mesh
    let Some(mut mesh) = Mesh::try_decode(args[0]) else {
        return Mesh::default().encode();
    };
    let Some(pattern) = Pattern::decode(args[2]) else {
        return args[0].to_vec();
    };
    let items = split_args(args[1]);
    let caps = Cap::all(&items);
    let mut ctx = Ctx::default();
    let tops: Vec<Top> = caps
        .iter()
        .enumerate()
        .filter_map(|(rank, cap)| {
            ctx.set_path(items[cap.stipe], rank, caps.len());
            ctx.alpha = cap.frame.alpha;
            let relief = AlongRay::read(&mut ctx, args[3], even);
            let opacity = AlongRay::read(&mut ctx, args[5], even);
            Top::new(cap, &items, relief, opacity)
        })
        .filter(|top| !(top.relief.is_zero() && top.opacity.is_zero()))
        .collect();
    if tops.is_empty() || !paint(&mut mesh, &tops, &pattern, read_color(args[4])) {
        return args[0].to_vec();
    }
    mesh.encode()
}

#[cfg(test)]
mod tests;
