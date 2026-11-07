use myzel_core::{curl_turn, margin_points, pose_caps};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{concat_arg_vecs, decode_points, encode_points, split_args, Ctx};

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> (Vec<i32>, Vec<i32>) {
    let mut ctx = Ctx::default();
    let items = split_args(args[0]);
    let hinges = decode_points(args[1]);
    let count = items.len();
    let posed = pose_caps(&items, &hinges, |cap, k| {
        let ray = &cap.rays[k];
        ctx.set_path(items[ray.path], ray.path, count);
        let amount = ctx.float(args[2]).clamp(0.0, 1.0);
        let reach = ctx.float(args[3]).clamp(0.0, 1.0);
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
}
