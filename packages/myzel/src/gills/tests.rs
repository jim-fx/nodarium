use super::*;
use nodarium_utils::{concat_arg_vecs, concat_args, curve, encode_float, geometry::path_link};

const CREATOR: i32 = 9;
const STIPE_ID: i32 = 1234;
const STIPE_RADIUS: f32 = 0.005;
const RADIUS: f32 = 0.025;
const HINGE: Vec3 = Vec3::new(0.0, 0.05, 0.0);
/// The depth curve `lamellae` had built in, from the stipe to the margin
const OLD_DEPTH_CURVE: [f32; 10] = [0.0, 0.3, 0.2, 0.85, 0.45, 1.0, 0.8, 0.65, 1.0, 0.1];

/// The middle of the flesh of an open cap in the plane of a ray: it starts inside the stipe
/// on the far side of the axis, rises over the axis and falls along a straight slope to the
/// margin, `RADIUS` out and 6 mm below the hinge
fn midline(x: f32) -> f32 {
    let top = 0.004;
    if x <= 0.0 {
        top + x * 2.0
    } else {
        top - (top + 0.006) * x / RADIUS
    }
}

/// Half the flesh thickness, from 4 mm over the axis to 1 mm at the margin
fn flesh(x: f32) -> f32 {
    0.004 - 0.003 * (x / RADIUS).clamp(0.0, 1.0)
}

/// The rays and the stipe they grow on
fn cap(bend: f32) -> Vec<i32> {
    with_stipe(rays(bend))
}

fn with_stipe(rays: Vec<Vec<i32>>) -> Vec<i32> {
    with(rays, stipe())
}

fn with(rays: Vec<Vec<i32>>, stipe: Vec<i32>) -> Vec<i32> {
    concat_arg_vecs(rays.into_iter().chain([stipe]).collect())
}

/// 12 rays that start in the stipe and 12 that start 12 mm out between them, like `cap`
/// makes them. `bend` turns the outer part of every ray down, like a closing cap.
fn rays(bend: f32) -> Vec<Vec<i32>> {
    let mut rays = Vec::new();
    for k in 0..24 {
        let primary = k % 2 == 0;
        let angle = TAU * k as f32 / 24.0 + 0.01;
        let out = Vec3::new(angle.cos(), 0.0, angle.sin());
        let start = if primary { -0.0015 } else { 0.012 };
        let n = 48;
        let mut data = create_path(n, 2);
        set_path_link(&mut data, PathLink::child(7, STIPE_ID, 1.0, k));
        let path = wrap_path_mut(&mut data);
        let mut previous = Vec2::new(start, midline(start));
        let mut position = previous;
        for i in 0..n {
            let x = start + (RADIUS - start) * i as f32 / (n - 1) as f32;
            let target = Vec2::new(x, midline(x));
            if i > 0 {
                // the segment keeps its length and turns down by `bend` towards the margin
                let step = target - previous;
                let turn = bend * (x / RADIUS).max(0.0);
                position += Vec2::from_angle(-turn).rotate(step);
            }
            previous = target;
            let p = HINGE + out * position.x + Vec3::Y * position.y;
            // an added ray starts at half the flesh and grows to all of it, like in `cap`
            let ramp = if primary { 1.0 } else { 0.5 + 0.5 * smoothstep(0.0, 2.0 * flesh(start), x - start) };
            path.points[i * 4..i * 4 + 4].copy_from_slice(&[p.x, p.y, p.z, flesh(x) * ramp]);
        }
        rays.push(data);
    }
    rays
}

/// `inner` stripes from 4 mm out, every gap gets a new stripe from `split` on
fn pattern(inner: usize, split: f32) -> Pattern {
    let (from, to) = (0.004, 0.03);
    let rings = 105;
    let rings = (0..rings)
        .map(|i| {
            let r = from + (to - from) * i as f32 / (rings - 1) as f32;
            let n = (TAU * r / 0.0001) as usize;
            let stripes = if r < split { inner } else { inner * 2 };
            (0..n).map(|j| 0.5 + 0.5 * (stripes as f32 * nodarium_utils::ring_angle(j, n)).cos()).collect()
        })
        .collect();
    Pattern { inner: from, outer: to, spacing: TAU * 0.01 / inner as f32, rings }
}

/// The stipe under the cap: upright from `base` to the hinge
fn stipe_from(base: f32) -> Vec<i32> {
    let n = 21;
    let mut data = create_path(n, 1);
    set_path_link(&mut data, PathLink { id: STIPE_ID, ..PathLink::root(3, 0) });
    let path = wrap_path_mut(&mut data);
    for i in 0..n {
        let y = base + (HINGE.y - base) * i as f32 / (n - 1) as f32;
        path.points[i * 4..i * 4 + 4].copy_from_slice(&[HINGE.x, y, HINGE.z, STIPE_RADIUS]);
    }
    data
}

fn stipe() -> Vec<i32> {
    stipe_from(0.0)
}

/// The output of a curve node over alpha, from 0 to `max`
fn curve_node(points: &[f32], max: f32) -> Vec<i32> {
    let mut args: Vec<Vec<i32>> =
        vec![vec![3], concat_args(vec![&[2], &[0]]), vec![encode_float(0.0)], vec![encode_float(max)]];
    args.extend(points.iter().map(|&p| vec![encode_float(p)]));
    concat_args(args.iter().map(|a| a.as_slice()).collect())
}

