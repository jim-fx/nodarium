use std::collections::{HashMap, HashSet};
use std::f32::consts::PI;

use super::__nodarium_execute_inner;
use glam::{Quat, Vec2, Vec3};
use nodarium_utils::{
    concat_arg_vecs, concat_args, encode_float,
    geometry::{create_path, set_path_link, wrap_geometry_data, wrap_path_mut, PathLink},
};

fn path(points: &[(Vec3, f32)], link: PathLink, depth: i32) -> Vec<i32> {
    let mut data = create_path(points.len(), depth);
    set_path_link(&mut data, link);
    let path = wrap_path_mut(&mut data);
    for (i, (p, r)) in points.iter().enumerate() {
        path.points[i * 4..i * 4 + 4].copy_from_slice(&[p.x, p.y, p.z, *r]);
    }
    data
}

fn line(from: Vec3, to: Vec3, reach: impl Fn(f32) -> f32, n: usize) -> Vec<(Vec3, f32)> {
    (0..n)
        .map(|i| {
            let t = i as f32 / (n - 1) as f32;
            (from.lerp(to, t), reach(t))
        })
        .collect()
}

/// A midrib 1 long along +z with `pairs` pairs of side veins, which reach `reach(t)` around them
/// at `t` along their length. Returns the veins and the side veins as their base and tip.
pub fn pinnate(pairs: usize, reach: impl Fn(f32) -> f32) -> (Vec<i32>, Vec<[Vec3; 2]>) {
    let midrib = line(Vec3::ZERO, Vec3::Z, |t| 0.05 * (1.0 - t), 24);
    let root = PathLink::root(1, 0);
    let mut paths = vec![path(&midrib, root, 1)];
    let mut sides = Vec::new();
    for i in 0..pairs * 2 {
        let a = 0.1 + 0.75 * (i / 2) as f32 / (pairs - 1) as f32;
        let side = if i % 2 == 0 { 1.0 } else { -1.0 };
        let length = 0.15 + 0.25 * (a * PI).sin();
        let base = Vec3::Z * a;
        let tip = base + Vec3::new(side * 0.64, 0.0, 0.77) * length;
        let link = PathLink::child(2, root.id, a, i as i32);
        paths.push(path(&line(base, tip, &reach, 12), link, 2));
        sides.push([base, tip]);
    }
    (concat_arg_vecs(paths), sides)
}

pub fn beech() -> Vec<i32> {
    pinnate(7, |t| 0.08 * (1.0 - 0.5 * t)).0
}

/// A short midrib that ends in a ring of veins around a gap.
pub fn ring() -> Vec<i32> {
    let corners = [Vec3::new(-0.2, 0.0, 0.1), Vec3::new(0.2, 0.0, 0.1), Vec3::new(0.2, 0.0, 0.6), Vec3::new(-0.2, 0.0, 0.6)];
    let midrib = line(Vec3::ZERO, Vec3::Z * 0.1, |_| 0.03, 4);
    let root = PathLink::root(1, 0);
    let mut paths = vec![path(&midrib, root, 1)];
    for k in 0..4 {
        let link = PathLink::child(2, root.id, 1.0, k as i32);
        paths.push(path(&line(corners[k], corners[(k + 1) % 4], |_| 0.03, 8), link, 2));
    }
    concat_arg_vecs(paths)
}

/// A margin input of 16 teeth `depth` deep along alpha.
pub fn teeth(depth: f32) -> Vec<i32> {
    let alpha = concat_args(vec![&[2], &[0]]);
    let teeth = concat_args(vec![&[8], &alpha, &[encode_float(16.0)], &[1]]);
    concat_args(vec![&[0], &[2], &teeth, &[encode_float(depth)]])
}

fn run(veins: &[i32], smooth: f32, margin: &[i32], relief: f32) -> Vec<i32> {
    run_at(veins, smooth, margin, relief, 64)
}

fn run_at(veins: &[i32], smooth: f32, margin: &[i32], relief: f32, resolution: i32) -> Vec<i32> {
    __nodarium_execute_inner(&[
        veins,
        &[encode_float(1.0)],
        &[encode_float(smooth)],
        margin,
        &[encode_float(relief)],
        &[resolution],
    ])
}

