use glam::{Mat4, Quat, Vec3};
use nodarium_macros::{nodarium_execute, nodarium_definition_file};
use nodarium_utils::{
    concat_args,
    geometry::{
        create_instance_data, is_geometry, path_id, split_items, wrap_geometry_data,
        wrap_instance_data, wrap_path,
    },
    split_args, Ctx,
};

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let mut ctx = Ctx::default();
    let mut inputs = split_args(args[0]);

    let geometries: Vec<&[i32]> = split_items(args[1])
        .into_iter()
        .filter(|item| is_geometry(item))
        .collect();
    if geometries.is_empty() {
        return concat_args(inputs);
    }
    let seed = args[9][0];

    // the transforms of the instances of each geometry and the paths they sit on
    let mut groups: Vec<(Vec<Mat4>, Vec<i32>)> = vec![Default::default(); geometries.len()];

    let mut max_depth = 0;
    for path_data in inputs.iter() {
        if path_data[2] != 0 {
            continue;
        }
        max_depth = max_depth.max(path_data[3]);
    }

    let depth = ctx.int(args[6]);

    let path_count = inputs
        .iter()
        .filter(|p| p[2] == 0 && p[3] >= (max_depth - depth + 1))
        .count();
    let mut path_index = 0;

    for path_data in inputs.iter() {
        // skip everything that is not a path, e.g. instances from a previous instance node
        if path_data[2] != 0 || path_data[3] < (max_depth - depth + 1) {
            continue;
        }

        // alpha is where this parent path sits among all paths, from 0 at the first to 1 at the last
        ctx.set_path(path_data, path_index, path_count);
        path_index += 1;
        let amount = ctx.int(args[2]);
        let lowest_instance = ctx.float(args[3]);
        let highest_instance = ctx.float(args[4]);

        let path = wrap_path(path_data);

        // each instance reads the rotation step itself, so a random node gives every
        // instance its own turn around the path
        let mut turn = 0.0;

        for i in 0..amount {
            let alpha = lowest_instance
                + (i as f32 / (amount - 1).max(1) as f32) * (highest_instance - lowest_instance);
            ctx.alpha = alpha;

            let point = path.get_point_at(alpha);
            let tangent = Vec3::from_slice(&path.get_direction_at(alpha)).normalize();
            let size = (point[3] + 0.01) * ctx.float(args[7]);

            // the geometry grows along its -z axis, its face looks along +y.
            // It sticks out sideways from the path, turned around the path by the rotation,
            // and faces up to the light as far as it can
            let side = tangent.cross(Vec3::Y);
            let base = if side.length_squared() > 1e-6 {
                side.normalize()
            } else {
                Vec3::NEG_Z
            };
            let around = Quat::from_axis_angle(tangent, turn) * base;
            turn += ctx.float(args[5]);
            // the angle tilts it along the path, positive toward the tip
            let angle = ctx.float(args[8]).to_radians();
            let out = around * angle.cos() + tangent * angle.sin();
            let up = Vec3::Y - out * Vec3::Y.dot(out);
            let face = if up.length_squared() > 1e-6 {
                up.normalize()
            } else {
                tangent * angle.cos() - around * angle.sin()
            };
            let back = -out;
            let across = face.cross(back);

            let transform = Mat4::from_cols(
                (across * size).extend(0.0),
                (face * size).extend(0.0),
                (back * size).extend(0.0),
                Vec3::from_slice(&point).extend(1.0),
            );

            let choice = path_id(seed, path.link.id, i) as usize % geometries.len();
            groups[choice].0.push(transform);
            groups[choice].1.push(path.link.id);
        }
    }

    let instance_data: Vec<Vec<i32>> = geometries
        .iter()
        .zip(&groups)
        .filter(|(_, (transforms, _))| !transforms.is_empty())
        .map(|(geometry, (transforms, path_ids))| {
            let mut geo_data = geometry.to_vec();
            let geo = wrap_geometry_data(&mut geo_data);
            let mut data = create_instance_data(
                geo.positions.len() / 3,
                geo.faces.len() / 3,
                transforms.len(),
            );
            let mut instances = wrap_instance_data(&mut data);
            instances.set_geometry(geo);
            for (i, (transform, path_id)) in transforms.iter().zip(path_ids).enumerate() {
                instances.set_transformation_matrix(i, &transform.to_cols_array());
                instances.set_path_id(i, *path_id);
            }
            data
        })
        .collect();

    inputs.extend(instance_data.iter().map(Vec::as_slice));

    concat_args(inputs)
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use nodarium_utils::{
        concat_args, encode_float,
        geometry::{
            create_multiple_paths, path_link, wrap_instance_data, wrap_multiple_paths, NO_PATH,
        },
        split_args, wrap_arg,
    };

    /// Two straight stems of four points each.
    fn stems() -> Vec<i32> {
        let mut stems = create_multiple_paths(2, 4, 1, 6);
        for stem in wrap_multiple_paths(&mut stems).iter_mut() {
            for i in 0..4 {
                stem.points[i * 4..i * 4 + 4].copy_from_slice(&[0.0, i as f32, 0.0, 0.1]);
            }
        }
        stems
    }

    /// A geometry of `vertices` points at the origin and a single face.
    fn geometry(vertices: i32) -> Vec<i32> {
        let mut data = vec![1, vertices, 1, NO_PATH, 0, 1, 2];
        data.extend(vec![0; vertices as usize * 6]);
        wrap_arg(&data)
    }

    fn run(stems: &[i32], geometry: &[i32], amount: i32, seed: i32) -> Vec<i32> {
        __nodarium_execute_inner(&[
            stems,
            geometry,
            &[amount],
            &[encode_float(0.0)],
            &[encode_float(1.0)],
            &[encode_float(0.0)],
            &[1],
            &[encode_float(1.0)],
            &[encode_float(0.0)],
            &[seed],
        ])
    }

    /// The vertex amount and path ids of every instance group in the result.
    fn groups(result: &[i32]) -> Vec<(usize, Vec<i32>)> {
        split_args(result)
            .iter()
            .filter(|item| item[2] == 2)
            .map(|item| {
                let mut data = item.to_vec();
                let instances = wrap_instance_data(&mut data);
                (instances.positions.len() / 3, instances.path_ids.to_vec())
            })
            .collect()
    }

    #[test]
    fn every_instance_knows_the_path_it_sits_on() {
        let stems = stems();
        let stem_ids: Vec<i32> = split_args(&stems).iter().map(|p| path_link(p).id).collect();

        let groups = groups(&run(&stems, &geometry(3), 3, 0));

        assert_eq!(groups.len(), 1);
        let path_ids = &groups[0].1;
        assert_eq!(path_ids.len(), 6);
        assert_eq!(&path_ids[..3], &[stem_ids[0]; 3]);
        assert_eq!(&path_ids[3..], &[stem_ids[1]; 3]);
    }

    #[test]
    fn a_list_with_one_geometry_is_the_same_as_the_geometry() {
        let stems = stems();
        let single = run(&stems, &geometry(3), 3, 0);
        let list = run(&stems, &concat_args(vec![&geometry(3)]), 3, 0);
        assert_eq!(single, list);
    }

    #[test]
    fn every_instance_picks_one_geometry_of_the_list() {
        let stems = stems();
        let list = concat_args(vec![&geometry(3), &geometry(4), &geometry(5)]);

        let groups = groups(&run(&stems, &list, 20, 0));

        let mut vertices: Vec<usize> = groups.iter().map(|(v, _)| *v).collect();
        vertices.sort();
        assert_eq!(vertices, vec![3, 4, 5]);
        let total: usize = groups.iter().map(|(_, ids)| ids.len()).sum();
        assert_eq!(total, 40);
    }

    #[test]
    fn the_seed_decides_which_geometry_an_instance_picks() {
        let stems = stems();
        let list = concat_args(vec![&geometry(3), &geometry(4), &geometry(5)]);

        assert_eq!(run(&stems, &list, 20, 1), run(&stems, &list, 20, 1));
        assert_ne!(run(&stems, &list, 20, 1), run(&stems, &list, 20, 2));
    }

    #[test]
    fn the_paths_stay_in_the_output() {
        let stems = stems();
        let list = concat_args(vec![&geometry(3), &geometry(4)]);

        let result = run(&stems, &list, 5, 0);

        assert_eq!(&split_args(&result)[..2], &split_args(&stems)[..]);
    }

    #[test]
    fn an_unconnected_geometry_places_nothing() {
        let stems = stems();
        // the default value `[0, 0, 0]`, and the empty list the runtime sends without one
        for geometry in [&[0, 4, 0, 0, 0, 1, 1][..], &[0, 1, 1, 1][..]] {
            let result = run(&stems, geometry, 3, 0);
            assert!(groups(&result).is_empty());
            assert_eq!(split_args(&result).len(), 2);
        }
    }
}

