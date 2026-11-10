use myzel_core::{closing_turn, pose_caps_of, Cap};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{concat_arg_vecs, geometry::wrap_path, split_args, Ctx};
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

/// How far a cap closes, around its axis
struct Closing {
    opening: f32,
    /// The closing of every ray, by angle
    samples: Vec<(f32, f32)>,
}

impl Closing {
    fn at(&self, angle: f32) -> f32 {
        let n = self.samples.len();
        if n == 0 {
            return 0.0;
        }
        let next = self.samples.partition_point(|s| s.0 <= angle);
        let (a, b) = (self.samples[(next + n - 1) % n], self.samples[next % n]);
        let span = (b.0 - a.0).rem_euclid(TAU);
        let t = if span > 0.0 { (angle - a.0).rem_euclid(TAU) / span } else { 0.0 };
        a.1 + (b.1 - a.1) * t.clamp(0.0, 1.0)
    }
}

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let mut ctx = Ctx::default();
    let items = split_args(args[0]);
    let count = items.len();
    let caps = Cap::all(&items);

    let closings: Vec<Closing> = caps
        .iter()
        .map(|cap| {
            let first = cap.rays.iter().find(|r| r.crosses).unwrap_or(&cap.rays[0]);
            ctx.set_path(items[first.path], first.path, count);
            // a random node gives the whole cap one value, rays that differ would tear it
            ctx.seed_path(cap.frame.stipe_id);
            let opening = ctx.float(args[1]).clamp(0.0, 1.0);
            let samples = if opening < 1.0 {
                (0..cap.rays.len())
                    .map(|k| {
                        let path = wrap_path(items[cap.rays[k].path]);
                        let margin_radius = path.points[(path.length - 1) * 4 + 3].abs();
                        // the margin of a closed button lies against the stipe
                        (cap.rays[k].angle, closing(cap, k, cap.frame.stipe_radius + margin_radius))
                    })
                    .collect()
            } else {
                Vec::new()
            };
            Closing { opening, samples }
        })
        .collect();

    let posed = pose_caps_of(&caps, &items, |cap, k| {
        let c = &closings[caps.iter().position(|c| std::ptr::eq(c, cap))?];
        if c.opening >= 1.0 {
            return None;
        }
        let turn = (1.0 - c.opening) * c.at(cap.rays[k].angle);
        Some(cap.bend(k, |s| closing_turn(s, turn)))
    });
    concat_arg_vecs(posed)
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use glam::Vec2;
    use myzel_core::testing::{cone, grow, list, margins, place, points, rays, steps, unlist, TestCap, STIPE_RADIUS};
    use nodarium_utils::encode_float;

    fn open_with(paths: &[Vec<i32>], opening: &[i32]) -> Vec<Vec<i32>> {
        unlist(&__nodarium_execute_inner(&[&list(paths), opening]))
    }

    /// The posed rays and where their margin points are
    fn open(cap: &TestCap, opening: f32) -> (Vec<Vec<i32>>, Vec<Vec2>) {
        let posed = open_with(&grow(cap), &[encode_float(opening)]);
        let margin = margins(&posed).iter().map(|m| place(m.position)).collect();
        (rays(&posed), margin)
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
        let input = rays(&grow(&p));
        assert!(input.len() > 10);
        for opening in [0.0, 0.5] {
            let (posed, _) = open(&p, opening);
            assert_eq!(posed.len(), input.len());
            for (a, b) in posed.iter().zip(&input) {
                assert_eq!(a[..9], b[..9]);
                for (x, y) in steps(a).iter().zip(steps(b)) {
                    assert!((x - y).abs() < 1e-5, "a step of {y} is {x} long");
                }
            }
        }
        let (posed, _) = open(&p, 1.0);
        assert_eq!(posed, input);
    }

    #[test]
    fn rays_without_their_stipe_pass_unchanged() {
        let only_rays = rays(&grow(&thick_margin()));
        assert_eq!(open_with(&only_rays, &[encode_float(0.3)]), only_rays);
    }

    #[test]
    fn rays_added_on_the_cap_close_with_the_rays_from_the_stipe() {
        let p = thick_margin();
        let (posed, margin) = open(&p, 0.3);
        let spread = margin.iter().map(|m| m.distance(mean(&margin))).fold(0.0, f32::max);
        assert!(spread < 2e-4, "margin points up to {spread} apart");
        // an added ray starts on the ray it was added from
        let starts: Vec<Vec2> = posed[10..].iter().map(|r| place(points(r)[0])).collect();
        let base: Vec<Vec<Vec2>> = posed[..10].iter().map(|r| points(r).into_iter().map(place).collect()).collect();
        for s in starts {
            let nearest = base[0].windows(2).map(|w| distance_to_segment(s, w[0], w[1])).fold(f32::MAX, f32::min);
            assert!(nearest < 3e-4, "an added ray starts {nearest} off the cap");
        }
    }

    #[test]
    fn a_random_opening_opens_the_whole_cap_alike() {
        let p = thick_margin();
        let opening = nodarium_utils::concat_args(vec![&[1], &[encode_float(0.2)], &[encode_float(0.8)], &[5]]);
        let margin: Vec<Vec2> = margins(&open_with(&grow(&p), &opening)).iter().map(|m| place(m.position)).collect();
        let spread = margin.iter().map(|m| m.distance(mean(&margin))).fold(0.0, f32::max);
        assert!(spread < 2e-4, "margin points up to {spread} apart");
        assert!(mean(&margin).x < mean(&open(&p, 1.0).1).x);
    }

    fn distance_to_segment(p: Vec2, a: Vec2, b: Vec2) -> f32 {
        let t = ((p - a).dot(b - a) / (b - a).length_squared()).clamp(0.0, 1.0);
        p.distance(a + (b - a) * t)
    }
}
