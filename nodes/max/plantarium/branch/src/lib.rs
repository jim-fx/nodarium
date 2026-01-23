use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;
use nodarium_utils::read_i32_slice;
use nodarium_utils::{
    concat_arg_vecs, evaluate_float, evaluate_int,
    geometry::{
        create_path, interpolate_along_path, rotate_vector_by_angle, wrap_path, wrap_path_mut,
    },
    log, split_args,
};

use std::f32::consts::PI;

nodarium_definition_file!("src/input.json");

#[nodarium_execute]
pub fn execute(
    path: (i32, i32),
    length: (i32, i32),
    thickness: (i32, i32),
    offset_single: (i32, i32),
    lowest_branch: (i32, i32),
    highest_branch: (i32, i32),
    depth: (i32, i32),
    amount: (i32, i32),
    resolution_curve: (i32, i32),
    rotation: (i32, i32),
) -> Vec<i32> {
    let arg = read_i32_slice(path);
    let paths = split_args(arg.as_slice());

    let mut output: Vec<Vec<i32>> = Vec::new();

    let resolution = evaluate_int(read_i32_slice(resolution_curve).as_slice()).max(4) as usize;
    let depth = evaluate_int(read_i32_slice(depth).as_slice());

    let mut max_depth = 0;
    for path_data in paths.iter() {
        if path_data[2] != 0 {
            continue;
        }
        max_depth = max_depth.max(path_data[3]);
    }

    for path_data in paths.iter() {
        // if this is not a path ignore it
        if path_data[2] != 0 || path_data[3] < (max_depth - depth + 1) {
            output.push(path_data.to_vec());
            continue;
        }

        let path = wrap_path(path_data);

        let branch_amount = evaluate_int(read_i32_slice(amount).as_slice()).max(1);

        let lowest_branch = evaluate_float(read_i32_slice(lowest_branch).as_slice());
        let highest_branch = evaluate_float(read_i32_slice(highest_branch).as_slice());

        for i in 0..branch_amount {
            let a = i as f32 / (branch_amount - 1).max(1) as f32;

            let length = evaluate_float(read_i32_slice(length).as_slice());
            let thickness = evaluate_float(read_i32_slice(thickness).as_slice());
            let offset_single = if i % 2 == 0 {
                evaluate_float(read_i32_slice(offset_single).as_slice())
            } else {
                0.0
            };

            let root_alpha = (a * (highest_branch - lowest_branch) + lowest_branch)
                .min(1.0)
                .max(0.0);

            let (branch_origin, orthogonal, direction) = interpolate_along_path(
                path.points,
                root_alpha + (offset_single - 0.5) * 6.0 / resolution as f32,
            );

            let rotation_angle =
                (evaluate_float(read_i32_slice(rotation).as_slice()) * PI / 180.0) * i as f32;

            // check if diration contains NaN
            if orthogonal[0].is_nan() || orthogonal[1].is_nan() || orthogonal[2].is_nan() {
                log!(
                    "BRANCH direction contains NaN: {:?}, branch_origin: {:?}, branch: {}",
                    direction,
                    branch_origin,
                    i
                );
                continue;
            }

            let branch_direction = rotate_vector_by_angle(orthogonal, direction, rotation_angle);

            log!(
                "BRANCH depth: {}, branch_origin: {:?}, direction_at: {:?}, branch_direction: {:?}",
                depth,
                branch_origin,
                direction,
                branch_direction
            );
            let mut branch_data = create_path(resolution, depth + 1);
            let branch = wrap_path_mut(&mut branch_data);

            for j in 0..resolution {
                let _a = j as f32 / (resolution - 1) as f32;
                branch.points[j * 4] = branch_origin[0] + branch_direction[0] * _a * length;
                branch.points[j * 4 + 1] = branch_origin[1] + branch_direction[1] * _a * length;
                branch.points[j * 4 + 2] = branch_origin[2] + branch_direction[2] * _a * length;
                branch.points[j * 4 + 3] = branch_origin[3] * thickness * (1.0 - _a);
            }

            output.push(branch_data);
        }

        output.push(path_data.to_vec());
    }

    concat_arg_vecs(output)
}
