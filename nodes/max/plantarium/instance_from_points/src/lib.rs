use glam::{Mat4, Vec3};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{
    concat_args, decode_points,
    geometry::{
        create_instance_data, is_path, path_link, wrap_geometry_data, wrap_instance_data,
    },
    group_points_by_parent, split_args, Ctx, Point,
};

nodarium_definition_file!("src/definition.json");

/// Where and how an instance sits on `point`. The geometry grows along its -z axis, its face
/// looks along +y. It sticks out along the direction of the point and faces up to the light as
/// far as it can.
fn transform(point: &Point, size: f32, angle: f32) -> Mat4 {
    // the angle tilts it along the path, positive toward the tip
    let out = point.direction * angle.cos() + point.tangent * angle.sin();
    let up = Vec3::Y - out * Vec3::Y.dot(out);
    let face = if up.length_squared() > 1e-6 {
        up.normalize()
    } else {
        point.tangent * angle.cos() - point.direction * angle.sin()
    };
    let back = -out;
    let across = face.cross(back);

    Mat4::from_cols(
        (across * size).extend(0.0),
        (face * size).extend(0.0),
        (back * size).extend(0.0),
        point.position.extend(1.0),
    )
}

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let mut ctx = Ctx::default();
    let mut plant = split_args(args[0]);
    let points = decode_points(args[1]);

    let mut geo_data = args[2].to_vec();
    let geo = wrap_geometry_data(&mut geo_data);

    let mut transforms: Vec<Mat4> = Vec::new();
    // the path each of the instances sits on
    let mut path_ids: Vec<i32> = Vec::new();

    let groups = group_points_by_parent(&points);
    for (rank, (parent_id, group)) in groups.iter().enumerate() {
        // the random values of the instances start from the path they sit on
        match plant
            .iter()
            .find(|p| is_path(p) && path_link(p).id == *parent_id)
        {
            Some(parent) => ctx.set_path(parent, rank, groups.len()),
            None => ctx.seed_path(*parent_id),
        }
        for point in group {
            ctx.alpha = point.alpha;
            let size = (point.scale + 0.01) * ctx.float(args[3]);
            let angle = ctx.float(args[4]).to_radians();
            transforms.push(transform(point, size, angle));
            path_ids.push(point.parent_id);
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

    plant.push(&instance_data);
    concat_args(plant)
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use glam::{Mat4, Vec3};
    use nodarium_utils::{
        concat_args, encode_float, encode_points,
        geometry::{create_multiple_paths, path_link, wrap_instance_data, NO_PATH},
        split_args, Point,
    };

    fn triangle() -> Vec<i32> {
        concat_args(vec![&[
            1, 3, 1, NO_PATH, 0, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ]])
    }

    #[test]
    fn an_instance_sits_on_every_point_and_knows_its_path() {
        let plant = create_multiple_paths(1, 4, 1, 6);
        let stem_id = path_link(split_args(&plant)[0]).id;
        let points: Vec<Point> = (0..3)
            .map(|i| Point {
                position: Vec3::new(0.0, i as f32, 0.0),
                direction: Vec3::X,
                tangent: Vec3::Y,
                scale: 0.09,
                parent_id: stem_id,
                alpha: i as f32 / 2.0,
                depth: 1,
                index: i,
            })
            .collect();

        let result = __nodarium_execute_inner(&[
            &plant,
            &encode_points(&points),
            &triangle(),
            &[encode_float(10.0)],
            &[encode_float(0.0)],
        ]);

        let items = split_args(&result);
        assert_eq!(items.len(), 2);
        let mut instance_data = items[1].to_vec();
        let instances = wrap_instance_data(&mut instance_data);
        assert_eq!(instances.instance_amount, 3);
        assert_eq!(instances.path_ids, &[stem_id; 3]);

        for (i, point) in points.iter().enumerate() {
            let matrix = Mat4::from_cols_slice(&instances.instances[i * 16..i * 16 + 16]);
            // it sits on the point and sticks out along its direction (the geometry grows along -z)
            assert!((matrix.w_axis.truncate() - point.position).length() < 1e-5);
            let out = -matrix.z_axis.truncate();
            assert!((out.normalize() - Vec3::X).length() < 1e-5);
            // the size is the thickness of the path at the point times the size input
            assert!((out.length() - 1.0).abs() < 1e-5);
        }
    }

    #[test]
    fn the_angle_tilts_the_instance_toward_the_tip() {
        let plant = create_multiple_paths(1, 4, 1, 6);
        let stem_id = path_link(split_args(&plant)[0]).id;
        let point = Point {
            position: Vec3::ZERO,
            direction: Vec3::X,
            tangent: Vec3::Y,
            scale: 0.09,
            parent_id: stem_id,
            alpha: 0.5,
            depth: 1,
            index: 0,
        };

        let result = __nodarium_execute_inner(&[
            &plant,
            &encode_points(&[point]),
            &triangle(),
            &[encode_float(10.0)],
            &[encode_float(90.0)],
        ]);

        let mut instance_data = split_args(&result)[1].to_vec();
        let instances = wrap_instance_data(&mut instance_data);
        let matrix = Mat4::from_cols_slice(&instances.instances[..16]);
        let out = -matrix.z_axis.truncate();
        assert!((out.normalize() - Vec3::Y).length() < 1e-5);
        // pointing straight up, its face turns away from the direction of the point
        let face = matrix.y_axis.truncate().normalize();
        assert!((face - Vec3::NEG_X).length() < 1e-5);
    }
}