#[derive(Clone)]
struct Params {
    /// The depth input: a value or an expression
    depth: Vec<i32>,
    free_distance: f32,
    decurrent: f32,
    thickness: f32,
    edge_wave: f32,
    seed: i32,
}

/// What lamellae made by default: 6 mm deep along the curve it had built in
fn params() -> Params {
    Params {
        depth: curve_node(&OLD_DEPTH_CURVE, 0.006),
        free_distance: 0.001,
        decurrent: 0.0,
        thickness: 0.0004,
        edge_wave: 0.3,
        seed: 2,
    }
}

fn even(depth: f32) -> Vec<i32> {
    vec![encode_float(depth)]
}

fn build(cap: &[i32], pattern: &Pattern, p: &Params, profile: GillProfile) -> Gills {
    let encoded = pattern.encode();
    let (free, decurrent, thickness, wave) =
        ([encode_float(p.free_distance)], [encode_float(p.decurrent)], [encode_float(p.thickness)], [encode_float(p.edge_wave)]);
    let inputs = GillInputs {
        cap,
        pattern: &encoded,
        depth: &p.depth,
        free_distance: Some(&free),
        decurrent: &decurrent,
        thickness: &thickness,
        edge_wave: &wave,
        seed: p.seed,
        creator: CREATOR,
    };
    gills(&inputs, profile)
}

fn run(cap: &[i32], pattern: &Pattern, p: &Params) -> Vec<Vec<i32>> {
    build(cap, pattern, p, GillProfile::Plate).paths
}

fn plates(cap: &[i32], pattern: &Pattern, p: &Params) -> Mesh {
    build(cap, pattern, p, GillProfile::Plate).mesh
}

fn points(path: &[i32]) -> Vec<Vec3> {
    wrap_path(path).points.chunks_exact(4).map(|p| Vec3::new(p[0], p[1], p[2])).collect()
}

fn vertices(mesh: &Mesh) -> Vec<Vec3> {
    mesh.positions.chunks_exact(3).map(Vec3::from_slice).collect()
}

fn from_axis(p: Vec3) -> f32 {
    Vec2::new(p.x - HINGE.x, p.z - HINGE.z).length()
}

/// The underside of the test cap, `x` from the axis
fn underside(x: f32) -> f32 {
    HINGE.y + midline(x) - flesh(x)
}

#[test]
fn gills_stay_under_the_cap_and_inside_the_margin() {
    let gills = run(&cap(0.0), &pattern(24, 0.014), &params());
    assert!(!gills.is_empty());
    for gill in &gills {
        for p in points(gill) {
            let x = from_axis(p);
            assert!(x <= RADIUS, "gill {x} from the axis, the margin is at {RADIUS}");
            // below the top of the flesh, and the deepest strands below its underside
            let top = HINGE.y + midline(x) + flesh(x);
            assert!(p.y < top, "gill at {} over a cap top at {top}", p.y);
        }
    }
    // the gill edges hang below the underside
    let lowest = gills.iter().flat_map(|g| points(g)).map(|p| p.y - underside(from_axis(p))).fold(0.0, f32::min);
    assert!(lowest < -0.004, "the gills reach {lowest} below the underside");
}

#[test]
fn the_underside_has_no_step_where_rays_are_added() {
    let rays = concat_arg_vecs(rays(0.0));
    let frame = CapFrame::on(&stipe(), 1.0);
    let underside = Underside::new(frame, &split_args(&rays)).unwrap();
    for angle in [0.01, 0.2, TAU * 1.5 / 24.0] {
        let height = |s: f32| {
            let spot = underside.at(angle, s).unwrap();
            (spot.position - spot.normal * spot.radius).y
        };
        let mut s = 0.008;
        while s < 0.022 {
            let step = (height(s + 0.0005) - height(s)).abs();
            assert!(step < 0.0003, "the underside steps {step} at {s} on angle {angle}");
            s += 0.0005;
        }
    }
}

#[test]
fn no_gill_is_closer_to_the_axis_than_the_free_distance() {
    for free_distance in [0.0, 0.001, 0.004] {
        let gills = run(&cap(0.0), &pattern(24, 0.014), &Params { free_distance, ..params() });
        let closest = gills.iter().flat_map(|g| points(g)).map(from_axis).fold(f32::MAX, f32::min);
        assert!(closest >= STIPE_RADIUS + free_distance - 1e-5, "a gill is {closest} from the axis");
        // and they start close to it
        assert!(closest < STIPE_RADIUS + free_distance + 0.001, "the gills start {closest} from the axis");
    }
}

/// The gills that cross a circle `x` from the axis, by the angle of their top strand
fn plates_at(gills: &[Vec<i32>], x: f32) -> usize {
    let mut angles: Vec<f32> = gills
        .iter()
        .filter_map(|g| {
            let pts = points(g);
            let k = pts.windows(2).position(|w| (from_axis(w[0]) - x) * (from_axis(w[1]) - x) <= 0.0)?;
            let p = pts[k];
            Some(p.z.atan2(p.x).rem_euclid(TAU))
        })
        .collect();
    angles.sort_by(f32::total_cmp);
    angles.dedup_by(|a, b| (*a - *b).abs() < 0.02);
    let wraps = angles.len() > 1 && angles[0] + TAU - angles[angles.len() - 1] < 0.02;
    angles.len() - wraps as usize
}

