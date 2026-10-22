use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;
use nodarium_utils::{
    concat_arg_vecs,
    geometry::{
        create_path, interpolate_along_path, path_link, rotate_vector_by_angle, set_path_link,
        wrap_path, wrap_path_mut, PathLink,
    },
    int, log, split_args, Ctx,
};

use std::f32::consts::PI;

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let mut ctx = Ctx::default();

    let paths = split_args(args[0]);

    let mut output: Vec<Vec<i32>> = Vec::new();

    let max_resolution = int(args[8]).max(4) as usize;
    let depth = ctx.int(args[6]);

    let mut max_depth = 0;
    for path_data in paths.iter() {
        if path_data[2] != 0 {
            continue;
        }
        max_depth = max_depth.max(path_data[3]);
    }

    // the longest path sets the point density, so branches on every level share it
    let reference_length = paths
        .iter()
        .filter(|p| p[2] == 0)
        .map(|p| wrap_path(p).get_length())
        .fold(0.0_f32, f32::max);

    let path_count = paths
        .iter()
        .filter(|p| p[2] == 0 && p[3] >= (max_depth - depth + 1))
        .count();
    let mut path_index = 0;
    // new branches get ids that no path in the list has yet
    let mut next_id = paths
        .iter()
        .filter(|p| p[2] == 0)
        .map(|p| path_link(p).id)
        .max()
        .map_or(0, |id| id + 1);

    for path_data in paths.iter() {
        // if this is not a path ignore it
        if path_data[2] != 0 || path_data[3] < (max_depth - depth + 1) {
            output.push(path_data.to_vec());
            continue;
        }

        let path = wrap_path(path_data);

        // alpha is where this parent path sits among all paths, from 0 at the first to 1 at the last
        ctx.alpha = path_index as f32 / (path_count - 1).max(1) as f32;
        path_index += 1;
        let branch_amount = ctx.int(args[7]).max(1);

        let lowest_branch = ctx.float(args[4]);
        let highest_branch = ctx.float(args[5]);
        // the distance between two neighbouring branches along the path
        let step = (highest_branch - lowest_branch) / (branch_amount - 1).max(1) as f32;

        // each branch reads the rotation step itself, so a random node gives every
        // branch its own turn around the path
        let mut turn = 0.0;

        for i in 0..branch_amount {
            let a = i as f32 / (branch_amount - 1).max(1) as f32;

            let root_alpha = (a * (highest_branch - lowest_branch) + lowest_branch)
                .min(1.0)
                .max(0.0);
            ctx.alpha = root_alpha;

            let length = ctx.float(args[1]);
            let taper = ctx.float(args[11]);
            // a branch as long as the longest path gets the full resolution, shorter ones get
            // fewer points, no matter which layer they grow on
            let resolution = if reference_length > 0.0 {
                ((max_resolution as f32 * length / reference_length).round() as usize)
                    .clamp(4, max_resolution)
            } else {
                max_resolution
            };
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

            let rotation_angle = turn;
            turn += ctx.float(args[9]) * PI / 180.0;

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
            set_path_link(
                &mut branch_data,
                PathLink {
                    id: next_id,
                    parent_id: path.link.id,
                    parent_alpha: root_alpha + offset_single,
                },
            );
            next_id += 1;
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

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use nodarium_utils::{
        encode_float,
        geometry::{create_multiple_paths, path_link, wrap_multiple_paths},
        split_args,
    };

    fn run(stems: &[i32], amount: i32) -> Vec<i32> {
        __nodarium_execute_inner(&[
            stems,
            &[encode_float(0.8)],
            &[encode_float(0.8)],
            &[encode_float(0.0)],
            &[encode_float(0.2)],
            &[encode_float(1.0)],
            &[1],
            &[amount],
            &[8],
            &[encode_float(137.5)],
            &[encode_float(0.0)],
            &[encode_float(1.0)],
        ])
    }

    fn straight_stems(amount: usize) -> Vec<i32> {
        let mut stems = create_multiple_paths(amount, 5, 1);
        for stem in wrap_multiple_paths(&mut stems).iter_mut() {
            for i in 0..5 {
                stem.points[i * 4..i * 4 + 4].copy_from_slice(&[0.0, i as f32, 0.0, 0.1]);
            }
        }
        stems
    }

    #[test]
    fn branches_know_their_parent_and_get_new_ids() {
        let result = run(&straight_stems(2), 3);
        let links: Vec<_> = split_args(&result).iter().map(|p| path_link(p)).collect();

        assert_eq!(links.len(), 2 + 2 * 3);
        let mut ids: Vec<i32> = links.iter().map(|l| l.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), links.len(), "ids are not unique: {:?}", links);

        for link in links.iter().filter(|l| l.has_parent()) {
            assert!(link.parent_id == 0 || link.parent_id == 1);
            assert!((0.0..=1.0).contains(&link.parent_alpha));
        }
        assert_eq!(links.iter().filter(|l| l.has_parent()).count(), 6);
        assert_eq!(links.iter().filter(|l| l.parent_id == 0).count(), 3);
    }

    #[test]
    fn a_branch_of_a_branch_points_at_the_branch() {
        let first = run(&straight_stems(1), 2);
        let second = __nodarium_execute_inner(&[
            &first,
            &[encode_float(0.5)],
            &[encode_float(0.8)],
            &[encode_float(0.0)],
            &[encode_float(0.2)],
            &[encode_float(1.0)],
            &[1],
            &[2],
            &[8],
            &[encode_float(137.5)],
            &[encode_float(0.0)],
            &[encode_float(1.0)],
        ]);
        let links: Vec<_> = split_args(&second).iter().map(|p| path_link(p)).collect();

        // the stem has depth 1, so only its branches (depth 2) were branched again
        let ids: Vec<i32> = links.iter().map(|l| l.id).collect();
        let mut unique = ids.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), ids.len());
        for link in links.iter().filter(|l| l.has_parent()) {
            assert!(ids.contains(&link.parent_id));
        }
    }
}
