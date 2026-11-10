use glam::{Quat, Vec3, Vec4, Vec4Swizzles};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{
    concat_args,
    geometry::{orthogonal_to, path_link, wrap_path, wrap_path_mut, DeepestPaths},
    int, split_args, Ctx,
};
use std::collections::HashMap;
use std::f32::consts::{PI, TAU};

nodarium_definition_file!("src/definition.json");

/// How many directions across a bundle its reach is measured in, each one both ways
const SIDES: usize = 4;

/// A path of a bundle: its points with their thickness and how far along its own length each
/// one is
struct Member {
    index: usize,
    points: Vec<Vec4>,
    alpha: Vec<f32>,
}

impl Member {
    fn new(index: usize, path: &[i32]) -> Self {
        let points: Vec<Vec4> = wrap_path(path).points.chunks_exact(4).map(Vec4::from_slice).collect();
        let mut along = vec![0.0f32; points.len()];
        for i in 1..points.len() {
            along[i] = along[i - 1] + points[i].xyz().distance(points[i - 1].xyz());
        }
        let length = along.last().copied().unwrap_or(0.0);
        let last = points.len().saturating_sub(1).max(1) as f32;
        let alpha = along
            .iter()
            .enumerate()
            .map(|(i, &l)| if length > 0.0 { l / length } else { i as f32 / last })
            .collect();
        Member { index, points, alpha }
    }

    fn at(&self, alpha: f32) -> Vec4 {
        let j = self.alpha.partition_point(|&a| a < alpha).clamp(1, self.points.len() - 1);
        let (a, b) = (self.alpha[j - 1], self.alpha[j]);
        let t = if b > a { ((alpha - a) / (b - a)).clamp(0.0, 1.0) } else { 0.0 };
        self.points[j - 1].lerp(self.points[j], t)
    }
}

/// The line a bundle runs along, in the middle of its paths at the same share of their length.
/// The mean of the paths drifts to the side where more of them meander, so the middle is the
/// midpoint of how far the bundle reaches to either side, which stays on the axis of a tube
/// that the outer paths touch all the way around.
struct Spine {
    points: Vec<Vec3>,
    tangent: Vec<Vec3>,
}

impl Spine {
    fn new(members: &[Member], count: usize) -> Self {
        let last = (count - 1) as f32;
        let samples: Vec<Vec<Vec4>> =
            (0..count).map(|k| members.iter().map(|m| m.at(k as f32 / last)).collect()).collect();
        let mean: Vec<Vec3> =
            samples.iter().map(|s| s.iter().map(|p| p.xyz()).sum::<Vec3>() / s.len() as f32).collect();
        let tangent: Vec<Vec3> = (0..count)
            .map(|k| (mean[(k + 1).min(count - 1)] - mean[k.saturating_sub(1)]).try_normalize().unwrap_or(Vec3::Y))
            .collect();
        let points = (0..count)
            .map(|k| {
                let across = orthogonal_to(tangent[k]);
                let around = tangent[k].cross(across);
                let mut middle = Vec3::ZERO;
                for axis in 0..SIDES {
                    let angle = PI * axis as f32 / SIDES as f32;
                    let side = across * angle.cos() + around * angle.sin();
                    let (low, high) = samples[k].iter().fold((f32::MAX, f32::MIN), |(low, high), p| {
                        let at = (p.xyz() - mean[k]).dot(side);
                        (low.min(at - p.w.abs()), high.max(at + p.w.abs()))
                    });
                    middle += side * (0.5 * (low + high));
                }
                mean[k] + middle * (2.0 / SIDES as f32)
            })
            .collect();
        Spine { points, tangent }
    }

    /// The spine and its direction at `alpha`, from 0 at the base to 1 at the tip
    fn at(&self, alpha: f32) -> (Vec3, Vec3) {
        let u = alpha.clamp(0.0, 1.0) * (self.points.len() - 1) as f32;
        let k = (u.floor() as usize).min(self.points.len() - 2);
        let t = u - k as f32;
        let tangent = self.tangent[k].lerp(self.tangent[k + 1], t).try_normalize().unwrap_or(self.tangent[k]);
        (self.points[k].lerp(self.points[k + 1], t), tangent)
    }
}

