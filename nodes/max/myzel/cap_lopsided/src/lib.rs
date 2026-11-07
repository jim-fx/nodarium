use glam::Vec2;
use myzel_core::{margin_points, pose_caps, Cap};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{concat_arg_vecs, decode_points, encode_points, geometry::path_random, int, split_args, Ctx};
use std::collections::HashMap;
use std::f32::consts::TAU;

nodarium_definition_file!("src/definition.json");

/// Keeps the random numbers of the outline apart from the ones of the rays and the waves
const SALT: i32 = 0x4c4f_0000;
/// A third harmonic at this weight keeps two lopsided caps from looking like the same oval
const DENT: f32 = 0.2;
/// The shortest side never reaches less than this share of the radius
const MIN_REACH: f32 = 0.2;
/// The tilt grows in over this much of the way from the top of the cap to the margin
const TILT_RAMP: f32 = 0.1;
const SAMPLES: usize = 256;

/// How far a cap reaches around its axis, the same for every cap on one stipe
struct Outline {
    phases: [f32; 3],
    weights: [f32; 3],
    norm: f32,
    /// The angle of the longest side
    long: f32,
    amount: f32,
    tilt: f32,
}

impl Outline {
    fn new(seed: i32, stipe: i32, amount: f32, offset: f32, tilt: f32) -> Outline {
        let offset = offset.clamp(0.0, 1.0);
        let phases = [0, 1, 2].map(|k| TAU * path_random(seed, stipe, SALT + k));
        let mut outline = Outline { phases, weights: [offset, 1.0 - offset, DENT], norm: 1.0, long: 0.0, amount, tilt };
        let (mut max, mut long) = (f32::MIN, 0.0);
        let mut norm = 0.0f32;
        for i in 0..SAMPLES {
            let theta = TAU * i as f32 / SAMPLES as f32;
            let n = outline.raw(theta);
            norm = norm.max(n.abs());
            if n > max {
                (max, long) = (n, theta);
            }
        }
        outline.norm = norm.max(1e-6);
        outline.long = long;
        outline
    }

    fn raw(&self, theta: f32) -> f32 {
        (0..3).map(|k| self.weights[k] * ((k + 1) as f32 * theta - self.phases[k]).cos()).sum()
    }

    /// How far the cap reaches at `theta`, as a share of how far it reached before
    fn reach(&self, theta: f32) -> f32 {
        (1.0 + self.amount * self.raw(theta) / self.norm).max(MIN_REACH)
    }

    fn turn(&self, theta: f32, alpha: f32) -> f32 {
        -self.tilt * (theta - self.long).cos() * (alpha / TILT_RAMP).clamp(0.0, 1.0)
    }
}

/// Where a ray crosses the axis, at the top of the cap
fn crossing(plane: &[Vec2]) -> Option<(usize, Vec2)> {
    let k = (1..plane.len()).find(|&k| plane[k - 1].x < 0.0 && plane[k].x >= 0.0)?;
    let t = -plane[k - 1].x / (plane[k].x - plane[k - 1].x);
    Some((k, plane[k - 1].lerp(plane[k], t)))
}

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> (Vec<i32>, Vec<i32>) {
    let mut ctx = Ctx::default();
    let items = split_args(args[0]);
    let hinges = decode_points(args[1]);
    let seed = int(args[5]);
    let mut outlines: HashMap<i32, Outline> = HashMap::new();
    for hinge in &hinges {
        ctx.seed_path(hinge.parent_id);
        let amount = ctx.float(args[2]).max(0.0);
        let offset = ctx.float(args[3]);
        let tilt = ctx.float(args[4]).to_radians();
        outlines.insert(hinge.parent_id, Outline::new(seed, hinge.parent_id, amount, offset, tilt));
    }

    // the top of every cap, for the rays that start out on the cap
    let mut tops: HashMap<(i32, usize), Vec2> = HashMap::new();
    let posed = pose_caps(&items, &hinges, |cap: &Cap, k| {
        let outline = outlines.get(&cap.frame.stipe_id)?;
        if outline.amount == 0.0 && outline.tilt == 0.0 {
            return None;
        }
        let top = *tops.entry((cap.frame.stipe_id, cap.rays[0].path)).or_insert_with(|| {
            let heights: Vec<f32> = cap.rays.iter().filter_map(|r| crossing(&r.plane)).map(|(_, c)| c.y).collect();
            Vec2::new(0.0, heights.iter().sum::<f32>() / heights.len().max(1) as f32)
        });
        let ray = &cap.rays[k];
        let mut plane = if outline.tilt != 0.0 { cap.bend(k, |s| outline.turn(ray.angle, s.alpha())) } else { ray.plane.clone() };
        let (from, pivot) = match crossing(&plane) {
            Some(c) if ray.crosses => c,
            _ => (0, top),
        };
        let s = outline.reach(ray.angle);
        for p in &mut plane[from..] {
            *p = pivot + (*p - pivot) * s;
        }
        Some(plane)
    });
    let paths: Vec<&[i32]> = posed.iter().map(|p| p.as_slice()).collect();
    let margins = margin_points(&paths, &hinges);
    (concat_arg_vecs(posed), encode_points(&margins))
}

#[cfg(test)]
mod tests {
    use super::{Outline, __nodarium_execute_inner};
    use myzel_core::{
        testing::{cone, grow, place, TestCap},
        CapFrame,
    };
    use nodarium_utils::{concat_arg_vecs, decode_points, encode_float, geometry::PathLink, split_args, Point};
    use std::f32::consts::TAU;

    struct Params {
        cap: TestCap,
        amount: f32,
        offset: f32,
        tilt: f32,
        seed: i32,
    }