struct Mesh {
    positions: Vec<Vec3>,
    normals: Vec<Vec3>,
    faces: Vec<[usize; 3]>,
}

fn read(geometry: &[i32]) -> Mesh {
    let mut data = geometry.to_vec();
    let g = wrap_geometry_data(&mut data);
    let vec3s = |v: &[f32]| v.chunks(3).map(Vec3::from_slice).collect::<Vec<_>>();
    Mesh {
        positions: vec3s(g.positions),
        normals: vec3s(g.normals),
        faces: g.faces.chunks(3).map(|f| [f[0] as usize, f[1] as usize, f[2] as usize]).collect(),
    }
}

impl Mesh {
    /// The edges that belong to a single face: the margin.
    fn boundary(&self) -> Vec<(usize, usize)> {
        let mut count: HashMap<(usize, usize), usize> = HashMap::new();
        for f in &self.faces {
            for k in 0..3 {
                let (a, b) = (f[k], f[(k + 1) % 3]);
                *count.entry((a.min(b), a.max(b))).or_default() += 1;
            }
        }
        assert!(count.values().all(|&c| c <= 2), "an edge with more than two faces");
        count.into_iter().filter(|&(_, c)| c == 1).map(|(e, _)| e).collect()
    }

    fn edges(&self) -> HashSet<(usize, usize)> {
        self.faces
            .iter()
            .flat_map(|f| (0..3).map(move |k| (f[k].min(f[(k + 1) % 3]), f[k].max(f[(k + 1) % 3]))))
            .collect()
    }

    /// A disc: one piece, no holes, every edge in at most two faces, a single margin, facing up
    /// while it is flat.
    fn assert_disc(&self) {
        let euler = self.positions.len() as i64 - self.edges().len() as i64 + self.faces.len() as i64;
        assert_eq!(euler, 1);

        let boundary = self.boundary();
        let mut next: HashMap<usize, Vec<usize>> = HashMap::new();
        for &(a, b) in &boundary {
            next.entry(a).or_default().push(b);
            next.entry(b).or_default().push(a);
        }
        assert!(next.values().all(|n| n.len() == 2), "the margin branches");
        let (start, mut previous, mut at, mut walked) = (boundary[0].0, boundary[0].0, boundary[0].1, 1);
        while at != start {
            let n = &next[&at];
            let step = if n[0] == previous { n[1] } else { n[0] };
            (previous, at) = (at, step);
            walked += 1;
        }
        assert_eq!(walked, boundary.len(), "the margin is more than one line");

        if self.positions.iter().all(|p| p.y.abs() < 1e-4) {
            assert!(self.normals.iter().all(|n| n.y > 0.99), "a flat leaf faces up");
        }
    }

    /// The vertices on the line through `points` (x, z), by how far along it they lie.
    fn on_line(&self, points: &[Vec2]) -> Vec<(f32, usize)> {
        let mut on = Vec::new();
        for (i, p) in self.positions.iter().enumerate() {
            let p = Vec2::new(p.x, p.z);
            let mut walked = 0.0;
            for w in points.windows(2) {
                let (a, b) = (w[0], w[1]);
                let t = ((p - a).dot(b - a) / (b - a).length_squared()).clamp(0.0, 1.0);
                if (a.lerp(b, t) - p).length() < 1e-3 {
                    on.push((walked + t * (b - a).length(), i));
                    break;
                }
                walked += (b - a).length();
            }
        }
        on.sort_by(|a, b| a.0.total_cmp(&b.0));
        on
    }

    /// Whether the mesh has a chain of edges along the line through `points` (x, z), from its
    /// start to `reach` along it.
    fn assert_chain(&self, points: &[Vec2], reach: f32) {
        let on = self.on_line(points);
        assert!(on.len() >= 2, "no vertices on the vein from {}", points[0]);
        assert!(on[0].0 < 1e-3, "the chain starts {} along the vein", on[0].0);
        assert!(on[on.len() - 1].0 >= reach, "the chain ends {} along the vein", on[on.len() - 1].0);
        let edges = self.edges();
        for w in on.windows(2) {
            let (a, b) = (w[0].1, w[1].1);
            assert!(edges.contains(&(a.min(b), a.max(b))), "a gap in the chain {} along the vein", w[0].0);
        }
    }