#[test]
fn every_stripe_is_a_plate_and_new_stripes_start_between_them() {
    let gills = run(&cap(0.0), &pattern(24, 0.014), &Params { edge_wave: 0.0, ..params() });
    assert_eq!(plates_at(&gills, 0.01), 24);
    assert_eq!(plates_at(&gills, 0.02), 48);
    // a plate is a stack of paths, all children of the stipe made by this node
    for (k, gill) in gills.iter().enumerate() {
        assert_eq!(path_link(gill), PathLink::child(CREATOR, STIPE_ID, 1.0, k as i32));
    }
    let short: Vec<&Vec<i32>> = gills.iter().filter(|g| from_axis(points(g)[0]) > 0.012).collect();
    assert!(!short.is_empty());
}

/// How deep the gills reach below the underside around `x` from the axis
fn deepest_near(gills: &[Vec<i32>], x: f32) -> f32 {
    gills
        .iter()
        .flat_map(|g| points(g))
        .filter(|p| (from_axis(*p) - x).abs() < 0.001)
        .map(|p| underside(from_axis(p)) - p.y)
        .fold(0.0, f32::max)
}

#[test]
fn depth_follows_the_input() {
    let reach = |depth: f32| {
        let gills = run(&cap(0.0), &pattern(24, 0.03), &Params { depth: even(depth), edge_wave: 0.0, ..params() });
        gills.iter().flat_map(|g| points(g)).map(|p| underside(from_axis(p)) - p.y).fold(0.0, f32::max)
    };
    let (a, b) = (reach(0.004), reach(0.008));
    // measured vertically under a sloped cap, so a bit less than the depth
    assert!((a - 0.004).abs() < 0.0008, "depth 4 mm reaches {a}");
    assert!((b - 0.008).abs() < 0.0012, "depth 8 mm reaches {b}");
    assert!(reach(0.0) < 0.0005);
    // a plain value is as deep at the stipe as at the margin
    let gills = run(&cap(0.0), &pattern(24, 0.03), &Params { depth: even(0.004), edge_wave: 0.0, ..params() });
    let (near, far) = (deepest_near(&gills, 0.008), deepest_near(&gills, 0.02));
    assert!((near - far).abs() < 0.0008, "{near} at the stipe, {far} at the margin");
}

#[test]
fn depth_is_read_along_the_gill_from_the_stipe_to_the_margin() {
    let deepest = |curve: &[f32], x: f32| {
        let gills = run(&cap(0.0), &pattern(24, 0.03), &Params { depth: curve_node(curve, 0.006), edge_wave: 0.0, ..params() });
        deepest_near(&gills, x)
    };
    let rising = [0.0, 0.2, 1.0, 1.0];
    assert!(deepest(&rising, 0.008) < 0.5 * deepest(&rising, 0.022));
    let falling = [0.0, 1.0, 1.0, 0.2];
    assert!(deepest(&falling, 0.008) > 2.0 * deepest(&falling, 0.022));
}

#[test]
fn a_curve_that_draws_the_old_depth_curve_gives_the_old_plates() {
    // lamellae made its depth as depth * depth_curve(along), read here at every point
    let old = |along: f32| 0.006 * curve(&OLD_DEPTH_CURVE, along).max(0.0);
    let p = params();
    let encoded = pattern(24, 0.014).encode();
    let (free, decurrent, thickness, wave) =
        ([encode_float(p.free_distance)], [encode_float(0.0)], [encode_float(p.thickness)], [encode_float(p.edge_wave)]);
    let cap = cap(0.0);
    let inputs = GillInputs {
        cap: &cap,
        pattern: &encoded,
        depth: &p.depth,
        free_distance: Some(&free),
        decurrent: &decurrent,
        thickness: &thickness,
        edge_wave: &wave,
        seed: p.seed,
        creator: CREATOR,
    };
    let exact = gills_by(&inputs, GillProfile::Plate, |_| DepthAlong((0..=4096).map(|i| old(i as f32 / 4096.0)).collect()));
    let from_curve = gills(&inputs, GillProfile::Plate);
    assert_eq!(from_curve.mesh.vertex_count(), exact.mesh.vertex_count());
    assert_eq!(from_curve.mesh.faces, exact.mesh.faces);
    let apart = vertices(&from_curve.mesh).iter().zip(vertices(&exact.mesh)).map(|(a, b)| a.distance(b)).fold(0.0, f32::max);
    assert!(apart < 1e-5, "the plates are up to {apart} apart");
}

