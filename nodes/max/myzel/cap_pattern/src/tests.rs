use super::*;
use myzel_core::{
    closing_turn, curl_spread, encode_color, gills, paths_mesh, pose_caps, read_paths, testing, GillInputs, GillProfile,
    HyphaeMeshParams, PathStyle,
};
use nodarium_utils::{
    concat_args, encode_float,
    geometry::{path_link, wrap_path_mut},
    ring_angle,
};
use std::f32::consts::PI;

const PALE: [u8; 3] = [245, 238, 222];

/// A pattern on a disc 4.5 cm across, `value` of the angle and the distance from the axis
fn disc(value: impl Fn(f32, f32) -> f32) -> Vec<i32> {
    let (outer, rings) = (0.045, 91);
    let rings = (0..rings)
        .map(|i| {
            let r = outer * i as f32 / (rings - 1) as f32;
            let n = ((TAU * r / 0.0005) as usize).max(3);
            (0..n).map(|j| value(ring_angle(j, n), r)).collect()
        })
        .collect();
    Pattern { inner: 0.0, outer, spacing: 0.002, rings }.encode()
}

/// The test cap on its stipe, the stipe of the mesh ends below the top of the cap
fn cap() -> Vec<Vec<i32>> {
    testing::grow(&testing::cone())
}

/// The body of the cap and its stipe, groups 0 (stipe) and 1 (rays)
fn body(paths: &[Vec<i32>]) -> Mesh {
    let mut paths = paths.to_vec();
    for path in paths.iter_mut().filter(|p| !path_link(p).has_parent()) {
        let path = wrap_path_mut(path);
        for point in path.points.chunks_exact_mut(4) {
            point[1] *= 0.88;
        }
    }
    let list = testing::list(&paths);
    let meshed = read_paths(&list, PathStyle::BODY);
    paths_mesh(&meshed, &HyphaeMeshParams { fibres: 0.0, resolution: 96, ..HyphaeMeshParams::default() })
}

fn stripes() -> Vec<i32> {
    disc(|angle, _| 0.5 + 0.5 * (40.0 * angle).cos())
}

/// The body with gills under the cap, group 2
fn body_and_gills(paths: &[Vec<i32>]) -> Mesh {
    let list = testing::list(paths);
    let (pattern, depth, free, none, thickness) =
        (stripes(), [encode_float(0.003)], [encode_float(0.001)], [encode_float(0.0)], [encode_float(0.0003)]);
    let inputs = GillInputs {
        cap: &list,
        pattern: &pattern,
        depth: &depth,
        free_distance: Some(&free),
        decurrent: &none,
        thickness: &thickness,
        edge_wave: &none,
        seed: 1,
        creator: 20,
    };
    Mesh::join(&[body(paths), gills(&inputs, GillProfile::Plate).mesh])
}

fn run(mesh: &Mesh, cap: &[Vec<i32>], pattern: &[i32], relief: &[i32], opacity: &[i32]) -> Vec<i32> {
    __nodarium_execute_inner(&[&mesh.encode(), &testing::list(cap), pattern, relief, &encode_color(PALE), opacity])
}

fn value(v: f32) -> Vec<i32> {
    vec![encode_float(v)]
}

fn changed(before: &Mesh, after: &Mesh, v: usize) -> bool {
    let color = |m: &Mesh| m.colors.get(v).copied().unwrap_or(NO_COLOR);
    before.positions[v * 3..v * 3 + 3] != after.positions[v * 3..v * 3 + 3] || color(before) != color(after)
}

