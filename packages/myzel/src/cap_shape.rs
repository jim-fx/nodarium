//! The shape of a cap as the `cap` node grows it: guide rays from a drawn profile, all alike,
//! evenly around the stipe axis. Shared so the nodes that pose a cap can test with real caps.

use glam::{Vec2, Vec3};
use nodarium_utils::{
    geometry::{create_path, orthogonal_to, path_random, set_path_link, wrap_path, wrap_path_mut, PathLink},
    Point,
};
use std::f32::consts::TAU;

/// Points the drawn outline is resampled to before it is offset
const OUTLINE_SAMPLES: usize = 96;
/// How far down into the stipe the rays reach, in radii of the stipe top. They overlap the ends
/// of the stipe strands there, so stipe and cap close into one body without a seam.
const ROOT_DEPTH: f32 = 0.6;
/// How far across the axis the rays start in the stipe, as a share of the flesh half thickness
/// (or the stipe radius, when it is thinner)
const ROOT_OFFSET: f32 = 0.5;
const ROOT_SAMPLES: usize = 8;
/// How much a ray may turn away from its even place between its neighbours, as a share of the gap
const ANGLE_JITTER: f32 = 0.3;

/// The drawn outline as (distance from the axis, height), with the top over the axis at 0.
/// The shape editor draws the left half of a 100 x 100 box, mirrored at x = 50, with y going
/// down. Its half width is the radius and both directions share the scale, so the cap keeps the
/// proportions it is drawn with.
fn outline(points: &[f32], radius: f32) -> Vec<Vec2> {
    let scale = radius / 50.0;
    let mut outline: Vec<Vec2> = points
        .chunks_exact(2)
        .map(|p| Vec2::new((50.0 - p[0].min(100.0 - p[0])) * scale, -p[1] * scale))
        .collect();
    outline.sort_by(|a, b| a.x.total_cmp(&b.x));
    outline.dedup_by(|a, b| a.x - b.x <= f32::EPSILON * radius);
    if outline.len() < 2 {
        return vec![Vec2::ZERO, Vec2::new(radius, 0.0)];
    }
    if outline[0].x > 0.0 {
        outline.insert(0, Vec2::new(0.0, outline[0].y));
    }
    let top = outline[0].y;
    outline.iter().map(|p| p - Vec2::Y * top).collect()
}

