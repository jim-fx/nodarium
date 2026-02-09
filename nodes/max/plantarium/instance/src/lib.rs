use glam::{Mat4, Quat, Vec3};
use nodarium_macros::{nodarium_execute, nodarium_definition_file};
use nodarium_utils::{
    concat_args, evaluate_float, evaluate_int,
    geometry::{create_instance_data, wrap_geometry_data, wrap_instance_data, wrap_path},
    split_args,
};

nodarium_definition_file!("src/input.json");

#[nodarium_execute]
pub fn execute(input: &[i32]) -> Vec<i32> {
    let args = split_args(input);
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

    let rotation = evaluate_float(args[5]);
    let depth = evaluate_int(args[6]);

    for path_data in inputs.iter() {
        if path_data[3] < (max_depth - depth + 1) {
            continue;
        }

        let amount = evaluate_int(args[2]);
        let lowest_instance = evaluate_float(args[3]);
        let highest_instance = evaluate_float(args[4]);

        let path = wrap_path(path_data);

        for i in 0..amount {
            let alpha = lowest_instance
                + (i as f32 / (amount - 1) as f32) * (highest_instance - lowest_instance);

            let point = path.get_point_at(alpha);
            let tangent = path.get_direction_at(alpha);
            let size = point[3] + 0.01;

            let axis_rotation = Quat::from_axis_angle(
                Vec3::from_slice(&tangent).normalize(),
                i as f32 * rotation,
            );

            let path_rotation = Quat::from_rotation_arc(Vec3::Y, Vec3::from_slice(&tangent).normalize());

            let rotation = path_rotation * axis_rotation;

            let transform = Mat4::from_scale_rotation_translation(
                Vec3::new(size, size, size),
                rotation,
                Vec3::from_slice(&point),
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
