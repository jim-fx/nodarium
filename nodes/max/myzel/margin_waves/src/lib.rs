use myzel_core::{margin_points, pose_caps, Cap};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{concat_arg_vecs, curve, decode_points, encode_points, geometry::path_random, int, split_args, Ctx};
use std::f32::consts::TAU;

nodarium_definition_file!("src/definition.json");

/// Sine terms that bend the lobes out of even spacing and give them uneven heights
const WAVE_TERMS: usize = 3;
/// At full irregularity the phase of the waves still advances at this share of its even rate,
/// so lobes get narrower and wider but never fold over each other
const MIN_PHASE_RATE: f32 = 0.4;
/// Keeps the random numbers of the waves apart from the ones that place the rays
const WAVE_SALT: i32 = 0x5741_0000;
/// How a plain amplitude spreads from the centre (left) to the margin (right): only the outer
/// part waves, a cap that rises and falls from the centre on would look tilted
const PLAIN_PROFILE: [f32; 6] = [0.0, 0.0, 0.5, 0.25, 1.0, 1.0];
/// Samples of a connected amplitude from the centre to the margin
const AMPLITUDE_SAMPLES: usize = 65;

/// Lobes around the margin that lift and lower it, in the frame of the cap
struct Waves {
    count: u32,
    irregularity: f32,
}

/// How far a cap is lifted at `theta` around the axis, as a share of `amplitude`. It depends on
/// nothing else of the cap, so the caps on one stipe with the same waves (a cap and its ridges)
/// wave alike, whatever their rays.
fn wave_shape(waves: &Waves, seed: i32, id: i32, theta: f32) -> f32 {
    if waves.count == 0 {
        return 0.0;
    }
    let irregularity = waves.irregularity.clamp(0.0, 1.0);
    let random = |salt: usize| path_random(seed, id, WAVE_SALT + salt as i32);
    let (mut warp, mut warp_rate, mut height, mut height_range) = (0.0, 0.0, 0.0, 0.0);
    for k in 1..=WAVE_TERMS {
        let (kf, salt) = (k as f32, 4 * k);
        let c = (0.25 + 0.75 * random(salt)) / kf;
        warp += c * (kf * theta + TAU * random(salt + 1)).sin();
        warp_rate += c * kf;
        let b = (0.25 + 0.75 * random(salt + 2)) / kf;
        height += b * (kf * theta + TAU * random(salt + 3)).cos();
        height_range += b;
    }
    let n = waves.count as f32;
    // warp / warp_rate changes by at most 1 per radian
    let phase = n * theta + irregularity * (1.0 - MIN_PHASE_RATE) * n * warp / warp_rate;
    let height = 1.0 + 0.5 * irregularity * height / height_range;
    height * phase.cos()
}

/// The amplitude from the centre (alpha 0) to the margin (alpha 1), as a share of the radius
struct Amplitude(Vec<f32>);

impl Amplitude {
    fn at(&self, alpha: f32) -> f32 {
        let x = alpha.clamp(0.0, 1.0) * (self.0.len() - 1) as f32;
        let i = (x as usize).min(self.0.len() - 2);
        self.0[i] + (self.0[i + 1] - self.0[i]) * (x - i as f32)
    }
}

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> (Vec<i32>, Vec<i32>) {
    let mut ctx = Ctx::default();
    let items = split_args(args[0]);
    let hinges = decode_points(args[1]);
    let waves = Waves { count: ctx.int(args[2]).max(0) as u32, irregularity: ctx.float(args[4]) };
    let seed = int(args[5]);
    let alphas = (0..AMPLITUDE_SAMPLES).map(|k| k as f32 / (AMPLITUDE_SAMPLES - 1) as f32);
    let mut amplitudes: Vec<(i32, Amplitude)> = Vec::new();
    for hinge in &hinges {
        let values = if args[3].len() == 1 {
            let plain = ctx.float(args[3]);
            alphas.clone().map(|a| plain * curve(&PLAIN_PROFILE, a)).collect()
        } else {
            // one amplitude for the whole cap, the same for every ray
            ctx.seed_path(hinge.parent_id);
            ctx.float_along(args[3], alphas.clone())
        };
        amplitudes.push((hinge.parent_id, Amplitude(values)));
    }

    // caps on one stipe (a cap and its ridges) share the radius, so they wave alike
    let caps = Cap::all(&items, &hinges);
    let radius_of = |stipe: i32| caps.iter().filter(|c| c.frame.stipe_id == stipe).map(|c| c.reach()).fold(0.0, f32::max);

    let posed = pose_caps(&items, &hinges, |cap, k| {
        let amplitude = &amplitudes.iter().find(|a| a.0 == cap.frame.stipe_id)?.1;
        if waves.count == 0 || amplitude.0.iter().all(|&a| a == 0.0) {
            return None;
        }
        let radius = radius_of(cap.frame.stipe_id);
        if radius <= 0.0 {
            return None;
        }
        let lift = radius * wave_shape(&waves, seed, cap.frame.stipe_id, cap.rays[k].angle);
        let offset = |x: f32| lift * amplitude.at(x / radius);
        Some(cap.bend(k, |s| {
            let length = s.length();
            if length > 0.0 {
                -((offset(s.to.x) - offset(s.from.x)) / length).atan()
            } else {
                0.0
            }
        }))
    });
    let paths: Vec<&[i32]> = posed.iter().map(|p| p.as_slice()).collect();
    let margins = margin_points(&paths, &hinges);
    (concat_arg_vecs(posed), encode_points(&margins))
}

