//! The rays of a cap, as the `cap` node makes them, seen in the plane through the stipe axis
//! that each of them lies in. The nodes that pose a cap (`cap_open`, `margin_curl`,
//! `margin_waves`) turn the steps of every ray in that plane by an angle that depends on how far
//! along the cap the step is. That keeps every step as long as it was, and turns add up, so the
//! poses can follow each other in a graph.
//!
//! A cap is found in a list of paths by its rays: the paths one node grew at one place of a path
//! of the list, its stipe, one layer deeper than it, at least one of them starting in the stipe
//! below that place or on the far side of the axis. A branch that grows out of the stipe starts
//! at the place, so it is no ray. The frame of the cap is read from the stipe at that place, see
//! [`CapFrame::on`].
//!
//! A ray that leaves the stipe starts on the far side of the axis and crosses it at the top of
//! the cap, from there on it measures the cap. Every ray bends by how far along its own length a
//! step is, so on a cap that reaches further on one side the poses stretch with the rays. A ray
//! that starts further out (added by `fill_gaps`) starts between two neighbours that reach
//! further in, and stays between them while they bend, so it bends as if it had been part of the
//! cap from the axis on.

use glam::{Vec2, Vec3};
use nodarium_utils::{
    geometry::{is_path, orthogonal_to, path_link, wrap_path, wrap_path_mut},
    Point,
};
use std::collections::HashMap;
use std::f32::consts::{PI, TAU};

/// How far a ray starts from the place its cap sits at least, below it or across the axis, as a
/// share of the stipe radius
const OFF_PLACE: f32 = 1e-4;

/// The frame of a cap: where it sits on its stipe, its axis and the side its angles count from
#[derive(Clone, Copy, Debug)]
pub struct CapFrame {
    pub hinge: Vec3,
    pub axis: Vec3,
    pub side: Vec3,
    pub around: Vec3,
    pub stipe_radius: f32,
    /// The stipe the cap grows on, the rays are its children
    pub stipe_id: i32,
    /// Where on the stipe the cap sits, 0 at its base to 1 at its tip
    pub alpha: f32,
}

impl CapFrame {
    /// The frame of a cap `alpha` along the length of `stipe`: the place there, the direction
    /// of the stipe as the axis and its radius. Angles count from `orthogonal_to` the axis,
    /// whatever made the cap, so every node finds the same frame from the stipe alone. A quarter
    /// turn on is the side the stipe leans to, so a cap turns with its stipe.
    pub fn on(stipe: &[i32], alpha: f32) -> CapFrame {
        let path = wrap_path(stipe);
        let (hinge, axis, radius) = place_on(path.points, alpha);
        CapFrame::new(hinge, axis, radius, path.link.id, alpha)
    }

    /// The frame of a cap at `point`, for a point whose path is not at hand
    pub fn at(point: &Point) -> CapFrame {
        CapFrame::new(point.position, point.tangent, point.scale, point.parent_id, point.alpha)
    }

    fn new(hinge: Vec3, tangent: Vec3, radius: f32, stipe_id: i32, alpha: f32) -> CapFrame {
        let axis = tangent.try_normalize().unwrap_or(Vec3::Y);
        let side = orthogonal_to(axis);
        CapFrame { hinge, axis, side, around: axis.cross(side), stipe_radius: radius.abs(), stipe_id, alpha }
    }

    pub fn out(&self, angle: f32) -> Vec3 {
        self.side * angle.cos() + self.around * angle.sin()
    }

    /// Angle around the axis, 0 to 2π
    pub fn angle(&self, p: Vec3) -> f32 {
        let q = p - self.hinge;
        q.dot(self.around).atan2(q.dot(self.side)).rem_euclid(TAU)
    }
}

/// The place `alpha` along the length of a path, its segment there and the
/// radius. The tip is the last point and the last segment.
fn place_on(points: &[f32], alpha: f32) -> (Vec3, Vec3, f32) {
    let n = points.len() / 4;
    let point = |i: usize| Vec3::from_slice(&points[i * 4..i * 4 + 3]);
    let radius = |i: usize| points[i * 4 + 3];
    if n < 2 {
        return (if n == 1 { point(0) } else { Vec3::ZERO }, Vec3::Y, if n == 1 { radius(0) } else { 0.0 });
    }
    let direction = |i: usize| point(i + 1) - point(i);
    if alpha >= 1.0 {
        return (point(n - 1), direction(n - 2), radius(n - 1));
    }
    let lengths: Vec<f32> = (0..n - 1).map(|i| point(i).distance(point(i + 1))).collect();
    let target = alpha.max(0.0) * lengths.iter().sum::<f32>();
    let mut along = 0.0;
    for (i, &length) in lengths.iter().enumerate() {
        if along + length >= target && length > 0.0 {
            let t = ((target - along) / length).clamp(0.0, 1.0);
            return (point(i).lerp(point(i + 1), t), direction(i), radius(i) + (radius(i + 1) - radius(i)) * t);
        }
        along += length;
    }
    (point(n - 1), direction(n - 2), radius(n - 1))
}

