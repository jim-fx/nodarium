//! New paths between neighbouring paths of a fan, where they drift further apart than a few
//! path thicknesses, like the rays of a cap towards its margin.
//!
//! A fan is the paths one node made on the same parent. Their neighbours are found by the angle of
//! their tips around the axis of the fan, the normal of the plane their tips spread in. A new
//! path follows both neighbours, at the same share of their length, and turns around the axis
//! with them, so it keeps their distance from the axis instead of cutting the corner.

use super::{create_path, path_link, path_random, set_path_link, wrap_path, wrap_path_mut, PathLink};
use glam::{Vec3, Vec4, Vec4Swizzles};
use std::collections::VecDeque;
use std::f32::consts::{PI, TAU};

/// A fan never grows past this many paths
const MAX_PATHS: usize = 1024;
/// A new path grows to full thickness over this many of its radii, so its start sinks into its
/// neighbours instead of showing a cut end
const RAMP_LENGTH: f32 = 2.0;
/// How far from the middle between its neighbours a new path may be, as a share of the gap
const JITTER: f32 = 0.3;
const SALT: i32 = 0x4647_0000;
/// A new path starts no closer to the tips than this share of the length, so it has a few points
const MIN_REST: f32 = 0.05;
/// Where along its neighbours the start of a new path is looked for
const SEARCH_STEPS: usize = 128;
/// A ring has no gap between its last and first path larger than this many of its usual gaps,
/// a fan that is open on one side has
const RING_GAP: f32 = 2.5;

/// A path of the fan, its points and how far along the fan each one is
struct Strand {
    points: Vec<Vec4>,
    /// Share of the length of the fan, from 0 at its base to 1 at the tips
    along: Vec<f32>,
    /// The radius without the ramp of a new path
    full: Vec<f32>,
    angle: f32,
}

impl Strand {
    fn from_path(points: Vec<Vec4>) -> Strand {
        let mut along = vec![0.0f32; points.len()];
        for i in 1..points.len() {
            along[i] = along[i - 1] + points[i].xyz().distance(points[i - 1].xyz());
        }
        let length = along[points.len() - 1];
        if length > 0.0 {
            along.iter_mut().for_each(|a| *a /= length);
        }
        let full = points.iter().map(|p| p.w.abs()).collect();
        Strand { points, along, full, angle: 0.0 }
    }

    fn start(&self) -> f32 {
        self.along[0]
    }

    /// Position and full radius at `t` along the fan
    fn at(&self, t: f32) -> (Vec3, f32) {
        let n = self.along.len();
        let k = self.along.partition_point(|&a| a <= t).clamp(1, n - 1);
        let (a, b) = (self.along[k - 1], self.along[k]);
        let s = if b > a { ((t - a) / (b - a)).clamp(0.0, 1.0) } else { 0.0 };
        (self.points[k - 1].xyz().lerp(self.points[k].xyz(), s), self.full[k - 1] + (self.full[k] - self.full[k - 1]) * s)
    }
}

/// The plane of a fan: a point on its axis, the axis and two directions across it
struct Frame {
    centre: Vec3,
    axis: Vec3,
    u: Vec3,
    v: Vec3,
}

impl Frame {
    fn polar(&self, p: Vec3) -> (f32, f32, f32) {
        let q = p - self.centre;
        let h = q.dot(self.axis);
        let (x, y) = (q.dot(self.u), q.dot(self.v));
        (x.hypot(y), y.atan2(x), h)
    }

    fn point(&self, rho: f32, angle: f32, h: f32) -> Vec3 {
        self.centre + self.axis * h + (self.u * angle.cos() + self.v * angle.sin()) * rho
    }
}

/// The largest eigenvector of a symmetric 3 x 3 matrix and its eigenvalue
fn largest_eigen(m: [Vec3; 3]) -> (Vec3, f32) {
    let apply = |v: Vec3| Vec3::new(m[0].dot(v), m[1].dot(v), m[2].dot(v));
    let mut v = *m.iter().max_by(|a, b| a.length_squared().total_cmp(&b.length_squared())).unwrap();
    if v.length_squared() == 0.0 {
        return (Vec3::X, 0.0);
    }
    v = v.normalize();
    for _ in 0..64 {
        let next = apply(v);
        match next.try_normalize() {
            Some(n) => v = n,
            None => return (v, 0.0),
        }
    }
    (v, apply(v).dot(v))
}

