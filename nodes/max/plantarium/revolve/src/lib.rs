use glam::Vec3;
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{
    decode_float,
    geometry::{create_geometry_data, wrap_geometry_data},
    int, split_args, Ctx,
};
use std::f32::consts::TAU;

nodarium_definition_file!("src/definition.json");

/// The leaf of `leaf` is 100 long at size 1, this body too
const LENGTH: f32 = 100.0;
/// A ring narrower than this is a single point, so a closed end has no triangles without area
const POLE: f32 = 1e-4;

/// The outline as (radius, z): the shape is one half of a leaf, x from the edge to the midrib at
/// 50 and y from the tip (0) to the base (100), placed along -z like the leaf
fn outline(shape: &[i32]) -> Vec<(f32, f32)> {
    let count = shape.len().saturating_sub(4) / 2;
    (0..count)
        .map(|i| {
            let x = decode_float(shape[2 + i * 2]);
            let y = decode_float(shape[2 + i * 2 + 1]);
            ((50.0 - x).abs(), y - LENGTH)
        })
        .collect()
}

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let mut ctx = Ctx::default();
    // a connected shape node wraps the outline in another bracket, the default value does not
    let shape = match split_args(args[0]).first() {
        Some(inner) if inner.len() > 1 => *inner,
        _ => args[0],
    };
    let profile = outline(shape);
    assert!(profile.len() >= 2, "revolve needs a shape of at least two points, got {}", profile.len());
    let segments = int(args[2]).max(3) as usize;
    let last = profile.len() - 1;
    let sizes = ctx.float_along(args[1], (0..=last).map(|i| i as f32 / last as f32));

    // every ring is `segments` vertices, a ring at a closed end only its centre
    let mut starts = Vec::with_capacity(profile.len());
    let mut positions: Vec<Vec3> = Vec::new();
    for (i, &(radius, z)) in profile.iter().enumerate() {
        starts.push(positions.len());
        let s = sizes[i];
        let pole = radius < POLE && (i == 0 || i == last);
        if pole {
            positions.push(Vec3::new(0.0, 0.0, z * s));
            continue;
        }
        for j in 0..segments {
            let (sin, cos) = (TAU * j as f32 / segments as f32).sin_cos();
            positions.push(Vec3::new(radius * cos, radius * sin, z) * s);
        }
    }
    starts.push(positions.len());
    let rings: Vec<Vec<usize>> = starts.windows(2).map(|w| (w[0]..w[1]).collect()).collect();
    let ring = |i: usize| -> &Vec<usize> { &rings[i] };

    let mut faces: Vec<[usize; 3]> = Vec::new();
    for i in 0..last {
        let (a, b) = (ring(i), ring(i + 1));
        for j in 0..segments {
            let k = (j + 1) % segments;
            match (a.len(), b.len()) {
                (1, 1) => {}
                (1, _) => faces.push([a[0], b[j], b[k]]),
                (_, 1) => faces.push([a[j], b[0], a[k]]),
                _ => {
                    faces.push([a[j], b[j], a[k]]);
                    faces.push([a[k], b[j], b[k]]);
                }
            }
        }
    }
    // an end that is not a point is closed by a disc around its centre
    for (i, cap_from_inside) in [(0, true), (last, false)] {
        let rim = ring(i);
        if rim.len() == 1 {
            continue;
        }
        let centre = positions.len();
        positions.push(Vec3::new(0.0, 0.0, profile[i].1 * sizes[i]));
        for j in 0..segments {
            let k = (j + 1) % segments;
            faces.push(if cap_from_inside {
                [centre, rim[j], rim[k]]
            } else {
                [centre, rim[k], rim[j]]
            });
        }
    }

    let mut normals = vec![Vec3::ZERO; positions.len()];
    for face in &faces {
        let [a, b, c] = face.map(|v| positions[v]);
        let normal = (b - a).cross(c - a);
        for v in face {
            normals[*v] += normal;
        }
    }

    let mut out = create_geometry_data(positions.len(), faces.len());
    let geometry = wrap_geometry_data(&mut out);
    for (v, p) in positions.iter().enumerate() {
        geometry.positions[v * 3..v * 3 + 3].copy_from_slice(&p.to_array());
        let n = normals[v].normalize_or_zero();
        geometry.normals[v * 3..v * 3 + 3].copy_from_slice(&n.to_array());
    }
    for (f, face) in faces.iter().enumerate() {
        geometry.faces[f * 3..f * 3 + 3].copy_from_slice(&face.map(|v| v as i32));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodarium_utils::{encode_float, wrap_arg};

    fn shape(points: &[(f32, f32)]) -> Vec<i32> {
        let words: Vec<i32> = points.iter().flat_map(|&(x, y)| [encode_float(x), encode_float(y)]).collect();
        wrap_arg(&words)
    }

    fn run(points: &[(f32, f32)], size: f32, segments: i32) -> (Vec<Vec3>, Vec<Vec3>, Vec<[usize; 3]>) {
        let mut out = __nodarium_execute_inner(&[&shape(points), &[encode_float(size)], &[segments]]);
        let geometry = wrap_geometry_data(&mut out);
        let vec3s = |floats: &[f32]| floats.chunks_exact(3).map(Vec3::from_slice).collect::<Vec<_>>();
        let faces = geometry.faces.chunks_exact(3).map(|f| [f[0] as usize, f[1] as usize, f[2] as usize]).collect();
        (vec3s(geometry.positions), vec3s(geometry.normals), faces)
    }

    /// Every edge of a closed surface belongs to two faces, once in each direction
    fn is_closed(faces: &[[usize; 3]]) -> bool {
        let mut edges = std::collections::HashMap::new();
        for f in faces {
            for e in 0..3 {
                *edges.entry((f[e], f[(e + 1) % 3])).or_insert(0) += 1;
            }
        }
        edges.iter().all(|(&(a, b), &n)| n == 1 && edges.get(&(b, a)) == Some(&1))
    }

    fn centre(positions: &[Vec3]) -> Vec3 {
        positions.iter().copied().sum::<Vec3>() / positions.len() as f32
    }

    #[test]
    fn a_pointed_body_is_closed_and_faces_out() {
        let (positions, normals, faces) = run(&[(50.0, 100.0), (20.0, 50.0), (50.0, 0.0)], 1.0, 8);
        // two poles and one ring
        assert_eq!(positions.len(), 10);
        assert_eq!(faces.len(), 16);
        assert!(is_closed(&faces));
        let middle = centre(&positions);
        for face in &faces {
            let [a, b, c] = face.map(|v| positions[v]);
            let out = (b - a).cross(c - a);
            assert!(out.dot((a + b + c) / 3.0 - middle) > 0.0, "a face looks inward");
        }
        for (p, n) in positions.iter().zip(&normals) {
            assert!(n.dot(*p - middle) > 0.0);
        }
    }

    #[test]
    fn it_grows_along_minus_z_from_the_origin_and_is_as_wide_as_the_leaf() {
        let (positions, _, _) = run(&[(50.0, 100.0), (10.0, 60.0), (50.0, 0.0)], 0.01, 16);
        let (min, max) = positions.iter().fold((Vec3::MAX, Vec3::MIN), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
        assert!((max.z - 0.0).abs() < 1e-6);
        assert!((min.z + 1.0).abs() < 1e-5);
        // the leaf is 50 - x wide on each side of its midrib
        assert!((max.x - 0.4).abs() < 1e-5 && (min.x + 0.4).abs() < 1e-5);
        assert!((max.y - min.y - 0.8).abs() < 0.01);
    }

    #[test]
    fn open_ends_get_a_disc() {
        let (positions, _, faces) = run(&[(20.0, 100.0), (20.0, 0.0)], 1.0, 6);
        // two rings and two centres
        assert_eq!(positions.len(), 14);
        assert_eq!(faces.len(), 12 + 12);
        assert!(is_closed(&faces));
    }

    #[test]
    fn the_size_is_read_along_the_body() {
        let points = [(50.0, 100.0), (0.0, 50.0), (0.0, 0.0)];
        let ramp = nodarium_utils::concat_args(vec![&[3], &nodarium_utils::concat_args(vec![&[2], &[0]]), &[encode_float(1.0)], &[encode_float(2.0)], &[encode_float(0.0)], &[encode_float(0.0)], &[encode_float(1.0)], &[encode_float(1.0)]]);
        let mut out = __nodarium_execute_inner(&[&shape(&points), &ramp, &[4]]);
        let geometry = wrap_geometry_data(&mut out);
        let zs: Vec<f32> = geometry.positions.chunks_exact(3).map(|p| p[2]).collect();
        // the tip at alpha 1 is twice as far out as it would be at size 1
        assert!((zs.iter().cloned().fold(f32::MAX, f32::min) + 200.0).abs() < 1e-3);
    }
}