    /// Whether the blade, seen from above, covers `p` (x, z).
    fn covers(&self, p: Vec2) -> bool {
        let flat = |i: usize| Vec2::new(self.positions[i].x, self.positions[i].z);
        self.faces.iter().any(|f| {
            let [a, b, c] = f.map(flat);
            let side = |u: Vec2, v: Vec2| (v - u).perp_dot(p - u);
            let (d0, d1, d2) = (side(a, b), side(b, c), side(c, a));
            (d0 >= 0.0 && d1 >= 0.0 && d2 >= 0.0) || (d0 <= 0.0 && d1 <= 0.0 && d2 <= 0.0)
        })
    }

    fn area(&self) -> f32 {
        self.faces
            .iter()
            .map(|f| {
                let [a, b, c] = f.map(|i| self.positions[i]);
                (b - a).cross(c - a).length() * 0.5
            })
            .sum()
    }

    fn bounds(&self) -> (Vec3, Vec3) {
        let min = self.positions.iter().fold(Vec3::MAX, |m, &p| m.min(p));
        let max = self.positions.iter().fold(Vec3::MIN, |m, &p| m.max(p));
        (min, max)
    }
}

/// Where a point of the veins ends up for a midrib along +z from the origin: x is mirrored, z
/// goes along -z, 100 units per metre.
fn placed(p: Vec3) -> Vec2 {
    Vec2::new(-p.x, -p.z) * 100.0
}

fn plain(v: f32) -> Vec<i32> {
    vec![encode_float(v)]
}

#[test]
fn an_ovate_leaf_is_closed_and_has_no_holes() {
    let leaf = read(&run(&beech(), 0.1, &plain(0.0), 0.0));
    assert!(leaf.faces.len() > 200, "{} faces", leaf.faces.len());
    leaf.assert_disc();
    assert!(leaf.normals.iter().all(|n| n.y > 0.99), "a flat leaf faces up");
}

#[test]
fn a_beech_has_500_to_1000_triangles() {
    let leaf = read(&run_at(&beech(), 0.1, &plain(0.0), 0.003, 96));
    assert!((500..=1000).contains(&leaf.faces.len()), "{} faces", leaf.faces.len());
    leaf.assert_disc();
}

#[test]
fn the_midrib_and_the_side_veins_are_chains_of_edges() {
    let (veins, sides) = pinnate(7, |t| 0.08 * (1.0 - 0.5 * t));
    for resolution in [64, 96] {
        let leaf = read(&run_at(&veins, 0.1, &plain(0.0), 0.0, resolution));
        // the midrib ends where the blade tapers into the margin, short of its last cell
        let cell = 116.0 / resolution as f32;
        leaf.assert_chain(&[placed(Vec3::ZERO), placed(Vec3::Z)], 100.0 - 2.0 * cell);
        for &[base, tip] in &sides {
            let line = [placed(base), placed(tip)];
            leaf.assert_chain(&line, (line[1] - line[0]).length() - 1e-3);
        }
    }
}

#[test]
fn a_midrib_that_tapers_to_nothing_reaches_its_tip() {
    // the grid of the field is coarser than the end of the midrib
    let midrib = line(Vec3::ZERO, Vec3::Z, |t| 0.06 * (1.0 - t), 24);
    let veins = concat_arg_vecs(vec![path(&midrib, PathLink::root(1, 0), 1)]);
    let leaf = read(&run(&veins, 0.0, &plain(0.0), 0.0));
    let (min, _) = leaf.bounds();
    assert!((min.z + 100.0).abs() < 0.1, "tip at {}", min.z);
    leaf.assert_disc();
    leaf.assert_chain(&[placed(Vec3::ZERO), placed(Vec3::Z)], 100.0 - 1e-3);
}

#[test]
fn a_wide_fillet_around_a_fine_leaf_keeps_the_whole_blade() {
    // the padding for a fillet of 1.5 around a leaf 1.1 across at 512 cells is more than 1024
    // cells, where the grid used to end, cutting the blade off flat
    let midrib = line(Vec3::ZERO, Vec3::Z, |t| 0.05 * (1.0 - t), 24);
    let veins = concat_arg_vecs(vec![path(&midrib, PathLink::root(1, 0), 1)]);
    let leaf = read(&run_at(&veins, 1.5, &plain(0.0), 0.0, 512));
    let (min, max) = leaf.bounds();
    assert!((min.z + 100.0).abs() < 0.5, "tip at {}", min.z);
    assert!((max.z - 5.0).abs() < 0.5, "base at {}", max.z);
    assert!((min.x + max.x).abs() < 0.5, "midrib off the middle: {min} {max}");
    leaf.assert_disc();
}

