use glam::Vec3;
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{
    encode_points,
    geometry::{interpolate_along_path, rotate_vector_by_angle, wrap_path, DeepestPaths},
    split_args, Ctx, Point,
};
use std::f32::consts::PI;

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let mut ctx = Ctx::default();
    let paths = split_args(args[0]);
    let depth = ctx.int(args[6]);

    let deepest = DeepestPaths::new(&paths, depth);
    let selected: Vec<&[i32]> = paths
        .iter()
        .copied()
        .filter(|p| deepest.contains(p))
        .collect();

    let mut points = Vec::new();

    for (path_index, path_data) in selected.iter().enumerate() {
        let path = wrap_path(path_data);
        if path.length < 2 {
            continue;
        }

        // alpha is where this path sits among all paths, from 0 at the first to 1 at the last
        ctx.set_path(path_data, path_index, selected.len());
        let amount = ctx.int(args[1]).max(0);
        let lowest = ctx.float(args[2]);
        let highest = ctx.float(args[3]);
        // the distance between two neighbouring points along the path
        let step = (highest - lowest) / (amount - 1).max(1) as f32;

        // each point reads the rotation step itself, so a random node gives every point its own
        // turn around the path
        let mut turn = 0.0;

        for i in 0..amount {
            let a = i as f32 / (amount - 1).max(1) as f32;
            let root_alpha = (lowest + a * (highest - lowest)).clamp(0.0, 1.0);
            ctx.alpha = root_alpha;

            // every even point moves up by a part of the step, 1 puts it at the height of the
            // next one, so the points come in pairs
            let offset = if i % 2 == 0 {
                ctx.float(args[5]) * step
            } else {
                0.0
            };
            let alpha = (root_alpha + offset).clamp(0.0, 1.0);

            let (position, orthogonal, tangent) = interpolate_along_path(path.points, alpha);
            let direction = rotate_vector_by_angle(orthogonal, tangent, turn);
            turn += ctx.float(args[4]) * PI / 180.0;

            points.push(Point {
                position: Vec3::new(position.x, position.y, position.z),
                direction,
                tangent,
                scale: position.w,
                parent_id: path.link.id,
                alpha,
                depth: path.depth,
                index: i,
            });
        }
    }

    encode_points(&points)
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use nodarium_utils::{
        decode_points, encode_float,
        geometry::{create_multiple_paths, path_link, wrap_multiple_paths},
        split_args,
    };

    fn straight_stems(amount: usize) -> Vec<i32> {
        straight_paths(amount, 1)
    }

    fn straight_paths(amount: usize, depth: i32) -> Vec<i32> {
        let mut stems = create_multiple_paths(amount, 5, depth, 3);
        for (s, stem) in wrap_multiple_paths(&mut stems).iter_mut().enumerate() {
            for i in 0..5 {
                stem.points[i * 4..i * 4 + 4]
                    .copy_from_slice(&[s as f32, i as f32, 0.0, 0.1]);
            }
        }
        stems
    }

    fn run(paths: &[i32], amount: i32, rotation: f32) -> Vec<i32> {
        run_with(paths, amount, rotation, 0.0, 1)
    }

    fn run_with(paths: &[i32], amount: i32, rotation: f32, offset: f32, depth: i32) -> Vec<i32> {
        __nodarium_execute_inner(&[
            paths,
            &[amount],
            &[encode_float(0.0)],
            &[encode_float(1.0)],
            &[encode_float(rotation)],
            &[encode_float(offset)],
            &[depth],
        ])
    }

    #[test]
    fn every_path_gets_the_amount_of_points_spread_from_lowest_to_highest() {
        let stems = straight_stems(2);
        let ids: Vec<i32> = split_args(&stems).iter().map(|p| path_link(p).id).collect();
        let points = decode_points(&run(&stems, 3, 180.0));

        assert_eq!(points.len(), 6);
        for (i, point) in points.iter().enumerate() {
            assert_eq!(point.parent_id, ids[i / 3]);
            assert_eq!(point.index, (i % 3) as i32);
            assert_eq!(point.depth, 1);
            assert!((point.alpha - [0.0, 0.5, 1.0][i % 3]).abs() < 1e-5);
            // the stems are 4 long and stand at x = 0 and x = 1
            assert!((point.position.y - point.alpha * 4.0).abs() < 1e-4);
            assert!((point.position.x - (i / 3) as f32).abs() < 1e-5);
            assert!((point.scale - 0.1).abs() < 1e-6);
        }
    }

    #[test]
    fn the_direction_points_away_from_the_path_and_turns_with_the_rotation() {
        let points = decode_points(&run(&straight_stems(1), 3, 180.0));

        for point in &points {
            assert!(point.direction.is_normalized());
            assert!(point.direction.dot(point.tangent).abs() < 1e-5);
        }
        // 180 degrees puts every point on the other side of the one before it
        assert!((points[0].direction + points[1].direction).length() < 1e-5);
        assert!((points[0].direction - points[2].direction).length() < 1e-5);
    }

    #[test]
    fn a_distance_over_the_path_length_holds_a_point_while_the_path_grows() {
        // math(divide, 1.5, path_info(length)) into lowest and highest point, like an anchor
        // that stays 1.5 from the base of the stem
        let length = nodarium_utils::concat_args(vec![&[5], &[5]]);
        let share = nodarium_utils::concat_args(vec![&[0], &[3], &[encode_float(1.5)], &length]);
        for height in [2.0, 3.0, 4.0] {
            let mut stem = create_multiple_paths(1, 5, 1, 3);
            for (i, p) in wrap_multiple_paths(&mut stem)[0].points.chunks_exact_mut(4).enumerate() {
                p.copy_from_slice(&[0.0, height * i as f32 / 4.0, 0.0, 0.1]);
            }
            let points = decode_points(&__nodarium_execute_inner(&[
                &stem,
                &[1],
                &share,
                &share,
                &[encode_float(0.0)],
                &[encode_float(0.0)],
                &[1],
            ]));
            assert_eq!(points.len(), 1);
            assert!((points[0].position.y - 1.5).abs() < 1e-4, "at {} on a stem {height} tall", points[0].position.y);
        }
    }

    #[test]
    fn no_paths_give_no_points() {
        let empty = nodarium_utils::concat_arg_vecs(vec![]);
        assert!(decode_points(&run(&empty, 3, 0.0)).is_empty());
    }

    #[test]
    fn offset_single_moves_every_second_point_up_to_the_next_one() {
        let points = decode_points(&run_with(&straight_stems(1), 4, 0.0, 1.0, 1));
        let alphas: Vec<f32> = points.iter().map(|p| p.alpha).collect();
        let expected = [1.0 / 3.0, 1.0 / 3.0, 1.0, 1.0];
        for (alpha, expected) in alphas.iter().zip(expected) {
            assert!((alpha - expected).abs() < 1e-5, "{:?}", alphas);
        }
    }

    #[test]
    fn depth_picks_how_many_of_the_deepest_layers_get_points() {
        let paths = nodarium_utils::concat_arg_vecs(
            [straight_paths(1, 1), straight_paths(2, 2)]
                .iter()
                .flat_map(|p| split_args(p).into_iter().map(|p| p.to_vec()).collect::<Vec<_>>())
                .collect(),
        );

        let deepest = decode_points(&run_with(&paths, 2, 0.0, 0.0, 1));
        assert_eq!(deepest.len(), 4);
        assert!(deepest.iter().all(|p| p.depth == 2));

        let both = decode_points(&run_with(&paths, 2, 0.0, 0.0, 2));
        assert_eq!(both.len(), 6);
    }
}
