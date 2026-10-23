use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;
use nodarium_utils::{
    concat_arg_vecs, concat_args, encode_float, geometry::wrap_path, int, split_args, Ctx,
};

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let mut ctx = Ctx::default();

    let paths = split_args(args[0]);
    let probability = ctx.float(args[1]).clamp(0.0, 1.0);
    let seed = int(args[2]);
    let depth = ctx.int(args[3]).max(1);
    let lowest_snap = ctx.float(args[4]);
    let highest_snap = ctx.float(args[5]);

    // The snap range is one range, so a reversed pair of inputs is swapped
    // instead of panicking in the clamp below.
    let (lowest_snap, highest_snap) = if lowest_snap <= highest_snap {
        (lowest_snap, highest_snap)
    } else {
        (highest_snap, lowest_snap)
    };

    let mut max_depth = 0;
    for path_data in &paths {
        if path_data[2] != 0 {
            continue;
        }
        max_depth = max_depth.max(path_data[3]);
    }

    // Paths at this depth or deeper get snapped. Once the plant has branches
    // the trunk (depth 0) never snaps, even when depth is set beyond the
    // deepest layer, but a lone stem is the deepest layer and still snaps.
    let snap_threshold = if max_depth == 0 {
        0
    } else {
        (max_depth - depth + 1).max(1)
    };

    let coin_flip_expr = concat_args(vec![&[1], &[0], &[encode_float(1.0)], &[seed]]);
    let snap_pos_expr = concat_args(vec![
        &[1],
        &[encode_float(lowest_snap)],
        &[encode_float(highest_snap)],
        &[seed],
    ]);

    let mut output: Vec<Vec<i32>> = Vec::new();

    for &path_data in &paths {
        // Only paths (node type 0) can be snapped; pass everything else through
        if path_data[2] != 0 {
            output.push(path_data.to_vec());
            continue;
        }

        // Only snap paths at the selected depth layer
        if path_data[3] < snap_threshold {
            output.push(path_data.to_vec());
            continue;
        }

        // The random values come from a noise function that can leave [0, 1],
        // so the boundaries are handled exactly: probability 0 never snaps,
        // probability 1 always snaps.
        let selected = if probability <= 0.0 {
            false
        } else if probability >= 1.0 {
            true
        } else {
            ctx.float(&coin_flip_expr).clamp(0.0, 1.0) < probability
        };
        if !selected {
            output.push(path_data.to_vec());
            continue;
        }

        // Truncate the path at a random point along its length
        let snap_alpha = ctx.float(&snap_pos_expr).clamp(lowest_snap, highest_snap);
        let path = wrap_path(path_data);
        let point_count = path.length;

        if point_count < 2 {
            output.push(path_data.to_vec());
            continue;
        }

        let snap_idx = (snap_alpha * (point_count - 1) as f32).floor() as usize;
        let new_point_count = snap_idx + 2;

        if new_point_count > point_count {
            // The snap point is the tip itself, there is nothing to remove
            output.push(path_data.to_vec());
            continue;
        }

        let snap_point = path.get_point_at(snap_alpha);

        // Build the truncated path: header + kept points + interpolated snap point + closing bracket
        let header_end = path_data.len() - path.length * 4 - 2;
        let mut new_path = Vec::with_capacity(header_end + new_point_count * 4 + 2);
        new_path.push(0); // opening bracket
        new_path.push((header_end - 2 + new_point_count * 4 + 1) as i32); // inner count

        // node type, depth and the link to the parent stay as they are
        new_path.extend_from_slice(&path_data[2..header_end]);

        // Copy original points 0..=snap_idx
        for i in 0..=snap_idx {
            let base = header_end + i * 4;
            new_path.extend_from_slice(&path_data[base..base + 4]);
        }

        // Append the interpolated point at the snap position
        new_path.extend(snap_point.iter().map(|&c| encode_float(c)));

        new_path.push(1); // closing bracket
        new_path.push(1); // closing count

        output.push(new_path);
    }

    concat_arg_vecs(output)
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use nodarium_utils::{
        concat_args, decode_float, encode_float,
        geometry::{create_multiple_paths, wrap_multiple_paths},
        split_args,
    };

    fn make_paths(amount: usize, point_count: usize, depth: i32) -> Vec<i32> {
        let mut data = create_multiple_paths(amount, point_count, depth, 0);
        for stem in wrap_multiple_paths(&mut data).iter_mut() {
            for i in 0..stem.points.len() / 4 {
                stem.points[i * 4] = 0.0;
                stem.points[i * 4 + 1] = i as f32;
                stem.points[i * 4 + 2] = 0.0;
                stem.points[i * 4 + 3] = 0.1;
            }
        }
        data
    }

    fn path_point_count(path_data: &[i32]) -> usize {
        ((path_data[1] - 6) / 4) as usize
    }

    #[test]
    fn test_probability_zero_keeps_all_unchanged() {
        let paths = make_paths(10, 20, 0);
        let result = __nodarium_execute_inner(&[
            &paths,
            &[encode_float(0.0)],
            &[42],
            &[1],
            &[encode_float(0.0)],
            &[encode_float(1.0)],
        ]);
        let output_paths = split_args(&result);
        assert_eq!(output_paths.len(), 10);
        for (i, p) in output_paths.iter().enumerate() {
            assert_eq!(path_point_count(p), 20, "path {} should have 20 points", i);
        }
    }

    #[test]
    fn test_probability_one_snaps_all_at_selected_depth() {
        let paths = make_paths(10, 20, 0);
        let result = __nodarium_execute_inner(&[
            &paths,
            &[encode_float(1.0)],
            &[42],
            &[1],
            &[encode_float(0.0)],
            &[encode_float(0.5)],
        ]);
        let output_paths = split_args(&result);
        assert_eq!(output_paths.len(), 10);
        for (i, p) in output_paths.iter().enumerate() {
            assert!(
                path_point_count(p) < 20,
                "path {} should be truncated, got {} points",
                i,
                path_point_count(p)
            );
        }
    }

    #[test]
    fn test_snap_range_limits_truncation() {
        let paths = make_paths(5, 20, 0);
        // highestSnap=0.25 means every snap happens in the lower quarter,
        // so the remaining path should never exceed ~6 points.
        let result = __nodarium_execute_inner(&[
            &paths,
            &[encode_float(1.0)],
            &[42],
            &[1],
            &[encode_float(0.0)],
            &[encode_float(0.25)],
        ]);
        let output_paths = split_args(&result);
        assert_eq!(output_paths.len(), 5);
        for (i, p) in output_paths.iter().enumerate() {
            assert!(
                path_point_count(p) <= 6,
                "path {} should have at most 6 points with highestSnap=0.25, got {}",
                i,
                path_point_count(p)
            );
        }
    }

    #[test]
    fn test_does_not_snap_shallower_paths() {
        let shallow = make_paths(10, 10, 0);
        let deep = make_paths(10, 10, 1);
        let all_paths = {
            let shallow_paths = split_args(&shallow);
            let deep_paths = split_args(&deep);
            let mut all: Vec<&[i32]> = Vec::new();
            all.extend(shallow_paths);
            all.extend(deep_paths);
            concat_args(all)
        };

        // depth=1 only snaps the deepest layer (depth 1)
        let result = __nodarium_execute_inner(&[
            &all_paths,
            &[encode_float(1.0)],
            &[42],
            &[1],
            &[encode_float(0.0)],
            &[encode_float(0.5)],
        ]);
        let output_paths = split_args(&result);
        assert_eq!(output_paths.len(), 20);

        let shallow_remaining: Vec<_> = output_paths.iter().filter(|p| p[3] == 0).collect();
        let deep_remaining: Vec<_> = output_paths.iter().filter(|p| p[3] == 1).collect();

        assert_eq!(
            shallow_remaining.len(),
            10,
            "shallow paths should be untouched"
        );
        for p in &shallow_remaining {
            assert_eq!(path_point_count(p), 10);
        }

        assert_eq!(
            deep_remaining.len(),
            10,
            "all deep paths should still exist"
        );
        for p in &deep_remaining {
            assert!(
                path_point_count(p) < 10,
                "deep paths should be truncated, got {} points",
                path_point_count(p)
            );
        }
    }

    #[test]
    fn test_deterministic_with_same_seed() {
        let paths = make_paths(20, 20, 0);
        let result_a = __nodarium_execute_inner(&[
            &paths,
            &[encode_float(0.5)],
            &[123],
            &[1],
            &[encode_float(0.0)],
            &[encode_float(1.0)],
        ]);
        let result_b = __nodarium_execute_inner(&[
            &paths,
            &[encode_float(0.5)],
            &[123],
            &[1],
            &[encode_float(0.0)],
            &[encode_float(1.0)],
        ]);
        assert_eq!(result_a, result_b);
    }

    fn tip_y(path_data: &[i32]) -> f32 {
        decode_float(path_data[path_data.len() - 5])
    }

    #[test]
    fn test_snaps_in_the_last_segment() {
        // A snap position in the last segment used to keep the whole path
        let paths = make_paths(10, 20, 0);
        // lowestSnap == highestSnap pins the snap position exactly,
        // independent of the seed
        let result = __nodarium_execute_inner(&[
            &paths,
            &[encode_float(1.0)],
            &[42],
            &[1],
            &[encode_float(0.95)],
            &[encode_float(0.95)],
        ]);
        let output_paths = split_args(&result);
        assert_eq!(output_paths.len(), 10);
        for (i, p) in output_paths.iter().enumerate() {
            assert_eq!(path_point_count(p), 20, "path {}", i);
            assert!(
                (18.0..19.0).contains(&tip_y(p)),
                "path {} should end at the snap point, got tip {}",
                i,
                tip_y(p)
            );
        }
    }

    #[test]
    fn test_two_point_paths_snap() {
        // Two-point paths could never be snapped at all
        let paths = make_paths(10, 2, 0);
        let result = __nodarium_execute_inner(&[
            &paths,
            &[encode_float(1.0)],
            &[42],
            &[1],
            &[encode_float(0.5)],
            &[encode_float(0.5)],
        ]);
        let output_paths = split_args(&result);
        assert_eq!(output_paths.len(), 10);
        for (i, p) in output_paths.iter().enumerate() {
            assert_eq!(path_point_count(p), 2, "path {}", i);
            assert!(
                (tip_y(p) - 0.5).abs() < 1e-6,
                "path {} should end at the snap point, got tip {}",
                i,
                tip_y(p)
            );
        }
    }

    #[test]
    fn test_probability_boundaries_across_seeds() {
        // The random values come from a noise function and can leave [0, 1],
        // so the boundaries need exact handling
        let paths = make_paths(20, 20, 0);
        for seed in 0..50 {
            let kept = __nodarium_execute_inner(&[
                &paths,
                &[encode_float(0.0)],
                &[seed],
                &[1],
                &[encode_float(0.0)],
                &[encode_float(0.5)],
            ]);
            for (i, p) in split_args(&kept).iter().enumerate() {
                assert_eq!(
                    path_point_count(p),
                    20,
                    "probability 0 must never snap (seed {}, path {})",
                    seed,
                    i
                );
            }

            let snapped = __nodarium_execute_inner(&[
                &paths,
                &[encode_float(1.0)],
                &[seed],
                &[1],
                &[encode_float(0.0)],
                &[encode_float(0.5)],
            ]);
            for (i, p) in split_args(&snapped).iter().enumerate() {
                assert!(
                    path_point_count(p) < 20,
                    "probability 1 must always snap (seed {}, path {})",
                    seed,
                    i
                );
            }
        }
    }

    #[test]
    fn test_reversed_snap_range_is_swapped() {
        let paths = make_paths(20, 20, 0);
        let result = __nodarium_execute_inner(&[
            &paths,
            &[encode_float(1.0)],
            &[42],
            &[1],
            &[encode_float(0.9)],
            &[encode_float(0.1)],
        ]);
        let output_paths = split_args(&result);
        assert_eq!(output_paths.len(), 20);
        for (i, p) in output_paths.iter().enumerate() {
            assert!(
                path_point_count(p) < 20,
                "path {} should be truncated despite the reversed range",
                i
            );
        }
    }

    #[test]
    fn test_depth_beyond_the_plant_protects_the_trunk() {
        let shallow = make_paths(5, 10, 0);
        let deep = make_paths(5, 10, 1);
        let all_paths = {
            let mut all: Vec<&[i32]> = Vec::new();
            all.extend(split_args(&shallow));
            all.extend(split_args(&deep));
            concat_args(all)
        };

        // depth=5 is beyond the two layers of this plant, the trunk must survive
        let result = __nodarium_execute_inner(&[
            &all_paths,
            &[encode_float(1.0)],
            &[42],
            &[5],
            &[encode_float(0.0)],
            &[encode_float(0.5)],
        ]);
        let output_paths = split_args(&result);
        assert_eq!(output_paths.len(), 10);
        for p in &output_paths {
            if p[3] == 0 {
                assert_eq!(path_point_count(p), 10, "trunk must not be snapped");
            } else {
                assert!(path_point_count(p) < 10, "deep paths should be truncated");
            }
        }
    }

    #[test]
    fn test_lone_stem_still_snaps() {
        // A plant without branches has its stem as the deepest layer
        let paths = make_paths(10, 20, 0);
        let result = __nodarium_execute_inner(&[
            &paths,
            &[encode_float(1.0)],
            &[42],
            &[1],
            &[encode_float(0.0)],
            &[encode_float(0.5)],
        ]);
        for (i, p) in split_args(&result).iter().enumerate() {
            assert!(path_point_count(p) < 20, "path {} should be truncated", i);
        }
    }
}