#[cfg(test)]
mod tests {
    use super::{__nodarium_execute_inner, wave_shape, Waves};
    use myzel_core::{
        testing::{cone, grow, place, steps, TestCap},
        CapFrame,
    };
    use nodarium_utils::{concat_arg_vecs, decode_points, encode_float, geometry::PathLink, split_args, Point};
    use std::f32::consts::TAU;

    #[derive(Clone)]
    struct Params {
        cap: TestCap,
        waves: i32,
        amplitude: f32,
        irregularity: f32,
        seed: i32,
    }

    fn wavy() -> Params {
        Params { cap: cone(), waves: 6, amplitude: 0.15, irregularity: 0.0, seed: 3 }
    }

    struct Out {
        rays: Vec<Vec<i32>>,
        hinge: Point,
        margins: Vec<Point>,
    }

    fn run(p: &Params) -> Out {
        let (rays, hinges) = grow(&p.cap);
        let (paths, margins) = __nodarium_execute_inner(&[
            &concat_arg_vecs(rays),
            &hinges,
            &[p.waves],
            &[encode_float(p.amplitude)],
            &[encode_float(p.irregularity)],
            &[p.seed],
        ]);
        Out {
            rays: split_args(&paths).iter().map(|s| s.to_vec()).collect(),
            hinge: decode_points(&hinges)[0],
            margins: decode_points(&margins),
        }
    }

    /// Angle around the axis and height over the stipe top of every margin point
    fn margin_heights(out: &Out) -> Vec<(f32, f32)> {
        let frame = CapFrame::new(&out.hinge);
        out.margins.iter().map(|m| (frame.angle(m.position), place(m.position).y)).collect()
    }

    #[test]
    fn no_waves_or_no_amplitude_changes_nothing() {
        let (flat, _) = grow(&cone());
        assert_eq!(run(&Params { amplitude: 0.0, ..wavy() }).rays, flat);
        assert_eq!(run(&Params { waves: 0, irregularity: 1.0, ..wavy() }).rays, flat);
        assert_ne!(run(&wavy()).rays, flat);
    }

    #[test]
    fn the_margin_changes_sign_twice_per_wave() {
        let p = wavy();
        for irregularity in [0.0, 0.5, 1.0] {
            let w = Waves { count: p.waves as u32, irregularity };
            let samples = 4096;
            let shape = |k: usize| wave_shape(&w, 5, 17, TAU * k as f32 / samples as f32);
            let changes = (0..samples).filter(|&k| shape(k).signum() != shape((k + 1) % samples).signum()).count();
            assert_eq!(changes, 2 * p.waves as usize, "irregularity {irregularity}");
        }
        // and on the cap itself, measured against the flat margin
        let flat = place(run(&Params { amplitude: 0.0, ..p.clone() }).margins[0].position).y;
        let out = run(&Params { cap: TestCap { rays: 64, spacing: 0.0, ..cone() }, ..p });
        let m = margin_heights(&out);
        let changes = (0..m.len()).filter(|&i| (m[i].1 - flat).signum() != (m[(i + 1) % m.len()].1 - flat).signum()).count();
        assert_eq!(changes, 12);
    }

    #[test]
    fn the_seed_only_moves_irregular_waves() {
        let p = wavy();
        assert_eq!(run(&p).margins, run(&p).margins);
        let at = |seed: i32, irregularity: f32| {
            let w = Waves { count: 6, irregularity };
            (0..64).map(|k| wave_shape(&w, seed, 17, k as f32 * 0.1)).collect::<Vec<_>>()
        };
        assert_eq!(at(1, 0.0), at(2, 0.0));
        assert_ne!(at(1, 0.5), at(2, 0.5));
        assert_eq!(at(1, 0.5), at(1, 0.5));
    }

    #[test]
    fn waves_keep_the_length_of_every_ray() {
        let (flat, _) = grow(&cone());
        let out = run(&Params { irregularity: 0.7, ..wavy() });
        for (a, b) in out.rays.iter().zip(&flat) {
            for (x, y) in steps(a).iter().zip(steps(b)) {
                assert!((x - y).abs() < 1e-5, "a step of {y} is {x} long");
            }
        }
    }