/// One ray in the plane through the axis
#[derive(Clone, Debug)]
pub struct CapRay {
    /// Which of the given paths it is
    pub path: usize,
    /// Angle around the axis
    pub angle: f32,
    /// Points as (distance from the axis, negative on the far side, height over the hinge)
    pub plane: Vec<Vec2>,
    /// Distance along the ray to its tip
    pub to_end: Vec<f32>,
    /// Distance along the ray from the axis to the margin. A ray that starts out on the cap
    /// takes it from its neighbours.
    pub length: f32,
    /// How far along the ray every point is, 0 at the axis to 1 at the margin
    alphas: Vec<f32>,
    /// Starts in the stipe and crosses the axis
    pub crosses: bool,
    out: Vec3,
    /// Distance out of the plane, kept as it is
    across: Vec<f32>,
}

/// A step of a ray, while it bends
#[derive(Clone, Copy, Debug)]
pub struct Step {
    /// How far along its ray the step starts and ends, 0 at the axis to 1 at the margin
    pub from_alpha: f32,
    pub to_alpha: f32,
    /// Where the step starts and ends before it bends, in the plane of the ray
    pub from: Vec2,
    pub to: Vec2,
}

impl Step {
    pub fn length(&self) -> f32 {
        self.from.distance(self.to)
    }

    pub fn alpha(&self) -> f32 {
        0.5 * (self.from_alpha + self.to_alpha)
    }
}

/// Where a ray that starts out on the cap sits between two rays that reach further in: on each
/// of them the place nearest its start, and how far it is from the first to the second
#[derive(Clone, Copy, Debug)]
struct Carrier {
    a: (usize, usize, f32),
    b: (usize, usize, f32),
    share: f32,
}

/// The rays of one cap: the rays one node made at one place of a stipe
#[derive(Clone)]
pub struct Cap {
    pub frame: CapFrame,
    /// Which of the given paths the stipe is
    pub stipe: usize,
    /// Sorted by angle
    pub rays: Vec<CapRay>,
    /// Distance along the rays from the axis to the margin, the middle of all rays
    pub length: f32,
    /// The neighbours of every ray that starts out on the cap, found once, a cap bends its
    /// rays many times
    carriers: Vec<Option<Carrier>>,
}

impl Cap {
    /// The caps among `paths`, see the module docs. Paths of no cap are not in any.
    pub fn all(paths: &[&[i32]]) -> Vec<Cap> {
        let index: HashMap<i32, usize> = paths
            .iter()
            .enumerate()
            .filter(|(_, p)| is_path(p) && wrap_path(p).length > 1)
            .map(|(i, p)| (path_link(p).id, i))
            .collect();
        let mut groups: Vec<((usize, i32, u32), Vec<usize>)> = Vec::new();
        for (i, item) in paths.iter().enumerate() {
            if !is_path(item) {
                continue;
            }
            let link = path_link(item);
            let Some(&stipe) = index.get(&link.parent_id) else {
                continue;
            };
            if item[3] != paths[stipe][3] + 1 {
                continue;
            }
            let key = (stipe, link.creator, link.parent_alpha.to_bits());
            match groups.iter_mut().find(|g| g.0 == key) {
                Some(group) => group.1.push(i),
                None => groups.push((key, vec![i])),
            }
        }
        groups
            .into_iter()
            .filter_map(|((stipe, _, alpha), rays)| {
                Cap::new(CapFrame::on(paths[stipe], f32::from_bits(alpha)), stipe, paths, &rays)
            })
            .collect()
    }

    /// The cap of the rays `members` of `paths` in `frame`, `None` when no ray starts in the
    /// stipe
    pub fn new(frame: CapFrame, stipe: usize, paths: &[&[i32]], members: &[usize]) -> Option<Cap> {
        let mut rays = Vec::new();
        let mut lengths = Vec::new();
        let mut in_stipe = false;
        for &index in members {
            let path = wrap_path(paths[index]);
            let n = path.length;
            if n < 2 {
                continue;
            }
            let rel = |i: usize| Vec3::from_slice(&path.points[i * 4..i * 4 + 3]) - frame.hinge;
            let flat = |q: Vec3| q - frame.axis * q.dot(frame.axis);
            // the point furthest from the axis is on the cap, never in the stipe, so it shows
            // the side of the axis the ray is on however the cap is posed
            let far = (0..n).map(|i| flat(rel(i))).max_by(|a, b| a.length_squared().total_cmp(&b.length_squared())).unwrap();
            let Some(out) = far.try_normalize() else {
                continue;
            };
            let normal = frame.axis.cross(out);
            let plane: Vec<Vec2> = (0..n).map(|i| Vec2::new(rel(i).dot(out), rel(i).dot(frame.axis))).collect();
            let across: Vec<f32> = (0..n).map(|i| rel(i).dot(normal)).collect();
            let mut to_end = vec![0.0f32; n];
            for i in (0..n - 1).rev() {
                to_end[i] = to_end[i + 1] + rel(i).distance(rel(i + 1));
            }
            let crossing = (1..n).find(|&k| plane[k - 1].x < 0.0 && plane[k].x >= 0.0);
            let length = crossing.map(|k| {
                let t = -plane[k - 1].x / (plane[k].x - plane[k - 1].x);
                to_end[k - 1] + (to_end[k] - to_end[k - 1]) * t
            });
            lengths.extend(length);
            let off = OFF_PLACE * frame.stipe_radius;
            in_stipe |= plane[0].y < -off || (crossing.is_some() && plane[0].x < -off);
            rays.push(CapRay {
                path: index,
                angle: frame.angle(frame.hinge + out),
                plane,
                to_end,
                length: length.unwrap_or(-1.0),
                alphas: Vec::new(),
                crosses: crossing.is_some(),
                out,
                across,
            });
        }
        if !in_stipe {
            return None;
        }
        lengths.sort_by(f32::total_cmp);
        let length = match lengths.len() {
            0 => rays.iter().map(|r| r.to_end[0]).fold(0.0, f32::max),
            n => lengths[n / 2],
        };
        rays.sort_by(|a, b| a.angle.total_cmp(&b.angle));
        let own: Vec<(f32, f32)> = rays.iter().filter(|r| r.crosses).map(|r| (r.angle, r.length)).collect();
        for ray in &mut rays {
            if !ray.crosses {
                ray.length = length_between(&own, ray.angle).unwrap_or(length);
            }
            let l = ray.length;
            ray.alphas = ray.to_end.iter().map(|&d| if l > 0.0 { (1.0 - d / l).clamp(0.0, 1.0) } else { 0.0 }).collect();
        }
        let mut cap = Cap { frame, stipe, rays, length, carriers: Vec::new() };
        cap.carriers = (0..cap.rays.len()).map(|k| cap.carrier(k)).collect();
        Some(cap)
    }

