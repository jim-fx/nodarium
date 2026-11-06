use glam::{Vec2, Vec3};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{
    concat_args, curve, decode_float,
    geometry::{orthogonal_to, path_id, path_link, path_random, wrap_path, wrap_path_mut, DeepestPaths},
    int, split_args, Ctx,
};
use std::collections::HashMap;
use std::f32::consts::{PI, TAU};

nodarium_definition_file!("src/definition.json");

const GOLDEN_ANGLE: f32 = PI * (3.0 - 2.236_068);
/// A path reaches its clump only after this share of its own length, so its base stays put
/// without a kink right above it
const BASE_FADE: f32 = 0.1;
/// How far a clump centre strays from an even pattern across the bundle, in clump spacings
const CLUMP_JITTER: f32 = 0.4;

fn smoothstep(value: f32) -> f32 {
    let t = value.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The points of a curve input, which arrives as `[0, n + 1, ...points, 1, 1]`
fn read_curve(arg: &[i32]) -> Vec<f32> {
    if arg.len() < 4 {
        return Vec::new();
    }
    arg[2..arg.len() - 2].iter().map(|&w| decode_float(w)).collect()
}

fn read_profile(points: &[f32], x: f32) -> f32 {
    if points.len() < 4 {
        1.0
    } else {
        curve(points, x).clamp(0.0, 1.0)
    }
}

/// A path of a bundle: its points and how far along its own length each one is
struct Member {
    index: usize,
    points: Vec<Vec3>,
    alpha: Vec<f32>,
}

impl Member {
    fn new(index: usize, path: &[i32]) -> Self {
        let points: Vec<Vec3> = wrap_path(path).points.chunks_exact(4).map(|p| Vec3::new(p[0], p[1], p[2])).collect();
        let mut along = vec![0.0f32; points.len()];
        for i in 1..points.len() {
            along[i] = along[i - 1] + points[i].distance(points[i - 1]);
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

    fn at(&self, alpha: f32) -> Vec3 {
        let j = self.alpha.partition_point(|&a| a < alpha).clamp(1, self.points.len() - 1);
        let (a, b) = (self.alpha[j - 1], self.alpha[j]);
        let t = if b > a { ((alpha - a) / (b - a)).clamp(0.0, 1.0) } else { 0.0 };
        self.points[j - 1].lerp(self.points[j], t)
    }
}

/// The line a bundle runs along, the mean of its paths at the same share of their length.
/// Every point of a path is placed on it by `project`, which gives the place along the bundle
/// and the offset across it.
struct Spine {
    points: Vec<Vec3>,
    /// unit direction of every segment
    direction: Vec<Vec3>,
    length_squared: Vec<f32>,
    /// a frame across the bundle at every point that turns as little as possible along it
    frame: Vec<(Vec3, Vec3)>,
}

impl Spine {
    fn new(members: &[Member], count: usize) -> Option<Spine> {
        let last = (count - 1) as f32;
        let points: Vec<Vec3> = (0..count)
            .map(|k| members.iter().map(|m| m.at(k as f32 / last)).sum::<Vec3>() / members.len() as f32)
            .collect();
        let segments: Vec<Vec3> = points.windows(2).map(|w| w[1] - w[0]).collect();
        let length: f32 = segments.iter().map(|s| s.length()).sum();
        if !length.is_finite() || length <= 0.0 {
            return None;
        }

        let tangent = |k: usize| {
            (points[(k + 1).min(count - 1)] - points[k.saturating_sub(1)]).try_normalize().unwrap_or(Vec3::Y)
        };
        let mut normal = orthogonal_to(tangent(0));
        let frame = (0..count)
            .map(|k| {
                let t = tangent(k);
                normal = (normal - t * normal.dot(t)).try_normalize().unwrap_or_else(|| orthogonal_to(t));
                (normal, t.cross(normal))
            })
            .collect();

        Some(Spine {
            direction: segments.iter().map(|s| s.normalize_or_zero()).collect(),
            length_squared: segments.iter().map(|s| s.length_squared().max(f32::MIN_POSITIVE)).collect(),
            points,
            frame,
        })
    }

    fn segments(&self) -> usize {
        self.points.len() - 1
    }

    fn param(&self, i: usize, p: Vec3) -> f32 {
        (p - self.points[i]).dot(self.points[i + 1] - self.points[i]) / self.length_squared[i]
    }

    /// The segment nearest to `p`, for the first point of a path
    fn nearest(&self, p: Vec3) -> usize {
        (0..self.segments())
            .map(|i| {
                let t = self.param(i, p).clamp(0.0, 1.0);
                (i, p.distance_squared(self.points[i].lerp(self.points[i + 1], t)))
            })
            .fold((0, f32::MAX), |best, c| if c.1 < best.1 { c } else { best })
            .0
    }

    /// Where `p` is along the spine, starting the search at segment `from`: the segment, the
    /// place `u` along the spine in segments (beyond the ends on the first and last segment)
    /// and the offset from the spine. Paths are walked point by point, so the search only moves
    /// a segment or two each time.
    fn project(&self, p: Vec3, from: usize) -> (usize, f32, Vec3) {
        let last = self.segments() - 1;
        let (mut i, mut step) = (from.min(last), 0);
        let t = loop {
            let t = self.param(i, p);
            if t > 1.0 && i < last && step >= 0 {
                (i, step) = (i + 1, 1);
            } else if t < 0.0 && i > 0 && step <= 0 {
                (i, step) = (i - 1, -1);
            } else {
                break t.clamp(if i == 0 { f32::MIN } else { 0.0 }, if i == last { f32::MAX } else { 1.0 });
            }
        };
        let on = self.points[i] + (self.points[i + 1] - self.points[i]) * t;
        (i, i as f32 + t, p - on)
    }
}

/// The spot `u` along the spine as the two spine points around it and the weight of the second
fn node_weights(u: f32, nodes: usize) -> (usize, usize, f32) {
    let u = u.clamp(0.0, (nodes - 1) as f32);
    let k = (u.floor() as usize).min(nodes - 2);
    (k, k + 1, u - k as f32)
}

/// Clump centres spread evenly over the unit disk across the bundle, each one moved a little
fn clump_centres(count: usize, seed: i32, bundle: i32) -> Vec<Vec2> {
    let spacing = (PI / count as f32).sqrt();
    let turn = TAU * path_random(seed, bundle, 0);
    (0..count)
        .map(|j| {
            let even = Vec2::from_angle(turn + j as f32 * GOLDEN_ANGLE) * ((j as f32 + 0.5) / count as f32).sqrt();
            let random = |salt: i32| path_random(seed, bundle, 1 + 2 * j as i32 + salt);
            let jitter = Vec2::from_angle(TAU * random(0)) * CLUMP_JITTER * spacing * random(1);
            (even + jitter).clamp_length_max(1.0)
        })
        .collect()
}

/// Fills the spine points no path reaches, those without weight, with the nearest one it does reach
fn fill_gaps<T: Copy>(values: &mut [T], weights: &[f32]) {
    let mut last = None;
    let mut missing = Vec::new();
    for k in 0..values.len() {
        if weights[k] > 0.0 {
            let value = values[k];
            for &m in &missing {
                values[m] = match last {
                    Some((l, before)) if m - l < k - m => before,
                    _ => value,
                };
            }
            missing.clear();
            last = Some((k, value));
        } else {
            missing.push(k);
        }
    }
    if let Some((_, value)) = last {
        for &m in &missing {
            values[m] = value;
        }
    }
}

struct Params {
    strength: f32,
    clumps: usize,
    profile: Vec<f32>,
    seed: i32,
}

/// Pulls the paths of one bundle onto the centres of their clumps
fn clump_bundle(output: &mut [Vec<i32>], indices: &[usize], bundle: i32, params: &Params) {
    let members: Vec<Member> =
        indices.iter().map(|&i| Member::new(i, &output[i])).filter(|m| m.points.len() >= 2).collect();
    if members.len() < 2 {
        return;
    }
    let nodes = members.iter().map(|m| m.points.len()).max().unwrap_or(2);
    let Some(spine) = Spine::new(&members, nodes) else {
        return;
    };

    // every point of every path: its segment, its place along the spine and its offset
    let projected: Vec<Vec<(usize, f32, Vec3)>> = members
        .iter()
        .map(|m| {
            let mut from = spine.nearest(m.points[0]);
            m.points
                .iter()
                .map(|&p| {
                    let hit = spine.project(p, from);
                    from = hit.0;
                    hit
                })
                .collect()
        })
        .collect();

    // how wide the bundle is along the spine, so a path is placed across it by its share of the
    // width wherever along the bundle it starts
    let mut width = vec![0.0f32; nodes];
    for &(_, u, offset) in projected.iter().flatten() {
        let k = (u.round().max(0.0) as usize).min(nodes - 1);
        width[k] = width[k].max(offset.length());
    }
    let reached = width.clone();
    fill_gaps(&mut width, &reached);

    let count = params.clumps.clamp(1, members.len());
    let centres = clump_centres(count, params.seed, bundle);
    // a path joins the clump nearest to where it is when the pull takes hold, not at its base,
    // where the blades of a tuft all start in one spot
    let clump_of: Vec<usize> = projected
        .iter()
        .zip(&members)
        .map(|(points, member)| {
            let hold = member.alpha.iter().position(|&a| a >= BASE_FADE).unwrap_or(points.len() - 1);
            let (_, u, offset) = points[hold];
            let k = (u.round().max(0.0) as usize).min(nodes - 1);
            let (normal, binormal) = spine.frame[k];
            let across = Vec2::new(offset.dot(normal), offset.dot(binormal)) / width[k].max(f32::MIN_POSITIVE);
            (0..count)
                .map(|c| (c, across.distance_squared(centres[c])))
                .fold((0, f32::MAX), |best, c| if c.1 < best.1 { c } else { best })
                .0
        })
        .collect();

    // the centre of a clump is the mean offset of its paths at every spine point
    let mut centre = vec![vec![Vec3::ZERO; nodes]; count];
    let mut weight = vec![vec![0.0f32; nodes]; count];
    for (points, &c) in projected.iter().zip(&clump_of) {
        for &(_, u, offset) in points {
            let (a, b, t) = node_weights(u, nodes);
            centre[c][a] += offset * (1.0 - t);
            centre[c][b] += offset * t;
            weight[c][a] += 1.0 - t;
            weight[c][b] += t;
        }
    }
    for (centre, weight) in centre.iter_mut().zip(&weight) {
        for (c, &w) in centre.iter_mut().zip(weight) {
            if w > 0.0 {
                *c /= w;
            }
        }
        fill_gaps(centre, weight);
    }

    let last = (nodes - 1) as f32;
    for ((member, points), &c) in members.iter().zip(&projected).zip(&clump_of) {
        let path = wrap_path_mut(&mut output[member.index]);
        for (j, &(segment, u, offset)) in points.iter().enumerate() {
            let pull = params.strength
                * read_profile(&params.profile, (u / last).clamp(0.0, 1.0))
                * smoothstep(member.alpha[j] / BASE_FADE);
            if pull <= 0.0 {
                continue;
            }
            let (a, b, t) = node_weights(u, nodes);
            let target = centre[c][a].lerp(centre[c][b], t);
            // only across the spine, so a path keeps its place along the bundle
            let tangent = spine.direction[segment];
            let delta = target - offset;
            let p = member.points[j] + (delta - tangent * delta.dot(tangent)) * pull;
            path.points[j * 4..j * 4 + 3].copy_from_slice(&[p.x, p.y, p.z]);
        }
    }
}

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let mut ctx = Ctx::default();

    let plants = split_args(args[0]);
    let params = Params {
        strength: ctx.float(args[1]).clamp(0.0, 1.0),
        clumps: ctx.int(args[2]).max(1) as usize,
        profile: read_curve(args[3]),
        seed: int(args[4]),
    };
    let depth = int(args[5]).max(1);

    let mut output: Vec<Vec<i32>> = plants.iter().map(|p| p.to_vec()).collect();
    if params.strength > 0.0 {
        // the paths a creator grew on the same parent are one bundle, e.g. the strands of a guide
        let selected = DeepestPaths::new(&plants, depth);
        let mut bundles: Vec<(i32, Vec<usize>)> = Vec::new();
        let mut lookup: HashMap<(i32, i32), usize> = HashMap::new();
        for (index, path) in plants.iter().enumerate() {
            if !selected.contains(path) {
                continue;
            }
            let link = path_link(path);
            let key = (link.parent_id, link.creator);
            let bundle = *lookup.entry(key).or_insert_with(|| {
                bundles.push((path_id(link.creator, link.parent_id, -1), Vec::new()));
                bundles.len() - 1
            });
            bundles[bundle].1.push(index);
        }
        for (bundle, indices) in &bundles {
            clump_bundle(&mut output, indices, *bundle, &params);
        }
    }

    concat_args(output.iter().map(|p| p.as_slice()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodarium_utils::{
        encode_float,
        geometry::{create_multiple_paths, set_path_link, wrap_multiple_paths, PathLink},
    };

    const CREATOR: i32 = 7;
    const POINTS: usize = 24;

    /// `amount` strands of a guide `guide` that stands at `origin`: straight up 5 cm in a tube of
    /// 7 mm, spread like a sunflower, every third one starting higher up, turned by `twist`
    fn bundle(amount: usize, guide: i32, origin: Vec3, twist: f32) -> Vec<Vec<i32>> {
        let mut data = create_multiple_paths(amount, POINTS, 2, CREATOR);
        for (k, path) in wrap_multiple_paths(&mut data).iter_mut().enumerate() {
            let home = Vec2::from_angle(k as f32 * GOLDEN_ANGLE) * ((k as f32 + 0.5) / amount as f32).sqrt();
            let start = if k % 3 == 0 { 0.2 } else { 0.0 };
            for i in 0..POINTS {
                let a = start + (1.0 - start) * i as f32 / (POINTS - 1) as f32;
                let p = Vec2::from_angle(TAU * twist * a).rotate(home) * 0.0065;
                path.points[i * 4..i * 4 + 4].copy_from_slice(&[origin.x + p.x, 0.05 * a, origin.z + p.y, 0.0005]);
            }
        }
        split_args(&data)
            .iter()
            .enumerate()
            .map(|(k, p)| {
                let mut p = p.to_vec();
                set_path_link(&mut p, PathLink::child(CREATOR, guide, 0.0, k as i32));
                p
            })
            .collect()
    }

    fn run(paths: &[Vec<i32>], strength: f32, clumps: i32, profile: &[f32], seed: i32) -> Vec<Vec<i32>> {
        let input = concat_args(paths.iter().map(|p| p.as_slice()).collect());
        let mut curve = vec![0, profile.len() as i32 + 1];
        curve.extend(profile.iter().map(|&v| encode_float(v)));
        curve.extend([1, 1]);
        let out = __nodarium_execute_inner(&[&input, &[encode_float(strength)], &[clumps], &curve, &[seed], &[1]]);
        split_args(&out).iter().map(|p| p.to_vec()).collect()
    }

    fn points(path: &[i32]) -> Vec<Vec3> {
        wrap_path(path).points.chunks_exact(4).map(|p| Vec3::new(p[0], p[1], p[2])).collect()
    }

    const FLAT: [f32; 4] = [0.0, 1.0, 1.0, 1.0];
    /// The spine is the mean of the strands and leans a little, so points at the same height sit
    /// a few micrometres apart along it
    const SAME: f32 = 1e-5;

    #[test]
    fn no_strength_changes_nothing() {
        let input = bundle(120, 3, Vec3::ZERO, 0.3);
        let encoded = concat_args(input.iter().map(|p| p.as_slice()).collect());
        let out = run(&input, 0.0, 8, &FLAT, 1);
        assert_eq!(concat_args(out.iter().map(|p| p.as_slice()).collect()), encoded);
    }

    #[test]
    fn full_strength_collapses_every_clump_to_one_curve() {
        let input = bundle(200, 3, Vec3::ZERO, 0.0);
        let out = run(&input, 1.0, 8, &FLAT, 1);
        // the tip of every strand, every clump ends in one spot
        let mut tips: Vec<Vec3> = Vec::new();
        // the first strand of every clump that starts at the base and the first that starts
        // higher up, the others are sampled at the same heights as one of them
        let mut curves: HashMap<(usize, bool), Vec<Vec3>> = HashMap::new();
        for (k, s) in out.iter().enumerate() {
            let p = points(s);
            let tip = p[POINTS - 1];
            let clump = tips.iter().position(|t| t.distance(tip) < SAME).unwrap_or_else(|| {
                tips.push(tip);
                tips.len() - 1
            });
            let curve = curves.entry((clump, k % 3 == 0)).or_insert_with(|| p.clone());
            // past the fade at the base the strands of a clump run on one curve
            for (a, b) in p.iter().zip(curve.iter()).skip(POINTS / 4) {
                assert!(a.distance(*b) < SAME, "{} apart", a.distance(*b));
            }
        }
        assert!((2..=8).contains(&tips.len()), "{} clumps", tips.len());
    }

    #[test]
    fn half_strength_goes_half_way() {
        let input = bundle(150, 3, Vec3::ZERO, 0.2);
        let (half, full) = (run(&input, 0.5, 6, &FLAT, 2), run(&input, 1.0, 6, &FLAT, 2));
        for ((a, h), f) in input.iter().zip(&half).zip(&full) {
            for ((a, h), f) in points(a).iter().zip(points(h)).zip(points(f)) {
                assert!(h.distance(a.lerp(f, 0.5)) < 1e-6);
            }
        }
    }

    #[test]
    fn the_base_stays_put_and_paths_keep_their_height() {
        let input = bundle(150, 3, Vec3::ZERO, 0.4);
        let out = run(&input, 1.0, 5, &FLAT, 2);
        let mut moved = 0.0f32;
        for (a, b) in input.iter().zip(&out) {
            let (a, b) = (points(a), points(b));
            assert_eq!(a[0], b[0]);
            for (p, q) in a.iter().zip(&b) {
                assert!((p.y - q.y).abs() < SAME, "a point moved from {} to {} along the bundle", p.y, q.y);
                moved = moved.max(p.distance(*q));
            }
        }
        assert!(moved > 0.001, "nothing moved");
    }

    #[test]
    fn thickness_and_links_stay() {
        let input = bundle(80, 3, Vec3::ZERO, 0.4);
        for (a, b) in input.iter().zip(run(&input, 1.0, 5, &FLAT, 2)) {
            assert_eq!(a[..9], b[..9]);
            let (pa, pb) = (wrap_path(a).points, wrap_path(&b).points);
            for i in 0..POINTS {
                assert_eq!(pa[i * 4 + 3], pb[i * 4 + 3]);
            }
        }
    }

    #[test]
    fn it_is_deterministic_and_the_seed_matters() {
        let input = bundle(150, 3, Vec3::ZERO, 0.2);
        assert_eq!(run(&input, 0.7, 6, &FLAT, 4), run(&input, 0.7, 6, &FLAT, 4));
        assert_ne!(run(&input, 0.7, 6, &FLAT, 4), run(&input, 0.7, 6, &FLAT, 5));
    }

    #[test]
    fn the_profile_lets_the_tips_loose() {
        let input = bundle(150, 3, Vec3::ZERO, 0.0);
        let out = run(&input, 1.0, 6, &[0.0, 1.0, 1.0, 0.0], 2);
        let mut middle = 0.0f32;
        for (a, b) in input.iter().zip(&out) {
            let (a, b) = (points(a), points(b));
            assert!(a[POINTS - 1].distance(b[POINTS - 1]) < 1e-6);
            middle = middle.max(a[POINTS / 2].distance(b[POINTS / 2]));
        }
        assert!(middle > 0.0005);
    }

    #[test]
    fn neighbours_at_the_base_end_up_in_the_same_clump() {
        let input = bundle(200, 3, Vec3::ZERO, 0.0);
        let out = run(&input, 1.0, 6, &FLAT, 3);
        let (mut together, mut pairs) = (0, 0);
        for i in 0..input.len() {
            for j in i + 1..input.len() {
                if points(&input[i])[POINTS - 1].distance(points(&input[j])[POINTS - 1]) < 0.0008 {
                    pairs += 1;
                    if points(&out[i])[POINTS - 1].distance(points(&out[j])[POINTS - 1]) < SAME {
                        together += 1;
                    }
                }
            }
        }
        assert!(together * 10 > pairs * 7, "{} of {} close pairs share a clump", together, pairs);
    }

    /// Blades of grass that all grow from one spot and fan out, like a tuft
    fn tuft(amount: usize) -> Vec<Vec<i32>> {
        let mut data = create_multiple_paths(amount, 12, 1, CREATOR);
        for (k, path) in wrap_multiple_paths(&mut data).iter_mut().enumerate() {
            let out = Vec2::from_angle(k as f32 * GOLDEN_ANGLE) * (0.3 + 0.7 * (k as f32 + 0.5) / amount as f32);
            for i in 0..12 {
                let a = i as f32 / 11.0;
                path.points[i * 4..i * 4 + 4].copy_from_slice(&[out.x * 0.1 * a * a, 0.2 * a, out.y * 0.1 * a * a, 0.002]);
            }
        }
        split_args(&data).iter().map(|p| p.to_vec()).collect()
    }

    #[test]
    fn blades_of_a_tuft_bundle_at_their_tips() {
        let input = tuft(60);
        let out = run(&input, 1.0, 5, &FLAT, 1);
        let mut tips: Vec<Vec3> = Vec::new();
        for (a, b) in input.iter().zip(&out) {
            let (a, b) = (points(a), points(b));
            assert_eq!(a[0], b[0]);
            assert!(b.iter().all(|p| p.is_finite()));
            // the tips of a clump meet, give or take how far apart along the tuft they end
            let tip = b[11];
            if !tips.iter().any(|t| t.distance(tip) < 0.001) {
                tips.push(tip);
            }
        }
        assert!((2..=5).contains(&tips.len()), "the tips end in {} spots", tips.len());
    }

    #[test]
    fn a_bent_bundle_clumps_along_its_bend() {
        // the bundle bends over by 45° at the tip
        let input: Vec<Vec<i32>> = bundle(120, 3, Vec3::ZERO, 0.0)
            .into_iter()
            .map(|mut s| {
                let path = wrap_path_mut(&mut s);
                for i in 0..POINTS {
                    let p = Vec3::from_slice(&path.points[i * 4..i * 4 + 3]);
                    let bent = glam::Quat::from_rotation_z(-(PI / 4.0) * p.y / 0.05) * Vec3::new(p.x, 0.0, p.z)
                        + Vec3::new(0.03 * (p.y / 0.05).powi(2), p.y, 0.0);
                    path.points[i * 4..i * 4 + 3].copy_from_slice(&bent.to_array());
                }
                s
            })
            .collect();
        let out = run(&input, 1.0, 6, &FLAT, 1);
        // the tips end a little apart along the bent bundle, so they are compared across it
        let along = out.iter().map(|s| { let p = points(s); p[POINTS - 1] - p[POINTS - 2] }).sum::<Vec3>().normalize();
        let across = |v: Vec3| v - along * v.dot(along);
        let mut tips: Vec<Vec3> = Vec::new();
        for (a, b) in input.iter().zip(&out) {
            let (a, b) = (points(a), points(b));
            assert_eq!(a[0], b[0]);
            for (p, q) in a.iter().zip(&b) {
                // pulled across the bundle, never further than its width
                assert!(p.distance(*q) < 0.014, "a point moved by {}", p.distance(*q));
            }
            let tip = b[POINTS - 1];
            if !tips.iter().any(|t| across(*t - tip).length() < 2e-4) {
                tips.push(tip);
            }
        }
        assert!((2..=6).contains(&tips.len()), "the tips end in {} spots", tips.len());
    }

    #[test]
    fn bundles_of_different_guides_stay_apart() {
        let mut input = bundle(60, 3, Vec3::ZERO, 0.0);
        input.extend(bundle(60, 4, Vec3::new(0.05, 0.0, 0.0), 0.0));
        let out = run(&input, 1.0, 4, &FLAT, 1);
        for (k, s) in out.iter().enumerate() {
            let x = if k < 60 { 0.0 } else { 0.05 };
            for p in points(s) {
                assert!((p.x - x).abs() < 0.007, "a strand of guide {} reached x = {}", k / 60, p.x);
            }
        }
    }
}
