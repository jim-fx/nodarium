use myzel_core::{closing_turn, margin_points, pose_caps_of, Cap};
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

/// How far one stipe's caps close, around its axis
struct Closing {
    stipe: i32,
    opening: f32,
    /// The closing of every ray of the outermost cap, by angle
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
pub fn execute(args: &[&[i32]]) -> (Vec<i32>, Vec<i32>) {
    let mut ctx = Ctx::default();
    let items = split_args(args[0]);
    let hinges = decode_points(args[1]);
    let count = items.len();
    let caps = Cap::all(&items, &hinges);

    // caps on one stipe (the flesh and its ridges) close by the same angle around the axis,
    // the one that reaches furthest decides how far, so the others stay against it
    let mut closings: Vec<Closing> = Vec::new();
    for cap in &caps {
        let stipe = cap.frame.stipe_id;
        if closings.iter().any(|c| c.stipe == stipe) {
            continue;
        }
        let joined: Vec<&Cap> = caps.iter().filter(|c| c.frame.stipe_id == stipe).collect();
        let outer = joined.iter().copied().max_by(|a, b| a.reach().total_cmp(&b.reach())).unwrap();
        let first = outer.rays.iter().find(|r| r.crosses).unwrap_or(&outer.rays[0]);
        ctx.set_path(items[first.path], first.path, count);
        // a random node gives the whole cap one value, rays that differ would tear it
        ctx.seed_path(stipe);
        let opening = ctx.float(args[2]).clamp(0.0, 1.0);
        let samples = if opening < 1.0 {
            (0..outer.rays.len())
                .map(|k| {
                    let path = wrap_path(items[outer.rays[k].path]);
                    let margin_radius = path.points[(path.length - 1) * 4 + 3].abs();
                    // the margin of a closed button lies against the stipe
                    (outer.rays[k].angle, closing(outer, k, outer.frame.stipe_radius + margin_radius))
                })
                .collect()
        } else {
            Vec::new()
        };
        closings.push(Closing { stipe, opening, samples });
    }

    let posed = pose_caps_of(&caps, &items, |cap, k| {
        let c = closings.iter().find(|c| c.stipe == cap.frame.stipe_id)?;
        if c.opening >= 1.0 {
            return None;
        }
        let turn = (1.0 - c.opening) * c.at(cap.rays[k].angle);
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

    #[test]
    fn a_random_opening_opens_the_whole_cap_alike() {
        let p = thick_margin();
        let (rays, hinges) = grow(&p);
        let opening = nodarium_utils::concat_args(vec![&[1], &[encode_float(0.2)], &[encode_float(0.8)], &[5]]);
        let (_, margins) = __nodarium_execute_inner(&[&concat_arg_vecs(rays), &hinges, &opening]);
        let margins: Vec<Vec2> = decode_points(&margins).iter().map(|m| place(m.position)).collect();
        let spread = margins.iter().map(|m| m.distance(mean(&margins))).fold(0.0, f32::max);
        assert!(spread < 2e-4, "margin points up to {spread} apart");
        assert!(mean(&margins).x < mean(&open(&p, 1.0).1).x);
    }

    /// Of every point of `ridge`, how far the nearest point of `flesh` is, and how far along
    /// `flesh` that is
    fn hang(ridge: &[i32], flesh: &[Vec<i32>]) -> Vec<(usize, usize, f32)> {
        points(ridge)
            .iter()
            .map(|q| {
                let mut best = (0, 0, f32::MAX);
                for (f, ray) in flesh.iter().enumerate() {
                    for (i, p) in points(ray).iter().enumerate() {
                        if p.distance(*q) < best.2 {
                            best = (f, i, p.distance(*q));
                        }
                    }
                }
                best
            })
            .collect()
    }

    #[test]
    fn joined_flesh_and_ridges_close_by_the_same_angle_at_every_ray() {
        let p = thick_margin();
        let (flesh, hinges) = grow(&p);
        let ridges = myzel_core::testing::ridges();
        let joined = concat_arg_vecs(flesh.iter().chain(&ridges).cloned().collect());
        for opening in [0.0, 0.3, 0.7] {
            let (paths, _) = __nodarium_execute_inner(&[&joined, &hinges, &[encode_float(opening)]]);
            let posed: Vec<Vec<i32>> = split_args(&paths).iter().map(|s| s.to_vec()).collect();
            let (posed_flesh, posed_ridges) = posed.split_at(flesh.len());
            // the flesh closes as it does alone
            let (alone, _) = open(&p, opening);
            assert_eq!(posed_flesh, alone.as_slice());
            for (before, after) in ridges.iter().zip(posed_ridges) {
                let (b, a) = (points(before), points(after));
                let tip = b.len() - 1;
                // the tip of a ridge turns as far as the flesh it lies against
                let (f, i, gap) = hang(before, &flesh)[tip];
                let i = i.min(points(&flesh[f]).len() - 2);
                let turned = |x: &[glam::Vec3], y: &[glam::Vec3], i: usize| (x[i + 1] - x[i]).angle_between(y[i + 1] - y[i]);
                let ridge_turn = turned(&b, &a, tip - 1);
                let flesh_turn = turned(&points(&flesh[f]), &points(&posed_flesh[f]), i);
                assert!((ridge_turn - flesh_turn).abs() < 0.1 * flesh_turn + 0.02, "opening {opening}: a ridge tip turns {ridge_turn}, the flesh over it {flesh_turn}");
                // and stays as close under it
                let now = hang(after, posed_flesh)[tip].2;
                assert!((now - gap).abs() < 0.3 * gap + 3e-4, "opening {opening}: a ridge tip {gap} from the flesh moves to {now}");
            }
        }
    }

    fn distance_to_segment(p: Vec2, a: Vec2, b: Vec2) -> f32 {
        let t = ((p - a).dot(b - a) / (b - a).length_squared()).clamp(0.0, 1.0);
        p.distance(a + (b - a) * t)
    }
}