    /// How far from the axis the furthest tip is
    pub fn reach(&self) -> f32 {
        self.rays.iter().map(|r| r.plane.last().unwrap().x).fold(0.0, f32::max)
    }

    fn step(&self, ray: &CapRay, i: usize, share: f32) -> Step {
        Step {
            from_alpha: ray.alphas[i],
            to_alpha: ray.alphas[i] + (ray.alphas[i + 1] - ray.alphas[i]) * share,
            from: ray.plane[i],
            to: ray.plane[i].lerp(ray.plane[i + 1], share),
        }
    }

    /// How far in a ray starts, the rays that leave the stipe start furthest in
    fn start_alpha(&self, ray: &CapRay) -> f32 {
        if ray.crosses {
            -1.0
        } else {
            ray.alphas[0]
        }
    }

    /// The neighbours of ray `k` on both sides that start further in, as `fill_gaps` placed
    /// it between them
    fn carrier(&self, k: usize) -> Option<Carrier> {
        let ray = &self.rays[k];
        if ray.crosses {
            return None;
        }
        let start = self.start_alpha(ray);
        let (mut before, mut after): (Option<(usize, f32)>, Option<(usize, f32)>) = (None, None);
        for (j, other) in self.rays.iter().enumerate() {
            if j == k || self.start_alpha(other) >= start - 1e-5 {
                continue;
            }
            let ahead = (other.angle - ray.angle).rem_euclid(TAU);
            let behind = TAU - ahead;
            if after.is_none_or(|(_, d)| ahead < d) {
                after = Some((j, ahead));
            }
            if before.is_none_or(|(_, d)| behind < d) {
                before = Some((j, behind));
            }
        }
        let ((a, da), (b, db)) = match (before, after) {
            (Some(a), Some(b)) => (a, b),
            (Some(a), None) | (None, Some(a)) => (a, a),
            _ => return None,
        };
        let nearest = |j: usize| {
            let (i, t) = nearest_on(&self.rays[j].plane, ray.plane[0]);
            (j, i, t)
        };
        let share = if da + db > 0.0 { da / (da + db) } else { 0.0 };
        Some(Carrier { a: nearest(a), b: nearest(b), share })
    }

    /// Where the place `share` along step `i` of ray `k` moves when the ray bends by `turn`,
    /// and the turn of that step
    fn bent_at(&self, k: usize, i: usize, share: f32, turn: &impl Fn(&Step) -> f32) -> (Vec2, f32) {
        let ray = &self.rays[k];
        let mut p = self.start(k, turn);
        let mut last = 0.0;
        for j in 0..i {
            let step = self.step(ray, j, 1.0);
            last = turn(&step);
            p += Vec2::from_angle(-last).rotate(step.to - step.from);
        }
        if i + 1 < ray.plane.len() {
            let step = self.step(ray, i, share);
            last = turn(&step);
            p += Vec2::from_angle(-last).rotate(step.to - step.from);
        }
        (p, last)
    }

    /// Where the start of ray `k` moves when the cap bends by `turn`. A ray that starts out on
    /// the cap keeps its place between its neighbours.
    fn start(&self, k: usize, turn: &impl Fn(&Step) -> f32) -> Vec2 {
        let ray = &self.rays[k];
        let Some(c) = self.carriers[k] else {
            return ray.plane[0];
        };
        let place = |(j, i, t): (usize, usize, f32)| {
            let plane = &self.rays[j].plane;
            plane[i].lerp(plane[(i + 1).min(plane.len() - 1)], t)
        };
        let (a, turn_a) = self.bent_at(c.a.0, c.a.1, c.a.2, turn);
        let (b, turn_b) = self.bent_at(c.b.0, c.b.1, c.b.2, turn);
        let between = place(c.a).lerp(place(c.b), c.share);
        let last = turn_a + (turn_b - turn_a) * c.share;
        a.lerp(b, c.share) + Vec2::from_angle(-last).rotate(ray.plane[0] - between)
    }

