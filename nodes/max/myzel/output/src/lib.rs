use myzel_core::{pack_rgba, path_lines, read_paths, Mesh, PathStyle, MESH_TAG};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{geometry::orthogonal_to, Pattern, PATTERN_TAG};
use std::f32::consts::TAU;

nodarium_definition_file!("src/definition.json");

/// A pattern is drawn with at most this many points around a ring
const PATTERN_SEGMENTS: usize = 2048;

/// A pattern as a flat disc around the y axis, dark where its values are low and light where
/// they are high. Its angles count around the axis like the ones of `cap` on an upright stipe.
fn pattern_mesh(pattern: &Pattern) -> Mesh {
    let rings = pattern.rings.len();
    let segments = pattern.rings.iter().map(|r| r.len()).max().unwrap_or(0).clamp(3, PATTERN_SEGMENTS);
    if rings < 2 {
        return Mesh::default();
    }
    let side = orthogonal_to(glam::Vec3::Y);
    let around = glam::Vec3::Y.cross(side);
    let mut mesh = Mesh::default();
    for i in 0..rings {
        let r = pattern.radius(i);
        for j in 0..segments {
            // on the middle of the values, like the pattern places them
            let angle = TAU * (j as f32 + 0.5) / segments as f32;
            let p = (side * angle.cos() + around * angle.sin()) * r;
            mesh.positions.extend([p.x, p.y, p.z]);
            mesh.normals.extend([0.0, 1.0, 0.0]);
            let v = 0.15 + 0.8 * pattern.ring_value(i, angle).clamp(0.0, 1.0);
            mesh.colors.push(pack_rgba(v, v * 0.92, v * 0.85, 1.0));
            mesh.groups.push(0);
        }
    }
    let vertex = |i: usize, j: usize| (i * segments + j % segments) as u32;
    for i in 0..rings - 1 {
        for j in 0..segments {
            // counter-clockwise seen from above
            mesh.faces.extend([vertex(i, j), vertex(i + 1, j), vertex(i + 1, j + 1)]);
            mesh.faces.extend([vertex(i, j), vertex(i + 1, j + 1), vertex(i, j + 1)]);
        }
    }
    mesh
}

/// Wraps items in the list the viewer reads: [0, 1, [0, n + 1, ...item, 1, 1]..., 1, 1]
fn list(items: &[&[i32]]) -> Vec<i32> {
    let mut output = Vec::with_capacity(items.iter().map(|i| i.len() + 4).sum::<usize>() + 4);
    output.extend([0, 1]);
    for item in items {
        output.extend([0, item.len() as i32 + 1]);
        output.extend_from_slice(item);
        output.extend([1, 1]);
    }
    output.extend([1, 1]);
    output
}

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    match args[0].first() {
        Some(&MESH_TAG) => list(&[args[0]]),
        Some(&PATTERN_TAG) => match Pattern::decode(args[0]) {
            Some(pattern) => list(&[&pattern_mesh(&pattern).encode()]),
            None => list(&[]),
        },
        _ => {
            let paths = read_paths(args[0], PathStyle::BODY);
            if paths.is_empty() {
                return list(&[]);
            }
            list(&[&path_lines(&paths)])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use glam::Vec3;
    use myzel_core::{Mesh, LINES_TAG};
    use nodarium_utils::{
        concat_arg_vecs,
        geometry::{create_multiple_paths, wrap_multiple_paths},
        split_args,
    };

    fn run(input: &[i32]) -> Vec<Vec<i32>> {
        let output = __nodarium_execute_inner(&[input]);
        split_args(&output).iter().map(|item| item[2..item.len() - 2].to_vec()).collect()
    }

    fn paths() -> Vec<i32> {
        let mut paths = create_multiple_paths(2, 3, 1, 9);
        for (k, path) in wrap_multiple_paths(&mut paths).iter_mut().enumerate() {
            for i in 0..3 {
                path.points[i * 4..i * 4 + 4].copy_from_slice(&[k as f32 * 0.5, i as f32, i as f32 - 1.0, 0.1]);
            }
        }
        paths
    }

    #[test]
    fn shows_paths_as_lines_without_a_surface() {
        let items = run(&paths());
        assert_eq!(items.len(), 1);
        assert_eq!(&items[0][..2], &[LINES_TAG, 4]);
    }

    #[test]
    fn colors_the_lines_by_the_group_of_their_creator() {
        let input = concat_arg_vecs(vec![paths(), create_multiple_paths(1, 2, 1, 4)]);
        let lines = &run(&input)[0];
        let groups: Vec<i32> = lines[2..].chunks_exact(7).map(|l| l[6]).collect();
        assert_eq!(groups, [1, 1, 1, 1, 0]);
    }

    #[test]
    fn passes_a_mesh_through() {
        let mesh = Mesh {
            faces: vec![0, 1, 2],
            positions: vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            normals: vec![0.0, 0.0, 1.0].repeat(3),
            colors: Vec::new(),
            groups: vec![0; 3],
        }
        .encode();
        assert_eq!(run(&mesh), vec![mesh]);
    }

    #[test]
    fn shows_a_pattern_as_a_disc_seen_from_above() {
        let pattern = nodarium_utils::Pattern {
            inner: 0.01,
            outer: 0.03,
            spacing: 0.005,
            rings: vec![vec![0.0, 1.0, 0.0, 1.0]; 3],
        };
        let items = run(&pattern.encode());
        assert_eq!(items.len(), 1);
        let mesh = Mesh::decode(&items[0]);
        assert_eq!(mesh.colors.len(), mesh.vertex_count());
        assert!(mesh.face_count() > 0);
        // flat, from the inner to the outer radius
        for v in mesh.positions.chunks_exact(3) {
            assert_eq!(v[1], 0.0);
            let r = (v[0] * v[0] + v[2] * v[2]).sqrt();
            assert!((0.01 - 1e-6..=0.03 + 1e-6).contains(&r));
        }
        // the faces look up
        let p = |i: u32| Vec3::from_slice(&mesh.positions[i as usize * 3..i as usize * 3 + 3]);
        for f in mesh.faces.chunks_exact(3) {
            assert!((p(f[1]) - p(f[0])).cross(p(f[2]) - p(f[0])).y > 0.0);
        }
        // light where the pattern is high, dark where it is low
        let red = |c: u32| c & 0xff;
        assert!(mesh.colors.iter().any(|&c| red(c) > 200) && mesh.colors.iter().any(|&c| red(c) < 60));
    }

    #[test]
    fn nothing_connected_shows_nothing() {
        assert!(run(&[]).is_empty());
    }
}
