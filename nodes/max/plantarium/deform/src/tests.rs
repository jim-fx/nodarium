use super::*;
use nodarium_utils::{
    concat_args, encode_float,
    geometry::{
        create_geometry_data, create_path, geometry_path_id, set_geometry_path_id, with_geometry_colors,
        wrap_geometry_data,
    },
    split_args,
};
use std::f32::consts::PI;

const BEND: i32 = 0;
const CURL: i32 = 1;
const TWIST: i32 = 2;

/// A flat strip facing +y, `columns` vertices across from -`half` to `half` and `rows` along from
/// its base at the origin to -`length`
fn strip(columns: usize, rows: usize, half: f32, length: f32) -> Vec<i32> {
    let faces = (columns - 1) * (rows - 1) * 2;
    let mut geometry = create_geometry_data(columns * rows, faces);
    let data = wrap_geometry_data(&mut geometry);
    for i in 0..rows {
        for j in 0..columns {
            let v = i * columns + j;
            let x = -half + 2.0 * half * j as f32 / (columns - 1) as f32;
            let z = -length * i as f32 / (rows - 1) as f32;
            data.positions[v * 3..v * 3 + 3].copy_from_slice(&[x, 0.0, z]);
            data.normals[v * 3..v * 3 + 3].copy_from_slice(&[0.0, 1.0, 0.0]);
        }
    }
    let mut f = 0;
    for i in 0..rows - 1 {
        for j in 0..columns - 1 {
            let a = (i * columns + j) as i32;
            let (right, next) = (a + 1, a + columns as i32);
            data.faces[f * 3..f * 3 + 6].copy_from_slice(&[a, right, next, right, next + 1, next]);
            f += 2;
        }
    }
    geometry
}

fn run(input: &[i32], mode: i32, amount: &[i32]) -> Vec<i32> {
    __nodarium_execute_inner(&[input, &[mode], amount])
}

fn vertices(geometry: &[i32]) -> (Vec<Vec3>, Vec<Vec3>) {
    let mut geometry = geometry.to_vec();
    let data = wrap_geometry_data(&mut geometry);
    let read = |s: &[f32]| s.chunks_exact(3).map(Vec3::from_slice).collect();
    (read(data.positions), read(data.normals))
}

fn plain(v: f32) -> Vec<i32> {
    vec![encode_float(v)]
}

fn alpha(field: i32) -> Vec<i32> {
    concat_args(vec![&[2], &[field]])
}

/// A curve of `value` from 0 to `top`, 0 up to `from` and `top` beyond `to`
fn ramp(value: &[i32], from: f32, to: f32, top: f32) -> Vec<i32> {
    let points = [0.0, 0.0, from, 0.0, to, 1.0, 1.0, 1.0].map(encode_float);
    let (bottom, top) = ([encode_float(0.0)], [encode_float(top)]);
    let mut parts: Vec<&[i32]> = vec![&[3], value, &bottom, &top];
    parts.extend(points.iter().map(std::slice::from_ref));
    concat_args(parts)
}

fn close(a: Vec3, b: Vec3, tolerance: f32) -> bool {
    a.distance(b) < tolerance
}

#[test]
fn no_amount_changes_nothing() {
    let input = strip(5, 9, 0.5, 1.0);
    for mode in [BEND, CURL, TWIST] {
        assert_eq!(run(&input, mode, &plain(0.0)), input);
    }
}

#[test]
fn a_quarter_bend_turns_the_tip_up_by_a_right_angle() {
    let input = strip(3, 41, 0.1, 1.0);
    let (positions, normals) = vertices(&run(&input, BEND, &plain(0.25)));
    // a quarter of a circle as long as the strip
    let radius = 2.0 / PI;
    let tip = 40 * 3 + 1;
    assert!(close(positions[tip], Vec3::new(0.0, radius, -radius), 1e-3), "{:?}", positions[tip]);
    // the normal of the last vertex is the one of the last row of faces, half a step back
    assert!(close(normals[tip], Vec3::Z, 3e-2), "{:?}", normals[tip]);
    assert!(close(positions[1], Vec3::ZERO, 1e-6));

    // the midrib keeps its length
    let midrib: f32 = (0..40).map(|i| positions[i * 3 + 1].distance(positions[(i + 1) * 3 + 1])).sum();
    assert!((midrib - 1.0).abs() < 1e-3, "{}", midrib);
    // and the width stays across
    for i in 0..41 {
        assert!((positions[i * 3].x + 0.1).abs() < 1e-6 && (positions[i * 3 + 2].x - 0.1).abs() < 1e-6);
    }
}