#[test]
fn the_gaps_the_veins_close_in_are_filled() {
    let leaf = read(&run(&ring(), 0.0, &plain(0.0), 0.0));

    assert!(leaf.covers(placed(Vec3::new(0.0, 0.0, 0.35))), "the middle of the ring is open");
    leaf.assert_disc();
}

#[test]
fn lobes_follow_the_side_veins() {
    // the blade is widest near the end of every side vein, like an oak
    let (veins, sides) = pinnate(4, |t| 0.01 + 0.04 * t * t);
    let tips: Vec<Vec3> = sides.iter().map(|[_, tip]| *tip).collect();
    let lobed = read(&run(&veins, 0.01, &plain(0.0), 0.0));
    let entire = read(&run(&veins, 0.5, &plain(0.0), 0.0));
    lobed.assert_disc();
    entire.assert_disc();

    for pair in tips.chunks(2).collect::<Vec<_>>().windows(2) {
        for side in 0..2 {
            let (a, b) = (pair[0][side], pair[1][side]);
            assert!(lobed.covers(placed(a)) && lobed.covers(placed(b)), "a lobe around the tip");
            // between two lobes the margin cuts in toward the midrib
            let sinus = (a + b) * 0.5;
            assert!(!lobed.covers(placed(sinus)), "no sinus between {a} and {b}");
            assert!(entire.covers(placed(sinus)), "smooth fills the sinus between {a} and {b}");
        }
    }
}

#[test]
fn a_margin_of_0_leaves_the_outline_unchanged() {
    let veins = beech();
    let plain_leaf = run(&veins, 0.1, &plain(0.0), 0.003);
    assert_eq!(run(&veins, 0.1, &teeth(0.0), 0.003), plain_leaf);

    let toothed = read(&run(&veins, 0.1, &teeth(0.3), 0.003));
    let entire = read(&plain_leaf);
    toothed.assert_disc();
    assert!(toothed.area() < entire.area() * 0.97, "the teeth cut into the blade");
    assert!(toothed.area() > entire.area() * 0.8, "the teeth only cut the margin");
}

#[test]
fn the_margin_cuts_a_share_of_the_reach() {
    // 0.05 m (5 units) around the midrib, a fifth of it cut
    let midrib = line(Vec3::ZERO, Vec3::Z, |_| 0.05, 24);
    let veins = concat_arg_vecs(vec![path(&midrib, PathLink::root(1, 0), 1)]);
    for (share, width) in [(0.2, 4.0), (-0.2, 6.0)] {
        let leaf = read(&run_at(&veins, 0.0, &plain(share), 0.0, 128));
        let (min, max) = leaf.bounds();
        assert!((max.x - width).abs() < 0.2 && (min.x + width).abs() < 0.2, "{share}: {min} {max}");
    }
}

#[test]
fn a_thin_lobe_keeps_its_tip_with_teeth() {
    // a lobe that tapers to nothing, thinner at its tip than the teeth are deep at its base
    let midrib = line(Vec3::ZERO, Vec3::Z, |t| 0.06 * (1.0 - t), 24);
    let veins = concat_arg_vecs(vec![path(&midrib, PathLink::root(1, 0), 1)]);
    let entire = read(&run(&veins, 0.0, &plain(0.0), 0.0));
    let toothed = read(&run(&veins, 0.0, &teeth(0.3), 0.0));
    toothed.assert_disc();
    let (min, _) = toothed.bounds();
    assert!((min.z + 100.0).abs() < 0.1, "tip at {}", min.z);
    toothed.assert_chain(&[placed(Vec3::ZERO), placed(Vec3::Z)], 100.0 - 1e-3);
    assert!(toothed.area() < entire.area() * 0.95, "the teeth cut into the blade");
    assert!(toothed.area() > entire.area() * 0.7, "the teeth only cut the margin");
}

