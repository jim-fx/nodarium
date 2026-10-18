use glam::{Mat4, Quat, Vec3};
use nodarium_macros::{nodarium_execute, nodarium_definition_file};
use nodarium_utils::{
    concat_args,
    geometry::{create_instance_data, wrap_geometry_data, wrap_instance_data, wrap_path},
    split_args, Ctx,
};

nodarium_definition_file!("src/input.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let mut ctx = Ctx::default();
    let mut inputs = split_args(args[0]);

    let mut geo_data = args[1].to_vec();
    let geo = wrap_geometry_data(&mut geo_data);

    let mut transforms: Vec<Mat4> = Vec::new();

    // Find max depth
    let mut max_depth = 0;
    for path_data in inputs.iter() {
        if path_data[2] != 0 {
            continue;
        }
        max_depth = max_depth.max(path_data[3]);
    }

    let rotation = ctx.float(args[5]);
    let depth = ctx.int(args[6]);

    for path_data in inputs.iter() {
        // skip everything that is not a path, e.g. instances from a previous instance node
        if path_data[2] != 0 || path_data[3] < (max_depth - depth + 1) {
            continue;
        }

        let amount = ctx.int(args[2]);
        let lowest_instance = ctx.float(args[3]);
        let highest_instance = ctx.float(args[4]);

        let path = wrap_path(path_data);

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
            let around = Quat::from_axis_angle(tangent, i as f32 * rotation) * base;
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
    }

    inputs.push(&instance_data);

    concat_args(inputs)
}