    #[test]
    fn caps_with_other_rays_wave_alike() {
        let a = margin_heights(&run(&Params {
            cap: TestCap { rays: 10, spacing: 0.0, seed: 5, ..cone() },
            irregularity: 0.6,
            seed: 5,
            ..wavy()
        }));
        let mut b = margin_heights(&run(&Params {
            cap: TestCap { rays: 7, spacing: 0.5, seed: 5, thickness_curve: vec![0.0, 1.0, 1.0, 0.0], ..cone() },
            irregularity: 0.6,
            seed: 5,
            ..wavy()
        }));
        b.sort_by(|x, y| x.0.total_cmp(&y.0));
        assert!(b.len() > 100, "{} rays", b.len());
        let amplitude = wavy().amplitude * cone().radius;
        for &(theta, y) in &a {
            let j = b.partition_point(|q| q.0 < theta);
            let (lo, hi) = (b[(j + b.len() - 1) % b.len()], b[j % b.len()]);
            let span = (hi.0 - lo.0).rem_euclid(TAU);
            let t = if span > 0.0 { (theta - lo.0).rem_euclid(TAU) / span } else { 0.0 };
            let expected = lo.1 + (hi.1 - lo.1) * t;
            assert!((y - expected).abs() < 0.03 * amplitude, "at {theta}: {y} and {expected}");
        }
    }

    #[test]
    fn doubling_the_radius_doubles_the_waves() {
        let p = Params { cap: TestCap { thickness: 0.0, rays: 16, ..cone() }, ..wavy() };
        let lift = |radius: f32| {
            let cap = TestCap { radius, ..p.cap.clone() };
            let flat = run(&Params { cap: cap.clone(), amplitude: 0.0, ..p.clone() });
            let wavy = run(&Params { cap, ..p.clone() });
            wavy.margins.iter().zip(&flat.margins).map(|(a, b)| a.position - b.position).collect::<Vec<_>>()
        };
        let (small, large) = (lift(0.03), lift(0.06));
        assert!(small.iter().any(|d| d.length() > 1e-3));
        // the root in the stipe keeps its size, so the points along the rays don't scale exactly
        for (s, l) in small.iter().zip(&large) {
            assert!((*l - *s * 2.0).length() < 0.02 * l.length() + 1e-6, "{s} and {l}");
        }
    }

    #[test]
    fn a_cap_joined_with_its_ridges_waves_as_it_does_alone() {
        let p = Params { irregularity: 0.5, ..wavy() };
        let (flesh, hinges) = grow(&p.cap);
        // ridges of another node under the cap, not as far out
        let (mut ridges, _) = grow(&TestCap { radius: 0.025, profile: vec![0.0, 80.0, 50.0, 30.0], rays: 7, spacing: 1.0, ..cone() });
        for ridge in &mut ridges {
            let link = nodarium_utils::geometry::path_link(ridge);
            nodarium_utils::geometry::set_path_link(ridge, PathLink::child(8, link.parent_id, 1.0, link.index));
        }
        let wave = |rays: Vec<Vec<i32>>| {
            let (paths, _) = __nodarium_execute_inner(&[
                &concat_arg_vecs(rays),
                &hinges,
                &[p.waves],
                &[encode_float(p.amplitude)],
                &[encode_float(p.irregularity)],
                &[p.seed],
            ]);
            split_args(&paths).iter().map(|s| s.to_vec()).collect::<Vec<_>>()
        };
        let alone = wave(flesh.clone());
        let joined = wave(flesh.iter().chain(&ridges).cloned().collect());
        assert_eq!(&joined[..flesh.len()], alone.as_slice());
        assert_ne!(&joined[flesh.len()..], ridges.as_slice());
    }

    #[test]
    fn a_connected_amplitude_sets_how_far_in_the_waves_reach() {
        let p = wavy();
        let (rays, hinges) = grow(&p.cap);
        let input = concat_arg_vecs(rays);
        let curve = |points: &[f32]| {
            let mut args: Vec<Vec<i32>> = vec![
                vec![3],
                nodarium_utils::concat_args(vec![&[2], &[0]]),
                vec![encode_float(0.0)],
                vec![encode_float(0.15)],
            ];
            args.extend(points.iter().map(|&v| vec![encode_float(v)]));
            nodarium_utils::concat_args(args.iter().map(|a| a.as_slice()).collect())
        };
        let waved = |amplitude: &[i32]| {
            let (paths, _) = __nodarium_execute_inner(&[&input, &hinges, &[6], amplitude, &[encode_float(0.0)], &[3]]);
            split_args(&paths).iter().map(|s| s.to_vec()).collect::<Vec<_>>()
        };
        // the same as the plain amplitude, which spreads the same way
        let same = waved(&curve(&[0.0, 0.0, 0.5, 0.25, 1.0, 1.0]));
        let plain = run(&p).rays;
        for (a, b) in same.iter().zip(&plain) {
            for (x, y) in myzel_core::testing::points(a).iter().zip(myzel_core::testing::points(b)) {
                assert!(x.distance(y) < 2e-5);
            }
        }
        // a constant lifts the whole cap alike, which leaves it as it is
        let constant = waved(&curve(&[0.0, 1.0, 1.0, 1.0]));
        let (flat, _) = grow(&p.cap);
        for (a, b) in constant.iter().zip(&flat) {
            for (x, y) in myzel_core::testing::points(a).iter().zip(myzel_core::testing::points(b)) {
                assert!(x.distance(y) < 1e-5);
            }
        }
    }
}
