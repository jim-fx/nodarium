use myzel_core::{curl_turn, margin_points, pose_caps_of, Cap};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{concat_arg_vecs, decode_points, encode_points, split_args, Ctx};

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> (Vec<i32>, Vec<i32>) {
    let mut ctx = Ctx::default();
    let items = split_args(args[0]);
    let hinges = decode_points(args[1]);
    let count = items.len();
    let caps = Cap::all(&items, &hinges);
    // read once for every stipe, from the cap that reaches furthest: a random node gives the
    // whole cap one value, and caps on one stipe (the flesh and its ridges) curl alike
    let mut curls: Vec<(i32, f32, f32)> = Vec::new();
    for cap in &caps {
        let stipe = cap.frame.stipe_id;
        if curls.iter().any(|c| c.0 == stipe) {
            continue;
        }
        let outer = caps.iter().filter(|c| c.frame.stipe_id == stipe).max_by(|a, b| a.reach().total_cmp(&b.reach())).unwrap();
        let first = outer.rays.iter().find(|r| r.crosses).unwrap_or(&outer.rays[0]);
        ctx.set_path(items[first.path], first.path, count);
        ctx.seed_path(stipe);
        let amount = ctx.float(args[2]).clamp(0.0, 1.0);
        let reach = ctx.float(args[3]).clamp(0.0, 1.0);
        curls.push((stipe, amount, reach));
    }
    let posed = pose_caps_of(&caps, &items, |cap, k| {
        let &(_, amount, reach) = curls.iter().find(|c| c.0 == cap.frame.stipe_id)?;
        if amount <= 0.0 || reach <= 0.0 {
            return None;
        }
        Some(cap.bend(k, |s| curl_turn(s, amount, reach)))
    });
    let paths: Vec<&[i32]> = posed.iter().map(|p| p.as_slice()).collect();
    let margins = margin_points(&paths, &hinges);
    (concat_arg_vecs(posed), encode_points(&margins))
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use glam::Vec3;
    use myzel_core::testing::{cone, grow, place, points, steps, TestCap};
    use nodarium_utils::{concat_arg_vecs, decode_points, encode_float, split_args, Point};

    fn curl(cap: &TestCap, amount: f32, reach: f32) -> (Vec<Vec<i32>>, Vec<Point>) {
        let (rays, hinges) = grow(cap);
        let (paths, margins) =
            __nodarium_execute_inner(&[&concat_arg_vecs(rays), &hinges, &[encode_float(amount)], &[encode_float(reach)]]);
        (split_args(&paths).iter().map(|p| p.to_vec()).collect(), decode_points(&margins))
    }

    #[test]
    fn margin_curl_rolls_the_margin_under() {
        let (_, flat) = curl(&cone(), 0.0, 0.35);
        let (_, curled) = curl(&cone(), 1.0, 0.35);
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
    fn a_larger_reach_rolls_more_of_the_cap() {
        let (_, short) = curl(&cone(), 0.5, 0.2);
        let (_, long) = curl(&cone(), 0.5, 0.6);
        assert!(place(long[0].position).y < place(short[0].position).y);
    }

    #[test]
    fn curling_keeps_the_lengths_and_leaves_the_centre() {
        let (input, _) = grow(&cone());
        let (rays, _) = curl(&cone(), 0.8, 0.35);
        for (a, b) in rays.iter().zip(&input) {
            for (x, y) in steps(a).iter().zip(steps(b)) {
                assert!((x - y).abs() < 1e-5);
            }
        }
        // the inner part of a ray from the stipe stays where it was
        for (x, y) in points(&rays[0]).iter().zip(points(&input[0])).take(12) {
            assert!(x.distance(y) < 1e-7);
        }
        assert_eq!(curl(&cone(), 0.0, 0.35).0, input);
    }

    #[test]
    fn a_random_amount_rolls_the_whole_cap_alike() {
        use nodarium_utils::concat_args;
        let (rays, hinges) = grow(&cone());
        let amount = concat_args(vec![&[1], &[encode_float(0.2)], &[encode_float(1.0)], &[5]]);
        let (_, margins) = __nodarium_execute_inner(&[&concat_arg_vecs(rays), &hinges, &amount, &[encode_float(0.35)]]);
        let margins = decode_points(&margins);
        let (_, flat) = curl(&cone(), 0.0, 0.35);
        assert!(place(margins[0].position).x < place(flat[0].position).x);
        for m in &margins {
            assert!((place(m.position) - place(margins[0].position)).length() < 2e-4);
        }
    }

    #[test]
    fn ridges_joined_with_the_flesh_roll_under_with_it() {
        let (flesh, hinges) = grow(&cone());
        let ridges = myzel_core::testing::ridges();
        let joined = concat_arg_vecs(flesh.iter().chain(&ridges).cloned().collect());
        let (paths, _) = __nodarium_execute_inner(&[&joined, &hinges, &[encode_float(1.0)], &[encode_float(0.5)]]);
        let posed: Vec<Vec<i32>> = split_args(&paths).iter().map(|p| p.to_vec()).collect();
        let (posed_flesh, posed_ridges) = posed.split_at(flesh.len());
        assert_eq!(posed_flesh, curl(&cone(), 1.0, 0.5).0.as_slice());
        let gap = |ridge: &[i32], flesh: &[Vec<i32>]| {
            let tip = *points(ridge).last().unwrap();
            flesh.iter().flat_map(|f| points(f)).map(|p| p.distance(tip)).fold(f32::MAX, f32::min)
        };
        for (before, after) in ridges.iter().zip(posed_ridges) {
            let (b, a) = (gap(before, &flesh), gap(after, posed_flesh));
            assert!((a - b).abs() < 0.3 * b + 3e-4, "a ridge tip {b} under the flesh moves to {a}");
        }
    }
}