fn twist_bundle(output: &mut [Vec<i32>], indices: &[usize], turns: f32) {
    let members: Vec<Member> =
        indices.iter().map(|&i| Member::new(i, &output[i])).filter(|m| m.points.len() >= 2).collect();
    if members.len() < 2 {
        return;
    }
    let count = members.iter().map(|m| m.points.len()).max().unwrap_or(2);
    let spine = Spine::new(&members, count);

    for member in &members {
        let path = wrap_path_mut(&mut output[member.index]);
        for (j, point) in member.points.iter().enumerate() {
            let alpha = member.alpha[j];
            let (on, tangent) = spine.at(alpha);
            let p = on + Quat::from_axis_angle(tangent, TAU * turns * alpha) * (point.xyz() - on);
            path.points[j * 4..j * 4 + 3].copy_from_slice(&[p.x, p.y, p.z]);
        }
    }
}

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let mut ctx = Ctx::default();
    let plants = split_args(args[0]);
    let depth = int(args[2]).max(1);

    let mut output: Vec<Vec<i32>> = plants.iter().map(|p| p.to_vec()).collect();

    // the paths a creator grew on the same parent are one bundle, e.g. the strands of a guide
    let selected = DeepestPaths::new(&plants, depth);
    let mut bundles: Vec<Vec<usize>> = Vec::new();
    let mut lookup: HashMap<(i32, i32), usize> = HashMap::new();
    for (index, path) in plants.iter().enumerate() {
        let link = path_link(path);
        if !selected.contains(path) || !link.has_parent() {
            continue;
        }
        let bundle = *lookup.entry((link.parent_id, link.creator)).or_insert_with(|| {
            bundles.push(Vec::new());
            bundles.len() - 1
        });
        bundles[bundle].push(index);
    }

    for (rank, indices) in bundles.iter().enumerate() {
        ctx.set_path(&plants[indices[0]], rank, bundles.len());
        let turns = ctx.float(args[1]);
        if turns != 0.0 {
            twist_bundle(&mut output, indices, turns);
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
        geometry::{create_path, path_id, set_path_link, PathLink},
    };
    use std::f32::consts::PI;

    const CREATOR: i32 = 7;
    const POINTS: usize = 24;

    /// The tube of the guide at height `y`: 7 mm at the base, 3.5 mm at the tip of a 5 cm stem
    fn tube_at(y: f32) -> f32 {
        0.007 * (1.0 - 0.5 * y / 0.05)
    }

    /// Where strand `k` of `n` sits across the tube, in a unit disk, like `strands` places them:
    /// an even ring around the rim and a sunflower inside
    fn place(k: usize, n: usize) -> Vec2 {
        let spacing = (PI / n as f32).sqrt();
        let rim = ((TAU / spacing).round() as usize).min(n);
        if k < rim {
            return Vec2::from_angle(TAU * k as f32 / rim as f32);
        }
        let (j, inner) = (k - rim, n - rim);
        let outer = 1.0 - 0.8 * spacing;
        Vec2::from_angle(j as f32 * PI * (3.0 - 5f32.sqrt())) * outer * ((j as f32 + 0.5) / inner as f32).sqrt()
    }

    /// `amount` strands of the guide `guide` that stands up straight at `origin`, as thick as
    /// the tube allows and shortened by the share `short[k % short.len()]`
    fn bundle(amount: usize, guide: i32, origin: Vec3, short: &[f32]) -> Vec<Vec<i32>> {
        meandering(amount, guide, origin, short, Vec2::ZERO)
    }

    /// Like `bundle`, with the strands inside the rim pushed towards `drift` together, as the
    /// wobble of `strands` does, which moves the mean of the bundle off the axis of the guide
    fn meandering(amount: usize, guide: i32, origin: Vec3, short: &[f32], drift: Vec2) -> Vec<Vec<i32>> {
        (0..amount)
            .map(|k| {
                let mut path = create_path(POINTS, 2);
                set_path_link(&mut path, PathLink::child(CREATOR, guide, 0.0, k as i32));
                let end = 1.0 - short[k % short.len()];
                let points = wrap_path_mut(&mut path).points;
                for i in 0..POINTS {
                    let y = 0.05 * end * i as f32 / (POINTS - 1) as f32;
                    let r = 0.0005;
                    let home = place(k, amount);
                    let home = (home + drift * (1.0 - home.length_squared())) * (tube_at(y) - r);
                    points[i * 4..i * 4 + 4].copy_from_slice(&[origin.x + home.x, y, origin.z + home.y, r]);
                }
                path
            })
            .collect()
    }

    fn run(paths: &[Vec<i32>], turns: f32) -> Vec<Vec<i32>> {
        let input = concat_args(paths.iter().map(|p| p.as_slice()).collect());
        let out = __nodarium_execute_inner(&[&input, &[encode_float(turns)], &[1]]);
        split_args(&out).iter().map(|p| p.to_vec()).collect()
    }

    fn points(path: &[i32]) -> Vec<[f32; 4]> {
        wrap_path(path).points.chunks_exact(4).map(|p| [p[0], p[1], p[2], p[3]]).collect()
    }

    fn side(p: [f32; 4], origin: Vec3) -> Vec3 {
        Vec3::new(p[0] - origin.x, 0.0, p[2] - origin.z)
    }

    #[test]
    fn no_turns_change_nothing() {
        let input = bundle(60, 3, Vec3::ZERO, &[0.0]);
        assert_eq!(run(&input, 0.0), input);
    }

    #[test]
    fn the_base_stays_and_the_tip_turns_by_the_turns() {
        let input = bundle(120, 3, Vec3::ZERO, &[0.0]);
        let out = run(&input, 0.5);
        for (before, after) in input.iter().zip(&out) {
            let (a, b) = (points(before), points(after));
            assert!((0..3).all(|i| (a[0][i] - b[0][i]).abs() < 1e-7), "{:?} -> {:?}", a[0], b[0]);
            let tip = POINTS - 1;
            // half a turn puts a strand opposite to where it was
            let (from, to) = (side(a[tip], Vec3::ZERO), side(b[tip], Vec3::ZERO));
            assert!((from + to).length() < 3e-4, "{:?} -> {:?}", from, to);
        }
    }

    #[test]
    fn the_turn_grows_from_base_to_tip() {
        let input = bundle(120, 3, Vec3::ZERO, &[0.0]);
        let out = run(&input, 0.25);
        // a strand on the rim, so its place across the tube is easy to read
        let (before, after) = (points(&input[5]), points(&out[5]));
        let turned = |i: usize| side(before[i], Vec3::ZERO).angle_between(side(after[i], Vec3::ZERO));
        let (quarter, half) = (turned(POINTS / 4), turned(POINTS / 2));
        assert!(quarter > 0.0 && half > quarter, "{} then {}", quarter, half);
        assert!((half - TAU * 0.25 * (POINTS / 2) as f32 / (POINTS - 1) as f32).abs() < 0.05, "{}", half);
    }

    #[test]
    fn twisted_strands_stay_inside_the_tube_of_their_guide() {
        for drift in [Vec2::ZERO, Vec2::new(0.3, 0.1), Vec2::new(-0.2, 0.25)] {
            // strands cut short are not at the same height at the same share of their length, which
            // blurs the middle of a tapering bundle a little
            for (short, room) in [(&[0.0][..], 0.01 * 0.007), (&[0.0, 0.1, 0.03][..], 0.025 * 0.007)] {
                let input = meandering(200, 3, Vec3::ZERO, short, drift);
                let mut worst = f32::MIN;
                for turns in [0.1, 0.5, -2.0] {
                    for strand in run(&input, turns) {
                        for q in points(&strand) {
                            worst = worst.max(side(q, Vec3::ZERO).length() + q[3] - tube_at(q[1]));
                        }
                    }
                }
                assert!(worst < room, "a strand reaches {} m out of its tube at a drift of {}", worst, drift);
            }
        }
    }

    #[test]
    fn strands_keep_their_links_and_thickness() {
        let input = bundle(40, 3, Vec3::ZERO, &[0.0, 0.2]);
        let out = run(&input, 0.7);
        assert_eq!(out.len(), input.len());
        for (before, after) in input.iter().zip(&out) {
            assert_eq!(path_link(before), path_link(after));
            assert_eq!(before.len(), after.len());
            assert_eq!(points(before).iter().map(|p| p[3]).collect::<Vec<_>>(), points(after).iter().map(|p| p[3]).collect::<Vec<_>>());
        }
    }

    #[test]
    fn every_bundle_turns_around_its_own_line() {
        let (a, b) = (Vec3::ZERO, Vec3::new(0.1, 0.0, 0.0));
        let mut input = bundle(60, 3, a, &[0.0]);
        input.extend(bundle(60, 4, b, &[0.0]));
        let out = run(&input, 1.0);
        for (before, after) in input.iter().zip(&out) {
            let origin = if path_link(before).parent_id == 3 { a } else { b };
            let tip = POINTS - 1;
            let (p, q) = (side(points(before)[tip], origin), side(points(after)[tip], origin));
            assert!((p.length() - q.length()).abs() < 0.01 * 0.007, "{} vs {}", p.length(), q.length());
        }
    }

    #[test]
    fn a_path_without_a_parent_stays() {
        let mut root = create_path(POINTS, 2);
        set_path_link(&mut root, PathLink::root(CREATOR, 0));
        let mut input = vec![root];
        input.extend(bundle(30, path_id(CREATOR, -1, 0), Vec3::ZERO, &[0.0]));
        let out = run(&input, 1.0);
        assert_eq!(out[0], input[0]);
        assert_ne!(out[1], input[1]);
    }

    #[test]
    fn a_lone_strand_stays() {
        let input = bundle(1, 3, Vec3::ZERO, &[0.0]);
        assert_eq!(run(&input, 1.0), input);
    }
}