#[test]
fn edge_wave_moves_the_edge_and_leaves_the_top() {
    let straight = run(&cap(0.0), &pattern(24, 0.03), &Params { edge_wave: 0.0, ..params() });
    let wavy = run(&cap(0.0), &pattern(24, 0.03), &Params { edge_wave: 1.0, ..params() });
    assert_eq!(straight.len(), wavy.len());
    // the edge of the first gill: the last strand of its plate
    let strands = straight.iter().filter(|g| from_axis(points(g)[0]) < 0.008).count() / 24;
    let sideways = |gills: &[Vec<i32>]| {
        let edge = points(&gills[strands - 1]);
        let angles: Vec<f32> = edge.iter().map(|p| (p.z - HINGE.z).atan2(p.x - HINGE.x)).collect();
        let mean = angles.iter().sum::<f32>() / angles.len() as f32;
        angles.iter().map(|a| (a - mean).abs() * 0.015).fold(0.0, f32::max)
    };
    assert!(sideways(&straight) < 1e-4, "a straight edge moves {}", sideways(&straight));
    assert!(sideways(&wavy) > 2e-4, "a wavy edge moves {}", sideways(&wavy));
    // the top strand stays where it is
    assert_eq!(points(&straight[0]), points(&wavy[0]));
}

#[test]
fn gills_follow_the_cap_as_it_closes() {
    let open = run(&cap(0.0), &pattern(24, 0.014), &params());
    let closing = run(&cap(0.8), &pattern(24, 0.014), &params());
    // the same gills, the pattern sits on the same spots of the tissue
    assert_eq!(open.len(), closing.len());
    for (a, b) in open.iter().zip(&closing) {
        assert_eq!(path_link(a), path_link(b));
        assert_eq!(points(a).len(), points(b).len());
    }
    // the margin comes down, so do the outer gills
    let low = |gills: &[Vec<i32>]| gills.iter().flat_map(|g| points(g)).map(|p| p.y).fold(f32::MAX, f32::min);
    assert!(low(&closing) < low(&open) - 0.003);
}

#[test]
fn plates_and_folds_are_closed_sheets_under_the_cap() {
    let cases = [
        (params(), GillProfile::Plate),
        (Params { edge_wave: 1.0, free_distance: 0.003, ..params() }, GillProfile::Plate),
        (Params { thickness: 0.0015, free_distance: 0.0, ..params() }, GillProfile::Fold),
    ];
    for (p, profile) in cases {
        let mesh = build(&cap(0.0), &pattern(24, 0.014), &p, profile).mesh;
        assert!(mesh.face_count() > 0);
        assert!(mesh.groups.iter().all(|&g| g == 0));
        for (v, q) in vertices(&mesh).into_iter().enumerate() {
            let x = from_axis(q);
            assert!(x <= RADIUS + 1e-4, "plate {x} from the axis");
            assert!(x >= STIPE_RADIUS + p.free_distance - 1e-5, "plate {x} from the axis, free distance {}", p.free_distance);
            assert!(q.y < HINGE.y + midline(x) + flesh(x));
            let n = Vec3::from_slice(&mesh.normals[v * 3..v * 3 + 3]);
            assert!((n.length() - 1.0).abs() < 1e-4);
        }
        // closed: every edge belongs to two faces, walked once each way
        let mut edges = std::collections::HashMap::new();
        for f in mesh.faces.chunks_exact(3) {
            for (a, b) in [(f[0], f[1]), (f[1], f[2]), (f[2], f[0])] {
                *edges.entry((a, b)).or_insert(0) += 1;
            }
        }
        for (&(a, b), &n) in &edges {
            assert_eq!(n, 1, "edge {a}-{b} is used {n} times the same way");
            assert!(edges.contains_key(&(b, a)), "edge {a}-{b} is open");
        }
    }
}

#[test]
fn plates_and_folds_lie_where_the_same_pattern_has_its_stripes() {
    for decurrent in [0.0, 0.01] {
        let p = Params { depth: even(0.002), thickness: 0.0011, free_distance: 0.0, decurrent, ..params() };
        let plate = build(&cap(0.0), &pattern(24, 0.014), &p, GillProfile::Plate);
        let fold = build(&cap(0.0), &pattern(24, 0.014), &p, GillProfile::Fold);
        assert_eq!(plate.paths.len(), fold.paths.len());
        assert_eq!(plate.mesh.vertex_count(), fold.mesh.vertex_count());
        // every gill is a stack of 6 paths here, ⌈(half the 4 mm flesh + 2 mm) / (0.75 × 1.1 mm)⌉ + 1
        let strands = 6;
        assert_eq!(plate.paths.len() % strands, 0);
        assert_eq!(plates_at(&plate.paths, 0.02), 48);
        // the top of every gill, inside the flesh, runs along the same line for both
        for (a, b) in plate.paths.iter().zip(&fold.paths).step_by(strands) {
            assert_eq!(points(a), points(b));
        }
        // the edge of a fold is no deeper than the fold is wide, the plate goes on down
        let deepest = |g: &Gills| {
            let under_cap = g.paths.iter().flat_map(|path| points(path)).filter(|q| from_axis(*q) > 0.008);
            under_cap.map(|q| underside(from_axis(q)) - q.y).fold(0.0, f32::max)
        };
        assert!(deepest(&fold) < deepest(&plate) - 0.0005, "a fold reaches {}, a plate {}", deepest(&fold), deepest(&plate));
    }
}

