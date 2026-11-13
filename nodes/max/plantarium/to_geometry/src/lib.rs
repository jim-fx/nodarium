use glam::{Mat3, Mat4, Vec3, Vec4, Vec4Swizzles};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{
    concat_args, encode_float,
    geometry::{
        extrude_path, geometry_path_id, is_collapsed, path_link, colors_at_lengths, split_items,
        wrap_geometry_data, wrap_instance_data, wrap_path, PathData, PathStructure, COLORED_PATH,
        NO_PATH,
    },
    int, wrap_arg, NO_COLOR,
};
use std::collections::HashMap;

nodarium_definition_file!("src/definition.json");

#[derive(Default)]
struct Mesh {
    positions: Vec<f32>,
    normals: Vec<f32>,
    faces: Vec<i32>,
    path_ids: Vec<i32>,
    /// Empty, or an RGBA8 color per vertex once a part with colors was added
    colors: Vec<u32>,
}

/// A part to bake: the vertices, faces and colors of a geometry
struct Part<'a> {
    positions: &'a [f32],
    normals: &'a [f32],
    faces: &'a [i32],
    /// Empty for a part without colors
    colors: &'a [u32],
}

impl Mesh {
    fn add(&mut self, part: &Part, transform: Mat4, path_id: i32) {
        let Part { positions, normals, faces, colors } = *part;
        let offset = (self.positions.len() / 3) as i32;
        // the vertices of a part without colors keep the material, as do those before the first
        if !colors.is_empty() || !self.colors.is_empty() {
            self.colors.resize(offset as usize, NO_COLOR);
            if colors.is_empty() {
                self.colors.extend(std::iter::repeat_n(NO_COLOR, positions.len() / 3));
            } else {
                self.colors.extend_from_slice(colors);
            }
        }
        let normal_matrix = Mat3::from_mat4(transform).inverse().transpose();
        for (position, normal) in positions.chunks_exact(3).zip(normals.chunks_exact(3)) {
            let position = transform.transform_point3(Vec3::from_slice(position));
            let normal = (normal_matrix * Vec3::from_slice(normal)).normalize_or_zero();
            self.positions.extend_from_slice(&position.to_array());
            self.normals.extend_from_slice(&normal.to_array());
        }
        self.faces.extend(faces.iter().map(|i| i + offset));
        self.path_ids.push(path_id);
    }

    /// `[1, vertex_amount, face_amount, path_id, ...faces, ...positions, ...normals]` in brackets.
    /// The path id is kept when every part comes from the same path.
    fn encode(self) -> Vec<i32> {
        let path_id = match self.path_ids.split_first() {
            Some((first, rest)) if rest.iter().all(|id| id == first) => *first,
            _ => NO_PATH,
        };
        let mut out = vec![
            1,
            (self.positions.len() / 3) as i32,
            (self.faces.len() / 3) as i32,
            path_id,
        ];
        out.extend(self.faces);
        out.extend(self.positions.iter().map(|&f| encode_float(f)));
        out.extend(self.normals.iter().map(|&f| encode_float(f)));
        out.extend(self.colors.iter().map(|&c| c as i32));
        wrap_arg(&out)
    }
}

/// The meshes the parts are baked into. A part goes into the mesh of the path it was made
/// from, a part of no known path goes into every mesh.
struct Variants {
    meshes: Vec<Mesh>,
    mesh_of_path: HashMap<i32, usize>,
}

impl Variants {
    /// One mesh for everything.
    fn single() -> Self {
        Self {
            meshes: vec![Mesh::default()],
            mesh_of_path: HashMap::new(),
        }
    }

    /// One mesh per path without a parent in `items`, in the order of the list. The paths
    /// growing on it, and theirs, go into the same mesh.
    fn per_stem(items: &[&[i32]]) -> Self {
        let paths: Vec<Vec<i32>> = items
            .iter()
            .filter(|item| matches!(item.get(2), Some(&0) | Some(&COLORED_PATH)))
            .map(|item| item.to_vec())
            .collect();
        let structure = PathStructure::new(&paths);
        let root = |mut index: usize| {
            for _ in 0..paths.len() {
                match structure.parent(index) {
                    Some(parent) => index = parent,
                    None => break,
                }
            }
            index
        };

        let mesh_of_stem: HashMap<usize, usize> = (0..paths.len())
            .filter(|&index| structure.parent(index).is_none())
            .enumerate()
            .map(|(mesh, stem)| (stem, mesh))
            .collect();

        Self {
            meshes: (0..mesh_of_stem.len()).map(|_| Mesh::default()).collect(),
            mesh_of_path: paths
                .iter()
                .enumerate()
                .filter_map(|(index, path)| {
                    let mesh = *mesh_of_stem.get(&root(index))?;
                    Some((path_link(path).id, mesh))
                })
                .collect(),
        }
    }