#[test]
fn a_bend_goes_past_a_right_angle_and_down_with_a_negative_amount() {
    let input = strip(3, 41, 0.1, 1.0);
    let (positions, _) = vertices(&run(&input, BEND, &plain(0.4)));
    let tip = positions[40 * 3 + 1];
    // past upright, the tip leans back towards the base
    assert!(tip.y > 0.3 && tip.z > -0.4, "{:?}", tip);

    let (positions, normals) = vertices(&run(&input, BEND, &plain(-0.25)));
    let radius = 2.0 / PI;
    assert!(close(positions[40 * 3 + 1], Vec3::new(0.0, -radius, -radius), 1e-3));
    assert!(close(normals[40 * 3 + 1], -Vec3::Z, 3e-2));
}

#[test]
fn a_curve_of_alpha_bends_only_where_it_is_more_than_0() {
    let input = strip(3, 41, 0.1, 1.0);
    let (before, _) = vertices(&input);
    let (after, _) = vertices(&run(&input, BEND, &ramp(&alpha(0), 0.5, 0.6, 1.0)));
    for (i, (a, b)) in before.iter().zip(&after).enumerate() {
        if a.z >= -0.5 {
            assert!(close(*a, *b, 1e-5), "{}: {:?} -> {:?}", i, a, b);
        }
    }
    assert!(after[40 * 3 + 1].y > 0.1);
}

#[test]
fn a_half_curl_rolls_both_halves_into_a_tube() {
    let input = strip(41, 5, 1.0, 1.0);
    let (positions, normals) = vertices(&run(&input, CURL, &plain(0.5)));
    // each half becomes half a circle, its edge meets the other one above the middle
    let radius = 1.0 / PI;
    for row in 0..5 {
        let z = -(row as f32) / 4.0;
        let (left, middle, right) = (positions[row * 41], positions[row * 41 + 20], positions[row * 41 + 40]);
        assert!(close(left, Vec3::new(0.0, 2.0 * radius, z), 2e-3), "{:?}", left);
        assert!(close(right, Vec3::new(0.0, 2.0 * radius, z), 2e-3), "{:?}", right);
        assert!(close(middle, Vec3::new(0.0, 0.0, z), 1e-6));
        // the quarter is at the side of the tube
        let quarter = positions[row * 41 + 30];
        assert!(close(quarter, Vec3::new(radius, radius, z), 2e-3), "{:?}", quarter);
        assert!(close(normals[row * 41 + 30], -Vec3::X, 3e-2), "{:?}", normals[row * 41 + 30]);
    }
}

#[test]
fn a_negative_curl_rolls_down() {
    let input = strip(41, 5, 1.0, 1.0);
    let (positions, _) = vertices(&run(&input, CURL, &plain(-0.25)));
    let edge = positions[40];
    assert!(edge.y < -0.3 && edge.x > 0.3, "{:?}", edge);
    assert!(positions[0].y < -0.3 && positions[0].x < -0.3);
}

#[test]
fn a_curve_of_across_rolls_only_the_margin() {
    let input = strip(41, 5, 1.0, 1.0);
    let (before, _) = vertices(&input);
    let (after, _) = vertices(&run(&input, CURL, &ramp(&alpha(1), 0.7, 0.8, 3.0)));
    for (a, b) in before.iter().zip(&after) {
        if a.x.abs() <= 0.7 {
            assert!(close(*a, *b, 1e-5), "{:?} -> {:?}", a, b);
        }
    }
    // the margin rolled most of a turn, back under itself
    let edge = after[40];
    assert!(edge.y > 0.05 && edge.x < 0.95, "{:?}", edge);
}

#[test]
fn a_curl_shaped_by_alpha_rolls_the_tip_more_than_the_base() {
    let input = strip(21, 9, 1.0, 1.0);
    let (after, _) = vertices(&run(&input, CURL, &ramp(&alpha(0), 0.0, 1.0, 0.25)));
    let edge = |row: usize| after[row * 21 + 20];
    assert!(edge(0).y.abs() < 1e-5);
    assert!(edge(4).y > 0.05 && edge(8).y > edge(4).y, "{:?} {:?}", edge(4), edge(8));
}