#[test]
fn the_edge_waves_and_the_waves_die_away_towards_the_cap() {
    let slice = Slice { top: Vec3::ZERO, normal: Vec3::Y, sway: Vec3::X * 0.001, reach: 0.004, surface: 0.25, taper: 1.0, damping: 8.0 };
    let side = |f: f32| slice.at(f).x;
    assert_eq!(side(0.0), 0.0);
    assert!((side(1.0) - 0.001).abs() < 1e-9);
    // within about a wavelength / TAU of the edge, a quarter of the way up only a little is left
    assert!(side(0.75) < 0.2 * 0.001, "{} left a quarter up", side(0.75));
    assert!((1..=10).all(|k| side(k as f32 / 10.0) >= side((k - 1) as f32 / 10.0)));
}

/// 12 stripes from 4 mm out, every one splits in two from 12 mm on, the halves move apart
/// to evenly spaced stripes by 20 mm
fn forking_pattern() -> Pattern {
    let (from, to, rings) = (0.004, 0.03, 131);
    let width = 0.0006;
    let rings = (0..rings)
        .map(|i| {
            let r = from + (to - from) * i as f32 / (rings - 1) as f32;
            let apart = ((r - 0.012) / 0.008).clamp(0.0, 1.0) * TAU / 48.0;
            let n = (TAU * r / 0.0001) as usize;
            (0..n)
                .map(|j| {
                    let angle = nodarium_utils::ring_angle(j, n);
                    (0..12)
                        .flat_map(|k| [TAU * k as f32 / 12.0 - apart, TAU * k as f32 / 12.0 + apart])
                        .map(|c| (-(r * turn(c, angle)).powi(2) / (2.0 * width * width)).exp())
                        .fold(0.0, f32::max)
                })
                .collect()
        })
        .collect();
    Pattern { inner: from, outer: to, spacing: TAU * 0.008 / 12.0, rings }
}

#[test]
fn forks_open_away_from_the_stipe() {
    // Moore (1987): forked gills always point the fork away from the stipe
    let pattern = forking_pattern();
    let gills = trace_gills(&pattern, 0, pattern.rings.len() - 1, LINK_REACH * pattern.spacing);
    let forks: Vec<&Gill> = gills.iter().filter(|g| !g.free_start && g.points[0].1 > pattern.inner + 1e-6).collect();
    assert_eq!(forks.len(), 12, "{} forks", forks.len());
    assert!(gills.iter().all(|g| !g.free_start), "the halves split off, none starts on its own");
    for fork in forks {
        let (at, r) = fork.points[0];
        // the branch goes outward from the fork point
        assert!(fork.points[1].1 > r);
        // and the stripe it splits off runs through the fork point and on outward
        let parent = gills
            .iter()
            .find(|g| !std::ptr::eq(*g, fork) && g.points.iter().any(|&(a, s)| (s - r).abs() < 1e-6 && turn(a, at).abs() < 1e-4))
            .expect("the stripe the fork splits off");
        assert!(parent.points[0].1 < r && parent.points.last().unwrap().1 > r);
    }
}

#[test]
fn a_plate_is_as_thick_as_the_input_at_its_top_and_thinner_at_its_edge() {
    // a straight plate along x, hanging down along -y
    let plate: Vec<Slice> = (0..5)
        .map(|i| Slice {
            top: Vec3::new(i as f32 * 0.001, 0.0, 0.0),
            normal: Vec3::Y,
            sway: Vec3::ZERO,
            reach: 0.004,
            surface: 0.25,
            taper: 1.0,
            damping: 8.0,
        })
        .collect();
    let mut mesh = Mesh::default();
    add_plate(&mut mesh, &plate, 0.0004, GillProfile::Plate, |q, _| q);
    let vertex = |v: usize| Vec3::from_slice(&mesh.positions[v * 3..v * 3 + 3]);
    let (m, rows) = (plate.len(), PLATE_ROWS);
    assert_eq!(mesh.vertex_count(), m * (2 * rows + 1));
    let thick = |k: usize| vertex(2 * rows + k).distance(vertex((m + 2) * rows + k));
    assert!((thick(0) - 0.0004).abs() < 1e-6, "the top is {} thick", thick(0));
    assert!((thick(rows - 1) - 0.0004 * (1.0 - EDGE_THINNING)).abs() < 1e-6, "the edge is {} thick", thick(rows - 1));
    // the edge hangs reach below the top, and its rim a little further
    assert!((vertex(2 * rows + rows - 1).y + 0.004).abs() < 1e-6);
    assert!(vertex(2 * m * rows + 2).y < -0.004);
}