    /// The ray `k` with every step turned by `turn` radians, down and in towards the stipe for a
    /// positive turn. The steps keep their lengths, the ray keeps its points.
    pub fn bend(&self, k: usize, turn: impl Fn(&Step) -> f32) -> Vec<Vec2> {
        let ray = &self.rays[k];
        let mut p = self.start(k, &turn);
        let mut points = Vec::with_capacity(ray.plane.len());
        points.push(p);
        for i in 0..ray.plane.len() - 1 {
            let step = self.step(ray, i, 1.0);
            p += Vec2::from_angle(-turn(&step)).rotate(step.to - step.from);
            points.push(p);
        }
        points
    }

    /// The ray `k` of `paths` (the paths the cap was made from) moved to `plane`
    pub fn write(&self, k: usize, paths: &[&[i32]], plane: &[Vec2]) -> Vec<i32> {
        let ray = &self.rays[k];
        let mut data = paths[ray.path].to_vec();
        let path = wrap_path_mut(&mut data);
        let f = &self.frame;
        let normal = f.axis.cross(ray.out);
        for (i, q) in plane.iter().enumerate() {
            let p = f.hinge + ray.out * q.x + f.axis * q.y + normal * ray.across[i];
            path.points[i * 4..i * 4 + 3].copy_from_slice(&p.to_array());
        }
        data
    }
}

