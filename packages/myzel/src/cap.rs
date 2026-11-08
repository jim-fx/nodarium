//! The rays of a cap, as the `cap` node makes them, seen in the plane through the stipe axis
//! that each of them lies in. The nodes that pose a cap (`cap_open`, `margin_curl`,
//! `margin_waves`) turn the steps of every ray in that plane by an angle that depends on how far
//! along the cap the step is. That keeps every step as long as it was, and turns add up, so the
//! poses can follow each other in a graph.
//!
//! A ray that leaves the stipe starts on the far side of the axis and crosses it at the top of
//! the cap, from there on it measures the cap. A ray that starts further out (added by
//! `fill_gaps`) starts between two neighbours that reach further in, and stays between them
//! while they bend, so it bends as if it had been part of the cap from the axis on.

use glam::{Vec2, Vec3};
use nodarium_utils::{
    geometry::{orthogonal_to, path_link, wrap_path, wrap_path_mut},
    Point,
};
use std::f32::consts::{PI, TAU};

/// The frame of a cap from its hinge point: where it sits, its axis and the side its angles
/// count from, as the `cap` node sets them
#[derive(Clone, Copy, Debug)]
pub struct CapFrame {
    pub hinge: Vec3,
    pub axis: Vec3,
    pub side: Vec3,
    pub around: Vec3,
    pub stipe_radius: f32,
    /// The stipe the cap grows on, the rays are its children
    pub stipe_id: i32,
}

