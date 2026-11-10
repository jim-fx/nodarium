use glam::Vec2;
use myzel_core::{pose_caps_of, Cap};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{concat_arg_vecs, geometry::path_random, int, split_args, Ctx};
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

/// How far a cap reaches around its axis
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
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let mut ctx = Ctx::default();
    let items = split_args(args[0]);
    let seed = int(args[4]);
    let caps = Cap::all(&items);
    let outlines: Vec<Outline> = caps
        .iter()
        .map(|cap| {
            let stipe = cap.frame.stipe_id;
            ctx.seed_path(stipe);
            let amount = ctx.float(args[1]).max(0.0);
            let offset = ctx.float(args[2]);
            let tilt = ctx.float(args[3]).to_radians();
            Outline::new(seed, stipe, amount, offset, tilt)
        })
        .collect();
    // the top of every cap, for the rays that start out on the cap
    let tops: Vec<Vec2> = caps
        .iter()
        .map(|cap| {
            let heights: Vec<f32> = cap.rays.iter().filter_map(|r| crossing(&r.plane)).map(|(_, c)| c.y).collect();
            Vec2::new(0.0, heights.iter().sum::<f32>() / heights.len().max(1) as f32)
        })
        .collect();

    let posed = pose_caps_of(&caps, &items, |cap: &Cap, k| {
        let i = caps.iter().position(|c| std::ptr::eq(c, cap))?;
        let (outline, top) = (&outlines[i], tops[i]);
        if outline.amount == 0.0 && outline.tilt == 0.0 {
            return None;
        }
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
    concat_arg_vecs(posed)
}

#[cfg(test)]
mod tests {
    use super::{Outline, __nodarium_execute_inner};
    use myzel_core::{
        margin_points,
        testing::{cone, grow, hinge, list, paths, place, TestCap},
        CapFrame,
    };
    use nodarium_utils::{concat_arg_vecs, encode_float, split_args, Point};
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

    fn execute(input: &[i32], p: &Params) -> Vec<i32> {
        __nodarium_execute_inner(&[
            input,
            &[encode_float(p.amount)],
            &[encode_float(p.offset)],
            &[encode_float(p.tilt)],
            &[p.seed],
        ])
    }

    fn run_on(rays: Vec<Vec<i32>>, item: &[i32], p: &Params) -> (Vec<Vec<i32>>, Vec<Point>) {
        let output = execute(&list(&rays, item), p);
        (paths(&output), margin_points(&split_args(&output)))
    }

    /// Angle, distance from the axis and height of every margin point
    fn margin(p: &Params) -> Vec<(f32, f32, f32)> {
        let (rays, item) = grow(&p.cap);
        let (_, margins) = run_on(rays, &item, p);
        let frame = CapFrame::new(&hinge(&item));
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
    fn paths_without_a_hinge_pass_unchanged() {
        let p = lopsided();
        let (rays, _) = grow(&p.cap);
        let input = concat_arg_vecs(rays);
        assert_eq!(execute(&input, &p), input);
    }

    #[test]
    fn tilt_lifts_the_long_side_and_lowers_the_other() {
        let level = margin(&Params { tilt: 0.0, ..lopsided() });
        let tilted = margin(&Params { tilt: 15.0, ..lopsided() });
        let p = lopsided();
        let (_, item) = grow(&p.cap);
        let stipe = hinge(&item).stipe;
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
