//! Neighbour-Sensing as a path modifier, after the growth model of Meškauskas, Fricker and Moore
//! (2004): a hypha tip turns by what it senses of the hyphae around it, away from crowded places
//! (negative autotropism) or towards its own kind (positive autotropism, bundling). Here the paths
//! already exist, e.g. strands that follow their guide. Every path is grown again from its base to
//! its tip along where it was, and at every point it turns by what it senses there. A turn carries
//! on along the path like the direction of a growing tip, while the path keeps being pulled back
//! onto where it was, so it never strays further than `max_offset`.
//!
//! What a path senses is the slope of the paths' density, blurred to the range (see `field`), over
//! the density itself. Near the surface of the body the paths make up, found at a larger scale,
//! what the paths sense across the surface where they were is taken off: that is the pull of the
//! body as a whole, which would otherwise draw the paths on its surface into it (bundling) or push
//! them out (avoiding). So the body keeps its shape while the paths on it part, bundle and meander. The paths sense each other a few times, every pass from
//! where the one before left them, and what they sense adds up, so cords that begin to form draw
//! their neighbours in.

use glam::Vec3;
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{
    concat_args,
    geometry::{path_link, path_random, wrap_path, wrap_path_mut, DeepestPaths},
    int, split_args, Ctx,
};

mod field;
use field::Field;

nodarium_definition_file!("src/definition.json");

/// How often the paths sense each other anew. Every pass reads where the pass before left them,
/// so a bundle that begins to form draws its neighbours in.
const PASSES: usize = 6;
/// How much of a full turn a path adds per pass where it senses a clear direction
const STEP: f32 = 0.35;
/// How far along a path a turn carries on, in ranges
const MEMORY: f32 = 2.0;
/// How strongly a path turns by what it senses: at the edge of a bundle, or where a cord begins to
/// form, it turns as far as the strength lets it
const GAIN: f32 = 8.0;
/// The shape of the body the paths make up is sensed at this many ranges
const SHAPE_SCALE: f32 = 1.5;
/// Where the surface of the body begins and where it is fully there, read on the slope of the
/// blurred density over the density, times the blur width: on the surface of a body that is about
/// 0.8, one blur width inside 0.3, and the uneven spread of the paths deep inside reads below 0.2
const SURFACE: (f32, f32) = (0.25, 0.65);
/// A turn every path makes on its own, as a share of the strongest turn it can sense, so a
/// regular bundle doesn't bundle regularly
const WANDER: f32 = 0.3;

