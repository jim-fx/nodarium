use myzel_core::{pose_caps_of, Cap};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{concat_arg_vecs, curve, geometry::path_random, int, split_args, Ctx, PathInfo};
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
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let mut ctx = Ctx::default();
    let items = split_args(args[0]);
    let seed = int(args[4]);
    let alphas = (0..AMPLITUDE_SAMPLES).map(|k| k as f32 / (AMPLITUDE_SAMPLES - 1) as f32);
    let caps = Cap::all(&items);
    // every cap reads its own waves, so a random node gives every cap its own lobes, and
    // path_info length reads how long its rays are, so the lobes can follow the size of the cap
    let shapes: Vec<(Waves, Amplitude)> = caps
        .iter()
        .map(|cap| {
            ctx.seed_path(cap.frame.stipe_id);
            ctx.path = PathInfo { length: cap.length, ..PathInfo::default() };
            let waves = Waves { count: ctx.int(args[1]).max(0) as u32, irregularity: ctx.float(args[3]) };
            let values = if args[2].len() == 1 {
                let plain = ctx.float(args[2]);
                alphas.clone().map(|a| plain * curve(&PLAIN_PROFILE, a)).collect()
            } else {
                // one amplitude for the whole cap, the same for every ray
                ctx.float_along(args[2], alphas.clone())
            };
            (waves, Amplitude(values))
        })
        .collect();

    let posed = pose_caps_of(&caps, &items, |cap, k| {
        let (waves, amplitude) = &shapes[caps.iter().position(|c| std::ptr::eq(c, cap))?];
        if waves.count == 0 || amplitude.0.iter().all(|&a| a == 0.0) {
            return None;
        }
        let radius = cap.reach();
        if radius <= 0.0 {
            return None;
        }
        let lift = radius * wave_shape(waves, seed, cap.frame.stipe_id, cap.rays[k].angle);
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
    concat_arg_vecs(posed)
}

#[cfg(test)]
mod tests {
    use super::{__nodarium_execute_inner, wave_shape, Waves};
    use myzel_core::testing::{
        caps, cone, frame, grow, grow_on, list, margins, place, points, rays, steps, stipes, unlist, TestCap,
    };
    use nodarium_utils::{concat_args, encode_float};
    use glam::Vec3;
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

    fn wave(paths: &[Vec<i32>], p: &Params) -> Vec<Vec<i32>> {
        wave_with(paths, &[p.waves], &[encode_float(p.amplitude)], p)
    }

    fn wave_with(paths: &[Vec<i32>], waves: &[i32], amplitude: &[i32], p: &Params) -> Vec<Vec<i32>> {
        unlist(&__nodarium_execute_inner(&[&list(paths), waves, amplitude, &[encode_float(p.irregularity)], &[p.seed]]))
    }

    fn run(p: &Params) -> Vec<Vec<i32>> {
        wave(&grow(&p.cap), p)
    }

    /// Angle around the axis and height over the stipe top of every margin point
    fn margin_heights(paths: &[Vec<i32>]) -> Vec<(f32, f32)> {
        let f = frame(paths);
        margins(paths).iter().map(|m| (f.angle(m.position), place(m.position).y)).collect()
    }

    /// How many lobes the margin of every cap has
    fn lobes(paths: &[Vec<i32>]) -> Vec<usize> {
        let margin = margins(paths);
        caps(paths)
            .iter()
            .map(|cap| {
                let f = cap.frame;
                let mut m: Vec<(f32, f32)> = margin
                    .iter()
                    .filter(|m| m.position.distance(f.hinge) < 0.1)
                    .map(|m| (f.angle(m.position), (m.position - f.hinge).dot(f.axis)))
                    .collect();
                m.sort_by(|a, b| a.0.total_cmp(&b.0));
                let mean = m.iter().map(|p| p.1).sum::<f32>() / m.len() as f32;
                let changes = (0..m.len()).filter(|&i| (m[i].1 - mean).signum() != (m[(i + 1) % m.len()].1 - mean).signum()).count();
                changes / 2
            })
            .collect()
    }

    #[test]
    fn no_waves_or_no_amplitude_changes_nothing() {
        let flat = grow(&cone());
        assert_eq!(run(&Params { amplitude: 0.0, ..wavy() }), flat);
        assert_eq!(run(&Params { waves: 0, irregularity: 1.0, ..wavy() }), flat);
        assert_ne!(run(&wavy()), flat);
    }

    #[test]
    fn rays_without_their_stipe_pass_unchanged() {
        let only_rays = rays(&grow(&cone()));
        assert_eq!(wave(&only_rays, &wavy()), only_rays);
    }

    #[test]
    fn the_stipe_passes_unchanged() {
        let flat = grow(&cone());
        let waved = run(&wavy());
        let stipe = flat.iter().position(|p| !nodarium_utils::geometry::path_link(p).has_parent()).unwrap();
        assert_eq!(waved[stipe], flat[stipe]);
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
        let flat = place(margins(&run(&Params { amplitude: 0.0, ..p.clone() }))[0].position).y;
        let m = margin_heights(&run(&Params { cap: TestCap { rays: 64, spacing: 0.0, ..cone() }, ..p }));
        let changes = (0..m.len()).filter(|&i| (m[i].1 - flat).signum() != (m[(i + 1) % m.len()].1 - flat).signum()).count();
        assert_eq!(changes, 12);
    }

    #[test]
    fn the_seed_only_moves_irregular_waves() {
        let p = wavy();
        assert_eq!(run(&p), run(&p));
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
        let flat = grow(&cone());
        let waved = run(&Params { irregularity: 0.7, ..wavy() });
        for (a, b) in waved.iter().zip(&flat) {
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
            cap: TestCap { rays: 7, spacing: 0.5, seed: 5, ..cone() },
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
            let flat = margins(&run(&Params { cap: cap.clone(), amplitude: 0.0, ..p.clone() }));
            let wavy = margins(&run(&Params { cap, ..p.clone() }));
            wavy.iter().zip(&flat).map(|(a, b)| a.position - b.position).collect::<Vec<_>>()
        };
        let (small, large) = (lift(0.03), lift(0.06));
        assert!(small.iter().any(|d| d.length() > 1e-3));
        // the root in the stipe keeps its size, so the points along the rays don't scale exactly
        for (s, l) in small.iter().zip(&large) {
            assert!((*l - *s * 2.0).length() < 0.02 * l.length() + 1e-6, "{s} and {l}");
        }
    }

    #[test]
    fn a_connected_amplitude_sets_how_far_in_the_waves_reach() {
        let p = wavy();
        let input = grow(&p.cap);
        let curve = |points: &[f32]| {
            let mut args: Vec<Vec<i32>> =
                vec![vec![3], concat_args(vec![&[2], &[0]]), vec![encode_float(0.0)], vec![encode_float(0.15)]];
            args.extend(points.iter().map(|&v| vec![encode_float(v)]));
            concat_args(args.iter().map(|a| a.as_slice()).collect())
        };
        let waved = |amplitude: &[i32]| wave_with(&input, &[6], amplitude, &p);
        // the same as the plain amplitude, which spreads the same way
        let same = waved(&curve(&[0.0, 0.0, 0.5, 0.25, 1.0, 1.0]));
        for (a, b) in same.iter().zip(&run(&p)) {
            for (x, y) in points(a).iter().zip(points(b)) {
                assert!(x.distance(y) < 2e-5);
            }
        }
        // a constant lifts the whole cap alike, which leaves it as it is
        let constant = waved(&curve(&[0.0, 1.0, 1.0, 1.0]));
        for (a, b) in constant.iter().zip(&input) {
            for (x, y) in points(a).iter().zip(points(b)) {
                assert!(x.distance(y) < 1e-5);
            }
        }
    }

    #[test]
    fn path_info_length_reads_how_long_the_rays_are() {
        let lobes_of = |radius: f32| {
            let paths = grow(&TestCap { radius, rays: 64, spacing: 0.0, ..cone() });
            let length = caps(&paths)[0].length;
            // round(path_info length * 150)
            let product = concat_args(vec![&[0], &[2], &concat_args(vec![&[5], &[5]]), &[encode_float(150.0)]]);
            let waves = concat_args(vec![&[6], &product]);
            let waved = wave_with(&paths, &waves, &[encode_float(0.15)], &wavy());
            (lobes(&waved)[0], (length * 150.0).round() as usize)
        };
        let (small, expected_small) = lobes_of(0.03);
        let (large, expected_large) = lobes_of(0.06);
        assert_eq!(small, expected_small);
        assert_eq!(large, expected_large);
        assert!(large > small);
    }

    #[test]
    fn every_cap_reads_its_own_waves() {
        // two upright stipes 20 cm apart, each with a cap of many rays
        let paths = grow_on(&TestCap { rays: 64, spacing: 0.0, ..cone() }, &stipes(&[(Vec3::ZERO, 0.0), (Vec3::new(0.2, 0.0, 0.0), 0.0)]));
        let counts: Vec<Vec<usize>> = (0..8)
            .map(|seed| {
                let waves = concat_args(vec![&[4], &[3], &[9], &[seed]]);
                lobes(&wave_with(&paths, &waves, &[encode_float(0.15)], &wavy()))
            })
            .collect();
        for c in &counts {
            assert_eq!(c.len(), 2);
            assert!(c.iter().all(|n| (3..=9).contains(n)), "{counts:?}");
        }
        assert!(counts.iter().any(|c| c[0] != c[1]), "{counts:?}");
    }
}