/// The step of `line` with the place nearest to `p`, and how far along the step it is
fn nearest_on(line: &[Vec2], p: Vec2) -> (usize, f32) {
    let (mut best, mut at) = (f32::MAX, (0, 0.0));
    for i in 0..line.len().saturating_sub(1) {
        let (u, v) = (line[i], line[i + 1]);
        let d = v - u;
        let t = if d.length_squared() > 0.0 { ((p - u).dot(d) / d.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
        let distance = p.distance(u + d * t);
        if distance < best {
            (best, at) = (distance, (i, t));
        }
    }
    at
}

/// The length at `angle` between the two rays of `own` (angle and length, sorted by angle) on
/// either side of it
fn length_between(own: &[(f32, f32)], angle: f32) -> Option<f32> {
    let (first, last) = (own.first()?, own.last()?);
    let after = own.partition_point(|r| r.0 <= angle);
    let (a, b) = match after {
        0 => ((last.0 - TAU, last.1), *first),
        i if i == own.len() => (*last, (first.0 + TAU, first.1)),
        i => (own[i - 1], own[i]),
    };
    let t = if b.0 > a.0 { (angle - a.0) / (b.0 - a.0) } else { 0.0 };
    Some(a.1 + (b.1 - a.1) * t)
}

/// The turn that closes a ray by `closing` radians, spread evenly from the axis to the margin
pub fn closing_turn(step: &Step, closing: f32) -> f32 {
    closing * step.alpha()
}

/// The turn that rolls the outer `reach` of a ray under by `curl` (1 = half a turn)
pub fn curl_turn(step: &Step, curl: f32, reach: f32) -> f32 {
    let a = step.alpha();
    let edge = 1.0 - reach.clamp(1e-3, 1.0);
    let t = ((a - edge) / (1.0 - edge)).clamp(0.0, 1.0);
    curl * PI * t * t * (3.0 - 2.0 * t)
}

/// Poses the caps among `paths`. `pose` gives the new plane of ray `k` of a cap, or `None` to
/// leave it. Every other path passes as it is.
pub fn pose_caps(paths: &[&[i32]], pose: impl FnMut(&Cap, usize) -> Option<Vec<Vec2>>) -> Vec<Vec<i32>> {
    pose_caps_of(&Cap::all(paths), paths, pose)
}

/// Like `pose_caps`, for the caps of `paths` found before
pub fn pose_caps_of(caps: &[Cap], paths: &[&[i32]], mut pose: impl FnMut(&Cap, usize) -> Option<Vec<Vec2>>) -> Vec<Vec<i32>> {
    let mut output: Vec<Vec<i32>> = paths.iter().map(|p| p.to_vec()).collect();
    for cap in caps {
        for k in 0..cap.rays.len() {
            if let Some(plane) = pose(cap, k) {
                output[cap.rays[k].path] = cap.write(k, paths, &plane);
            }
        }
    }
    output
}

/// The tips of the rays of every cap among `paths`, in a ring around its axis
pub fn margin_points(paths: &[&[i32]]) -> Vec<Point> {
    let mut margins = Vec::new();
    for cap in Cap::all(paths) {
        for ray in &cap.rays {
            let path = wrap_path(paths[ray.path]);
            let n = path.length;
            let point = |i: usize| Vec3::from_slice(&path.points[i * 4..i * 4 + 3]);
            let tip = point(n - 1);
            let tangent = (tip - point(n - 2)).try_normalize().unwrap_or(ray.out);
            margins.push(Point {
                position: tip,
                direction: (ray.out - tangent * ray.out.dot(tangent))
                    .try_normalize()
                    .unwrap_or_else(|| orthogonal_to(tangent)),
                tangent,
                scale: path.points[(n - 1) * 4 + 3],
                parent_id: path.link.id,
                alpha: 1.0,
                depth: path.depth,
                index: 0,
            });
        }
    }
    margins
}

/// Caps on a stipe for the tests of the nodes that pose them
#[doc(hidden)]
pub mod testing {
    use crate::{grow_cap, ray_angles, Cap, CapFrame, CapShape};
    use glam::{Quat, Vec2, Vec3};
    use nodarium_utils::{
        concat_arg_vecs, curve,
        geometry::{create_multiple_paths, fill_gaps, path_link, wrap_multiple_paths, wrap_path},
        split_args, Point,
    };
    use std::f32::consts::TAU;

    pub const STIPE_TOP: f32 = 0.05;
    pub const STIPE_RADIUS: f32 = 0.005;

    /// An upright stipe 5 cm tall, 5 mm in radius
    pub fn stipe() -> Vec<i32> {
        leaning_stipe(0.0)
    }

    /// The stipe, leaning `lean` radians away from upright
    pub fn leaning_stipe(lean: f32) -> Vec<i32> {
        stipes(&[(Vec3::ZERO, lean)])
    }

    /// Stipes standing at `base`, each leaning its own way, made by node 3
    pub fn stipes(stipes: &[(Vec3, f32)]) -> Vec<i32> {
        let mut data = create_multiple_paths(stipes.len(), 12, 1, 3);
        for (path, &(base, lean)) in wrap_multiple_paths(&mut data).iter_mut().zip(stipes) {
            let tilt = Quat::from_rotation_z(lean);
            for i in 0..12 {
                let p = base + tilt * Vec3::new(0.0, STIPE_TOP * i as f32 / 11.0, 0.0);
                path.points[i * 4..i * 4 + 4].copy_from_slice(&[p.x, p.y, p.z, STIPE_RADIUS]);
            }
        }
        data
    }

    #[derive(Clone)]
    pub struct TestCap {
        pub radius: f32,
        /// A curve of the share of the radius once around the axis, empty for a round cap
        pub around: Vec<f32>,
        pub profile: Vec<f32>,
        pub thickness: f32,
        /// Share of the thickness from the centre to the margin
        pub thickness_curve: Vec<f32>,
        pub rays: usize,
        /// How far apart `fill_gaps` keeps the rays, 0 adds none
        pub spacing: f32,
        /// The seed of `fill_gaps`
        pub seed: i32,
    }

    /// A cone from the top at y = 20 down to the margin at y = 60 of the shape editor: the
    /// margin is 0.8 radii below the top. The flesh thins to nothing at the margin.
    pub fn cone() -> TestCap {
        TestCap {
            radius: 0.03,
            around: Vec::new(),
            profile: vec![0.0, 60.0, 50.0, 20.0],
            thickness: 0.006,
            thickness_curve: vec![0.0, 1.0, 1.0, 0.0],
            rays: 10,
            spacing: 0.5,
            seed: 3,
        }
    }

    /// The paths after `cap` and `fill_gaps`: the rays at the tip of the stipe, the stipe and
    /// the rays `fill_gaps` adds
    pub fn grow(c: &TestCap) -> Vec<Vec<i32>> {
        grow_on(c, &stipe())
    }

    /// Like `grow`, at the tip of every path of `stipes`
    pub fn grow_on(c: &TestCap, stipes: &[i32]) -> Vec<Vec<i32>> {
        let mut paths = Vec::new();
        for stipe in split_args(stipes) {
            paths.extend(grow_rays(c, stipe, 1.0));
            paths.push(stipe.to_vec());
        }
        let refs: Vec<&[i32]> = paths.iter().map(|r| r.as_slice()).collect();
        let added = fill_gaps(&refs, c.spacing, c.seed);
        paths.extend(added);
        paths
    }

    /// The rays of the open cap `alpha` along `stipe`, as the `cap` node grows them
    pub fn grow_rays(c: &TestCap, stipe: &[i32], alpha: f32) -> Vec<Vec<i32>> {
        let thickness: Vec<f32> = crate::cap_alphas().map(|a| c.thickness * curve(&c.thickness_curve, a)).collect();
        let frame = CapFrame::on(stipe, alpha);
        let radii: Vec<f32> = ray_angles(c.rays, frame.stipe_id)
            .into_iter()
            .map(|a| if c.around.is_empty() { c.radius } else { c.radius * curve(&c.around, a.rem_euclid(TAU) / TAU) })
            .collect();
        let shape = CapShape { profile: &c.profile, radii: &radii, thickness: &thickness, resolution: 24, creator: 7 };
        grow_cap(&frame, wrap_path(stipe).depth, 0, &shape)
    }

    /// The paths that grow on another path, the rays without their stipe
    pub fn rays(paths: &[Vec<i32>]) -> Vec<Vec<i32>> {
        paths.iter().filter(|p| path_link(p).has_parent()).cloned().collect()
    }

    pub fn list(paths: &[Vec<i32>]) -> Vec<i32> {
        concat_arg_vecs(paths.to_vec())
    }

    pub fn unlist(list: &[i32]) -> Vec<Vec<i32>> {
        split_args(list).iter().map(|p| p.to_vec()).collect()
    }

    /// The caps among the paths
    pub fn caps(paths: &[Vec<i32>]) -> Vec<Cap> {
        let refs: Vec<&[i32]> = paths.iter().map(|r| r.as_slice()).collect();
        Cap::all(&refs)
    }

    /// The frame of the first cap among the paths
    pub fn frame(paths: &[Vec<i32>]) -> CapFrame {
        caps(paths)[0].frame
    }

    pub fn margins(paths: &[Vec<i32>]) -> Vec<Point> {
        let refs: Vec<&[i32]> = paths.iter().map(|r| r.as_slice()).collect();
        crate::margin_points(&refs)
    }

    /// How far the margin points stray from the middle of their neighbours around the axis at
    /// most, a margin that is smooth all round strays little
    pub fn roughness(margins: &[Point], frame: &CapFrame) -> f32 {
        let mut ring: Vec<(f32, Vec3)> = margins.iter().map(|m| (frame.angle(m.position), m.position)).collect();
        ring.sort_by(|a, b| a.0.total_cmp(&b.0));
        let n = ring.len();
        (0..n).map(|k| ring[k].1.distance(0.5 * (ring[(k + n - 1) % n].1 + ring[(k + 1) % n].1))).fold(0.0, f32::max)
    }

    /// Angle around the axis, distance from it and height along it of every margin point,
    /// sorted by angle
    pub fn ring(margins: &[Point], frame: &CapFrame) -> Vec<[f32; 3]> {
        let mut ring: Vec<[f32; 3]> = margins
            .iter()
            .map(|m| {
                let q = m.position - frame.hinge;
                [frame.angle(m.position), q.reject_from(frame.axis).length(), q.dot(frame.axis)]
            })
            .collect();
        ring.sort_by(|a, b| a[0].total_cmp(&b[0]));
        ring
    }

    /// The largest distance between two rings of margin points, each point of `a` against the
    /// place of `b` at its angle
    pub fn ring_distance(a: &[[f32; 3]], b: &[[f32; 3]]) -> f32 {
        let at = |angle: f32| {
            let n = b.len();
            let j = b.partition_point(|q| q[0] < angle);
            let (lo, hi) = (b[(j + n - 1) % n], b[j % n]);
            let span = (hi[0] - lo[0]).rem_euclid(std::f32::consts::TAU);
            let t = if span > 0.0 { (angle - lo[0]).rem_euclid(std::f32::consts::TAU) / span } else { 0.0 };
            Vec2::new(lo[1] + (hi[1] - lo[1]) * t, lo[2] + (hi[2] - lo[2]) * t)
        };
        a.iter().map(|p| Vec2::new(p[1], p[2]).distance(at(p[0]))).fold(0.0, f32::max)
    }

    /// Distance from the axis and height over the stipe top
    pub fn place(p: Vec3) -> Vec2 {
        Vec2::new(Vec2::new(p.x, p.z).length(), p.y - STIPE_TOP)
    }

    pub fn points(path: &[i32]) -> Vec<Vec3> {
        wrap_path(path).points.chunks_exact(4).map(|p| Vec3::new(p[0], p[1], p[2])).collect()
    }

    /// How long every step of a path is
    pub fn steps(path: &[i32]) -> Vec<f32> {
        points(path).windows(2).map(|w| w[0].distance(w[1])).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::testing::{
        caps, cone, frame, grow, grow_on, grow_rays, leaning_stipe, margins, points, rays, ring, roughness, stipe, stipes,
        TestCap, STIPE_RADIUS, STIPE_TOP,
    };
    use super::{curl_turn, pose_caps, Cap, CapFrame, CapRay};
    use glam::{Vec2, Vec3};
    use nodarium_utils::{
        geometry::{create_path, path_link, set_path_link, wrap_path, wrap_path_mut, PathLink},
        split_args,
    };

    fn smoothstep(x: f32) -> f32 {
        let t = x.clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    }

    /// Waves the margin like `margin_waves` (6 lobes), then rolls it under like `margin_curl`
    fn wave_and_curl(paths: Vec<Vec<i32>>, amplitude: f32, curl: f32) -> Vec<Vec<i32>> {
        let refs: Vec<&[i32]> = paths.iter().map(|r| r.as_slice()).collect();
        let radius = 0.03;
        let waved = pose_caps(&refs, |cap, k| {
            let lift = amplitude * radius * (6.0 * cap.rays[k].angle).cos();
            let offset = |x: f32| lift * smoothstep(x / radius);
            Some(cap.bend(k, |s| -((offset(s.to.x) - offset(s.from.x)) / s.length().max(1e-9)).atan()))
        });
        let refs: Vec<&[i32]> = waved.iter().map(|r| r.as_slice()).collect();
        pose_caps(&refs, |cap, k| Some(cap.bend(k, |s| curl_turn(s, curl, 0.4))))
    }

    fn many_added() -> TestCap {
        TestCap { rays: 12, spacing: 0.25, ..cone() }
    }

    /// Twice as long at alpha 0 as halfway round
    fn lopsided() -> TestCap {
        TestCap { around: vec![0.0, 1.0, 0.5, 0.5, 1.0, 1.0], ..many_added() }
    }

    /// The rays that leave the stipe on either side of `angle`
    fn neighbours(cap: &Cap, angle: f32) -> (&CapRay, &CapRay) {
        let own: Vec<&CapRay> = cap.rays.iter().filter(|r| r.crosses).collect();
        let after = own.iter().position(|r| r.angle > angle).unwrap_or(0);
        (own[(after + own.len() - 1) % own.len()], own[after])
    }

    #[test]
    fn rays_added_on_a_lopsided_cap_end_on_the_outline_between_their_neighbours() {
        let paths = grow(&lopsided());
        let cap = &caps(&paths)[0];
        let tip = |r: &CapRay| super::testing::place(*points(&paths[r.path]).last().unwrap());
        let added: Vec<&CapRay> = cap.rays.iter().filter(|r| !r.crosses).collect();
        assert!(added.len() > 20, "only {} rays added", added.len());
        for ray in added {
            let (a, b) = neighbours(cap, ray.angle);
            let (p, pa, pb) = (tip(ray), tip(a), tip(b));
            let (lo, hi) = (pa.min(pb) - Vec2::splat(1e-5), pa.max(pb) + Vec2::splat(1e-5));
            assert!(p.cmpge(lo).all() && p.cmple(hi).all(), "tip {p} between {pa} and {pb}");
            assert!(ray.length >= a.length.min(b.length) - 1e-6 && ray.length <= a.length.max(b.length) + 1e-6);
        }
    }

    #[test]
    fn a_lopsided_cap_bends_every_ray_by_its_own_length() {
        let paths = grow(&lopsided());
        let cap = &caps(&paths)[0];
        let lengths = || cap.rays.iter().map(|r| r.length);
        let (long, short) = (lengths().fold(0.0, f32::max), lengths().fold(f32::MAX, f32::min));
        assert!(long > 1.7 * short, "rays from {short} to {long}");
        // the margin curls under the same share of every ray, so it drops by about the length of
        // its ray. By the middle length of the cap the long side would drop half the share.
        let refs: Vec<&[i32]> = paths.iter().map(|r| r.as_slice()).collect();
        let curled = pose_caps(&refs, |cap, k| Some(cap.bend(k, |s| curl_turn(s, 0.5, 0.4))));
        let drops: Vec<f32> = cap
            .rays
            .iter()
            .filter(|r| r.crosses)
            .map(|r| {
                let (before, after) = (points(&paths[r.path]), points(&curled[r.path]));
                (before.last().unwrap().y - after.last().unwrap().y) / r.length
            })
            .collect();
        let (most, least) = (drops.iter().cloned().fold(0.0, f32::max), drops.iter().cloned().fold(f32::MAX, f32::min));
        assert!(least > 0.0 && most < 1.3 * least, "the margin drops {least} to {most} of its ray");
    }

    #[test]
    fn the_frame_is_the_stipe_top_with_its_last_segment_as_the_axis() {
        let f = CapFrame::on(split_args(&leaning_stipe(0.3))[0], 1.0);
        let lean = glam::Quat::from_rotation_z(0.3);
        assert!(f.hinge.distance(lean * Vec3::new(0.0, STIPE_TOP, 0.0)) < 1e-7);
        assert!(f.axis.distance(lean * Vec3::Y) < 1e-6);
        assert_eq!(f.stipe_radius, STIPE_RADIUS);
        assert!(f.side.dot(f.axis).abs() < 1e-6 && f.around.dot(f.axis).abs() < 1e-6);
        // halfway up, between two points of the stipe
        let half = CapFrame::on(split_args(&stipe())[0], 0.5);
        assert!(half.hinge.distance(Vec3::new(0.0, 0.5 * STIPE_TOP, 0.0)) < 1e-6);
        assert!(half.axis.distance(Vec3::Y) < 1e-6);
    }

    #[test]
    fn rays_added_on_the_cap_stay_between_their_neighbours_on_a_wavy_curled_cap() {
        let paths = grow_on(&many_added(), &stipe());
        let f = frame(&paths);
        let added = rays(&paths).len() - 12;
        assert!(added > 30, "only {added} rays added");
        let flat = roughness(&margins(&paths), &f);
        let posed = wave_and_curl(paths, 0.18, 0.8);
        let rough = roughness(&margins(&posed), &f);
        // the ring of a wavy margin bends, but no ray juts out of it
        assert!(rough < 0.04 * 0.03, "a margin point strays {rough} from its neighbours, {flat} before");
        // the start of every added ray stays on the cap, inside the flesh of its neighbours
        let posed = rays(&posed);
        for ray in &posed[12..] {
            let start = points(ray)[0];
            let others = posed.iter().filter(|r| !std::ptr::eq(*r, ray));
            let nearest = others.flat_map(|r| points(r)).map(|p| p.distance(start)).fold(f32::MAX, f32::min);
            assert!(nearest < 0.004, "an added ray starts {nearest} from every other ray");
        }
    }

    #[test]
    fn a_cap_on_a_leaning_stipe_poses_as_it_does_upright() {
        // three kinks of 14° in the chanterelle troop lean a stipe by up to 42°
        let pose = |lean: f32| {
            let paths = grow_on(&many_added(), &leaning_stipe(lean));
            let f = frame(&paths);
            let posed = wave_and_curl(paths, 0.18, 0.8);
            (ring(&margins(&posed), &f), roughness(&margins(&posed), &f))
        };
        let (upright, _) = pose(0.0);
        let (leaning, rough) = pose(42f32.to_radians());
        assert!(rough < 0.04 * 0.03, "a margin point strays {rough} from its neighbours");
        assert_eq!(upright.len(), leaning.len());
        for (a, b) in upright.iter().zip(&leaning) {
            for d in 0..3 {
                assert!((a[d] - b[d]).abs() < 2e-4, "margin {a:?} upright, {b:?} leaning");
            }
        }
    }

    #[test]
    fn several_stipes_keep_every_cap_on_its_own_stipe() {
        let list = stipes(&[(Vec3::ZERO, 0.0), (Vec3::new(0.1, 0.0, 0.0), 0.4), (Vec3::new(0.0, 0.0, 0.1), -0.2)]);
        let paths = grow_on(&TestCap { spacing: 0.0, ..cone() }, &list);
        let found = caps(&paths);
        assert_eq!(found.len(), 3);
        for (cap, stipe) in found.iter().zip(split_args(&list)) {
            let id = wrap_path(stipe).link.id;
            assert_eq!(cap.frame.stipe_id, id);
            assert_eq!(path_link(&paths[cap.stipe]).id, id);
            assert_eq!(cap.rays.len(), 10);
            assert!(cap.rays.iter().all(|r| path_link(&paths[r.path]).parent_id == id));
            assert!(cap.frame.hinge.distance(*points(stipe).last().unwrap()) < 1e-7);
        }
        // every cap curls around its own axis, as it does alone
        let posed = wave_and_curl(paths.clone(), 0.18, 0.8);
        for cap in &found {
            let id = cap.frame.stipe_id;
            let own: Vec<usize> = (0..paths.len())
                .filter(|&i| path_link(&paths[i]).id == id || path_link(&paths[i]).parent_id == id)
                .collect();
            let alone = wave_and_curl(own.iter().map(|&i| paths[i].clone()).collect(), 0.18, 0.8);
            for (&i, path) in own.iter().zip(&alone) {
                assert_eq!(&posed[i], path);
            }
        }
    }

    #[test]
    fn paths_that_are_no_rays_pass_unchanged() {
        let mut paths = grow(&TestCap { spacing: 0.0, ..cone() });
        let id = wrap_path(split_args(&stipe())[0]).link.id;
        // branches out of the stipe halfway up and at the top, made by other nodes
        for (k, alpha) in [0.5f32, 1.0].into_iter().enumerate() {
            let mut branch = create_path(8, 2);
            set_path_link(&mut branch, PathLink::child(20 + k as i32, id, alpha, 0));
            let path = wrap_path_mut(&mut branch);
            for i in 0..8 {
                let p = Vec3::new(0.004 * i as f32, alpha * STIPE_TOP + 0.002 * i as f32, 0.0);
                path.points[i * 4..i * 4 + 4].copy_from_slice(&[p.x, p.y, p.z, 0.002]);
            }
            paths.push(branch);
        }
        // rays whose stipe is not in the list
        let mut orphans = grow_rays(&cone(), split_args(&leaning_stipe(0.2))[0], 1.0);
        for ray in &mut orphans {
            let link = path_link(ray);
            set_path_link(ray, PathLink::child(link.creator, 77, 1.0, link.index));
        }
        paths.extend(orphans);
        assert_eq!(caps(&paths).len(), 1);
        let posed = wave_and_curl(paths.clone(), 0.18, 0.8);
        let n = paths.len();
        assert_eq!(posed[11..], paths[11..]);
        assert_ne!(posed[..10], paths[..10]);
        assert_eq!(posed[10], paths[10]);
        assert_eq!(n, 11 + 2 + 10);
    }

    #[test]
    fn a_cap_partway_up_its_stipe_poses_around_that_place() {
        let s = stipe();
        let mut paths = grow_rays(&TestCap { spacing: 0.0, ..cone() }, split_args(&s)[0], 0.6);
        paths.push(split_args(&s)[0].to_vec());
        let f = frame(&paths);
        assert!(f.hinge.distance(Vec3::new(0.0, 0.6 * STIPE_TOP, 0.0)) < 1e-6);
        assert_eq!(f.alpha, 0.6);
        let at_top = grow(&TestCap { spacing: 0.0, ..cone() });
        let lowered = |q: Vec3| q - Vec3::new(0.0, 0.4 * STIPE_TOP, 0.0);
        let curled = wave_and_curl(paths, 0.18, 0.8);
        for (a, b) in rays(&curled).iter().zip(rays(&wave_and_curl(at_top, 0.18, 0.8)).iter()) {
            for (x, y) in points(a).iter().zip(points(b)) {
                assert!(x.distance(lowered(y)) < 1e-5, "{x} and {y}");
            }
        }
    }
}