fn smoothstep(value: f32) -> f32 {
    let t = value.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn across(v: Vec3, tangent: Vec3) -> Vec3 {
    v - tangent * v.dot(tangent)
}

/// A path with everything the passes need worked out once
struct Strand {
    index: usize,
    points: Vec<Vec3>,
    radius: Vec<f32>,
    tangent: Vec<Vec3>,
    /// how much of the offset of the point before carries on to this one
    carry: Vec<f32>,
    strength: Vec<f32>,
    bundling: Vec<f32>,
    limit: Vec<f32>,
    /// the turn the path makes on its own
    wander: Vec<Vec3>,
}

struct Params {
    range: f32,
    seed: i32,
}

impl Strand {
    fn new(index: usize, path: &[i32], rank: usize, count: usize, args: &[&[i32]], params: &Params, ctx: &mut Ctx) -> Strand {
        let data = wrap_path(path);
        let points: Vec<Vec3> = data.points.chunks_exact(4).map(|p| Vec3::new(p[0], p[1], p[2])).collect();
        let radius: Vec<f32> = data.points.chunks_exact(4).map(|p| p[3].abs()).collect();
        let n = points.len();
        let mut along = vec![0.0f32; n];
        for i in 1..n {
            along[i] = along[i - 1] + points[i].distance(points[i - 1]);
        }
        let length = along[n - 1];
        let alpha: Vec<f32> =
            along.iter().enumerate().map(|(i, &l)| if length > 0.0 { l / length } else { i as f32 / (n - 1).max(1) as f32 }).collect();

        let tangent = (0..n)
            .map(|i| (points[(i + 1).min(n - 1)] - points[i.saturating_sub(1)]).try_normalize().unwrap_or(Vec3::Y))
            .collect();
        let memory = MEMORY * params.range;
        let carry = (0..n).map(|i| if i == 0 { 0.0 } else { (-(along[i] - along[i - 1]) / memory).exp() }).collect();

        ctx.set_path(path, rank, count);
        let strength = ctx.float_along(args[1], alpha.iter().copied()).into_iter().map(|v| v.clamp(0.0, 1.0)).collect();
        let bundling = ctx.float_along(args[3], alpha.iter().copied()).into_iter().map(|v| v.clamp(-1.0, 1.0)).collect();
        let limit = ctx.float_along(args[4], alpha.iter().copied()).into_iter().map(|v| v.max(0.0)).collect();

        // a random direction every memory length along the path, eased from one to the next
        let id = path_link(path).id;
        let knot = |k: usize| {
            let random = |axis: i32| 2.0 * path_random(params.seed, id, 3 * k as i32 + axis) - 1.0;
            Vec3::new(random(0), random(1), random(2)).clamp_length_max(1.0)
        };
        let wander = along
            .iter()
            .map(|&l| {
                let u = l / memory;
                let k = u.floor() as usize;
                knot(k).lerp(knot(k + 1), smoothstep(u - k as f32))
            })
            .collect();

        Strand { index, points, radius, tangent, carry, strength, bundling, limit, wander }
    }

    fn senses(&self) -> bool {
        self.points.len() >= 2 && self.strength.iter().zip(&self.limit).any(|(&s, &l)| s > 0.0 && l > 0.0)
    }

    /// Grows the path again from its base along where it was, turning by what it has sensed
    /// `surface` is the inward normal of the body the paths make up, scaled from 0 inside to 1 on
    /// its surface. A path there never moves out of the body, so the body keeps its outline.
    fn grow(&self, sensed: &[Vec3], surface: &[Vec3]) -> Vec<Vec3> {
        let mut offset = Vec3::ZERO;
        let mut offsets = vec![Vec3::ZERO; self.points.len()];
        for i in 1..self.points.len() {
            let inward = across(surface[i], self.tangent[i]);
            let normal = inward.normalize_or_zero();
            let inside = |v: Vec3| v - normal * (v.dot(normal).min(0.0) * inward.length());
            let turn = inside(across(sensed[i] + self.wander[i] * WANDER, self.tangent[i])).clamp_length_max(1.0);
            let target = turn * self.strength[i] * self.limit[i];
            offset = offset * self.carry[i] + target * (1.0 - self.carry[i]);
            offset = inside(across(offset, self.tangent[i])).clamp_length_max(self.limit[i]);
            offsets[i] = offset;
        }
        offsets
    }
}

/// The paths moved by `offsets`
fn moved(strands: &[Strand], offsets: &[Vec<Vec3>]) -> Vec<Vec<Vec3>> {
    strands.iter().zip(offsets).map(|(s, o)| s.points.iter().zip(o).map(|(p, o)| *p + *o).collect()).collect()
}

fn neighbour_sensing(strands: &[Strand], range: f32) -> Vec<Vec<Vec3>> {
    let mut offsets: Vec<Vec<Vec3>> = strands.iter().map(|s| vec![Vec3::ZERO; s.points.len()]).collect();
    let reach = strands.iter().flat_map(|s| s.limit.iter().copied()).fold(0.0, f32::max);
    let original: Vec<Vec<Vec3>> = strands.iter().map(|s| s.points.clone()).collect();
    let (sum, count) = strands.iter().flat_map(|s| s.radius.iter()).fold((0.0f64, 0usize), |(s, c), r| (s + *r as f64, c + 1));
    let thickness = (sum / count.max(1) as f64) as f32;
    // paths without thickness splat nothing, and sensing an empty field divides 0 by 0
    if !(thickness > 0.0) || !thickness.is_finite() {
        return offsets;
    }
    let Some(mut field) = Field::new(&original, range, reach) else {
        return offsets;
    };
    let radius: Vec<&[f32]> = strands.iter().map(|s| s.radius.as_slice()).collect();
    // the blurred share a single path fills right where it runs, so a lone neighbour at the edge
    // of the range is sensed faintly and the paths of a full bundle balance out
    let own = |field: &Field| thickness * thickness / (2.0 * field.sigma * field.sigma);
    // a huge range or max offset makes cells so large that the splat underflows to nothing
    if !(own(&field) > 0.0) || !own(&field).is_finite() {
        return offsets;
    }

    // the surface of the body: its normal, scaled from 0 inside to 1 on the surface
    let mut surface: Vec<Vec<Vec3>> = original.iter().map(|p| vec![Vec3::ZERO; p.len()]).collect();
    if let Some(mut shape) = Field::new(&original, SHAPE_SCALE * range, reach).filter(|shape| own(shape) > 0.0) {
        shape.set(&original, &radius);
        for (points, surface) in original.iter().zip(&mut surface) {
            for (q, n) in points.iter().zip(surface.iter_mut()) {
                let (density, slope) = shape.sample(*q);
                let edge = shape.sigma * slope.length() / (density + own(&shape));
                *n = slope.normalize_or_zero() * smoothstep((edge - SURFACE.0) / (SURFACE.1 - SURFACE.0));
            }
        }
    }
    let sense = |field: &Field, q: Vec3| {
        let (density, slope) = field.sample(q);
        slope * field.sigma / (density + own(field))
    };
    // what the paths sense across the surface where they were, the pull of the body as a whole
    field.set(&original, &radius);
    let bulk: Vec<Vec<Vec3>> = original
        .iter()
        .zip(&surface)
        .map(|(points, surface)| points.iter().zip(surface).map(|(q, n)| *n * sense(&field, *q).dot(*n)).collect())
        .collect();

    // what a path has sensed so far, every pass adds to it, so the paths keep turning until
    // what they sense balances out or they reach the max offset
    let mut drive: Vec<Vec<Vec3>> = offsets.clone();
    for _ in 0..PASSES {
        let current = moved(strands, &offsets);
        if offsets.iter().flatten().any(|o| *o != Vec3::ZERO) {
            field.set(&current, &radius);
        }
        for (((strand, current), drive), bulk) in strands.iter().zip(&current).zip(&mut drive).zip(&bulk) {
            for (((q, bundling), drive), bulk) in current.iter().zip(&strand.bundling).zip(drive.iter_mut()).zip(bulk) {
                let sensed = ((sense(&field, *q) - *bulk) * GAIN).clamp_length_max(1.0);
                *drive = (*drive + sensed * (*bundling * STEP)).clamp_length_max(1.0);
            }
        }
        offsets = strands
            .iter()
            .zip(&drive)
            .zip(&surface)
            .map(|((strand, drive), surface)| {
                if strand.senses() {
                    strand.grow(drive, surface)
                } else {
                    vec![Vec3::ZERO; strand.points.len()]
                }
            })
            .collect();
    }
    offsets
}

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let mut ctx = Ctx::default();

    let plants = split_args(args[0]);
    let params = Params { range: ctx.float(args[2]), seed: int(args[5]) };
    let depth = int(args[6]).max(1);

    let mut output: Vec<Vec<i32>> = plants.iter().map(|p| p.to_vec()).collect();
    // a subnormal range would make every length measured in ranges overflow
    if params.range > 0.0 && params.range.is_normal() {
        let selected = DeepestPaths::new(&plants, depth);
        let count = selected.count(&plants);
        let strands: Vec<Strand> = plants
            .iter()
            .enumerate()
            .filter(|(_, p)| selected.contains(p))
            .enumerate()
            .map(|(rank, (index, path))| Strand::new(index, path, rank, count, args, &params, &mut ctx))
            .collect();

        if strands.iter().any(Strand::senses) {
            let offsets = neighbour_sensing(&strands, params.range);
            for (strand, offsets) in strands.iter().zip(&offsets) {
                let path = wrap_path_mut(&mut output[strand.index]);
                for (i, offset) in offsets.iter().enumerate() {
                    if *offset != Vec3::ZERO && offset.is_finite() {
                        let p = strand.points[i] + *offset;
                        path.points[i * 4..i * 4 + 3].copy_from_slice(&[p.x, p.y, p.z]);
                    }
                }
            }
        }
    }

    concat_args(output.iter().map(|p| p.as_slice()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec2;
    use nodarium_utils::{
        encode_float,
        geometry::{create_multiple_paths, path_id, set_path_link, wrap_multiple_paths, PathLink},
    };
    use std::f32::consts::TAU;

    const CREATOR: i32 = 7;
    const POINTS: usize = 24;
    const TUBE: f32 = 0.007;
    const THICKNESS: f32 = 0.0002;
    const HEIGHT: f32 = 0.05;

    /// `amount` strands of one guide standing straight up 5 cm in a tube of 7 mm, scattered at
    /// random across it, every third one starting higher up
    fn bundle(amount: usize) -> Vec<i32> {
        bundle_of(amount, THICKNESS)
    }

    fn bundle_of(amount: usize, thickness: f32) -> Vec<i32> {
        let mut data = create_multiple_paths(amount, POINTS, 2, CREATOR);
        for (k, path) in wrap_multiple_paths(&mut data).iter_mut().enumerate() {
            let random = |salt: i32| path_random(3, k as i32, salt);
            let home = Vec2::from_angle(TAU * random(0)) * random(1).sqrt() * (TUBE - THICKNESS);
            let start = if k % 3 == 0 { 0.2 } else { 0.0 };
            for i in 0..POINTS {
                let a = start + (1.0 - start) * i as f32 / (POINTS - 1) as f32;
                path.points[i * 4..i * 4 + 4].copy_from_slice(&[home.x, HEIGHT * a, home.y, thickness]);
            }
        }
        let paths: Vec<Vec<i32>> = split_args(&data)
            .iter()
            .enumerate()
            .map(|(k, p)| {
                let mut p = p.to_vec();
                set_path_link(&mut p, PathLink::child(CREATOR, path_id(1, -1, 0), 0.0, k as i32));
                p
            })
            .collect();
        concat_args(paths.iter().map(|p| p.as_slice()).collect())
    }

    struct Run {
        strength: f32,
        range: f32,
        bundling: f32,
        max_offset: f32,
        seed: i32,
    }

    const DEFAULT: Run = Run { strength: 1.0, range: 0.002, bundling: 0.6, max_offset: 0.0006, seed: 1 };

    fn run(input: &[i32], r: &Run) -> Vec<i32> {
        __nodarium_execute_inner(&[
            input,
            &[encode_float(r.strength)],
            &[encode_float(r.range)],
            &[encode_float(r.bundling)],
            &[encode_float(r.max_offset)],
            &[r.seed],
            &[1],
        ])
    }

    fn points(data: &[i32]) -> Vec<Vec<Vec3>> {
        split_args(data)
            .iter()
            .map(|p| wrap_path(p).points.chunks_exact(4).map(|q| Vec3::new(q[0], q[1], q[2])).collect())
            .collect()
    }

    /// The mean distance from every strand to its nearest neighbour across the bundle at `height`
    fn mean_nearest(data: &[i32], height: f32) -> f32 {
        let across: Vec<Vec2> = points(data)
            .iter()
            .filter_map(|p| p.iter().find(|q| (q.y - height).abs() < 0.5 * HEIGHT / (POINTS - 1) as f32).map(|q| Vec2::new(q.x, q.z)))
            .collect();
        let sum: f32 = across
            .iter()
            .enumerate()
            .map(|(k, a)| across.iter().enumerate().filter(|(l, _)| *l != k).map(|(_, b)| a.distance(*b)).fold(f32::MAX, f32::min))
            .sum();
        sum / across.len() as f32
    }

    #[test]
    fn no_strength_changes_nothing() {
        let input = bundle(150);
        assert_eq!(run(&input, &Run { strength: 0.0, ..DEFAULT }), input);
        assert_eq!(run(&input, &Run { max_offset: 0.0, ..DEFAULT }), input);
        assert_ne!(run(&input, &DEFAULT), input);
    }

    #[test]
    fn strands_never_leave_their_tube_by_more_than_the_max_offset() {
        let input = bundle(200);
        let before = points(&input);
        for bundling in [-1.0, 0.0, 1.0] {
            for max_offset in [0.0003, 0.002] {
                let out = run(&input, &Run { bundling, max_offset, range: 0.003, ..DEFAULT });
                let mut furthest = 0.0f32;
                for (a, b) in before.iter().zip(points(&out)) {
                    for (p, q) in a.iter().zip(&b) {
                        let moved = p.distance(*q);
                        assert!(moved <= max_offset * (1.0 + 1e-4), "a point moved {} with a max offset of {}", moved, max_offset);
                        furthest = furthest.max(Vec2::new(q.x, q.z).length() + THICKNESS);
                    }
                }
                assert!(furthest <= TUBE + max_offset * (1.0 + 1e-4), "a strand reaches {} out of a tube of {}", furthest, TUBE);
            }
        }
    }

    #[test]
    fn paths_without_thickness_stay_as_they_are() {
        let input = bundle_of(120, 0.0);
        let out = run(&input, &DEFAULT);
        assert!(points(&out).iter().flatten().all(|q| q.is_finite()));
        assert_eq!(out, input);
    }

    #[test]
    fn an_infinite_max_offset_leaves_the_paths_and_returns() {
        let input = bundle(120);
        for max_offset in [f32::INFINITY, f32::NAN] {
            let out = run(&input, &Run { max_offset, ..DEFAULT });
            assert!(points(&out).iter().flatten().all(|q| q.is_finite()));
        }
        assert_eq!(run(&input, &Run { range: 1e-42, ..DEFAULT }), input);
    }

    #[test]
    fn a_huge_range_or_max_offset_keeps_the_points_finite() {
        let input = bundle(120);
        for r in [Run { range: 1e38, ..DEFAULT }, Run { max_offset: 1e24, ..DEFAULT }] {
            assert!(points(&run(&input, &r)).iter().flatten().all(|q| q.is_finite()));
        }
    }

    #[test]
    fn the_base_of_every_strand_stays() {
        let input = bundle(120);
        let out = run(&input, &DEFAULT);
        for (a, b) in points(&input).iter().zip(points(&out)) {
            assert_eq!(a[0], b[0]);
        }
    }

    #[test]
    fn the_result_is_deterministic_and_follows_the_seed() {
        let input = bundle(150);
        let first = run(&input, &DEFAULT);
        assert_eq!(first, run(&input, &DEFAULT));
        assert_ne!(first, run(&input, &Run { seed: 2, ..DEFAULT }));
    }

    #[test]
    fn bundling_brings_neighbours_together_and_avoiding_spreads_them() {
        let input = bundle(200);
        let height = 0.8 * HEIGHT;
        let still = mean_nearest(&input, height);
        // about two strand spacings
        let range = 0.001;
        let bundled = mean_nearest(&run(&input, &Run { bundling: 1.0, range, ..DEFAULT }), height);
        let spread = mean_nearest(&run(&input, &Run { bundling: -1.0, range, ..DEFAULT }), height);
        assert!(bundled < 0.8 * still, "bundled strands are {} apart, {} before", bundled, still);
        assert!(spread > 1.05 * still, "avoiding strands are {} apart, {} before", spread, still);
    }

    /// How wide the bundle is: the distance from its axis that 95 % of the points stay within
    fn width(data: &[i32]) -> f32 {
        let mut all: Vec<f32> = points(data).iter().flatten().map(|q| Vec2::new(q.x, q.z).length()).collect();
        all.sort_by(f32::total_cmp);
        all[all.len() * 19 / 20]
    }

    #[test]
    fn the_bundle_keeps_its_width() {
        let input = bundle(200);
        let max_offset = 0.002;
        for bundling in [1.0, -1.0, 0.0] {
            let out = run(&input, &Run { bundling, max_offset, range: 0.003, ..DEFAULT });
            let (before, after) = (width(&input), width(&out));
            assert!((after - before).abs() < 0.2 * max_offset, "the bundle goes from {} to {} wide", before, after);
        }
    }

    #[test]
    fn strands_turn_only_across_their_way() {
        let input = bundle(150);
        let out = run(&input, &Run { max_offset: 0.002, ..DEFAULT });
        for (a, b) in points(&input).iter().zip(points(&out)) {
            for (p, q) in a.iter().zip(&b) {
                assert!((p.y - q.y).abs() < 1e-6);
            }
        }
    }

    #[test]
    fn strength_can_change_along_the_strands() {
        // the strength as a curve over the place along the path, none on the lower half
        let alpha = concat_args(vec![&[2], &[0]]);
        let mut parts: Vec<Vec<i32>> = vec![vec![3], alpha, vec![encode_float(0.0)], vec![encode_float(1.0)]];
        parts.extend([0.0, 0.0, 0.5, 0.0, 0.55, 1.0, 1.0, 1.0].iter().map(|&v| vec![encode_float(v)]));
        let strength = concat_args(parts.iter().map(|p| p.as_slice()).collect());

        let input = bundle(150);
        let out = __nodarium_execute_inner(&[
            &input,
            &strength,
            &[encode_float(0.002)],
            &[encode_float(0.6)],
            &[encode_float(0.0006)],
            &[1],
            &[1],
        ]);
        let (mut lower, mut upper) = (0.0f32, 0.0f32);
        for (a, b) in points(&input).iter().zip(points(&out)) {
            let n = a.len();
            for i in 0..n {
                let moved = a[i].distance(b[i]);
                if i < n / 3 {
                    lower = lower.max(moved);
                } else if i > 2 * n / 3 {
                    upper = upper.max(moved);
                }
            }
        }
        assert!(lower < 1e-9, "the lower third moves {}", lower);
        assert!(upper > 1e-4, "the upper third moves {}", upper);
    }
}