#[test]
fn at_zero_the_mesh_stays_as_it_is() {
    let cap = cap();
    let mesh = body(&cap);
    let words = mesh.encode();
    assert_eq!(run(&mesh, &cap, &disc(|_, _| 1.0), &value(0.0), &value(0.0)), words);
    // a pattern without features, or nothing connected
    assert_eq!(run(&mesh, &cap, &disc(|_, _| 0.0), &value(0.001), &value(1.0)), words);
    assert_eq!(run(&mesh, &cap, &[0, 1, 1, 1], &value(0.001), &value(1.0)), words);
    assert_eq!(run(&mesh, &[], &disc(|_, _| 1.0), &value(0.001), &value(1.0)), words);
    let empty = __nodarium_execute_inner(&[&[0, 1, 1, 1], &[0, 1, 1, 1], &[0, 1, 1, 1], &value(0.001), &encode_color(PALE), &value(1.0)]);
    assert_eq!(Mesh::decode(&empty), Mesh::default());
}

#[test]
fn relief_and_color_go_alone() {
    let cap = cap();
    let mesh = body(&cap);
    let full = disc(|_, _| 1.0);
    let relief = Mesh::decode(&run(&mesh, &cap, &full, &value(0.001), &value(0.0)));
    assert!(relief.colors.is_empty());
    assert_ne!(relief.positions, mesh.positions);
    let color = Mesh::decode(&run(&mesh, &cap, &full, &value(0.0), &value(1.0)));
    // the faces on the top are split, the vertices that were there stay where they were
    let n = mesh.positions.len();
    assert!(color.vertex_count() > mesh.vertex_count());
    assert_eq!((&color.positions[..n], &color.normals[..n]), (&mesh.positions[..], &mesh.normals[..]));
    assert!(color.colors.contains(&opaque(PALE)));
}

#[test]
fn the_pattern_lands_on_the_top_of_the_cap_only() {
    let cap = cap();
    let mesh = body_and_gills(&cap);
    assert_eq!(mesh.groups.iter().max(), Some(&2));
    let after = Mesh::decode(&run(&mesh, &cap, &disc(|_, _| 1.0), &value(0.0005), &value(1.0)));
    let (mut top, mut top_changed) = (0, 0);
    for v in 0..mesh.vertex_count() {
        let group = mesh.groups[v];
        if group != 1 {
            assert!(!changed(&mesh, &after, v), "a vertex of group {group} changed");
            assert_eq!(after.colors[v], NO_COLOR);
            continue;
        }
        let up = mesh.normals[v * 3 + 1];
        if up < -0.5 {
            assert!(!changed(&mesh, &after, v), "the underside changed");
        } else if up > 0.5 {
            top += 1;
            top_changed += changed(&mesh, &after, v) as usize;
        }
    }
    assert!(top > 100 && top_changed as f32 > 0.95 * top as f32, "{top_changed} of {top} vertices on the top changed");
    // raised along the normal
    let raised = (0..mesh.vertex_count()).filter(|&v| after.positions[v * 3 + 1] > mesh.positions[v * 3 + 1]).count();
    assert!(raised as f32 > 0.9 * top as f32);
}

/// A spot of the pattern at `angle` and `s` from the axis along the cap, 1 cm across
fn spot(angle: f32, s: f32) -> Vec<i32> {
    disc(move |a, r| {
        let d = Vec2::new(r * a.cos(), r * a.sin()).distance(Vec2::new(s * angle.cos(), s * angle.sin()));
        if d < 0.005 {
            1.0
        } else {
            0.0
        }
    })
}

/// Where the middle of the flesh of the cap is at `angle` and `s` along the rays from the axis
fn on_cap(paths: &[Vec<i32>], angle: f32, s: f32) -> Vec3 {
    let cap = &testing::caps(paths)[0];
    let ray = cap
        .rays
        .iter()
        .filter(|r| r.crosses)
        .min_by(|a, b| {
            let turn = |r: &&myzel_core::CapRay| (r.angle - angle + std::f32::consts::PI).rem_euclid(TAU) - std::f32::consts::PI;
            turn(a).abs().total_cmp(&turn(b).abs())
        })
        .unwrap();
    let from_axis: Vec<f32> = ray.to_end.iter().map(|d| ray.length - d).collect();
    let k = from_axis.partition_point(|&x| x < s).clamp(1, from_axis.len() - 1);
    let t = (s - from_axis[k - 1]) / (from_axis[k] - from_axis[k - 1]);
    let p = ray.plane[k - 1].lerp(ray.plane[k], t);
    let f = cap.frame;
    f.hinge + f.axis * p.y + f.out(ray.angle) * p.x
}