#[test]
fn a_quarter_twist_turns_the_tip_upright() {
    let input = strip(3, 41, 0.5, 1.0);
    let (positions, normals) = vertices(&run(&input, TWIST, &plain(0.25)));
    assert!(close(positions[0], Vec3::new(-0.5, 0.0, 0.0), 1e-6));
    let tip = 40 * 3;
    assert!(close(positions[tip + 2], Vec3::new(0.0, 0.5, -1.0), 1e-4), "{:?}", positions[tip + 2]);
    assert!(close(positions[tip], Vec3::new(0.0, -0.5, -1.0), 1e-4));
    // halfway it turned by half as much
    let half = 20 * 3 + 2;
    let eighth = Vec2::from_angle(PI / 4.0) * 0.5;
    assert!(close(positions[half], Vec3::new(eighth.x, eighth.y, -0.5), 1e-4), "{:?}", positions[half]);
    // the middle faces sideways at the tip, tilted towards the base by the faces next to it,
    // which lean with the helix
    let n = normals[tip + 1];
    assert!(n.x < -0.95 && n.y.abs() < 0.02 && n.z > 0.1, "{:?}", n);
    assert!(normals.iter().all(|n| (n.length() - 1.0).abs() < 1e-4));
}

#[test]
fn a_curve_of_alpha_twists_only_the_end() {
    let input = strip(3, 41, 0.5, 1.0);
    let (before, _) = vertices(&input);
    let (after, _) = vertices(&run(&input, TWIST, &ramp(&alpha(0), 0.5, 0.6, 1.0)));
    for (a, b) in before.iter().zip(&after) {
        if a.z >= -0.5 {
            assert!(close(*a, *b, 1e-5), "{:?} -> {:?}", a, b);
        }
    }
    assert!(after[40 * 3 + 2].y.abs() > 0.1, "{:?}", after[40 * 3 + 2]);
}

#[test]
fn colors_faces_and_the_path_id_are_kept() {
    let mut input = strip(5, 9, 0.5, 1.0);
    set_geometry_path_id(&mut input, 1234);
    let colors: Vec<u32> = (0..45).map(|i| 0xff00_0000 | i).collect();
    let input = with_geometry_colors(&input, &colors);
    for mode in [BEND, CURL, TWIST] {
        let mut out = run(&input, mode, &plain(0.3));
        assert_eq!(out.len(), input.len());
        assert_eq!(geometry_path_id(&out), 1234);
        let mut before = input.clone();
        let (a, b) = (wrap_geometry_data(&mut before), wrap_geometry_data(&mut out));
        assert_eq!(a.colors, b.colors);
        assert_eq!(a.faces, b.faces);
        assert_ne!(a.positions, b.positions);
    }
}

#[test]
fn every_geometry_of_a_list_is_deformed_and_paths_pass() {
    let (a, b) = (strip(3, 5, 0.2, 1.0), strip(3, 9, 0.1, 0.5));
    let path = create_path(4, 1);
    let input = concat_args(vec![&a, &path, &b]);
    let out = run(&input, BEND, &plain(0.25));
    let items = split_args(&out);
    assert_eq!(items.len(), 3);
    assert_eq!(items[0], run(&a, BEND, &plain(0.25)).as_slice());
    assert_eq!(items[1], path.as_slice());
    assert_eq!(items[2], run(&b, BEND, &plain(0.25)).as_slice());

    // the shorter one bends as far, it is a quarter circle as well
    let (positions, _) = vertices(items[2]);
    let radius = 0.5 * 2.0 / PI;
    assert!(close(positions[8 * 3 + 1], Vec3::new(0.0, radius, -radius), 1e-3));
}

#[test]
fn a_geometry_that_does_not_reach_along_the_axis_stays() {
    let mut input = strip(3, 3, 0.5, 1.0);
    let data = wrap_geometry_data(&mut input);
    for p in data.positions.chunks_exact_mut(3) {
        p[2] = -p[2];
    }
    for mode in [BEND, CURL, TWIST] {
        assert_eq!(run(&input, mode, &plain(0.5)), input);
    }
}

#[test]
fn a_curl_keeps_the_height_of_a_bent_geometry() {
    let flat = strip(21, 9, 1.0, 1.0);
    let mut bent = flat.clone();
    let data = wrap_geometry_data(&mut bent);
    for p in data.positions.chunks_exact_mut(3) {
        p[1] = -0.6 * p[2] * p[2];
    }
    let (heights, _) = vertices(&bent);
    let (rolled, _) = vertices(&run(&flat, CURL, &plain(0.5)));
    let (both, _) = vertices(&run(&bent, CURL, &plain(0.5)));
    for ((r, b), h) in rolled.iter().zip(&both).zip(&heights) {
        assert!(close(*b, *r + Vec3::Y * h.y, 1e-5), "{:?} vs {:?}", b, r);
    }
}

#[test]
fn nothing_connected_gives_nothing() {
    assert_eq!(run(&[0, 1, 1, 1], BEND, &plain(0.5)), vec![0, 1, 1, 1]);
}