#[test]
fn a_fold_is_at_least_as_wide_as_it_is_deep() {
    // a straight fold along x out of flesh above y = 0, its top 1 mm up in the flesh
    let (width, depth) = (0.0015, 0.0012);
    let plate: Vec<Slice> = (0..5)
        .map(|i| Slice {
            top: Vec3::new(i as f32 * 0.001, 0.001, 0.0),
            normal: Vec3::Y,
            sway: Vec3::ZERO,
            reach: 0.001 + depth,
            surface: 0.001 / (0.001 + depth),
            taper: 1.0,
            damping: 8.0,
        })
        .collect();
    let mut mesh = Mesh::default();
    add_plate(&mut mesh, &plate, width, GillProfile::Fold, |q, _| q);
    // the cross-section of the middle slice: its width at every height out of the flesh
    let section: Vec<Vec3> = vertices(&mesh).into_iter().filter(|q| (q.x - 0.002).abs() < 1e-6).collect();
    let deepest = section.iter().map(|q| -q.y).fold(f32::MIN, f32::max);
    assert!((deepest - depth).abs() < 1e-6, "the fold reaches {deepest} out of the flesh");
    let wide_at = |h: f32| {
        let near: Vec<&Vec3> = section.iter().filter(|q| -q.y <= h + 1e-6 && q.y <= 1e-6).collect();
        near.iter().map(|q| q.z).fold(f32::MIN, f32::max) - near.iter().map(|q| q.z).fold(f32::MAX, f32::min)
    };
    assert!(wide_at(0.0) >= width, "{} wide where it leaves the flesh", wide_at(0.0));
    assert!(wide_at(0.0) >= deepest);
    // a round back, not a sharp edge: still most of its width half way out
    let half_way = 2.0 * half_share(plate[2].surface + 0.5 * (1.0 - plate[2].surface), plate[2].surface, GillProfile::Fold);
    assert!(half_way > 0.8, "{half_way} of the width half way out");
}

#[test]
fn folds_are_never_deeper_than_wide() {
    let p = Params { thickness: 0.0015, edge_wave: 0.0, ..params() };
    let folds = build(&cap(0.0), &pattern(24, 0.03), &p, GillProfile::Fold);
    assert!(!folds.paths.is_empty());
    let deepest = folds.paths.iter().flat_map(|g| points(g)).map(|q| underside(from_axis(q)) - q.y).fold(0.0, f32::max);
    // measured straight down under a cap that slopes by up to 22°, a bit more than across it
    let slope = 1.0 / (1.0f32 + (0.01 / RADIUS).powi(2)).sqrt();
    assert!(deepest * slope <= 1.05 * p.thickness, "folds reach {deepest} below the cap, {} wide", p.thickness);
    assert!(deepest > 0.5 * p.thickness, "folds reach only {deepest} below the cap");
    let lowest = vertices(&folds.mesh).into_iter().map(|q| underside(from_axis(q)) - q.y).fold(0.0, f32::max);
    assert!(lowest * slope <= 1.05 * p.thickness, "the fold mesh reaches {lowest} below the cap");
}

fn decurrent(length: f32) -> Params {
    Params { decurrent: length, free_distance: 0.0, depth: even(0.006), edge_wave: 0.0, ..params() }
}

/// The lowest vertex next to the stipe, the gills under the cap hang lower further out
fn lowest_next_to_the_stipe(mesh: &Mesh) -> f32 {
    vertices(mesh).into_iter().filter(|q| from_axis(*q) < 0.008).map(|q| q.y).fold(f32::MAX, f32::min)
}

#[test]
fn decurrent_gills_run_down_the_stipe_by_the_given_length() {
    let lowest = |length: f32| lowest_next_to_the_stipe(&plates(&cap(0.0), &pattern(24, 0.014), &decurrent(length)));
    let (none, short, long) = (lowest(0.0), lowest(0.01), lowest(0.02));
    assert!(short < none - 0.002, "gills of 1 cm reach {short}, without {none}");
    assert!(((short - long) - 0.01).abs() < 0.0005, "1 cm more reaches {} further down", short - long);
    // they start where the underside meets the stipe
    let junction = underside(STIPE_RADIUS);
    assert!((short - (junction - 0.01)).abs() < 0.001, "1 cm down from {junction} ends at {short}");
}

#[test]
fn decurrent_gills_stay_on_the_stipe_and_get_lower() {
    let p = decurrent(0.02);
    let junction = underside(STIPE_RADIUS);
    // the tops of the gills lie in the stipe, just under its surface
    let gills = run(&cap(0.0), &pattern(24, 0.014), &p);
    let embed = EMBED * flesh(0.0);
    let strands = (((embed + 0.006) / (STRAND_GAP * p.thickness)).ceil() as usize + 1).clamp(2, MAX_STRANDS);
    let mut decurrent = 0;
    for gill in gills.iter().step_by(strands) {
        // a gill that runs down the stipe starts at its lower end, on the stipe up to the cap
        let tops = points(gill);
        let on_stipe: Vec<&Vec3> = tops.iter().take_while(|q| from_axis(**q) <= STIPE_RADIUS + 1e-5).collect();
        if on_stipe.len() < 2 {
            continue;
        }
        decurrent += 1;
        for q in &on_stipe {
            assert!(from_axis(**q) > 0.5 * STIPE_RADIUS - 1e-5, "a gill top {} from the axis", from_axis(**q));
        }
        let (low, high) = (on_stipe[0].y, on_stipe[on_stipe.len() - 1].y);
        assert!((low - (junction - p.decurrent)).abs() < 0.001, "a gill reaches down to {low}");
        assert!(high > junction - 0.002, "a gill leaves the stipe at {high}");
    }
    assert_eq!(decurrent, 24, "every gill that leaves the stipe runs down it");
    // below where the gills under the cap hang, only the ones on the stipe are left
    let on_stipe: Vec<Vec3> = vertices(&plates(&cap(0.0), &pattern(24, 0.014), &p))
        .into_iter()
        .filter(|q| q.y < junction - 0.012 && from_axis(*q) < 0.015)
        .collect();
    assert!(on_stipe.len() > 100);
    for q in &on_stipe {
        let x = from_axis(*q);
        assert!(x > 0.5 * STIPE_RADIUS - 1e-4, "{x} from the axis, inside the stipe");
        assert!(x < STIPE_RADIUS + 0.006 + p.thickness, "{x} from the axis, off the stipe");
    }
    // lower and lower: how far they stand off the stipe, from the top down
    let stand_off = |y: f32| on_stipe.iter().filter(|q| (q.y - y).abs() < 0.0005).map(|q| from_axis(*q)).fold(0.0, f32::max) - STIPE_RADIUS;
    let (high, middle, low) = (stand_off(junction - 0.013), stand_off(junction - 0.016), stand_off(junction - 0.019));
    assert!(high > middle && middle > low, "{high}, {middle}, {low} off the stipe");
    assert!(high < 0.6 * 0.006 && low < 0.1 * 0.006);
}

