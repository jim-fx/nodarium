//! The rays of a cap, as the `cap` node makes them, seen in the plane through the stipe axis
//! that each of them lies in. The nodes that pose a cap (`cap_open`, `margin_curl`,
//! `margin_waves`) turn the steps of every ray in that plane by an angle that depends on how far
//! along the cap the step is. That keeps every step as long as it was, and turns add up, so the
//! poses can follow each other in a graph.
//!
//! A ray that leaves the stipe starts on the far side of the axis and crosses it at the top of
//! the cap, from there on it measures the cap. A ray that starts further out (added by
//! `fill_gaps`) is carried along by its nearest neighbour that leaves the stipe: it bends as if
//! it had been part of that ray from the axis on.

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

/// The rays of one cap: the rays one node made on one stipe
pub struct Cap {
    pub frame: CapFrame,
    /// Sorted by angle
    pub rays: Vec<CapRay>,
    /// Distance along the rays from the axis to the margin
    pub length: f32,
    /// The donor of every ray that starts out on the cap, found once, a cap bends its rays
    /// many times
    donors: Vec<Option<usize>>,
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
            caps.extend(creators.into_iter().filter_map(|creator| Cap::new(hinge, paths, creator)));
        }
        caps
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
        let donors = rays.iter().map(|ray| if ray.crosses { None } else { Self::donor(&rays, ray) }).collect();
        Some(Cap { frame, rays, length, donors })
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
        let to_end = ray.to_end[i] + (ray.to_end[i + 1] - ray.to_end[i]) * share;
        Step {
            from_alpha: self.alpha(ray.to_end[i]),
            to_alpha: self.alpha(to_end),
            from: ray.plane[i],
            to: ray.plane[i].lerp(ray.plane[i + 1], share),
        }
    }

    /// The nearest ray that leaves the stipe
    fn donor(rays: &[CapRay], ray: &CapRay) -> Option<usize> {
        let distance = |r: &CapRay| {
            let d = (r.angle - ray.angle).rem_euclid(TAU);
            d.min(TAU - d)
        };
        rays.iter()
            .enumerate()
            .filter(|(_, r)| r.crosses)
            .min_by(|(_, a), (_, b)| distance(a).total_cmp(&distance(b)))
            .map(|(k, _)| k)
    }

    /// Where the start of a ray that starts out on the cap moves when its donor bends from the
    /// axis to there: it stays put relative to the donor at the same distance from the margin
    fn carried_start(&self, ray: &CapRay, donor: &CapRay, turn: &impl Fn(&Step) -> f32) -> Vec2 {
        let target = ray.to_end[0];
        let (mut before, mut after, mut last) = (donor.plane[0], donor.plane[0], 0.0);
        for i in 0..donor.plane.len() - 1 {
            let (d0, d1) = (donor.to_end[i], donor.to_end[i + 1]);
            if d0 <= target {
                break;
            }
            let share = if d1 < target { (d0 - target) / (d0 - d1) } else { 1.0 };
            let step = self.step(donor, i, share);
            last = turn(&step);
            after += Vec2::from_angle(-last).rotate(step.to - step.from);
            before = step.to;
        }
        after + Vec2::from_angle(-last).rotate(ray.plane[0] - before)
    }

    /// The ray `k` with every step turned by `turn` radians, down and in towards the stipe for a
    /// positive turn. The steps keep their lengths, the ray keeps its points.
    pub fn bend(&self, k: usize, turn: impl Fn(&Step) -> f32) -> Vec<Vec2> {
        let ray = &self.rays[k];
        let mut p = match self.donors[k] {
            Some(donor) => self.carried_start(ray, &self.rays[donor], &turn),
            _ => ray.plane[0],
        };
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
pub fn pose_caps(paths: &[&[i32]], hinges: &[Point], mut pose: impl FnMut(&Cap, usize) -> Option<Vec<Vec2>>) -> Vec<Vec<i32>> {
    let mut output: Vec<Vec<i32>> = paths.iter().map(|p| p.to_vec()).collect();
    for cap in Cap::all(paths, hinges) {
        for k in 0..cap.rays.len() {
            if let Some(plane) = pose(&cap, k) {
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
    use crate::{grow_cap, CapShape};
    use glam::{Vec2, Vec3};
    use nodarium_utils::{
        curve, encode_points,
        geometry::{create_multiple_paths, fill_gaps, wrap_multiple_paths},
    };

    pub const STIPE_TOP: f32 = 0.05;
    pub const STIPE_RADIUS: f32 = 0.005;

    /// An upright stipe 5 cm tall, 5 mm in radius
    pub fn stipe() -> Vec<i32> {
        let mut data = create_multiple_paths(1, 12, 1, 3);
        for path in wrap_multiple_paths(&mut data).iter_mut() {
            for i in 0..12 {
                path.points[i * 4..i * 4 + 4].copy_from_slice(&[0.0, STIPE_TOP * i as f32 / 11.0, 0.0, STIPE_RADIUS]);
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
        let stipe = stipe();
        let (hinge, mut rays) = grow_cap(nodarium_utils::split_args(&stipe)[0], &shape);
        let refs: Vec<&[i32]> = rays.iter().map(|r| r.as_slice()).collect();
        let added = fill_gaps(&refs, c.spacing, c.seed);
        rays.extend(added);
        (rays, encode_points(&[hinge]))
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