impl CapFrame {
    pub fn new(hinge: &Point) -> CapFrame {
        let axis = hinge.tangent.try_normalize().unwrap_or(Vec3::Y);
        let side = orthogonal_to(axis);
        CapFrame {
            hinge: hinge.position,
            axis,
            side,
            around: axis.cross(side),
            stipe_radius: hinge.scale.abs(),
            stipe_id: hinge.parent_id,
        }
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
    /// How far along the cap every point is, 0 at the axis to 1 at the margin. On a cap that
    /// follows another one (ridges under the flesh) it is read off the cap it follows.
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
    /// How far along the cap the step starts and ends, 0 at the axis to 1 at the margin
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

/// How a ray of a cap that lies against another cap (ridges under the flesh) hangs on the two
/// rays of that cap around it: for every point the place nearest to it on each of them, and
/// where it is from there, along and across that ray
#[derive(Clone, Debug)]
struct Attachment {
    a: usize,
    b: usize,
    share: f32,
    points: Vec<[(usize, f32, Vec2); 2]>,
}

/// The rays of one cap: the rays one node made on one stipe
#[derive(Clone)]
pub struct Cap {
    pub frame: CapFrame,
    /// Sorted by angle
    pub rays: Vec<CapRay>,
    /// Distance along the rays from the axis to the margin
    pub length: f32,
    /// The neighbours of every ray that starts out on the cap, found once, a cap bends its
    /// rays many times
    carriers: Vec<Option<Carrier>>,
    /// The cap on the same stipe that reaches furthest, when this one lies against it: it
    /// bends with it instead of on its own
    outer: Option<Box<Cap>>,
    attachments: Vec<Attachment>,
}

impl Cap {
    /// The caps among `paths` with a hinge in `hinges`. Caps of different nodes on one stipe
    /// (a cap and its ridges, joined) stay apart.
    pub fn all(paths: &[&[i32]], hinges: &[Point]) -> Vec<Cap> {
        let mut caps = Vec::new();
        let mut stipes = Vec::new();
        for hinge in hinges {
            if stipes.contains(&hinge.parent_id) {
                continue;
            }
            stipes.push(hinge.parent_id);
            let mut creators = Vec::new();
            for item in paths {
                let link = path_link(item);
                if item.len() >= 9 && link.parent_id == hinge.parent_id && !creators.contains(&link.creator) {
                    creators.push(link.creator);
                }
            }
            let mut joined: Vec<Cap> = creators.into_iter().filter_map(|creator| Cap::new(hinge, paths, creator)).collect();
            if let Some(outer) = Self::outermost(&joined) {
                let reference = joined[outer].clone();
                for (i, cap) in joined.iter_mut().enumerate() {
                    if i != outer {
                        cap.follow(&reference);
                    }
                }
            }
            caps.extend(joined);
        }
        caps
    }

    /// Of the caps on one stipe, the one that reaches furthest, the others follow it
    pub fn outermost(caps: &[Cap]) -> Option<usize> {
        (caps.len() > 1).then(|| (0..caps.len()).max_by(|&a, &b| caps[a].reach().total_cmp(&caps[b].reach())).unwrap())
    }

    /// The rays that leave the stipe on both sides of `angle`, and how far it is from the first
    /// to the second
    pub fn around(&self, angle: f32) -> Option<(usize, usize, f32)> {
        let crossing: Vec<usize> = (0..self.rays.len()).filter(|&k| self.rays[k].crosses).collect();
        let first = *crossing.first()?;
        let mut before = *crossing.last()?;
        for &k in &crossing {
            if self.rays[k].angle > angle {
                let span = (self.rays[k].angle - self.rays[before].angle).rem_euclid(TAU);
                let into = (angle - self.rays[before].angle).rem_euclid(TAU);
                return Some((before, k, if span > 0.0 { (into / span).min(1.0) } else { 0.0 }));
            }
            before = k;
        }
        let span = (self.rays[first].angle - self.rays[before].angle).rem_euclid(TAU);
        let into = (angle - self.rays[before].angle).rem_euclid(TAU);
        Some((before, first, if span > 0.0 { (into / span).min(1.0) } else { 0.0 }))
    }

    /// Makes this cap hang on `outer`, so it bends with it: ridges stay under the flesh
    /// however far the flesh turns, they don't keep their own lengths
    fn follow(&mut self, outer: &Cap) {
        let mut attachments = Vec::with_capacity(self.rays.len());
        for ray in &self.rays {
            let Some((a, b, share)) = outer.around(ray.angle) else {
                return;
            };
            let hang = |r: usize, p: Vec2| {
                let line = &outer.rays[r].plane;
                let (i, t) = nearest_on(line, p);
                let (from, to) = (line[i], line[(i + 1).min(line.len() - 1)]);
                let along = (to - from).try_normalize().unwrap_or(Vec2::X);
                let o = p - from.lerp(to, t);
                (i, t, Vec2::new(o.dot(along), o.dot(along.perp())))
            };
            let points = ray.plane.iter().map(|&p| [hang(a, p), hang(b, p)]).collect();
            attachments.push(Attachment { a, b, share, points });
        }
        self.attachments = attachments;
        self.outer = Some(Box::new(outer.clone()));
    }

    /// Ray `k` of the cap this one hangs on, bent by `turn`, with the turn of every step
    fn bend_steps(&self, k: usize, turn: &impl Fn(&Step) -> f32) -> (Vec<Vec2>, Vec<f32>) {
        let ray = &self.rays[k];
        let mut p = self.start(k, turn);
        let mut points = Vec::with_capacity(ray.plane.len());
        let mut turns = Vec::with_capacity(ray.plane.len());
        points.push(p);
        for i in 0..ray.plane.len() - 1 {
            let step = self.step(ray, i, 1.0);
            let t = turn(&step);
            p += Vec2::from_angle(-t).rotate(step.to - step.from);
            points.push(p);
            turns.push(t);
        }
        (points, turns)
    }

    /// Ray `k` of a cap that hangs on `outer`, when `outer` bends by `turn`
    fn bend_attached(&self, outer: &Cap, k: usize, turn: &impl Fn(&Step) -> f32) -> Vec<Vec2> {
        let at = &self.attachments[k];
        let (bent_a, turns_a) = outer.bend_steps(at.a, turn);
        let (bent_b, turns_b) = outer.bend_steps(at.b, turn);
        let place = |r: usize, bent: &[Vec2], turns: &[f32], (i, t, o): (usize, f32, Vec2)| {
            let line = &outer.rays[r].plane;
            let j = (i + 1).min(line.len() - 1);
            let along = (line[j] - line[i]).try_normalize().unwrap_or(Vec2::X);
            let along = Vec2::from_angle(-turns.get(i).copied().unwrap_or(0.0)).rotate(along);
            bent[i].lerp(bent[j], t) + along * o.x + along.perp() * o.y
        };
        at.points
            .iter()
            .map(|[pa, pb]| place(at.a, &bent_a, &turns_a, *pa).lerp(place(at.b, &bent_b, &turns_b, *pb), at.share))
            .collect()
    }

    /// The rays among `paths` that `creator` made on the stipe of `hinge`, `None` when there
    /// are none
    pub fn new(hinge: &Point, paths: &[&[i32]], creator: i32) -> Option<Cap> {
        let frame = CapFrame::new(hinge);
        let mut rays = Vec::new();
        let mut lengths = Vec::new();
        for (index, item) in paths.iter().enumerate() {
            if item.len() < 9 || path_link(item).parent_id != frame.stipe_id || path_link(item).creator != creator {
                continue;
            }
            let path = wrap_path(item);
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
            if let Some(k) = crossing {
                let t = -plane[k - 1].x / (plane[k].x - plane[k - 1].x);
                lengths.push(to_end[k - 1] + (to_end[k] - to_end[k - 1]) * t);
            }
            rays.push(CapRay {
                path: index,
                angle: frame.angle(frame.hinge + out),
                plane,
                to_end,
                alphas: Vec::new(),
                crosses: crossing.is_some(),
                out,
                across,
            });
        }
        if rays.is_empty() {
            return None;
        }
        lengths.sort_by(f32::total_cmp);
        let length = match lengths.len() {
            0 => rays.iter().map(|r| r.to_end[0]).fold(0.0, f32::max),
            n => lengths[n / 2],
        };
        rays.sort_by(|a, b| a.angle.total_cmp(&b.angle));
        let mut cap = Cap { frame, rays, length, carriers: Vec::new(), outer: None, attachments: Vec::new() };
        for k in 0..cap.rays.len() {
            cap.rays[k].alphas = cap.rays[k].to_end.iter().map(|&d| cap.alpha(d)).collect();
        }
        cap.carriers = (0..cap.rays.len()).map(|k| cap.carrier(k)).collect();
        Some(cap)
    }

    /// How far from the axis the furthest tip is
    pub fn reach(&self) -> f32 {
        self.rays.iter().map(|r| r.plane.last().unwrap().x).fold(0.0, f32::max)
    }

    fn alpha(&self, to_end: f32) -> f32 {
        if self.length > 0.0 {
            (1.0 - to_end / self.length).clamp(0.0, 1.0)
        } else {
            0.0
        }
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
        if let Some(outer) = &self.outer {
            return self.bend_attached(outer, k, &turn);
        }
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

/// Poses the caps in `paths` that have a hinge in `hinges`. `pose` gives the new plane of ray
/// `k` of a cap, or `None` to leave it. Paths of no cap pass as they are.
pub fn pose_caps(paths: &[&[i32]], hinges: &[Point], pose: impl FnMut(&Cap, usize) -> Option<Vec<Vec2>>) -> Vec<Vec<i32>> {
    pose_caps_of(&Cap::all(paths, hinges), paths, pose)
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

/// The tips of the rays of every cap, in a ring around its axis
pub fn margin_points(paths: &[&[i32]], hinges: &[Point]) -> Vec<Point> {
    let mut margins = Vec::new();
    for cap in Cap::all(paths, hinges) {
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
    use crate::{grow_cap, CapFrame, CapShape};
    use glam::{Quat, Vec2, Vec3};
    use nodarium_utils::{
        curve, encode_points,
        geometry::{create_multiple_paths, fill_gaps, path_link, set_path_link, wrap_multiple_paths, PathLink},
        Point,
    };

    pub const STIPE_TOP: f32 = 0.05;
    pub const STIPE_RADIUS: f32 = 0.005;

    /// An upright stipe 5 cm tall, 5 mm in radius
    pub fn stipe() -> Vec<i32> {
        leaning_stipe(0.0)
    }

    /// The stipe, leaning `lean` radians away from upright
    pub fn leaning_stipe(lean: f32) -> Vec<i32> {
        let tilt = Quat::from_rotation_z(lean);
        let mut data = create_multiple_paths(1, 12, 1, 3);
        for path in wrap_multiple_paths(&mut data).iter_mut() {
            for i in 0..12 {
                let p = tilt * Vec3::new(0.0, STIPE_TOP * i as f32 / 11.0, 0.0);
                path.points[i * 4..i * 4 + 4].copy_from_slice(&[p.x, p.y, p.z, STIPE_RADIUS]);
            }
        }
        data
    }

    #[derive(Clone)]
    pub struct TestCap {
        pub radius: f32,
        pub profile: Vec<f32>,
        pub thickness: f32,
        /// Share of the thickness from the centre to the margin
        pub thickness_curve: Vec<f32>,
        pub rays: usize,
        pub spacing: f32,
        pub seed: i32,
    }

    /// A cone from the top at y = 20 down to the margin at y = 60 of the shape editor: the
    /// margin is 0.8 radii below the top. The flesh thins to nothing at the margin.
    pub fn cone() -> TestCap {
        TestCap {
            radius: 0.03,
            profile: vec![0.0, 60.0, 50.0, 20.0],
            thickness: 0.006,
            thickness_curve: vec![0.0, 1.0, 1.0, 0.0],
            rays: 10,
            spacing: 0.5,
            seed: 3,
        }
    }

    /// The rays of the cap, with the ones `fill_gaps` adds, and its hinges as the input of a node
    pub fn grow(c: &TestCap) -> (Vec<Vec<i32>>, Vec<i32>) {
        grow_on(c, &stipe())
    }

    /// Like `grow`, on another stipe
    pub fn grow_on(c: &TestCap, stipe: &[i32]) -> (Vec<Vec<i32>>, Vec<i32>) {
        let thickness: Vec<f32> = crate::cap_alphas().map(|a| c.thickness * curve(&c.thickness_curve, a)).collect();
        let shape = CapShape {
            profile: &c.profile,
            radius: c.radius,
            thickness: &thickness,
            rays: c.rays,
            seed: c.seed,
            resolution: 24,
            creator: 7,
        };
        let (hinge, mut rays) = grow_cap(nodarium_utils::split_args(stipe)[0], &shape);
        let refs: Vec<&[i32]> = rays.iter().map(|r| r.as_slice()).collect();
        let added = fill_gaps(&refs, c.spacing, c.seed);
        rays.extend(added);
        (rays, encode_points(&[hinge]))
    }

    /// Ridges of another node under the `cone`, to join with it: like the false gills of a
    /// chanterelle they run down the stipe, then out under the flesh, not quite to the margin
    pub fn ridges() -> Vec<Vec<i32>> {
        let profile = vec![50.0, 22.0, 45.0, 70.0, 30.0, 40.0, 10.0, 56.0];
        let (mut ridges, _) = grow(&TestCap { profile, thickness: 0.001, rays: 14, spacing: 1.0, ..cone() });
        for ridge in &mut ridges {
            let link = path_link(ridge);
            set_path_link(ridge, PathLink::child(8, link.parent_id, 1.0, link.index));
        }
        ridges
    }

    /// How far the margin points stray from the middle of their neighbours around the axis at
    /// most, a margin that is smooth all round strays little
    pub fn roughness(margins: &[Point], hinge: &Point) -> f32 {
        let frame = CapFrame::new(hinge);
        let mut ring: Vec<(f32, Vec3)> = margins.iter().map(|m| (frame.angle(m.position), m.position)).collect();
        ring.sort_by(|a, b| a.0.total_cmp(&b.0));
        let n = ring.len();
        (0..n).map(|k| ring[k].1.distance(0.5 * (ring[(k + n - 1) % n].1 + ring[(k + 1) % n].1))).fold(0.0, f32::max)
    }

    /// Distance from the axis and height over the stipe top
    pub fn place(p: Vec3) -> Vec2 {
        Vec2::new(Vec2::new(p.x, p.z).length(), p.y - STIPE_TOP)
    }

    pub fn points(path: &[i32]) -> Vec<Vec3> {
        nodarium_utils::geometry::wrap_path(path).points.chunks_exact(4).map(|p| Vec3::new(p[0], p[1], p[2])).collect()
    }

    /// How long every step of a path is
    pub fn steps(path: &[i32]) -> Vec<f32> {
        points(path).windows(2).map(|w| w[0].distance(w[1])).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::testing::{cone, grow_on, leaning_stipe, roughness, stipe, TestCap};
    use super::testing::points;
    use super::{curl_turn, margin_points, pose_caps, CapFrame};
    use nodarium_utils::{decode_points, Point};

    fn smoothstep(x: f32) -> f32 {
        let t = x.clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    }

    /// Waves the margin like `margin_waves` (6 lobes), then rolls it under like `margin_curl`
    fn wave_and_curl(rays: Vec<Vec<i32>>, hinges: &[Point], amplitude: f32, curl: f32) -> Vec<Vec<i32>> {
        let refs: Vec<&[i32]> = rays.iter().map(|r| r.as_slice()).collect();
        let radius = 0.03;
        let waved = pose_caps(&refs, hinges, |cap, k| {
            let lift = amplitude * radius * (6.0 * cap.rays[k].angle).cos();
            let offset = |x: f32| lift * smoothstep(x / radius);
            Some(cap.bend(k, |s| -((offset(s.to.x) - offset(s.from.x)) / s.length().max(1e-9)).atan()))
        });
        let refs: Vec<&[i32]> = waved.iter().map(|r| r.as_slice()).collect();
        pose_caps(&refs, hinges, |cap, k| Some(cap.bend(k, |s| curl_turn(s, curl, 0.4))))
    }

    fn margins(rays: &[Vec<i32>], hinges: &[Point]) -> Vec<Point> {
        let refs: Vec<&[i32]> = rays.iter().map(|r| r.as_slice()).collect();
        margin_points(&refs, hinges)
    }

    fn many_added() -> TestCap {
        TestCap { rays: 12, spacing: 0.25, ..cone() }
    }

    #[test]
    fn rays_added_on_the_cap_stay_between_their_neighbours_on_a_wavy_curled_cap() {
        let (rays, hinges) = grow_on(&many_added(), &stipe());
        let hinges = decode_points(&hinges);
        let added = rays.len() - 12;
        assert!(added > 30, "only {added} rays added");
        let flat = roughness(&margins(&rays, &hinges), &hinges[0]);
        let posed = wave_and_curl(rays, &hinges, 0.18, 0.8);
        let rough = roughness(&margins(&posed, &hinges), &hinges[0]);
        // the ring of a wavy margin bends, but no ray juts out of it
        assert!(rough < 0.04 * 0.03, "a margin point strays {rough} from its neighbours, {flat} before");
        // the start of every added ray stays on the cap, inside the flesh of its neighbours
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
        let place = |rays: &[Vec<i32>], hinges: &[Point]| {
            let frame = CapFrame::new(&hinges[0]);
            let mut ring: Vec<[f32; 3]> = margins(rays, hinges)
                .iter()
                .map(|m| {
                    let q = m.position - frame.hinge;
                    [frame.angle(m.position), q.reject_from(frame.axis).length(), q.dot(frame.axis)]
                })
                .collect();
            ring.sort_by(|a, b| a[0].total_cmp(&b[0]));
            ring
        };
        let pose = |lean: f32| {
            let (rays, hinges) = grow_on(&many_added(), &leaning_stipe(lean));
            let hinges = decode_points(&hinges);
            let posed = wave_and_curl(rays, &hinges, 0.18, 0.8);
            (place(&posed, &hinges), roughness(&margins(&posed, &hinges), &hinges[0]))
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
}