#[test]
fn folds_that_run_down_a_short_stipe_stop_where_it_starts() {
    let p = Params { thickness: 0.0015, ..decurrent(0.03) };
    let lowest = |base: f32| {
        let folds = build(&with(rays(0.0), stipe_from(base)), &pattern(24, 0.014), &p, GillProfile::Fold);
        lowest_next_to_the_stipe(&folds.mesh)
    };
    // on a long stipe they run the whole length down, on one that starts 1 cm below the cap
    // they end where it does
    let junction = underside(STIPE_RADIUS);
    assert!((lowest(0.0) - (junction - 0.03)).abs() < 0.001, "the folds reach {}", lowest(0.0));
    let base = junction - 0.01;
    assert!(lowest(base) >= base - 0.0005, "the folds reach {} below a stipe that starts at {base}", lowest(base));
    assert!(lowest(base) < base + 0.002);
}

#[test]
fn the_edge_of_a_decurrent_gill_turns_the_corner_without_folding() {
    let p = Params { depth: even(0.004), ..decurrent(0.015) };
    let gills = run(&cap(0.0), &pattern(24, 0.014), &p);
    let (top, edge) = (points(&gills[0]), points(&gills[gills.len() / 24 - 1]));
    assert!(top.iter().any(|q| q.y < underside(STIPE_RADIUS) - 0.005), "the first gill runs down the stipe");
    for k in 1..top.len() {
        let (a, b) = (top[k] - top[k - 1], edge[k] - edge[k - 1]);
        assert!(a.dot(b) > 0.0, "the edge runs back at {k}: {a} on top, {b} at the edge");
    }
}

#[test]
fn the_cap_sits_where_its_rays_grow_on_the_stipe() {
    // the same cap a centimetre lower, on a stipe that goes on above it
    let lower = |p: &mut [f32]| p[1] -= 0.01;
    let mut low_rays = rays(0.0);
    for ray in &mut low_rays {
        let link = path_link(ray);
        set_path_link(ray, PathLink::child(link.creator, STIPE_ID, 0.8, link.index));
        wrap_path_mut(ray).points.chunks_exact_mut(4).for_each(lower);
    }
    let low = run(&with_stipe(low_rays), &pattern(24, 0.014), &params());
    let high = run(&cap(0.0), &pattern(24, 0.014), &params());
    assert_eq!(low.len(), high.len());
    for (a, b) in low.iter().zip(&high) {
        for (x, y) in points(a).iter().zip(points(b)) {
            assert!(x.distance(y - Vec3::Y * 0.01) < 1e-5, "{x} and {y}");
        }
    }
}

#[test]
fn gills_reach_the_margin_of_a_lopsided_cap_all_round() {
    // every ray stretched from the axis, 1.4 times as long on one side, 0.6 times on the other
    let stretch = |angle: f32| 1.0 + 0.4 * angle.cos();
    let rays: Vec<Vec<i32>> = rays(0.0)
        .into_iter()
        .map(|mut ray| {
            let path = wrap_path_mut(&mut ray);
            let n = path.points.len() / 4;
            let tip = Vec3::from_slice(&path.points[(n - 1) * 4..(n - 1) * 4 + 3]);
            let k = stretch((tip.z - HINGE.z).atan2(tip.x - HINGE.x));
            for i in 0..n {
                let (x, z) = (path.points[i * 4] - HINGE.x, path.points[i * 4 + 2] - HINGE.z);
                if x * tip.x + z * tip.z > 0.0 {
                    path.points[i * 4] = HINGE.x + x * k;
                    path.points[i * 4 + 2] = HINGE.z + z * k;
                }
            }
            ray
        })
        .collect();
    let gills = run(&with_stipe(rays), &pattern(24, 0.014), &Params { edge_wave: 0.0, ..params() });
    let reach = |angle: f32| {
        gills
            .iter()
            .flat_map(|g| points(g))
            .filter(|q| turn((q.z - HINGE.z).atan2(q.x - HINGE.x), angle).abs() < 0.2)
            .map(from_axis)
            .fold(0.0, f32::max)
    };
    assert!(reach(0.0) > 0.85 * 1.4 * RADIUS, "the long side's gills reach {}", reach(0.0));
    assert!(reach(PI) > 0.85 * 0.6 * RADIUS && reach(PI) < 0.6 * RADIUS + 1e-3, "the short side's gills reach {}", reach(PI));
    let closest = gills.iter().flat_map(|g| points(g)).map(from_axis).fold(f32::MAX, f32::min);
    assert!(closest >= STIPE_RADIUS + 0.001 - 1e-5);
}