/// The normal of the plane `points` spread in the most, `None` when they lie on a line
fn spread_normal(points: &[Vec3]) -> Option<Vec3> {
    let mean = points.iter().copied().sum::<Vec3>() / points.len() as f32;
    let mut m = [Vec3::ZERO; 3];
    for p in points {
        let d = *p - mean;
        m[0] += d * d.x;
        m[1] += d * d.y;
        m[2] += d * d.z;
    }
    let (e1, l1) = largest_eigen(m);
    let deflated = [m[0] - e1 * e1.x * l1, m[1] - e1 * e1.y * l1, m[2] - e1 * e1.z * l1];
    let (e2, l2) = largest_eigen(deflated);
    if l1 <= 0.0 || l2 <= 1e-4 * l1 {
        return None;
    }
    let normal = e1.cross(e2).try_normalize()?;
    // the same side every time, so the order of the paths doesn't flip
    let flip = [normal.y, normal.z, normal.x].into_iter().find(|c| c.abs() > 1e-6).unwrap_or(1.0) < 0.0;
    Some(if flip { -normal } else { normal })
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    if edge1 <= edge0 {
        return if x < edge0 { 0.0 } else { 1.0 };
    }
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn wrap_pi(a: f32) -> f32 {
    (a + PI).rem_euclid(TAU) - PI
}

struct Filler<'a> {
    frame: &'a Frame,
    spacing: f32,
    seed: i32,
    id: i32,
    strands: Vec<Strand>,
    /// Points of a path that runs the whole fan
    resolution: f32,
}

impl Filler<'_> {
    /// Adds a strand in the gap between strands `a` and `b`, `b` after `a` going around the
    /// axis, if they drift apart. Gives where between them it is.
    fn fill(&mut self, a: usize, b: usize, gap: f32) -> Option<f32> {
        if self.strands.len() >= MAX_PATHS {
            return None;
        }
        let from = self.strands[a].start().max(self.strands[b].start());
        let wide = |t: f32| {
            let ((pa, ra), (pb, rb)) = (self.strands[a].at(t), self.strands[b].at(t));
            let thickness = 2.0 * ra.max(rb);
            thickness > 0.0 && pa.distance(pb) > self.spacing * thickness
        };
        let start = (0..=SEARCH_STEPS).map(|k| from + (1.0 - from) * k as f32 / SEARCH_STEPS as f32).find(|&t| wide(t))?;
        if 1.0 - start < MIN_REST {
            return None;
        }
        let share = 0.5 + (path_random(self.seed, self.id, SALT + self.strands.len() as i32) - 0.5) * JITTER;
        let count = ((self.resolution * (1.0 - start)).round() as usize).clamp(4, self.resolution.max(4.0) as usize);
        let f = self.frame;
        let mut points = Vec::with_capacity(count);
        let mut along = Vec::with_capacity(count);
        let mut full = Vec::with_capacity(count);
        for k in 0..count {
            let t = start + (1.0 - start) * k as f32 / (count - 1) as f32;
            let ((pa, ra), (pb, rb)) = (self.strands[a].at(t), self.strands[b].at(t));
            let ((rho_a, angle_a, h_a), (rho_b, angle_b, h_b)) = (f.polar(pa), f.polar(pb));
            // the turn from a to b near the turn between their tips, never the other way round
            let turn = gap + wrap_pi(angle_b - angle_a - gap);
            let p = f.point(rho_a + (rho_b - rho_a) * share, angle_a + turn * share, h_a + (h_b - h_a) * share);
            points.push(p.extend(0.0));
            along.push(t);
            full.push(ra + (rb - ra) * share);
        }
        let ramp = RAMP_LENGTH * full[0];
        let mut s = 0.0;
        for k in 0..count {
            if k > 0 {
                s += points[k].xyz().distance(points[k - 1].xyz());
            }
            points[k].w = full[k] * (0.5 + 0.5 * smoothstep(0.0, ramp, s));
        }
        let angle = self.strands[a].angle + gap * share;
        self.strands.push(Strand { points, along, full, angle });
        Some(share)
    }
}