#[test]
fn the_leaf_is_placed_like_the_one_of_the_leaf_node() {
    // the leaf node grows along -z from the origin, 100 long at size 1, and faces +y
    let leaf = |turn: f32, start: Vec3, size: f32| {
        let along = Quat::from_rotation_y(turn) * Vec3::Z;
        let midrib = line(start, start + along, |t| 0.02 + 0.06 * (1.0 - t), 24);
        let veins = path(&midrib, PathLink::root(1, 0), 1);
        read(&__nodarium_execute_inner(&[
            &concat_arg_vecs(vec![veins]),
            &[encode_float(size)],
            &[encode_float(0.0)],
            &plain(0.0),
            &[encode_float(0.0)],
            &[64],
        ]))
    };

    let straight = leaf(0.0, Vec3::ZERO, 1.0);
    let (min, max) = straight.bounds();
    // the blade reaches 0.02 m (2 units) beyond the tip and 0.08 m behind the base
    let cell = 116.0 / 64.0;
    assert!((min.z + 102.0).abs() < cell, "tip at {min}");
    assert!((max.z - 8.0).abs() < cell, "base at {max}");
    assert!((min.x + max.x).abs() < cell, "midrib off the middle: {min} {max}");
    assert!(min.y.abs() < 1e-4 && max.y.abs() < 1e-4);
    assert!(straight.normals.iter().all(|n| n.y > 0.99), "the leaf faces up");

    // the frame comes from the veins: turned and moved, the leaf is the same
    let moved = leaf(2.1, Vec3::new(3.0, 0.5, -2.0), 1.0);
    let (moved_min, moved_max) = moved.bounds();
    assert!((moved_min - min).abs().max_element() < cell, "{moved_min} vs {min}");
    assert!((moved_max - max).abs().max_element() < cell, "{moved_max} vs {max}");

    let half = leaf(0.0, Vec3::ZERO, 0.5);
    let (half_min, _) = half.bounds();
    assert!((half_min.z + 51.0).abs() < cell);
}

#[test]
fn the_heights_of_the_veins_bend_the_blade() {
    // a midrib that arches down toward its tip
    let midrib: Vec<(Vec3, f32)> = (0..24)
        .map(|i| {
            let t = i as f32 / 23.0;
            (Vec3::new(0.0, -0.3 * t * t, t), 0.1 * (1.0 - t * 0.8))
        })
        .collect();
    let veins = concat_arg_vecs(vec![path(&midrib, PathLink::root(1, 0), 1)]);
    let leaf = read(&run(&veins, 0.0, &plain(0.0), 0.0));
    let tip = leaf.positions.iter().min_by(|a, b| a.z.total_cmp(&b.z)).unwrap();
    let base = leaf.positions.iter().min_by(|a, b| a.length().total_cmp(&b.length())).unwrap();
    assert!(tip.y < -25.0, "tip at {tip}");
    assert!(base.y.abs() < 1.0, "base at {base}");

    // the midrib is a chain of edges that follows the arch without a zig-zag
    let along = leaf.on_line(&[placed(Vec3::ZERO), placed(Vec3::Z)]);
    assert!(along.len() > 10);
    for w in along.windows(2) {
        let (a, b) = (leaf.positions[w[0].1], leaf.positions[w[1].1]);
        assert!(b.y < a.y, "the midrib climbs from {a} to {b}");
    }
    for (s, i) in along {
        let t = s / 100.0;
        let y = leaf.positions[i].y;
        assert!((y + 30.0 * t * t).abs() < 0.5, "the midrib is at {y} at {t}");
    }
}

#[test]
fn relief_sinks_the_veins() {
    // 0.01 m deep, 1 unit at size 1
    let leaf = read(&run(&beech(), 0.1, &plain(0.0), 0.01));
    let (min, max) = leaf.bounds();
    assert!(min.y < -0.6 && min.y >= -1.0 - 1e-4, "the veins lie {} deep", min.y);
    assert!(max.y.abs() < 1e-3, "the blade between the veins stays at {}", max.y);
}

#[test]
fn no_veins_give_no_leaf() {
    assert_eq!(run(&concat_arg_vecs(vec![]), 0.1, &plain(0.0), 0.0), concat_arg_vecs(vec![]));
}
