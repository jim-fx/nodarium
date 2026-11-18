use myzel_core::{pack_rgba, path_lines, read_paths, Mesh, MeshWords, PathStyle};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::abi::{self, Output};
use nodarium_utils::{
    concat_args,
    geometry::{is_geometry, orthogonal_to, split_items},
    wrap_arg, Pattern, PATTERN_TAG,
};
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

/// The list the viewer reads of a single geometry, `[0, 1, ...geometry, 1, 1]`: framed in place
/// when it is an input the host copied in, the largest result of a build would otherwise be in
/// memory twice
fn geometry_list(geometry: &[i32]) -> Output {
    match abi::frame_input(geometry, &[0, 1], &[1, 1]) {
        Some(framed) => framed.into(),
        None => concat_args(vec![geometry]).into(),
    }
}

/// A list of geometry, like the one of `join`, goes to the viewer as it is, its paths as lines
fn show_list(list: &[i32]) -> Output {
    let items = split_items(list);
    let paths = read_paths(list, PathStyle::BODY);
    let geometry: Vec<&[i32]> = items.iter().copied().filter(|item| is_geometry(item)).collect();
    if paths.is_empty() && geometry.len() == items.len() && !items.is_empty() {
        return abi::frame_input(list, &[], &[]).map_or_else(|| list.to_vec().into(), Output::from);
    }
    let lines = (!paths.is_empty()).then(|| wrap_arg(&path_lines(&paths)));
    concat_args(geometry.into_iter().chain(lines.as_deref()).collect()).into()
}

#[nodarium_execute(arena)]
pub fn execute(args: &[&[i32]]) -> Output {
    if MeshWords::read(args[0]).is_some() {
        return geometry_list(args[0]);
    }
    if args[0].first() == Some(&PATTERN_TAG) {
        return match Pattern::decode(args[0]) {
            Some(pattern) => concat_args(vec![&pattern_mesh(&pattern).encode()]).into(),
            None => concat_args(vec![]).into(),
        };
    }
    if args[0].len() < 4 {
        return concat_args(vec![]).into();
    }
    show_list(args[0])
}

#[cfg(test)]
mod tests {
    use super::{__nodarium_execute_inner, Output};
    use glam::Vec3;
    use myzel_core::{Mesh, LINES_TAG};
    use nodarium_utils::{
        concat_arg_vecs, concat_args,
        geometry::{create_multiple_paths, wrap_multiple_paths},
        split_args,
    };

    /// The items of the list the viewer reads, brackets included
    fn run(input: &[i32]) -> Vec<Vec<i32>> {
        let output = __nodarium_execute_inner(&[input]);
        split_args(unsafe { output.words() }).iter().map(|item| item.to_vec()).collect()
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
        assert_eq!(&items[0][2..4], &[LINES_TAG, 4]);
    }

    #[test]
    fn colors_the_lines_by_the_group_of_their_creator() {
        let input = concat_arg_vecs(vec![paths(), create_multiple_paths(1, 2, 1, 4)]);
        let lines = &run(&input)[0];
        let groups: Vec<i32> = lines[4..lines.len() - 2].chunks_exact(7).map(|l| l[6]).collect();
        assert_eq!(groups, [1, 1, 1, 1, 0]);
    }

    fn triangle() -> Vec<i32> {
        Mesh {
            faces: vec![0, 1, 2],
            positions: vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            normals: vec![0.0, 0.0, 1.0].repeat(3),
            colors: Vec::new(),
            groups: vec![0; 3],
            creators: vec![12],
        }
        .encode()
    }

    #[test]
    fn passes_a_geometry_through() {
        let mesh = triangle();
        assert_eq!(run(&mesh), vec![mesh.clone()]);
        // a mesh the host copied in is framed where it is, as the copy would be
        nodarium_utils::abi::reset();
        let ptr = nodarium_utils::abi::alloc(mesh.len() * 4);
        let input = unsafe {
            std::ptr::copy_nonoverlapping(mesh.as_ptr(), ptr, mesh.len());
            std::slice::from_raw_parts(ptr, mesh.len())
        };
        let framed = __nodarium_execute_inner(&[input]);
        assert!(matches!(framed, Output::InPlace(_)));
        assert_eq!(unsafe { framed.words() }, concat_args(vec![&mesh]));
        nodarium_utils::abi::reset();
    }

    #[test]
    fn passes_a_list_of_geometry_through_and_shows_its_paths_as_lines() {
        let list = concat_args(vec![&triangle(), &triangle()]);
        assert_eq!(unsafe { __nodarium_execute_inner(&[&list]).words() }, &list[..]);
        let mixed = concat_arg_vecs(vec![triangle(), paths()]);
        let items = run(&mixed);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0], triangle());
        assert_eq!(&items[1][2..4], &[LINES_TAG, 4]);
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