/// The paths to add to `items` so that no two neighbouring paths of a fan are further apart
/// than `spacing` path thicknesses. They are made by the same node as their neighbours, and
/// count on from the highest index there. Paths that grow on no parent (several stems of one
/// node) are no fan.
pub fn fill_gaps(items: &[&[i32]], spacing: f32, seed: i32) -> Vec<Vec<i32>> {
    if spacing <= 0.0 {
        return Vec::new();
    }
    let mut fans: Vec<((i32, i32, i32), Vec<usize>)> = Vec::new();
    for (i, item) in items.iter().enumerate() {
        if !super::is_path(item) || wrap_path(item).length < 2 {
            continue;
        }
        let link = path_link(item);
        if !link.has_parent() {
            continue;
        }
        let key = (link.parent_id, item[3], link.creator);
        match fans.iter_mut().find(|f| f.0 == key) {
            Some(fan) => fan.1.push(i),
            None => fans.push((key, vec![i])),
        }
    }

    let mut added = Vec::new();
    for (_, members) in fans {
        if members.len() < 2 {
            continue;
        }
        let points = |i: usize| -> Vec<Vec4> { wrap_path(items[i]).points.chunks_exact(4).map(Vec4::from_slice).collect() };
        let paths: Vec<Vec<Vec4>> = members.iter().map(|&i| points(i)).collect();
        let tips: Vec<Vec3> = paths.iter().map(|p| p[p.len() - 1].xyz()).collect();
        let starts: Vec<Vec3> = paths.iter().map(|p| p[0].xyz()).collect();
        let all: Vec<Vec3> = paths.iter().flatten().map(|p| p.xyz()).collect();
        let Some(axis) = spread_normal(&tips).or_else(|| spread_normal(&all)) else {
            continue;
        };
        let centre = starts.iter().copied().sum::<Vec3>() / starts.len() as f32;
        let u = super::orthogonal_to(axis);
        let frame = Frame { centre, axis, u, v: axis.cross(u) };

        let mut strands: Vec<Strand> = paths.into_iter().map(Strand::from_path).collect();
        for (strand, tip) in strands.iter_mut().zip(&tips) {
            strand.angle = frame.polar(*tip).1.rem_euclid(TAU);
        }
        let mut order: Vec<usize> = (0..strands.len()).collect();
        order.sort_by(|&a, &b| strands[a].angle.total_cmp(&strands[b].angle));
        let m = order.len();
        let gaps: Vec<f32> = (0..m - 1).map(|k| strands[order[k + 1]].angle - strands[order[k]].angle).collect();
        let around = strands[order[0]].angle + TAU - strands[order[m - 1]].angle;
        let mut usual = gaps.clone();
        usual.sort_by(f32::total_cmp);
        let ring = around < PI && around <= RING_GAP * usual[usual.len() / 2];

        let link = path_link(items[members[order[0]]]);
        let next_index = members.iter().map(|&i| path_link(items[i]).index).max().unwrap_or(0) + 1;
        let depth = items[members[0]][3];
        let resolution = strands.iter().map(|s| s.points.len()).max().unwrap_or(4) as f32;
        let originals = strands.len();
        let mut filler = Filler { frame: &frame, spacing, seed, id: link.parent_id ^ link.creator, strands, resolution };
        let mut queue: VecDeque<(usize, usize, f32)> = (0..m - 1).map(|k| (order[k], order[k + 1], gaps[k])).collect();
        if ring {
            queue.push_back((order[m - 1], order[0], around));
        }
        // every gap of a round is filled before the gaps between the new paths, so the fan
        // fills evenly when it reaches the most paths it can have
        while let Some((a, b, gap)) = queue.pop_front() {
            if let Some(share) = filler.fill(a, b, gap) {
                let c = filler.strands.len() - 1;
                queue.push_back((a, c, gap * share));
                queue.push_back((c, b, gap * (1.0 - share)));
            }
        }

        let mut new: Vec<Strand> = filler.strands.split_off(originals);
        new.sort_by(|a, b| a.start().total_cmp(&b.start()));
        for (k, strand) in new.into_iter().enumerate() {
            let mut data = create_path(strand.points.len(), depth);
            set_path_link(&mut data, PathLink::child(link.creator, link.parent_id, link.parent_alpha, next_index + k as i32));
            let path = wrap_path_mut(&mut data);
            for (j, p) in strand.points.iter().enumerate() {
                path.points[j * 4..j * 4 + 4].copy_from_slice(&p.to_array());
            }
            added.push(data);
        }
    }
    added
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{path_link, wrap_path};

    const PARENT: i32 = 42;

    /// `count` straight paths from near the origin out to `reach`, evenly around the y axis
    /// over `spread` radians, each `radius` thick
    fn fan(count: usize, spread: f32, reach: f32, radius: f32) -> Vec<Vec<i32>> {
        (0..count)
            .map(|k| {
                let angle = spread * k as f32 / count as f32;
                let out = Vec3::new(angle.cos(), 0.0, angle.sin());
                let mut data = create_path(16, 2);
                set_path_link(&mut data, PathLink::child(3, PARENT, 1.0, k as i32));
                let path = wrap_path_mut(&mut data);
                for i in 0..16 {
                    let p = out * (0.001 + reach * i as f32 / 15.0) - Vec3::Y * 0.2 * reach * (i as f32 / 15.0).powi(2);
                    path.points[i * 4..i * 4 + 4].copy_from_slice(&[p.x, p.y, p.z, radius]);
                }
                data
            })
            .collect()
    }

    fn run(paths: &[Vec<i32>], spacing: f32) -> Vec<Vec<i32>> {
        let items: Vec<&[i32]> = paths.iter().map(|p| p.as_slice()).collect();
        fill_gaps(&items, spacing, 1)
    }

    fn tip(path: &[i32]) -> Vec3 {
        let p = wrap_path(path);
        Vec3::from_slice(&p.points[(p.length - 1) * 4..(p.length - 1) * 4 + 3])
    }

    #[test]
    fn no_paths_at_spacing_zero_or_when_neighbours_are_close() {
        let ring = fan(12, TAU, 0.03, 0.001);
        assert!(run(&ring, 0.0).is_empty());
        // the tips are 2π · 3 cm / 12 = 1.6 cm apart, 8 thicknesses
        assert!(run(&ring, 10.0).is_empty());
        assert!(run(&fan(12, TAU, 0.03, 0.0), 0.5).is_empty());
        assert!(!run(&ring, 4.0).is_empty());
    }

    #[test]
    fn new_paths_close_the_gaps_of_a_ring() {
        let ring = fan(12, TAU, 0.03, 0.001);
        let added = run(&ring, 1.0);
        assert!(added.len() > 60, "{} paths", added.len());
        let mut angles: Vec<f32> = ring.iter().chain(&added).map(|p| tip(p).z.atan2(tip(p).x).rem_euclid(TAU)).collect();
        angles.sort_by(f32::total_cmp);
        let largest = (0..angles.len()).map(|i| (angles[(i + 1) % angles.len()] - angles[i]).rem_euclid(TAU)).fold(0.0, f32::max);
        assert!(largest * 0.03 < 1.3 * 2.0 * 0.001, "the widest gap is {} thicknesses", largest * 0.03 / 0.002);
        for path in &added {
            let link = path_link(path);
            assert_eq!((link.parent_id, link.creator, path[3]), (PARENT, 3, 2));
            assert!(link.index >= 12);
            // it keeps the distance of its neighbours from the axis
            let t = tip(path);
            assert!((Vec3::new(t.x, 0.0, t.z).length() - (0.03 + 0.001)).abs() < 1e-4);
        }
        // the first ones start where the gap opens, the later ones further out
        let starts: Vec<f32> = added.iter().map(|p| Vec3::from_slice(&wrap_path(p).points[0..3]).length()).collect();
        assert!(starts.windows(2).all(|w| w[1] >= w[0] - 1e-4));
        let ids: std::collections::HashSet<i32> = ring.iter().chain(&added).map(|p| path_link(p).id).collect();
        assert_eq!(ids.len(), ring.len() + added.len());
    }

    #[test]
    fn new_paths_grow_in_thick_from_their_start() {
        let added = run(&fan(12, TAU, 0.03, 0.001), 1.0);
        for path in &added {
            let p = wrap_path(path);
            assert!((p.points[3] - 0.0005).abs() < 1e-6);
            assert!((p.points[(p.length - 1) * 4 + 3] - 0.001).abs() < 1e-6);
        }
    }

    #[test]
    fn an_open_fan_is_not_closed_around_the_back() {
        let open = fan(6, PI * 0.5, 0.03, 0.001);
        let added = run(&open, 1.0);
        assert!(!added.is_empty());
        for path in &added {
            let angle = tip(path).z.atan2(tip(path).x);
            assert!((-1e-3..PI * 0.5).contains(&angle), "a path at {angle}");
        }
    }

    #[test]
    fn paths_without_a_parent_get_none_between_them() {
        let mut stems = fan(6, TAU, 0.03, 0.001);
        for (k, stem) in stems.iter_mut().enumerate() {
            set_path_link(stem, PathLink::root(3, k as i32));
        }
        assert!(run(&stems, 1.0).is_empty());
    }

    #[test]
    fn the_same_paths_give_the_same_new_paths() {
        let ring = fan(9, TAU, 0.03, 0.001);
        assert_eq!(run(&ring, 1.0), run(&ring, 1.0));
    }
}