/// `count` points evenly spaced along a polyline
fn resample<T: Copy>(points: &[Vec2], values: &[T], count: usize, mix: impl Fn(T, T, f32) -> T) -> Vec<(Vec2, T)> {
    let mut along = vec![0.0f32; points.len()];
    for i in 1..points.len() {
        along[i] = along[i - 1] + points[i].distance(points[i - 1]);
    }
    let length = along[points.len() - 1];
    let mut j = 0;
    (0..count)
        .map(|k| {
            let s = k as f32 / (count - 1).max(1) as f32 * length;
            while j + 2 < points.len() && along[j + 1] < s {
                j += 1;
            }
            let span = along[j + 1] - along[j];
            let t = if span > 0.0 { ((s - along[j]) / span).clamp(0.0, 1.0) } else { 0.0 };
            (points[j].lerp(points[j + 1], t), mix(values[j], values[j + 1], t))
        })
        .collect()
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The line the rays follow through the middle of the flesh, from the axis to the margin
struct Midline {
    points: Vec<Vec2>,
    /// Half the flesh thickness at every point, the radius of a ray there
    half: Vec<f32>,
}

impl Midline {
    /// The outline moved in by half the flesh thickness, so the outside of the flesh is the
    /// drawn outline. `thickness` is the flesh at each of the `cap_alphas`. The hinge, where
    /// the flesh meets the stipe top, is at the origin.
    fn new(outline: &[Vec2], thickness: &[f32]) -> Self {
        let samples = resample(outline, &vec![(); outline.len()], OUTLINE_SAMPLES, |a, _, _| a);
        let n = samples.len();
        let half: Vec<f32> = thickness.iter().map(|t| 0.5 * t.max(0.0)).collect();
        let lift = Vec2::Y * 2.0 * half[0];
        let points = (0..n)
            .map(|i| {
                let (a, b) = (samples[i.saturating_sub(1)].0, samples[(i + 1).min(n - 1)].0);
                let tangent = (b - a).try_normalize().unwrap_or(Vec2::X);
                let normal = if i == 0 { Vec2::Y } else { tangent.perp() };
                samples[i].0 - normal * half[i] + lift
            })
            .collect();
        Midline { points, half }
    }
}

/// A ray in the plane through the axis: points as (distance from the axis, height over the
/// hinge) and the radius at every point
struct Ray2 {
    points: Vec<Vec2>,
    radius: Vec<f32>,
}

/// The angles of `rays` rays evenly around the axis, each moved a little by the seed
fn ray_angles(rays: usize, seed: i32, id: i32) -> Vec<f32> {
    let gap = TAU / rays as f32;
    (0..rays).map(|k| k as f32 * gap + (path_random(seed, id, k as i32) - 0.5) * ANGLE_JITTER * gap).collect()
}

/// The rays of one cap in the plane through the axis, the same for every angle
fn cap_ray(midline: &Midline, stipe_radius: f32, resolution: usize) -> Ray2 {
    let (line, half) = (&midline.points, &midline.half);
    // a ray starts on the far side of the axis and crosses it at the top of the cap, so every
    // ray passes over the centre and the flesh there is as thick as the rays
    let root_start = Vec2::new(-ROOT_OFFSET * half[0].min(stipe_radius), -ROOT_DEPTH * stipe_radius);
    // inside the stipe the rays stay within it, a thicker root would show as a collar under the cap
    let root_radius = half[0].min(stipe_radius + root_start.x);
    let end = line[0];
    let end_tangent = (line[1] - end).try_normalize().unwrap_or(Vec2::X);
    let k = root_start.distance(end);
    let root = (0..ROOT_SAMPLES).map(|i| {
        let t = i as f32 / ROOT_SAMPLES as f32;
        let (t2, t3) = (t * t, t * t * t);
        let p = root_start * (2.0 * t3 - 3.0 * t2 + 1.0)
            + Vec2::Y * k * (t3 - 2.0 * t2 + t)
            + end * (-2.0 * t3 + 3.0 * t2)
            + end_tangent * k * (t3 - t2);
        (p, root_radius + (half[0] - root_radius) * smoothstep(0.0, 1.0, t))
    });
    let (points, radius): (Vec<Vec2>, Vec<f32>) = root.chain((0..line.len()).map(|i| (line[i], half[i]))).unzip();
    let samples = resample(&points, &radius, resolution, |a, b, t| a + (b - a) * t);
    Ray2 { points: samples.iter().map(|s| s.0).collect(), radius: samples.iter().map(|s| s.1).collect() }
}

/// What a cap looks like when it is open
pub struct CapShape<'a> {
    /// The drawn outline from the shape editor
    pub profile: &'a [f32],
    pub radius: f32,
    /// The flesh at each of the `cap_alphas`
    pub thickness: &'a [f32],
    pub rays: usize,
    pub seed: i32,
    /// Points along a ray
    pub resolution: usize,
    pub creator: i32,
}

/// How far along the drawn outline the flesh is read, from the top over the axis (0) to the
/// margin (1)
pub fn cap_alphas() -> impl Iterator<Item = f32> + Clone {
    (0..OUTLINE_SAMPLES).map(|k| k as f32 / (OUTLINE_SAMPLES - 1) as f32)
}

/// The rays of the open cap at the tip of `stipe`, and its hinge: the stipe top, with the axis
/// as tangent and the stipe radius as scale
pub fn grow_cap(stipe: &[i32], shape: &CapShape) -> (Point, Vec<Vec<i32>>) {
    let stipe = wrap_path(stipe);
    let n = stipe.length;
    let point = |i: usize| Vec3::from_slice(&stipe.points[i * 4..i * 4 + 3]);
    let hinge = point(n - 1);
    let axis = (hinge - point(n.saturating_sub(2))).try_normalize().unwrap_or(Vec3::Y);
    let stipe_radius = stipe.points[(n - 1) * 4 + 3].abs();
    let side = orthogonal_to(axis);
    let around = axis.cross(side);

    let midline = Midline::new(&outline(shape.profile, shape.radius), shape.thickness);
    let ray = cap_ray(&midline, stipe_radius, shape.resolution.max(4));
    let rays = ray_angles(shape.rays.max(1), shape.seed, stipe.link.id)
        .into_iter()
        .enumerate()
        .map(|(index, angle)| {
            let out = side * angle.cos() + around * angle.sin();
            let mut data = create_path(ray.points.len(), stipe.depth + 1);
            set_path_link(&mut data, PathLink::child(shape.creator, stipe.link.id, 1.0, index as i32));
            let path = wrap_path_mut(&mut data);
            for (j, (p, r)) in ray.points.iter().zip(&ray.radius).enumerate() {
                let q = hinge + out * p.x + axis * p.y;
                path.points[j * 4..j * 4 + 4].copy_from_slice(&[q.x, q.y, q.z, *r]);
            }
            data
        })
        .collect();
    let hinge = Point {
        position: hinge,
        direction: side,
        tangent: axis,
        scale: stipe_radius,
        parent_id: stipe.link.id,
        alpha: 1.0,
        depth: stipe.depth,
        index: 0,
    };
    (hinge, rays)
}
