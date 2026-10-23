use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;
use nodarium_utils::{
    concat_arg_vecs,
    geometry::{
        create_path, interpolate_along_path, rotate_vector_by_angle, set_path_link, wrap_path,
        wrap_path_mut, PathLink,
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
    // the graph node that makes the branches, it is the last input
    let creator = int(args[12]);

    for path_data in paths.iter() {
        // if this is not a path ignore it
        if path_data[2] != 0 || path_data[3] < (max_depth - depth + 1) {
            output.push(path_data.to_vec());
            continue;
        }

        let path = wrap_path(path_data);

        // alpha is where this parent path sits among all paths, from 0 at the first to 1 at the last
        ctx.set_path(path_data, path_index, path_count);
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
                // the id only depends on this node, the parent and the number of the branch, so
                // it stays when the amount of other branches or paths changes. A skipped branch
                // leaves a gap instead of renumbering the ones after it.
                PathLink::child(creator, path.link.id, root_alpha + offset_single, i),
            );
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
        geometry::{create_multiple_paths, path_link, wrap_multiple_paths, PathLink},
        split_args,
    };

    const STEM_NODE: i32 = 3;
    const BRANCH_NODE: i32 = 9;

    fn run_as(creator: i32, paths: &[i32], amount: i32, rotation: f32) -> Vec<i32> {
        __nodarium_execute_inner(&[
            paths,
            &[encode_float(0.8)],
            &[encode_float(0.8)],
            &[encode_float(0.0)],
            &[encode_float(0.2)],
            &[encode_float(1.0)],
            &[1],
            &[amount],
            &[8],
            &[encode_float(rotation)],
            &[encode_float(0.0)],
            &[encode_float(1.0)],
            &[creator],
        ])
    }

    fn run(stems: &[i32], amount: i32) -> Vec<i32> {
        run_as(BRANCH_NODE, stems, amount, 137.5)
    }

    fn straight_stems(amount: usize) -> Vec<i32> {
        let mut stems = create_multiple_paths(amount, 5, 1, STEM_NODE);
        for stem in wrap_multiple_paths(&mut stems).iter_mut() {
            for i in 0..5 {
                stem.points[i * 4..i * 4 + 4].copy_from_slice(&[0.0, i as f32, 0.0, 0.1]);
            }
        }
        stems
    }

    fn links(result: &[i32]) -> Vec<PathLink> {
        split_args(result).iter().map(|p| path_link(p)).collect()
    }

    #[test]
    fn branches_know_their_parent_and_get_new_ids() {
        let stems = straight_stems(2);
        let stem_ids: Vec<i32> = links(&stems).iter().map(|l| l.id).collect();
        let links = links(&run(&stems, 3));

        assert_eq!(links.len(), 2 + 2 * 3);
        let mut ids: Vec<i32> = links.iter().map(|l| l.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), links.len(), "ids are not unique: {:?}", links);

        for link in links.iter().filter(|l| l.has_parent()) {
            assert!(stem_ids.contains(&link.parent_id));
            assert!((0.0..=1.0).contains(&link.parent_alpha));
        }
        assert_eq!(links.iter().filter(|l| l.has_parent()).count(), 6);
        assert_eq!(
            links.iter().filter(|l| l.parent_id == stem_ids[0]).count(),
            3
        );
    }

    #[test]
    fn branches_know_who_made_them_and_which_one_they_are() {
        let links = links(&run(&straight_stems(2), 3));

        let stems: Vec<_> = links.iter().filter(|l| !l.has_parent()).collect();
        assert!(stems.iter().all(|l| l.creator == STEM_NODE));

        for parent in stems {
            let mut indices: Vec<i32> = links
                .iter()
                .filter(|l| l.parent_id == parent.id)
                .map(|l| {
                    assert_eq!(l.creator, BRANCH_NODE);
                    l.index
                })
                .collect();
            indices.sort();
            assert_eq!(indices, vec![0, 1, 2]);
        }
    }

    #[test]
    fn the_ids_of_a_stem_do_not_depend_on_the_other_stems_or_the_amount() {
        let branches_of_first = |stems: usize, amount: i32| {
            let stems = straight_stems(stems);
            let first = links(&stems)[0].id;
            let mut ids: Vec<i32> = links(&run(&stems, amount))
                .iter()
                .filter(|l| l.parent_id == first)
                .map(|l| l.id)
                .collect();
            ids.sort();
            ids
        };

        let few = branches_of_first(1, 3);
        // more stems and more branches, the branches that were there keep their ids
        let many = branches_of_first(4, 5);

        assert_eq!(few.len(), 3);
        assert_eq!(many.len(), 5);
        assert!(few.iter().all(|id| many.contains(id)), "{:?} {:?}", few, many);
    }

    #[test]
    fn two_branch_nodes_on_the_same_path_do_not_share_ids() {
        let stems = straight_stems(1);
        let first = run_as(1, &stems, 2, 137.5);
        let second = run_as(2, &first, 2, 137.5);
        let links = links(&second);

        let mut ids: Vec<i32> = links.iter().map(|l| l.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), links.len());
    }

    #[test]
    fn a_branch_of_a_branch_points_at_the_branch() {
        let first = run(&straight_stems(1), 2);
        let second = run_as(4, &first, 2, 137.5);
        let links = links(&second);

        // the stem has depth 1, so only its branches (depth 2) were branched again
        let ids: Vec<i32> = links.iter().map(|l| l.id).collect();
        let mut unique = ids.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), ids.len());
        for link in links.iter().filter(|l| l.has_parent()) {
            assert!(ids.contains(&link.parent_id));
        }
        assert_eq!(links.iter().filter(|l| l.creator == 4).count(), 4);
    }
}