/// The middle of the vertices the pattern colors
fn painted(mesh: &Mesh) -> Vec3 {
    let vertices: Vec<usize> = (0..mesh.vertex_count()).filter(|&v| mesh.colors[v] >> 24 > 128).collect();
    assert!(vertices.len() > 5);
    vertices.iter().map(|&v| Vec3::from_slice(&mesh.positions[v * 3..v * 3 + 3])).sum::<Vec3>() / vertices.len() as f32
}

#[test]
fn the_pattern_follows_the_cap_as_it_opens() {
    let open = cap();
    let refs: Vec<&[i32]> = open.iter().map(|p| p.as_slice()).collect();
    let closed = pose_caps(&refs, |cap, k| Some(cap.bend(k, |step| closing_turn(step, 1.2))));
    let (angle, s) = (testing::frame(&open).angle(Vec3::new(0.0, 0.0, 1.0)), 0.025);
    let mut places = Vec::new();
    for paths in [&open, &closed] {
        let mesh = body(paths);
        let after = Mesh::decode(&run(&mesh, paths, &spot(angle, s), &value(0.0), &value(1.0)));
        let (found, expected) = (painted(&after), on_cap(paths, angle, s));
        // the spot sits on the top, its middle is half the flesh thickness above the rays
        assert!(found.distance(expected) < 0.003, "the spot is {} from where it belongs", found.distance(expected));
        places.push(found);
    }
    assert!(places[0].distance(places[1]) > 0.006, "the spot moved {} with the cap", places[0].distance(places[1]));
}

#[test]
fn the_top_of_a_closed_cap_is_covered_between_the_rays() {
    // a flat cap closed to a button with its margin rolled in: the rays fill_gaps adds start
    // near the margin, which comes close to the axis under the cap, but they don't reach in to
    // the top
    let flat = testing::TestCap { profile: vec![0.0, 32.0, 25.0, 24.0, 50.0, 22.0], radius: 0.025, thickness: 0.016, ..testing::cone() };
    let open = testing::grow(&flat);
    let refs: Vec<&[i32]> = open.iter().map(|p| p.as_slice()).collect();
    let closed = pose_caps(&refs, |cap, k| Some(cap.bend(k, |step| closing_turn(step, 1.2) + 2.5 * curl_spread(step.alpha()))));
    assert!(testing::caps(&closed)[0].rays.iter().any(|r| !r.crosses));
    let mesh = body(&closed);
    let after = Mesh::decode(&run(&mesh, &closed, &disc(|_, _| 1.0), &value(0.0), &value(1.0)));
    let frame = testing::frame(&closed);
    let near_axis: Vec<usize> = (0..mesh.vertex_count())
        .filter(|&v| mesh.groups[v] == 1 && mesh.normals[v * 3 + 1] > 0.8)
        .filter(|&v| {
            let p = Vec3::from_slice(&mesh.positions[v * 3..v * 3 + 3]) - frame.hinge;
            p.dot(frame.axis) > 0.0 && p.reject_from(frame.axis).length() < 0.008
        })
        .collect();
    let covered = near_axis.iter().filter(|&&v| after.colors[v] >> 24 > 200).count();
    assert!(near_axis.len() > 20 && covered as f32 > 0.95 * near_axis.len() as f32, "{covered} of {} covered", near_axis.len());
}

