use super::*;
use crate::sheet_checks::{open_edge, volume};
use crate::{closing_turn, pose_caps, testing, CapRay};
use glam::Vec2;
use nodarium_utils::{concat_args, encode_float, geometry::wrap_path};
use std::f32::consts::PI;

const SPACING: f32 = 0.002;

/// A pattern of one value all over a disc 5 cm in radius, as far as the test cap reaches
fn even_pattern(value: f32) -> Pattern {
    let rings = (0..51).map(|i| vec![value; 8 + i * 8]).collect();
    Pattern { inner: 0.0, outer: 0.05, spacing: SPACING, rings }
}

/// Walls at every other ring of 2 mm: pores in rings around the axis
fn ring_pattern() -> Pattern {
    let rings = (0..501).map(|i| vec![if (i / 10) % 2 == 0 { 1.0 } else { 0.0 }; 16]).collect();
    Pattern { inner: 0.0, outer: 0.05, spacing: SPACING, rings }
}

/// The cone of `testing` closed by `closing`, like `cap_open` closes it
fn cap(closing: f32) -> Vec<Vec<i32>> {
    let paths = testing::grow(&testing::cone());
    let refs: Vec<&[i32]> = paths.iter().map(|p| p.as_slice()).collect();
    pose_caps(&refs, |cap, k| Some(cap.bend(k, |step| closing_turn(step, closing))))
}

/// The output of a curve node over alpha, from 0 to `max`
fn curve_node(points: &[f32], max: f32) -> Vec<i32> {
    let mut args: Vec<Vec<i32>> =
        vec![vec![3], concat_args(vec![&[2], &[0]]), vec![encode_float(0.0)], vec![encode_float(max)]];
    args.extend(points.iter().map(|&p| vec![encode_float(p)]));
    concat_args(args.iter().map(|a| a.as_slice()).collect())
}

struct Run {
    depth: Vec<i32>,
    free_distance: f32,
    pore_depth: f32,
}

fn run() -> Run {
    Run { depth: vec![encode_float(0.004)], free_distance: 0.001, pore_depth: 0.001 }
}

fn build(paths: &[Vec<i32>], pattern: &Pattern, r: &Run) -> Mesh {
    let (cap, pattern) = (testing::list(paths), pattern.encode());
    let (free, pore) = ([encode_float(r.free_distance)], [encode_float(r.pore_depth)]);
    pores(&PoreInputs { cap: &cap, pattern: &pattern, depth: &r.depth, free_distance: &free, pore_depth: &pore })
}

fn vertices(mesh: &Mesh) -> Vec<Vec3> {
    mesh.positions.chunks_exact(3).map(Vec3::from_slice).collect()
}

fn from_axis(p: Vec3) -> f32 {
    Vec2::new(p.x, p.z).length()
}

/// Where a vertex is under a cap
#[derive(Clone, Copy)]
struct Place {
    p: Vec3,
    /// How far it is over the middle of the flesh of the ray nearest to it around the axis,
    /// negative under it
    over: f32,
    /// How far it is over the underside of that ray
    under: f32,
}

fn places(paths: &[Vec<i32>], mesh: &Mesh) -> Vec<Place> {
    let cap = &testing::caps(paths)[0];
    vertices(mesh).into_iter().map(|p| place(cap, paths, p)).collect()
}