#[test]
fn the_same_inputs_give_the_same_gills() {
    let p = pattern(24, 0.014);
    assert_eq!(run(&cap(0.0), &p, &params()), run(&cap(0.0), &p, &params()));
    assert_eq!(plates(&cap(0.0), &p, &params()), plates(&cap(0.0), &p, &params()));
    assert_ne!(run(&cap(0.0), &p, &params()), run(&cap(0.0), &p, &Params { seed: 3, ..params() }));
}

#[test]
fn nothing_without_a_pattern_or_a_stipe() {
    let empty = build(&cap(0.0), &Pattern { inner: 0.004, outer: 0.03, spacing: 0.001, rings: Vec::new() }, &params(), GillProfile::Plate);
    assert!(empty.paths.is_empty());
    assert_eq!(empty.mesh.face_count(), 0);
    // the stipe is found by the rays' parent: without it in the list there is no cap
    let mut other = rays(0.0);
    for p in other.iter_mut() {
        p[5] = STIPE_ID + 1;
    }
    for profile in [GillProfile::Plate, GillProfile::Fold] {
        assert!(build(&with_stipe(other.clone()), &pattern(24, 0.014), &params(), profile).paths.is_empty());
        assert!(build(&concat_arg_vecs(rays(0.0)), &pattern(24, 0.014), &decurrent(0.01), profile).paths.is_empty());
    }
}

#[test]
fn stripe_angles_find_the_middle_of_every_stripe() {
    let n = 360;
    let values: Vec<f32> = (0..n).map(|j| 0.5 + 0.5 * (8.0 * nodarium_utils::ring_angle(j, n)).cos()).collect();
    let angles = stripe_angles(&values);
    assert_eq!(angles.len(), 8);
    for k in 0..8 {
        let expected = TAU * k as f32 / 8.0;
        let closest = angles.iter().map(|a| turn(*a, expected).abs()).fold(f32::MAX, f32::min);
        assert!(closest < 0.01, "no stripe at {expected}: {angles:?}");
    }
    assert!(stripe_angles(&[1.0; 10]).is_empty());
    assert!(stripe_angles(&[0.0; 10]).is_empty());
}

/// `PATTERN=file.words OUT=file.ppm cargo test --release -p myzel_core gills::tests::trace -- --ignored`
/// draws the traced gills over a pattern written by the reaction_diffusion debug test:
/// gills that run on in blue, ones that start between others in red, forks in green
#[test]
#[ignore]
fn trace() {
    use std::io::Write;
    let words: Vec<i32> =
        std::fs::read_to_string(std::env::var("PATTERN").unwrap()).unwrap().split(' ').map(|w| w.parse().unwrap()).collect();
    let pattern = Pattern::decode(&words).unwrap();
    let gills = trace_gills(&pattern, 0, pattern.rings.len() - 1, LINK_REACH * pattern.spacing);
    let (free_starts, forks) = (
        gills.iter().filter(|g| g.free_start).count(),
        gills.iter().filter(|g| !g.free_start && g.points[0].1 > pattern.inner + 1e-6).count(),
    );
    println!("{} gills, {} start between others, {} fork, {} end between others", gills.len(), free_starts, forks, gills.iter().filter(|g| g.free_end).count());
    let size = 900usize;
    let scale = size as f32 / (2.0 * pattern.outer);
    let mut img = vec![[0u8; 3]; size * size];
    for y in 0..size {
        for x in 0..size {
            let (px, py) = ((x as f32 + 0.5) / scale - pattern.outer, (y as f32 + 0.5) / scale - pattern.outer);
            if let Some(v) = pattern.sample(py.atan2(px).rem_euclid(TAU), (px * px + py * py).sqrt()) {
                let g = (140.0 + 100.0 * v) as u8;
                img[y * size + x] = [g, g, g];
            }
        }
    }
    for gill in &gills {
        let color = if gill.free_start { [220, 30, 30] } else if gill.points[0].1 > pattern.inner + 1e-6 { [20, 160, 40] } else { [30, 60, 200] };
        for w in gill.points.windows(2) {
            for k in 0..20 {
                let t = k as f32 / 20.0;
                let (a, s) = (w[0].0 + turn(w[0].0, w[1].0) * t, w[0].1 + (w[1].1 - w[0].1) * t);
                let (x, y) = (((s * a.cos() + pattern.outer) * scale) as usize, ((s * a.sin() + pattern.outer) * scale) as usize);
                if x < size && y < size {
                    img[y * size + x] = color;
                }
            }
        }
    }
    let mut f = std::fs::File::create(std::env::var("OUT").unwrap()).unwrap();
    write!(f, "P6\n{size} {size}\n255\n").unwrap();
    f.write_all(&img.concat()).unwrap();
}