#[test]
fn relief_and_opacity_are_read_along_the_rays() {
    let cap = cap();
    let mesh = body(&cap);
    // alpha: nothing at the axis, everything at the margin
    let alpha = concat_args(vec![&[2], &[0]]);
    let after = Mesh::decode(&run(&mesh, &cap, &disc(|_, _| 1.0), &value(0.0), &alpha));
    let frame = testing::frame(&cap);
    let cover = |near_axis: bool| {
        let vertices: Vec<usize> = (0..mesh.vertex_count())
            .filter(|&v| after.colors[v] != NO_COLOR || mesh.normals[v * 3 + 1] > 0.5)
            .filter(|&v| {
                let p = Vec3::from_slice(&mesh.positions[v * 3..v * 3 + 3]) - frame.hinge;
                let out = p.reject_from(frame.axis).length();
                if near_axis {
                    out < 0.006
                } else {
                    (0.02..0.026).contains(&out)
                }
            })
            .collect();
        vertices.iter().map(|&v| (after.colors[v] >> 24) as f32 / 255.0).sum::<f32>() / vertices.len() as f32
    };
    assert!(cover(true) < 0.25 && cover(false) > 0.6, "covers {} at the axis, {} near the margin", cover(true), cover(false));
}

#[test]
fn the_color_goes_over_the_color_of_the_cap() {
    let brown = [120, 80, 50];
    assert_eq!(over(opaque(brown), linear_rgba(opaque(PALE)), 1.0), opaque(PALE));
    assert_eq!(over(opaque(brown), linear_rgba(opaque(PALE)), 0.0), opaque(brown));
    // over a vertex without a color it shows as much as it covers
    assert_eq!(over(NO_COLOR, linear_rgba(opaque(PALE)), 0.5) >> 24, 128);
}

/// The length of the edges no face walks the other way: along a hole, or where a face splits an
/// edge its neighbour doesn't
fn open_edges(mesh: &Mesh) -> f32 {
    let mut edges = HashMap::new();
    for t in mesh.faces.chunks_exact(3) {
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            *edges.entry((a, b)).or_insert(0) += 1;
        }
    }
    let at = |v: u32| Vec3::from_slice(&mesh.positions[v as usize * 3..v as usize * 3 + 3]);
    edges.keys().filter(|&&(a, b)| !edges.contains_key(&(b, a))).map(|&(a, b)| at(a).distance(at(b))).sum()
}

#[test]
fn the_top_gets_finer_faces_and_no_holes() {
    let cap = cap();
    let mesh = body(&cap);
    let after = Mesh::decode(&run(&mesh, &cap, &stripes(), &value(0.0), &value(1.0)));
    assert!(after.face_count() > 2 * mesh.face_count(), "{} faces from {}", after.face_count(), mesh.face_count());
    let (before, now) = (open_edges(&mesh), open_edges(&after));
    assert!((now - before).abs() < 1e-5, "open edges {now} m long, {before} m before");
    // the new vertices are on the top of the cap, in its group
    assert!(after.groups[mesh.vertex_count()..].iter().all(|&g| g == 1));
}

/// The stipe of `body`, alone
fn bare_stipe(paths: &[Vec<i32>]) -> Vec<Vec<i32>> {
    let mut stipe = paths.iter().find(|p| !path_link(p).has_parent()).unwrap().clone();
    for point in wrap_path_mut(&mut stipe).points.chunks_exact_mut(4) {
        point[1] *= 0.88;
    }
    vec![stipe]
}

fn side_of_stipe(mesh: &Mesh, v: usize) -> bool {
    mesh.groups[v] == 0 && mesh.normals[v * 3 + 1].abs() < 0.3 && (0.004..0.04).contains(&mesh.positions[v * 3 + 1])
}