    fn add(&mut self, part: &Part, transform: Mat4, path_id: i32) {
        match self.mesh_of_path.get(&path_id) {
            Some(&mesh) => self.meshes[mesh].add(part, transform, path_id),
            None => {
                for mesh in &mut self.meshes {
                    mesh.add(part, transform, path_id);
                }
            }
        }
    }

    fn add_geometry(&mut self, item: &[i32]) {
        let mut data = item.to_vec();
        let geometry = wrap_geometry_data(&mut data);
        let part = Part {
            positions: geometry.positions,
            normals: geometry.normals,
            faces: geometry.faces,
            colors: geometry.colors,
        };
        self.add(&part, Mat4::IDENTITY, geometry_path_id(item));
    }

    fn add_instances(&mut self, item: &[i32]) {
        let mut data = item.to_vec();
        let instances = wrap_instance_data(&mut data);
        let part = Part {
            positions: instances.positions,
            normals: instances.normals,
            faces: instances.faces,
            colors: instances.colors,
        };
        for (matrix, path_id) in instances
            .instances
            .chunks_exact(16)
            .zip(instances.path_ids.iter())
        {
            let transform = Mat4::from_cols_slice(matrix);
            if is_collapsed(&transform) {
                continue;
            }
            self.add(&part, transform, *path_id);
        }
    }
}

/// The points of a path spread evenly along its length, `count` of them.
fn resample(points: &[f32], count: usize) -> Vec<f32> {
    let points: Vec<Vec4> = points.chunks_exact(4).map(Vec4::from_slice).collect();
    let mut distances = vec![0.0];
    for pair in points.windows(2) {
        distances.push(distances[distances.len() - 1] + pair[0].xyz().distance(pair[1].xyz()));
    }
    let total = distances[distances.len() - 1];

    let mut resampled = Vec::with_capacity(count * 4);
    let mut segment = 0;
    for i in 0..count {
        let target = total * i as f32 / (count - 1) as f32;
        while segment + 2 < points.len() && distances[segment + 1] < target {
            segment += 1;
        }
        let length = distances[segment + 1] - distances[segment];
        let t = if length > 0.0 {
            ((target - distances[segment]) / length).clamp(0.0, 1.0)
        } else {
            0.0
        };
        resampled.extend(points[segment].lerp(points[segment + 1], t).to_array());
    }
    resampled
}

