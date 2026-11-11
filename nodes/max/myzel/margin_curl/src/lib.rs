use myzel_core::{curl_spread, pose_caps_of, AlongRay, Cap};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{concat_arg_vecs, split_args, Ctx};
use std::f32::consts::PI;

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let mut ctx = Ctx::default();
    let items = split_args(args[0]);
    let count = items.len();
    let caps = Cap::all(&items);
    // read once for every cap: a random node gives the whole cap one value
    let amounts: Vec<AlongRay> = caps
        .iter()
        .map(|cap| {
            let first = cap.rays.iter().find(|r| r.crosses).unwrap_or(&cap.rays[0]);
            ctx.set_path(items[first.path], first.path, count);
            ctx.seed_path(cap.frame.stipe_id);
            AlongRay::read(&mut ctx, args[1], curl_spread)
        })
        .collect();
    let posed = pose_caps_of(&caps, &items, |cap, k| {
        let amount = &amounts[caps.iter().position(|c| std::ptr::eq(c, cap))?];
        if amount.is_zero() {
            return None;
        }
        Some(cap.bend(k, |s| PI * amount.at(s.alpha()).clamp(0.0, 1.0)))
    });
    concat_arg_vecs(posed)
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use glam::Vec3;
    use myzel_core::{
        curl_spread, curl_turn, pose_caps,
        testing::{cone, grow, list, margins, place, points, rays, steps, unlist, TestCap},
    };
    use nodarium_utils::{concat_args, encode_float, Point};

    fn curl_with(paths: &[Vec<i32>], amount: &[i32]) -> Vec<Vec<i32>> {
        unlist(&__nodarium_execute_inner(&[&list(paths), amount]))
    }

    fn curl(cap: &TestCap, amount: f32) -> (Vec<Vec<i32>>, Vec<Point>) {
        let posed = curl_with(&grow(cap), &[encode_float(amount)]);
        let margin = margins(&posed);
        (posed, margin)
    }

    /// A curve node without a value, which reads alpha, through `points` from 0 to `max`
    fn curve(points: &[f32], max: f32) -> Vec<i32> {
        let mut args: Vec<Vec<i32>> =
            vec![vec![3], concat_args(vec![&[2], &[0]]), vec![encode_float(0.0)], vec![encode_float(max)]];
        args.extend(points.iter().map(|&v| vec![encode_float(v)]));
        concat_args(args.iter().map(|a| a.as_slice()).collect())
    }

    fn largest_distance(a: &[Vec<i32>], b: &[Vec<i32>]) -> f32 {
        a.iter()
            .zip(b)
            .flat_map(|(a, b)| points(a).into_iter().zip(points(b)).map(|(x, y)| x.distance(y)))
            .fold(0.0, f32::max)
    }

    #[test]
    fn margin_curl_rolls_the_margin_under() {
        let (_, flat) = curl(&cone(), 0.0);
        let (_, curled) = curl(&cone(), 1.0);
        let reach = |m: &[Point]| place(m[0].position).x;
        assert!(reach(&curled) < reach(&flat));
        // rolled under, the margin points back towards the stipe
        let outward = |m: &Point| Vec3::new(m.position.x, 0.0, m.position.z).normalize();
        assert!(curled[0].tangent.dot(outward(&curled[0])) < 0.0);
        assert!(flat[0].tangent.dot(outward(&flat[0])) > 0.0);
        // all the way round, added rays too
        for m in &curled {
            assert!((place(m.position) - place(curled[0].position)).length() < 2e-4);
        }
    }

    #[test]
    fn a_plain_amount_rolls_the_outer_part_as_curl_turn_does() {
        let input = grow(&cone());
        let refs: Vec<&[i32]> = input.iter().map(|p| p.as_slice()).collect();
        let expected = pose_caps(&refs, |cap, k| Some(cap.bend(k, |s| curl_turn(s, 0.6))));
        let apart = largest_distance(&curl_with(&input, &[encode_float(0.6)]), &expected);
        assert!(apart < 1e-7, "the caps are {apart} apart");
    }

    #[test]
    fn a_curve_that_draws_the_spread_gives_the_curl_of_a_plain_amount() {
        let input = grow(&cone());
        let points: Vec<f32> = (0..=40).flat_map(|i| [i as f32 / 40.0, curl_spread(i as f32 / 40.0)]).collect();
        let by_curve = curl_with(&input, &curve(&points, 0.6));
        let plain = curl_with(&input, &[encode_float(0.6)]);
        let apart = largest_distance(&by_curve, &plain);
        assert!(apart < 2e-3 * cone().radius, "the caps are {apart} apart");
    }

    #[test]
    fn the_curve_of_the_graph_migration_gives_the_old_curl() {
        // smoothstep over the outer 0.35 at nine points, as graph version 12 draws it
        let points = [
            0.0, 0.0, 0.65, 0.0, 0.69375, 0.042969, 0.7375, 0.15625, 0.78125, 0.316406, 0.825, 0.5, 0.86875, 0.683594, 0.9125,
            0.84375, 0.95625, 0.957031, 1.0, 1.0,
        ];
        let input = grow(&cone());
        for amount in [0.2, 0.5, 1.0] {
            let by_curve = curl_with(&input, &curve(&points, amount));
            let plain = curl_with(&input, &[encode_float(amount)]);
            let apart = largest_distance(&by_curve, &plain);
            assert!(apart < 3e-3 * cone().radius, "at {amount} the caps are {apart} apart");
        }
    }

    #[test]
    fn a_curve_that_rises_from_further_in_rolls_more_of_the_cap() {
        let input = grow(&cone());
        let short = margins(&curl_with(&input, &curve(&[0.0, 0.0, 0.8, 0.0, 1.0, 1.0], 0.5)));
        let long = margins(&curl_with(&input, &curve(&[0.0, 0.0, 0.4, 0.0, 1.0, 1.0], 0.5)));
        assert!(place(long[0].position).y < place(short[0].position).y);
    }

    #[test]
    fn curling_keeps_the_lengths_and_leaves_the_centre() {
        let input = grow(&cone());
        let (posed, _) = curl(&cone(), 0.8);
        for (a, b) in posed.iter().zip(&input) {
            for (x, y) in steps(a).iter().zip(steps(b)) {
                assert!((x - y).abs() < 1e-5);
            }
        }
        // the inner part of a ray from the stipe stays where it was
        for (x, y) in points(&posed[0]).iter().zip(points(&input[0])).take(12) {
            assert!(x.distance(y) < 1e-7);
        }
        assert_eq!(curl(&cone(), 0.0).0, input);
        assert_eq!(curl_with(&input, &curve(&[0.0, 0.0, 1.0, 1.0], 0.0)), input);
    }

    #[test]
    fn a_connected_amount_keeps_the_lengths() {
        let input = grow(&cone());
        let posed = curl_with(&input, &curve(&[0.0, 0.0, 0.3, 0.4, 1.0, 0.9], 1.0));
        assert_ne!(posed, input);
        for (a, b) in posed.iter().zip(&input) {
            for (x, y) in steps(a).iter().zip(steps(b)) {
                assert!((x - y).abs() < 1e-5);
            }
        }
    }

    #[test]
    fn rays_without_their_stipe_pass_unchanged() {
        let only_rays = rays(&grow(&cone()));
        assert_eq!(curl_with(&only_rays, &[encode_float(0.8)]), only_rays);
    }

    #[test]
    fn a_random_amount_rolls_the_whole_cap_alike() {
        let (_, flat) = curl(&cone(), 0.0);
        let drawn = concat_args(vec![&[1], &[encode_float(0.2)], &[encode_float(1.0)], &[5]]);
        let per_path = concat_args(vec![&[1], &[encode_float(0.2)], &[encode_float(1.0)], &[5], &[1], &[1], &[0]]);
        for random in [drawn, per_path] {
            // a curve whose max is the random, read along the ray
            let mut args: Vec<Vec<i32>> = vec![vec![3], concat_args(vec![&[2], &[0]]), vec![encode_float(0.0)], random.clone()];
            args.extend([0.0, 0.0, 0.6, 0.0, 1.0, 1.0].iter().map(|&v| vec![encode_float(v)]));
            let shaped = concat_args(args.iter().map(|a| a.as_slice()).collect());
            for amount in [random, shaped] {
                let margin = margins(&curl_with(&grow(&cone()), &amount));
                assert!(place(margin[0].position).x < place(flat[0].position).x);
                for m in &margin {
                    assert!((place(m.position) - place(margin[0].position)).length() < 2e-4);
                }
            }
        }
    }
}