fn place(cap: &Cap, paths: &[Vec<i32>], p: Vec3) -> Place {
    let frame = cap.frame;
    let angle = frame.angle(p);
    let off = |r: &CapRay| ((r.angle - angle + PI).rem_euclid(TAU) - PI).abs();
    let ray = cap.rays.iter().filter(|r| r.crosses).min_by(|a, b| off(a).total_cmp(&off(b))).unwrap();
    let path = wrap_path(&paths[ray.path]);
    let radius = |i: usize| path.points[i * 4 + 3];
    let q = p - frame.hinge;
    let up = q.dot(frame.axis);
    let at = Vec2::new((q - frame.axis * up).length(), up);
    let mut best = (f32::MAX, 0.0, 0.0);
    for i in 0..ray.plane.len() - 1 {
        let (a, b) = (ray.plane[i], ray.plane[i + 1]);
        if b.x < 0.0 {
            continue;
        }
        let ab = b - a;
        let t = ((at - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0);
        let near = a + ab * t;
        let d = at.distance(near);
        if d < best.0 {
            let r = radius(i) * (1.0 - t) + radius(i + 1) * t;
            best = (d, (at - near).dot(ab.perp().normalize()), r.abs());
        }
    }
    Place { p, over: best.1, under: best.1 + best.2 }
}

#[test]
fn a_cap_without_its_stipe_gives_nothing() {
    let rays = testing::rays(&cap(0.0));
    assert_eq!(build(&rays, &ring_pattern(), &run()), Mesh::default());
    assert!(build(&cap(0.0), &ring_pattern(), &run()).face_count() > 0);
}

#[test]
fn depth_0_gives_nothing() {
    assert_eq!(build(&cap(0.0), &ring_pattern(), &Run { depth: vec![encode_float(0.0)], ..run() }), Mesh::default());
    // nor does a curve that stays at 0
    let flat = curve_node(&[0.0, 0.0, 1.0, 0.0], 0.004);
    assert_eq!(build(&cap(0.0), &ring_pattern(), &Run { depth: flat, ..run() }), Mesh::default());
    // or no pattern
    let (paths, depth) = (testing::list(&cap(0.0)), [encode_float(0.004)]);
    let mesh = pores(&PoreInputs { cap: &paths, pattern: &[], depth: &depth, free_distance: &depth, pore_depth: &depth });
    assert_eq!(mesh, Mesh::default());
}

#[test]
fn pores_stay_under_the_cap_as_it_opens() {
    let mut lowest = Vec::new();
    for closing in [1.0, 0.6, 0.3, 0.0] {
        let paths = cap(closing);
        let mesh = build(&paths, &ring_pattern(), &run());
        assert!(mesh.face_count() > 0, "closing {closing}");
        let places = places(&paths, &mesh);
        for Place { p, over, .. } in &places {
            assert!(*over < 1e-5, "closing {closing}: a vertex {over} over the middle of the flesh at {p}");
            assert!(from_axis(*p) <= 0.03 + 1e-4, "closing {closing}: a vertex {} from the axis", from_axis(*p));
        }
        lowest.push(places.iter().map(|o| o.under).fold(0.0, f32::min));
    }
    // about as deep however far the cap is open, the tubes lean to hang down where it slopes
    for l in &lowest {
        assert!(*l < -0.003 && *l > -0.0042, "the layer reaches {l} under the underside");
    }
}

#[test]
fn the_layer_is_closed_and_looks_outwards() {
    for closing in [0.0, 0.9] {
        let mesh = build(&cap(closing), &ring_pattern(), &run());
        assert_eq!(open_edge(&mesh), None);
        assert!(volume(&mesh) > 0.0);
        assert!(mesh.groups.iter().all(|&g| g == 0));
        for n in mesh.normals.chunks_exact(3) {
            assert!((Vec3::from_slice(n).length() - 1.0).abs() < 1e-4);
        }
    }
}

#[test]
fn the_tubes_open_where_the_pattern_is_0() {
    let paths = cap(0.0);
    let lowest = |pattern: &Pattern| places(&paths, &build(&paths, pattern, &run())).iter().map(|o| o.under).fold(0.0, f32::min);
    let (walls, open) = (lowest(&even_pattern(1.0)), lowest(&even_pattern(0.0)));
    assert!((open - walls - 0.001).abs() < 1e-4, "the pore surface goes up by {} into open tubes", open - walls);
    // pores in rings: the pore surface is at the walls or up in the tubes
    let places = places(&paths, &build(&paths, &ring_pattern(), &run()));
    let near = |level: f32| places.iter().filter(|o| (o.under - level).abs() < 1e-4).count();
    let (at_walls, in_tubes) = (near(-0.004), near(-0.003));
    assert!(at_walls > 2000 && in_tubes > 2000, "{at_walls} at the walls, {in_tubes} in the tubes");
}

#[test]
fn the_depth_is_read_along_the_cap_from_the_stipe_to_the_margin() {
    let paths = cap(0.0);
    let deepening = Run { depth: curve_node(&[0.0, 0.2, 1.0, 1.0], 0.006), ..run() };
    let places = places(&paths, &build(&paths, &even_pattern(1.0), &deepening));
    let deepest_near = |x: f32| {
        places.iter().filter(|o| (from_axis(o.p) - x).abs() < 0.001).map(|o| o.under).fold(0.0, f32::min)
    };
    let (inner, outer) = (deepest_near(0.008), deepest_near(0.025));
    assert!(outer < inner - 0.002, "{inner} near the stipe, {outer} near the margin");
}

#[test]
fn the_tubes_keep_the_free_distance_to_the_stipe() {
    for free in [0.0, 0.003] {
        let mesh = build(&cap(0.0), &ring_pattern(), &Run { free_distance: free, ..run() });
        let closest = vertices(&mesh).iter().map(|&p| from_axis(p)).fold(f32::MAX, f32::min);
        assert!(closest > testing::STIPE_RADIUS + free - 2e-4, "free {free}: a vertex {closest} from the axis");
        assert!(closest < testing::STIPE_RADIUS + free + 1e-3, "free {free}: the closest vertex is {closest} from the axis");
    }
}