    fn lopsided() -> Params {
        Params { cap: TestCap { rays: 32, spacing: 0.0, ..cone() }, amount: 0.3, offset: 0.6, tilt: 0.0, seed: 3 }
    }

    fn run_on(rays: Vec<Vec<i32>>, hinges: &[i32], p: &Params) -> (Vec<Vec<i32>>, Vec<Point>) {
        let (paths, margins) = __nodarium_execute_inner(&[
            &concat_arg_vecs(rays),
            hinges,
            &[encode_float(p.amount)],
            &[encode_float(p.offset)],
            &[encode_float(p.tilt)],
            &[p.seed],
        ]);
        (split_args(&paths).iter().map(|s| s.to_vec()).collect(), decode_points(&margins))
    }

    /// Angle, distance from the axis and height of every margin point
    fn margin(p: &Params) -> Vec<(f32, f32, f32)> {
        let (rays, hinges) = grow(&p.cap);
        let (_, margins) = run_on(rays, &hinges, p);
        let frame = CapFrame::new(&decode_points(&hinges)[0]);
        margins.iter().map(|m| (frame.angle(m.position), place(m.position).x, place(m.position).y)).collect()
    }

    #[test]
    fn no_amount_and_no_tilt_change_nothing() {
        let (flat, hinges) = grow(&cone());
        let (rays, _) = run_on(flat.clone(), &hinges, &Params { amount: 0.0, cap: cone(), ..lopsided() });
        assert_eq!(rays, flat);
    }

    #[test]
    fn the_long_side_reaches_amount_further() {
        let p = lopsided();
        let flat = margin(&Params { amount: 0.0, tilt: 0.0, ..lopsided() });
        let reach = |m: &[(f32, f32, f32)]| m.iter().map(|q| q.1).collect::<Vec<_>>();
        let (before, after) = (reach(&flat), reach(&margin(&p)));
        let ratios: Vec<f32> = after.iter().zip(&before).map(|(a, b)| a / b).collect();
        let max = ratios.iter().cloned().fold(f32::MIN, f32::max);
        let min = ratios.iter().cloned().fold(f32::MAX, f32::min);
        assert!((max - 1.3).abs() < 0.02, "{ratios:?}");
        assert!(min >= 0.7 - 1e-3 && min < 0.95, "{ratios:?}");
    }

    #[test]
    fn the_outline_follows_the_seed_and_offset() {
        let o = |seed: i32, offset: f32| Outline::new(seed, 17, 0.3, offset, 0.0);
        let samples = |o: &Outline| (0..64).map(|k| o.reach(TAU * k as f32 / 64.0)).collect::<Vec<_>>();
        assert_eq!(samples(&o(1, 0.6)), samples(&o(1, 0.6)));
        assert_ne!(samples(&o(1, 0.6)), samples(&o(2, 0.6)));
        // fully off-centre: long on one side, short on the other
        let off = o(4, 1.0);
        let opposite = off.reach(off.long + TAU / 2.0);
        assert!((off.reach(off.long) - 1.3).abs() < 1e-3 && opposite < 0.8, "{opposite}");
    }

    #[test]
    fn a_cap_joined_with_its_ridges_stretches_as_it_does_alone() {
        let p = lopsided();
        let (flesh, hinges) = grow(&p.cap);
        let (mut ridges, _) = grow(&TestCap { radius: 0.025, profile: vec![0.0, 80.0, 50.0, 30.0], rays: 7, spacing: 1.0, ..cone() });
        for ridge in &mut ridges {
            let link = nodarium_utils::geometry::path_link(ridge);
            nodarium_utils::geometry::set_path_link(ridge, PathLink::child(8, link.parent_id, 1.0, link.index));
        }
        let (alone, _) = run_on(flesh.clone(), &hinges, &p);
        let (joined, _) = run_on(flesh.iter().chain(&ridges).cloned().collect(), &hinges, &p);
        assert_eq!(&joined[..flesh.len()], alone.as_slice());

        // every ridge tip moves out as far as the flesh does at its angle
        let frame = CapFrame::new(&decode_points(&hinges)[0]);
        let outline = Outline::new(p.seed, frame.stipe_id, p.amount, p.offset, 0.0);
        let tip = |path: &[i32]| *myzel_core::testing::points(path).last().unwrap();
        for (before, after) in ridges.iter().zip(&joined[flesh.len()..]) {
            let (b, a) = (tip(before), tip(after));
            let expected = outline.reach(frame.angle(b));
            let ratio = place(a).x / place(b).x;
            assert!((ratio - expected).abs() < 0.02, "{ratio} and {expected}");
        }
    }

    #[test]
    fn tilt_lifts_the_long_side_and_lowers_the_other() {
        let level = margin(&Params { tilt: 0.0, ..lopsided() });
        let tilted = margin(&Params { tilt: 15.0, ..lopsided() });
        let p = lopsided();
        let (_, hinges) = grow(&p.cap);
        let stipe = decode_points(&hinges)[0].parent_id;
        let outline = Outline::new(p.seed, stipe, p.amount, p.offset, 0.0);
        let near = |m: &[(f32, f32, f32)], theta: f32| {
            m.iter()
                .min_by(|a, b| {
                    let d = |q: &(f32, f32, f32)| (q.0 - theta).rem_euclid(TAU).min((theta - q.0).rem_euclid(TAU));
                    d(a).total_cmp(&d(b))
                })
                .unwrap()
                .2
        };
        let long = outline.long;
        let short = long + TAU / 2.0;
        assert!(near(&tilted, long) > near(&level, long) + 0.002);
        assert!(near(&tilted, short) < near(&level, short) - 0.002);
    }
}
