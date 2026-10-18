use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;
use nodarium_utils::{
    concat_arg_vecs, int,
    geometry::{
        create_path, interpolate_along_path, rotate_vector_by_angle, wrap_path, wrap_path_mut,
    },
    log, split_args, Ctx,
};

use std::f32::consts::PI;

nodarium_definition_file!("src/input.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let mut ctx = Ctx::default();

    let paths = split_args(args[0]);

    let mut output: Vec<Vec<i32>> = Vec::new();

    let resolution = int(args[8]).max(4) as usize;
    let depth = ctx.int(args[6]);

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

        let branch_amount = ctx.int(args[7]).max(1);

        let lowest_branch = ctx.float(args[4]);
        let highest_branch = ctx.float(args[5]);
        let taper = ctx.float(args[11]);
        // the distance between two neighbouring branches along the path
        let step = (highest_branch - lowest_branch) / (branch_amount - 1).max(1) as f32;

        for i in 0..branch_amount {
            let a = i as f32 / (branch_amount - 1).max(1) as f32;

            let root_alpha = (a * (highest_branch - lowest_branch) + lowest_branch)
                .min(1.0)
                .max(0.0);
            ctx.alpha = root_alpha;

            let length = ctx.float(args[1]);
            // alpha goes along the branch, so the thickness can follow a curve
            let alphas = (0..resolution).map(|j| j as f32 / (resolution - 1) as f32);
            let thickness = ctx.float_along(args[2], alphas);
            // every even branch moves up by a part of the step, 1 puts it at the height of the
            // next one, so the branches grow in pairs
            let offset_single = if i % 2 == 0 {
                ctx.float(args[3]) * step
            } else {
                0.0
            };

            let (branch_origin, orthogonal, direction) =
                interpolate_along_path(path.points, root_alpha + offset_single);

            let rotation_angle = (ctx.float(args[9]) * PI / 180.0) * i as f32;

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

            let up_angle = ctx.float(args[10]) * PI / 180.0;
            let tilted = (orthogonal * up_angle.cos() + direction * up_angle.sin()).normalize();
            let branch_direction = rotate_vector_by_angle(tilted, direction, rotation_angle);

            log!(
                "BRANCH depth: {}, branch_origin: {:?}, direction_at: {:?}, branch_direction: {:?}",
                depth,
                branch_origin,
                direction,
                branch_direction
            );
            // a branch is always one layer deeper than the path it grows on
            let mut branch_data = create_path(resolution, path_data[3] + 1);
            let branch = wrap_path_mut(&mut branch_data);

            for j in 0..resolution {
                let _a = j as f32 / (resolution - 1) as f32;
                branch.points[j * 4] = branch_origin[0] + branch_direction[0] * _a * length;
                branch.points[j * 4 + 1] = branch_origin[1] + branch_direction[1] * _a * length;
                branch.points[j * 4 + 2] = branch_origin[2] + branch_direction[2] * _a * length;
                branch.points[j * 4 + 3] = branch_origin[3] * thickness[j] * (1.0 - _a * taper);
            }

            output.push(branch_data);
        }

        output.push(path_data.to_vec());
    }

    concat_arg_vecs(output)
}