fn extrude(item: &[i32], circle_segments: usize, max_points: usize) -> Vec<i32> {
    let path = wrap_path(item);
    if max_points < 2 || path.length <= max_points {
        return extrude_path(path, circle_segments);
    }
    let points = resample(path.points, max_points);
    let colors = if path.colors.is_empty() {
        Vec::new()
    } else {
        let shares = (0..max_points).map(|i| i as f32 / (max_points - 1) as f32);
        colors_at_lengths(path.points, path.colors, shares)
    };
    extrude_path(
        PathData {
            length: max_points,
            points: &points,
            colors: &colors,
            ..path
        },
        circle_segments,
    )
}

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let input = args[0];
    let mut circle_segments = int(args[1]) as usize;
    let max_circle_segments = int(args[2]) as usize;
    if max_circle_segments > 0 {
        circle_segments = circle_segments.min(max_circle_segments);
    }
    let max_points = int(args[3]) as usize;
    let per_stem = int(args[4]) != 0;

    let items = split_items(input);
    let mut variants = if per_stem {
        Variants::per_stem(&items)
    } else {
        Variants::single()
    };
    if variants.meshes.is_empty() {
        variants = Variants::single();
    }

    for item in items {
        match item.get(2) {
            Some(0) | Some(&COLORED_PATH) => {
                variants.add_geometry(&extrude(item, circle_segments, max_points))
            }
            Some(1) => variants.add_geometry(item),
            Some(2) => variants.add_instances(item),
            _ => {}
        }
    }

    let mut meshes: Vec<Vec<i32>> = variants.meshes.into_iter().map(Mesh::encode).collect();
    if meshes.len() == 1 {
        return meshes.remove(0);
    }
    concat_args(meshes.iter().map(Vec::as_slice).collect())
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use glam::{Mat4, Vec3};
    use nodarium_utils::{
        concat_args,
        geometry::{
            create_instance_data, create_multiple_paths, create_path, extrude_path,
            geometry_path_id, is_geometry, path_link, set_path_link, split_items,
            wrap_geometry_data, wrap_instance_data, wrap_multiple_paths, wrap_path, wrap_path_mut,
            PathLink, NO_PATH, has_geometry_colors, with_geometry_colors, with_path_colors,
        },
        split_args, wrap_arg, NO_COLOR,
    };

    /// A triangle along +x and +y, its normals look along +z.
    fn triangle(path_id: i32) -> Vec<i32> {
        let mut data = vec![1, 3, 1, path_id, 0, 1, 2];
        let positions = [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0f32];
        let normals = [0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0f32];
        data.extend(positions.iter().chain(&normals).map(|f| f.to_bits() as i32));
        wrap_arg(&data)
    }

    fn instances_of(geometry: &[i32], transforms: &[(Mat4, i32)]) -> Vec<i32> {
        let mut geometry = geometry.to_vec();
        let geo = wrap_geometry_data(&mut geometry);
        let mut data = create_instance_data(3, 1, transforms.len());
        let mut instances = wrap_instance_data(&mut data);
        instances.set_geometry(geo);
        for (i, (transform, path_id)) in transforms.iter().enumerate() {
            instances.set_transformation_matrix(i, &transform.to_cols_array());
            instances.set_path_id(i, *path_id);
        }
        data
    }

    fn vec3s(data: &[f32]) -> Vec<Vec3> {
        data.chunks_exact(3).map(Vec3::from_slice).collect()
    }

    #[test]
    fn instances_are_baked_into_the_geometry() {
        let moved = Mat4::from_translation(Vec3::new(0.0, 5.0, 0.0));
        let turned = Mat4::from_scale_rotation_translation(
            Vec3::splat(2.0),
            glam::Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
            Vec3::ZERO,
        );
        let instances = instances_of(&triangle(NO_PATH), &[(moved, 7), (turned, 8)]);
        let input = concat_args(vec![&triangle(NO_PATH), &instances]);

        let mut result = __nodarium_execute_inner(&[&input, &[8], &[0], &[0], &[0]]);
        assert_eq!(geometry_path_id(&result), NO_PATH);
        let geometry = wrap_geometry_data(&mut result);

        assert_eq!(geometry.positions.len() / 3, 9);
        assert_eq!(geometry.faces, &[0, 1, 2, 3, 4, 5, 6, 7, 8]);
        let positions = vec3s(geometry.positions);
        let normals = vec3s(geometry.normals);
        assert!((positions[4] - Vec3::new(1.0, 5.0, 0.0)).length() < 1e-5);
        assert!((positions[7] - Vec3::new(0.0, 0.0, -2.0)).length() < 1e-5);
        assert!((normals[3] - Vec3::Z).length() < 1e-5);
        assert!((normals[6] - Vec3::X).length() < 1e-5);
    }

    #[test]
    fn paths_are_extruded_and_keep_their_id_when_alone() {
        let mut stems = create_multiple_paths(1, 3, 1, 6);
        for stem in wrap_multiple_paths(&mut stems).iter_mut() {
            for i in 0..3 {
                stem.points[i * 4..i * 4 + 4].copy_from_slice(&[0.0, i as f32, 0.0, 0.1]);
            }
        }
        let stem_id = path_link(split_args(&stems)[0]).id;

        let mut result = __nodarium_execute_inner(&[&stems, &[8], &[0], &[0], &[0]]);
        assert_eq!(geometry_path_id(&result), stem_id);
        let geometry = wrap_geometry_data(&mut result);
        assert!(geometry.faces.len() > 0);
        let highest = vec3s(geometry.positions)
            .iter()
            .map(|p| p.y)
            .fold(f32::MIN, f32::max);
        assert!((highest - 2.0).abs() < 1e-5);
    }

    /// A stem along +y with a point every meter.
    fn straight_stem(points: usize) -> Vec<i32> {
        let mut stems = create_multiple_paths(1, points, 1, 6);
        for stem in wrap_multiple_paths(&mut stems).iter_mut() {
            for i in 0..points {
                stem.points[i * 4..i * 4 + 4].copy_from_slice(&[0.0, i as f32, 0.0, 0.1]);
            }
        }
        stems
    }

    fn vertex_and_face_amount(geometry: &[i32]) -> (i32, i32) {
        (geometry[3], geometry[4])
    }

    #[test]
    fn without_limits_paths_get_the_full_resolution() {
        let stems = straight_stem(10);
        let expected = extrude_path(wrap_path(split_args(&stems)[0]), 8);

        let result = __nodarium_execute_inner(&[&stems, &[8], &[0], &[0], &[0]]);

        assert_eq!(
            vertex_and_face_amount(&result),
            vertex_and_face_amount(&expected)
        );
    }

    #[test]
    fn the_limits_lower_the_resolution_of_paths() {
        let stems = straight_stem(10);

        let mut result = __nodarium_execute_inner(&[&stems, &[8], &[4], &[3], &[0]]);

        // 3 rings of 4, a cap ring and its center
        assert_eq!(
            vertex_and_face_amount(&result),
            (3 * 4 + 4 + 1, 2 * 4 * 2 + 4)
        );
        let geometry = wrap_geometry_data(&mut result);
        let heights: Vec<f32> = vec3s(geometry.positions).iter().map(|p| p.y).collect();
        assert!(heights.iter().any(|y| (y - 4.5).abs() < 1e-5));
        assert!(heights.iter().any(|y| (y - 9.0).abs() < 1e-5));
    }

    #[test]
    fn a_lower_resolution_setting_wins_over_the_limit() {
        let stems = straight_stem(10);
        let expected = extrude_path(wrap_path(split_args(&stems)[0]), 4);

        let result = __nodarium_execute_inner(&[&stems, &[4], &[16], &[0], &[0]]);

        assert_eq!(
            vertex_and_face_amount(&result),
            vertex_and_face_amount(&expected)
        );
    }

    #[test]
    fn a_single_geometry_passes_through() {
        let input = triangle(3);
        let result = __nodarium_execute_inner(&[&input, &[8], &[0], &[0], &[0]]);
        assert_eq!(result, input);
    }

    #[test]
    fn the_result_can_be_split_from_a_list_like_any_geometry() {
        let result = __nodarium_execute_inner(&[&triangle(NO_PATH), &[8], &[0], &[0], &[0]]);
        let list = concat_args(vec![&result, &result]);
        let items = split_args(&list);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0], result.as_slice());
    }

    #[test]
    fn an_unconnected_input_gives_an_empty_geometry() {
        let mut result = __nodarium_execute_inner(&[&[0, 1, 1, 1], &[8], &[0], &[0], &[0]]);
        let geometry = wrap_geometry_data(&mut result);
        assert!(geometry.positions.is_empty());
        assert!(geometry.faces.is_empty());
    }

    /// `amount` stems along +y, stem i is i + 1 meters long.
    fn stems_of_growing_length(amount: usize) -> Vec<i32> {
        let mut stems = create_multiple_paths(amount, 3, 1, 6);
        for (i, stem) in wrap_multiple_paths(&mut stems).iter_mut().enumerate() {
            for j in 0..3 {
                let y = (i + 1) as f32 * j as f32 / 2.0;
                stem.points[j * 4..j * 4 + 4].copy_from_slice(&[0.0, y, 0.0, 0.1]);
            }
        }
        stems
    }

    fn highest_point(geometry: &[i32]) -> f32 {
        let mut geometry = geometry.to_vec();
        vec3s(wrap_geometry_data(&mut geometry).positions)
            .iter()
            .map(|p| p.y)
            .fold(f32::MIN, f32::max)
    }

    fn a_branch_on(parent_id: i32) -> Vec<i32> {
        let mut branch = create_path(2, 2);
        set_path_link(&mut branch, PathLink::child(9, parent_id, 0.5, 0));
        wrap_path_mut(&mut branch)
            .points
            .copy_from_slice(&[0.0, 10.0, 0.0, 0.1, 1.0, 10.0, 0.0, 0.1]);
        branch
    }

    #[test]
    fn variants_give_a_geometry_per_stem() {
        let stems = stems_of_growing_length(3);
        let ids: Vec<i32> = split_args(&stems).iter().map(|s| path_link(s).id).collect();

        let result = __nodarium_execute_inner(&[&stems, &[8], &[0], &[0], &[1]]);
        let variants = split_items(&result);

        assert_eq!(variants.len(), 3);
        for (i, variant) in variants.iter().enumerate() {
            assert!(is_geometry(variant));
            assert_eq!(geometry_path_id(variant), ids[i]);
            assert!((highest_point(variant) - (i + 1) as f32).abs() < 1e-5);
        }
    }

    #[test]
    fn without_variants_all_stems_are_one_geometry() {
        let stems = stems_of_growing_length(3);
        let result = __nodarium_execute_inner(&[&stems, &[8], &[0], &[0], &[0]]);
        assert!(is_geometry(&result));
        assert_eq!(geometry_path_id(&result), NO_PATH);
        assert!((highest_point(&result) - 3.0).abs() < 1e-5);
    }

    #[test]
    fn what_grows_on_a_stem_belongs_to_its_variant() {
        let stems = stems_of_growing_length(2);
        let first = path_link(split_args(&stems)[0]).id;
        let branch = a_branch_on(first);
        let leaves = instances_of(
            &triangle(NO_PATH),
            &[(
                Mat4::from_translation(Vec3::new(0.0, 20.0, 0.0)),
                path_link(&branch).id,
            )],
        );
        let mut input = split_args(&stems);
        input.push(&branch);
        input.push(&leaves);
        let input = concat_args(input);

        let result = __nodarium_execute_inner(&[&input, &[8], &[0], &[0], &[1]]);
        let variants = split_items(&result);

        assert_eq!(variants.len(), 2);
        assert!((highest_point(variants[0]) - 21.0).abs() < 1e-5);
        assert!((highest_point(variants[1]) - 2.0).abs() < 1e-5);
    }

    #[test]
    fn a_part_of_no_stem_goes_into_every_variant() {
        let stems = stems_of_growing_length(2);
        let mut input = split_args(&stems);
        let loose = triangle(NO_PATH);
        input.push(&loose);
        let input = concat_args(input);

        let alone = __nodarium_execute_inner(&[&stems, &[8], &[0], &[0], &[1]]);
        let result = __nodarium_execute_inner(&[&input, &[8], &[0], &[0], &[1]]);

        for (with, without) in split_items(&result).iter().zip(split_items(&alone)) {
            assert_eq!(with[3], without[3] + 3);
            assert_eq!(geometry_path_id(with), NO_PATH);
        }
    }

    #[test]
    fn variants_of_a_single_stem_are_one_geometry() {
        let stems = straight_stem(10);
        let with = __nodarium_execute_inner(&[&stems, &[8], &[0], &[0], &[1]]);
        let without = __nodarium_execute_inner(&[&stems, &[8], &[0], &[0], &[0]]);
        assert_eq!(with, without);
    }

    #[test]
    fn variants_without_stems_bake_everything_into_one_geometry() {
        let input = triangle(3);
        let result = __nodarium_execute_inner(&[&input, &[8], &[0], &[0], &[1]]);
        assert_eq!(result, input);
    }

    #[test]
    fn variants_of_an_unconnected_input_give_an_empty_geometry() {
        let mut result = __nodarium_execute_inner(&[&[0, 1, 1, 1], &[8], &[0], &[0], &[1]]);
        let geometry = wrap_geometry_data(&mut result);
        assert!(geometry.positions.is_empty());
        assert!(geometry.faces.is_empty());
    }

    #[test]
    fn colors_are_baked_and_parts_without_them_keep_the_material() {
        let colored = with_geometry_colors(&triangle(NO_PATH), &[1, 2, 3]);
        let input = concat_args(vec![&triangle(NO_PATH), &colored]);
        let mut result = __nodarium_execute_inner(&[&input, &[8], &[0], &[0], &[0]]);
        assert_eq!(wrap_geometry_data(&mut result).colors, &[NO_COLOR, NO_COLOR, NO_COLOR, 1, 2, 3]);

        let plain = concat_args(vec![&triangle(NO_PATH), &triangle(NO_PATH)]);
        assert!(!has_geometry_colors(&__nodarium_execute_inner(&[&plain, &[8], &[0], &[0], &[0]])));
    }

    #[test]
    fn a_resampled_path_keeps_its_colors() {
        let mut stem = create_path(5, 1);
        for (i, point) in wrap_path_mut(&mut stem).points.chunks_exact_mut(4).enumerate() {
            point.copy_from_slice(&[0.0, i as f32, 0.0, 0.1]);
        }
        let colors = [0xff00_0000, 0xff00_0040, 0xff00_0080, 0xff00_00c0, 0xff00_00ff];
        let stem = concat_args(vec![&with_path_colors(&stem, &colors)]);
        let mut result = __nodarium_execute_inner(&[&stem, &[4], &[0], &[3], &[0]]);
        let baked = wrap_geometry_data(&mut result).colors.to_vec();
        assert_eq!(&baked[..4], &[colors[0]; 4]);
        assert_eq!(&baked[4..8], &[colors[2]; 4]);
        assert!(baked[8..].iter().all(|&c| c == colors[4]));
    }
}