#[test]
fn a_stipe_on_its_own_gets_the_pattern_around_its_side() {
    let cap = cap();
    let mesh = body(&cap);
    let after = Mesh::decode(&run(&mesh, &bare_stipe(&cap), &disc(|_, _| 1.0), &value(0.0003), &value(1.0)));
    let (mut side, mut side_changed) = (0, 0);
    for v in 0..mesh.vertex_count() {
        if mesh.groups[v] != 0 {
            assert!(!changed(&mesh, &after, v), "a vertex of the cap changed");
            continue;
        }
        if side_of_stipe(&mesh, v) {
            side += 1;
            side_changed += changed(&mesh, &after, v) as usize;
            // raised out of the stipe
            let out = |m: &Mesh| Vec2::new(m.positions[v * 3], m.positions[v * 3 + 2]).length();
            assert!(out(&after) >= out(&mesh) - 1e-6);
        }
    }
    assert!(side > 100 && side_changed as f32 > 0.95 * side as f32, "{side_changed} of {side} vertices on the side changed");
    // faces of the cap next to the stipe are split with it, but stay as they were
    for v in mesh.vertex_count()..after.vertex_count() {
        assert!(after.groups[v] == 0 || after.colors[v] == NO_COLOR, "a new vertex of the cap is painted");
    }
}

#[test]
fn around_a_stipe_the_pattern_lies_like_a_cloth_over_its_tip() {
    let cap = cap();
    let mesh = body(&cap);
    let stipe = bare_stipe(&cap);
    // the tip is 4.4 cm up and 5 mm thick: 2 cm from the middle of the pattern are 1.5 cm down
    // the side, and one half of the disc is on one side of the stipe
    let pattern = disc(|angle, s| if s < 0.02 && angle < PI { 1.0 } else { 0.0 });
    let after = Mesh::decode(&run(&mesh, &stipe, &pattern, &value(0.0), &value(1.0)));
    let frame_side = orthogonal_to(Vec3::Y);
    let around = Vec3::Y.cross(frame_side);
    let (mut high, mut low) = (Vec::new(), Vec::new());
    for v in (0..mesh.vertex_count()).filter(|&v| side_of_stipe(&mesh, v)) {
        let p = Vec3::from_slice(&mesh.positions[v * 3..v * 3 + 3]);
        let angle = p.dot(around).atan2(p.dot(frame_side)).rem_euclid(TAU);
        let painted = after.colors[v] != NO_COLOR;
        if p.y < 0.027 || (0.3..PI - 0.3).contains(&(angle - PI)) {
            assert!(!painted, "painted at {p}, angle {angle}");
        } else if p.y > 0.031 && (0.3..PI - 0.3).contains(&angle) {
            high.push(painted);
        }
        if p.y < 0.027 {
            low.push(painted);
        }
    }
    assert!(high.len() > 20 && high.iter().filter(|&&p| p).count() as f32 > 0.9 * high.len() as f32);
    assert!(!low.is_empty());
}

#[test]
fn around_a_stipe_relief_and_opacity_are_read_along_it() {
    let cap = cap();
    let mesh = body(&cap);
    // alpha: nothing at the base, everything at the tip
    let alpha = concat_args(vec![&[2], &[0]]);
    let after = Mesh::decode(&run(&mesh, &bare_stipe(&cap), &disc(|_, _| 1.0), &value(0.0), &alpha));
    let cover = |from: f32, to: f32| {
        let vertices: Vec<usize> =
            (0..mesh.vertex_count()).filter(|&v| side_of_stipe(&mesh, v) && (from..to).contains(&mesh.positions[v * 3 + 1])).collect();
        vertices.iter().map(|&v| (after.colors[v] >> 24) as f32 / 255.0).sum::<f32>() / vertices.len() as f32
    };
    let (low, high) = (cover(0.003, 0.01), cover(0.03, 0.038));
    assert!(low < 0.25 && high > 0.6, "covers {low} near the base, {high} near the tip");
}

#[test]
fn the_inputs_are_few() {
    let def: serde_json::Value = serde_json::from_str(include_str!("definition.json")).unwrap();
    let inputs = def["inputs"].as_object().unwrap();
    assert!(inputs.len() <= 8);
    let mut keys: Vec<&String> = inputs.keys().collect();
    keys.sort();
    assert_eq!(keys, ["cap", "color", "mesh", "opacity", "pattern", "relief"]);
}

