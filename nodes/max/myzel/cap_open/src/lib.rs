use myzel_core::{closing_turn, margin_points, pose_caps, Cap};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{concat_arg_vecs, decode_points, encode_points, geometry::wrap_path, split_args, Ctx};
use std::f32::consts::TAU;

nodarium_definition_file!("src/definition.json");

/// The turn that brings the margin of ray `k` in to `reach` from the axis: the first turn that
/// gets it there, or the one that comes closest
fn closing(cap: &Cap, k: usize, reach: f32) -> f32 {
    let margin = |turn: f32| cap.bend(k, |s| closing_turn(s, turn)).last().unwrap().x;
    const STEPS: usize = 64;
    let (mut best, mut best_margin) = (0.0, margin(0.0));
    if best_margin <= reach {
        return 0.0;
    }
    for i in 1..=STEPS {
        let turn = TAU * i as f32 / STEPS as f32;
        let m = margin(turn);
        if m <= reach {
            let (mut lo, mut hi) = (TAU * (i - 1) as f32 / STEPS as f32, turn);
            for _ in 0..30 {
                let mid = 0.5 * (lo + hi);
                if margin(mid) <= reach {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            return hi;
        }
        if m < best_margin {
            (best, best_margin) = (turn, m);
        }
    }
    best
}

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> (Vec<i32>, Vec<i32>) {
    let mut ctx = Ctx::default();
    let items = split_args(args[0]);
    let hinges = decode_points(args[1]);
    let count = items.len();
    let posed = pose_caps(&items, &hinges, |cap, k| {
        let ray = &cap.rays[k];
        ctx.set_path(items[ray.path], ray.path, count);
        let opening = ctx.float(args[2]).clamp(0.0, 1.0);
        if opening >= 1.0 {
            return None;
        }
        let path = wrap_path(items[ray.path]);
        let margin_radius = path.points[(path.length - 1) * 4 + 3].abs();
        // the margin of a closed button lies against the stipe
        let turn = (1.0 - opening) * closing(cap, k, cap.frame.stipe_radius + margin_radius);
        Some(cap.bend(k, |s| closing_turn(s, turn)))
    });
    let paths: Vec<&[i32]> = posed.iter().map(|p| p.as_slice()).collect();
    let margins = margin_points(&paths, &hinges);
    (concat_arg_vecs(posed), encode_points(&margins))
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use glam::Vec2;
    use myzel_core::testing::{cone, grow, place, points, steps, TestCap, STIPE_RADIUS};
    use nodarium_utils::{concat_arg_vecs, decode_points, encode_float, split_args};

    fn open(cap: &TestCap, opening: f32) -> (Vec<Vec<i32>>, Vec<Vec2>) {
        let (rays, hinges) = grow(cap);
        let (paths, margins) = __nodarium_execute_inner(&[&concat_arg_vecs(rays), &hinges, &[encode_float(opening)]]);
        (
            split_args(&paths).iter().map(|p| p.to_vec()).collect(),
            decode_points(&margins).iter().map(|m| place(m.position)).collect(),
        )
    }

    fn thick_margin() -> TestCap {
        TestCap { thickness_curve: vec![0.0, 1.0, 1.0, 0.4], ..cone() }
    }

    fn mean(points: &[Vec2]) -> Vec2 {
        points.iter().copied().sum::<Vec2>() / points.len() as f32
    }

    #[test]
    fn opening_moves_the_margin_from_under_the_cap_to_the_end_of_the_profile() {
        let p = thick_margin();
        let margin_half = 0.5 * 0.4 * p.thickness;
        let (_, closed) = open(&p, 0.0);
        // every margin point lies against the stipe, below its top and the cap
        for m in &closed {
            assert!((m.x - (STIPE_RADIUS + margin_half)).abs() < 2e-4, "closed margin {} from the axis", m.x);
            assert!(m.y < 0.0, "closed margin {} over the stipe top", m.y);
        }

        let end = Vec2::new(p.radius, p.thickness - 0.8 * p.radius);
        let (_, opened) = open(&p, 1.0);
        assert!(mean(&opened).distance(end) < margin_half + 1e-4, "open margin at {:?}, profile ends at {:?}", mean(&opened), end);

        let mut previous = mean(&closed).x;
        for k in 1..=10 {
            let m = mean(&open(&p, k as f32 / 10.0).1);
            assert!(m.x > previous, "the margin moves in from {} to {} while opening", previous, m.x);
            previous = m.x;
        }
    }

    #[test]
    fn opening_keeps_the_rays_and_their_lengths() {
        let p = thick_margin();
        let (input, _) = grow(&p);
        assert!(input.len() > 10);
        for opening in [0.0, 0.5] {
            let (rays, _) = open(&p, opening);
            assert_eq!(rays.len(), input.len());
            for (a, b) in rays.iter().zip(&input) {
                assert_eq!(a[..9], b[..9]);
                for (x, y) in steps(a).iter().zip(steps(b)) {
                    assert!((x - y).abs() < 1e-5, "a step of {y} is {x} long");
                }
            }
        }
        let (rays, _) = open(&p, 1.0);
        assert_eq!(rays, input);
    }

    #[test]
    fn rays_added_on_the_cap_close_with_the_rays_from_the_stipe() {
        let p = thick_margin();
        let (rays, margins) = open(&p, 0.3);
        let spread = margins.iter().map(|m| m.distance(mean(&margins))).fold(0.0, f32::max);
        assert!(spread < 2e-4, "margin points up to {spread} apart");
        // an added ray starts on the ray it was added from
        let starts: Vec<Vec2> = rays[10..].iter().map(|r| place(points(r)[0])).collect();
        let base: Vec<Vec<Vec2>> = rays[..10].iter().map(|r| points(r).into_iter().map(place).collect()).collect();
        for s in starts {
            let nearest = base[0].windows(2).map(|w| distance_to_segment(s, w[0], w[1])).fold(f32::MAX, f32::min);
            assert!(nearest < 3e-4, "an added ray starts {nearest} off the cap");
        }
    }

    fn distance_to_segment(p: Vec2, a: Vec2, b: Vec2) -> f32 {
        let t = ((p - a).dot(b - a) / (b - a).length_squared()).clamp(0.0, 1.0);
        p.distance(a + (b - a) * t)
    }
}

