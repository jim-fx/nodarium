use myzel_core::{curl_turn, pose_caps_of, Cap};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{concat_arg_vecs, geometry::is_path, split_args, Ctx};

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let mut ctx = Ctx::default();
    let items = split_args(args[0]);
    let count = items.iter().filter(|item| is_path(item)).count();
    let caps = Cap::all(&items);
    // read once for every cap: a random node gives the whole cap one value
    let curls: Vec<(f32, f32)> = caps
        .iter()
        .map(|cap| {
            let first = cap.rays.iter().find(|r| r.crosses).unwrap_or(&cap.rays[0]);
            ctx.set_path(items[first.path], first.path, count);
            ctx.seed_path(cap.frame.stipe_id);
            (ctx.float(args[1]).clamp(0.0, 1.0), ctx.float(args[2]).clamp(0.0, 1.0))
        })
        .collect();
    let posed = pose_caps_of(&caps, &items, |cap, k| {
        let (amount, reach) = curls[caps.iter().position(|c| std::ptr::eq(c, cap))?];
        if amount <= 0.0 || reach <= 0.0 {
            return None;
        }
        Some(cap.bend(k, |s| curl_turn(s, amount, reach)))
    });
    concat_arg_vecs(posed)
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use glam::Vec3;
    use myzel_core::{
        margin_points,
        testing::{cone, grow, list, paths, place, points, steps, TestCap},
    };
    use nodarium_utils::{concat_arg_vecs, encode_float, split_args, Point};

    fn curl(cap: &TestCap, amount: f32, reach: f32) -> (Vec<Vec<i32>>, Vec<Point>) {
        let (rays, item) = grow(cap);
        let output = __nodarium_execute_inner(&[&list(&rays, &item), &[encode_float(amount)], &[encode_float(reach)]]);
        (paths(&output), margin_points(&split_args(&output)))
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
        let (rays, item) = grow(&cone());
        let amount = concat_args(vec![&[1], &[encode_float(0.2)], &[encode_float(1.0)], &[5]]);
        let output = __nodarium_execute_inner(&[&list(&rays, &item), &amount, &[encode_float(0.35)]]);
        let margins = margin_points(&split_args(&output));
        let (_, flat) = curl(&cone(), 0.0, 0.35);
        assert!(place(margins[0].position).x < place(flat[0].position).x);
        for m in &margins {
            assert!((place(m.position) - place(margins[0].position)).length() < 2e-4);
        }
    }

    #[test]
    fn paths_without_a_hinge_pass_unchanged() {
        let (rays, _) = grow(&cone());
        let input = concat_arg_vecs(rays);
        assert_eq!(__nodarium_execute_inner(&[&input, &[encode_float(1.0)], &[encode_float(0.5)]]), input);
    }
}
