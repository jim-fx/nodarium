use super::__nodarium_execute_inner;
use glam::Vec3;
use nodarium_utils::{
    concat_arg_vecs, concat_args, encode_float,
    geometry::{create_path, set_path_link, wrap_path_mut, PathLink},
};
use std::time::Instant;

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

/// A midrib along +z with `pairs` pairs of side veins, like a beech leaf, and `tertiary` veins on
/// every side vein.
fn pinnate(pairs: usize, tertiary: usize) -> Vec<i32> {
    let midrib = line(Vec3::ZERO, Vec3::Z, |t| 0.05 * (1.0 - t), 24);
    let root = PathLink::root(1, 0);
    let mut paths = vec![path(&midrib, root, 1)];
    for i in 0..pairs * 2 {
        let a = 0.08 + 0.84 * (i / 2) as f32 / (pairs - 1) as f32;
        let side = if i % 2 == 0 { 1.0 } else { -1.0 };
        let length = 0.12 + 0.3 * (a * std::f32::consts::PI).sin();
        let base = Vec3::Z * a;
        let direction = Vec3::new(side * 0.64, 0.0, 0.77);
        let link = PathLink::child(2, root.id, a, i as i32);
        let vein = line(base, base + direction * length, |t| 0.06 * (1.0 - t), 12);
        for j in 0..tertiary {
            let b = (j + 1) as f32 / (tertiary + 1) as f32;
            let start = base + direction * length * b;
            let out = Vec3::new(side * 0.95, 0.0, 0.3).normalize() * 0.04;
            let child = PathLink::child(3, link.id, b, j as i32);
            paths.push(path(&line(start, start + out, |t| 0.01 * (1.0 - t), 4), child, 3));
        }
        paths.push(path(&vein, link, 2));
    }
    concat_arg_vecs(paths)
}

fn run(veins: &[i32], method: i32, resolution: i32, margin: &[i32]) -> Vec<i32> {
    run_with_relief(veins, method, resolution, margin, 0.003)
}

fn run_with_relief(veins: &[i32], method: i32, resolution: i32, margin: &[i32], relief: f32) -> Vec<i32> {
    __nodarium_execute_inner(&[
        veins,
        &[encode_float(1.0)],
        &[encode_float(0.05)],
        margin,
        &[encode_float(relief)],
        &[method],
        &[resolution],
    ])
}

fn counts(geometry: &[i32]) -> (usize, usize) {
    (geometry[3] as usize, geometry[4] as usize)
}

fn teeth() -> Vec<i32> {
    let alpha = concat_args(vec![&[2], &[0]]);
    let wave = concat_args(vec![&[8], &alpha, &[encode_float(14.0)], &[2]]);
    concat_args(vec![&[0], &[2], &wave, &[encode_float(0.015)]])
}

#[test]
fn both_methods_make_a_blade_over_the_veins() {
    let veins = pinnate(8, 0);
    for method in [0, 1] {
        let geometry = run(&veins, method, 64, &[encode_float(0.0)]);
        let (v, f) = counts(&geometry);
        assert!(v > 100 && f > 100, "method {method}: {v} vertices, {f} faces");
    }
}

#[test]
fn timings() {
    let cases = [("pinnate", pinnate(8, 0)), ("pinnate + tertiary", pinnate(8, 4))];
    for (name, veins) in &cases {
        for method in [0, 1] {
            for resolution in [64, 96, 128] {
                for (margin_name, margin, relief) in [
                    ("flat", vec![encode_float(0.0)], 0.0),
                    ("entire", vec![encode_float(0.0)], 0.003),
                    ("serrate", teeth(), 0.003),
                ] {
                    let runs = 5;
                    let start = Instant::now();
                    let mut geometry = vec![];
                    for _ in 0..runs {
                        geometry = run_with_relief(veins, method, resolution, &margin, relief);
                    }
                    let ms = start.elapsed().as_secs_f64() * 1000.0 / runs as f64;
                    let (v, f) = counts(&geometry);
                    println!(
                        "{name:20} method {method} res {resolution:3} {margin_name:8}: {ms:7.2} ms, {v:6} vertices, {f:6} faces"
                    );
                }
            }
        }
    }
}
