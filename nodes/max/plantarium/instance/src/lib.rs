use glam::{Mat4, Quat, Vec3};
use nodarium_macros::{nodarium_execute, nodarium_definition_file};
use nodarium_utils::{
    concat_args,
    geometry::{create_instance_data, wrap_geometry_data, wrap_instance_data, wrap_path},
    split_args, Ctx,
};

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let mut ctx = Ctx::default();
    let mut inputs = split_args(args[0]);

    let mut geo_data = args[1].to_vec();
    let geo = wrap_geometry_data(&mut geo_data);

    let mut transforms: Vec<Mat4> = Vec::new();
    // the path each of the instances sits on
    let mut path_ids: Vec<i32> = Vec::new();

    // Find max depth
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

            transforms.push(transform);
            path_ids.push(path.link.id);
        }
    }

    let mut instance_data = create_instance_data(
        geo.positions.len() / 3,
        geo.faces.len() / 3,
        transforms.len(),
    );
    let mut instances = wrap_instance_data(&mut instance_data);
    instances.set_geometry(geo);

    for (i, transform) in transforms.iter().enumerate() {
        instances.set_transformation_matrix(i, &transform.to_cols_array());
        instances.set_path_id(i, path_ids[i]);
    }

    inputs.push(&instance_data);

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
        split_args,
    };

    #[test]
    fn every_instance_knows_the_path_it_sits_on() {
        let mut stems = create_multiple_paths(2, 4, 1, 6);
        for stem in wrap_multiple_paths(&mut stems).iter_mut() {
            for i in 0..4 {
                stem.points[i * 4..i * 4 + 4].copy_from_slice(&[0.0, i as f32, 0.0, 0.1]);
            }
        }
        let stem_ids: Vec<i32> = split_args(&stems).iter().map(|p| path_link(p).id).collect();

        // a triangle, not made from a path
        let geometry = concat_args(vec![&[
            1, 3, 1, NO_PATH, 0, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ]]);

        let result = __nodarium_execute_inner(&[
            &stems,
            &geometry,
            &[3],
            &[encode_float(0.0)],
            &[encode_float(1.0)],
            &[encode_float(0.0)],
            &[1],
            &[encode_float(1.0)],
            &[encode_float(0.0)],
        ]);

        let items = split_args(&result);
        let mut instance_data = items.last().unwrap().to_vec();
        let instances = wrap_instance_data(&mut instance_data);

        assert_eq!(instances.instance_amount, 6);
        assert_eq!(instances.path_ids.len(), 6);
        assert_eq!(&instances.path_ids[..3], &[stem_ids[0]; 3]);
        assert_eq!(&instances.path_ids[3..], &[stem_ids[1]; 3]);
    }
}
