use glam::{Mat3, Mat4, Vec3};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{
    encode_float,
    geometry::{
        extrude_path, geometry_path_id, wrap_geometry_data, wrap_instance_data, wrap_path, NO_PATH,
    },
    int, split_args, wrap_arg,
};

nodarium_definition_file!("src/definition.json");

#[derive(Default)]
struct Mesh {
    positions: Vec<f32>,
    normals: Vec<f32>,
    faces: Vec<i32>,
    path_ids: Vec<i32>,
}

impl Mesh {
    fn add(&mut self, positions: &[f32], normals: &[f32], faces: &[i32], transform: Mat4) {
        let offset = (self.positions.len() / 3) as i32;
        let normal_matrix = Mat3::from_mat4(transform).inverse().transpose();
        for (position, normal) in positions.chunks_exact(3).zip(normals.chunks_exact(3)) {
            let position = transform.transform_point3(Vec3::from_slice(position));
            let normal = (normal_matrix * Vec3::from_slice(normal)).normalize_or_zero();
            self.positions.extend_from_slice(&position.to_array());
            self.normals.extend_from_slice(&normal.to_array());
        }
        self.faces.extend(faces.iter().map(|i| i + offset));
    }

    fn add_geometry(&mut self, item: &[i32]) {
        let mut data = item.to_vec();
        let geometry = wrap_geometry_data(&mut data);
        self.add(
            geometry.positions,
            geometry.normals,
            geometry.faces,
            Mat4::IDENTITY,
        );
        self.path_ids.push(geometry_path_id(item));
    }

    fn add_instances(&mut self, item: &[i32]) {
        let mut data = item.to_vec();
        let instances = wrap_instance_data(&mut data);
        for (matrix, path_id) in instances
            .instances
            .chunks_exact(16)
            .zip(instances.path_ids.iter())
        {
            let transform = Mat4::from_cols_slice(matrix);
            // a copy scaled to nothing has no area and no normals
            if transform.determinant().abs() < 1e-12 {
                continue;
            }
            self.add(
                instances.positions,
                instances.normals,
                instances.faces,
                transform,
            );
            self.path_ids.push(*path_id);
        }
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
        wrap_arg(&out)
    }
}

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let input = args[0];
    let resolution = int(args[1]) as usize;
    let mut mesh = Mesh::default();

    // a single geometry comes without a list around it, an unconnected input is the empty
    // list `[0, 1, 1, 1]`
    let items = match input.get(2) {
        Some(1) if input.len() >= 8 => vec![input],
        _ => split_args(input),
    };

    for item in items {
        match item.get(2) {
            Some(0) => mesh.add_geometry(&extrude_path(wrap_path(item), resolution)),
            Some(1) => mesh.add_geometry(item),
            Some(2) => mesh.add_instances(item),
            _ => {}
        }
    }

    mesh.encode()
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use glam::{Mat4, Vec3};
    use nodarium_utils::{
        concat_args,
        geometry::{
            create_instance_data, create_multiple_paths, geometry_path_id, path_link,
            wrap_geometry_data, wrap_instance_data, wrap_multiple_paths, NO_PATH,
        },
        split_args, wrap_arg,
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

        let mut result = __nodarium_execute_inner(&[&input, &[8]]);
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

        let mut result = __nodarium_execute_inner(&[&stems, &[8]]);
        assert_eq!(geometry_path_id(&result), stem_id);
        let geometry = wrap_geometry_data(&mut result);
        assert!(geometry.faces.len() > 0);
        let highest = vec3s(geometry.positions)
            .iter()
            .map(|p| p.y)
            .fold(f32::MIN, f32::max);
        assert!((highest - 2.0).abs() < 1e-5);
    }

    #[test]
    fn a_single_geometry_passes_through() {
        let input = triangle(3);
        let result = __nodarium_execute_inner(&[&input, &[8]]);
        assert_eq!(result, input);
    }

    #[test]
    fn the_result_can_be_split_from_a_list_like_any_geometry() {
        let result = __nodarium_execute_inner(&[&triangle(NO_PATH), &[8]]);
        let list = concat_args(vec![&result, &result]);
        let items = split_args(&list);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0], result.as_slice());
    }

    #[test]
    fn an_unconnected_input_gives_an_empty_geometry() {
        let mut result = __nodarium_execute_inner(&[&[0, 1, 1, 1], &[8]]);
        let geometry = wrap_geometry_data(&mut result);
        assert!(geometry.positions.is_empty());
        assert!(geometry.faces.is_empty());
    }
}
