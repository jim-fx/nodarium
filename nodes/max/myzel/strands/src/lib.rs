use glam::{Vec2, Vec3};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{
    concat_arg_vecs,
    geometry::{create_path, orthogonal_to, path_id, path_random, set_path_link, wrap_path, wrap_path_mut, DeepestPaths, PathLink},
    int, split_args, Ctx, PathInfo,
};
use noise::{NoiseFn, OpenSimplex};
use std::f32::consts::{PI, TAU};

nodarium_definition_file!("src/definition.json");

/// Neighbouring strands are this far apart in angle, so no two line up across the tube
const GOLDEN_ANGLE: f32 = PI * (3.0 - 2.236_068);
/// Fewer strands than this are too few for a ring around the bundle
const MIN_STRANDS_FOR_RIM: usize = 12;
/// The inner strands keep this many strand spacings away from the outer ring
const RIM_GAP: f32 = 0.8;
/// Length of one meander, in radii of the guide where it is thickest
const WOBBLE_WAVELENGTH: f32 = 3.0;
/// Strands closer than this across the unit disk meander about the same way
const WOBBLE_COHERENCE: f32 = 0.8;
/// OpenSimplex rarely leaves ±0.5, this brings a wobble of 1 close to the full radius
const WOBBLE_GAIN: f32 = 2.0;
/// The most a length variation of 1 takes off either end of a strand, as a share of the guide
const MAX_SHORTENING: f32 = 0.5;

/// A frame that turns as little as possible along the path (parallel transport), so a strand
/// placed in it doesn't wind around the guide where the guide bends
fn frames(points: &[Vec3]) -> Vec<(Vec3, Vec3)> {
    let n = points.len();
    let tangent = |i: usize| {
        let (a, b) = (points[i.saturating_sub(1)], points[(i + 1).min(n - 1)]);
        (b - a).try_normalize().unwrap_or(Vec3::Y)
    };
    let mut normal = orthogonal_to(tangent(0));
    (0..n)
        .map(|i| {
            let t = tangent(i);
            normal = (normal - t * normal.dot(t)).try_normalize().unwrap_or_else(|| orthogonal_to(t));
            (normal, t.cross(normal))
        })
        .collect()
}

/// Where strand `k` of `n` sits across the tube, in a unit disk. Bundles with enough strands
/// get an evenly spaced outer ring: a plain sunflower leaves its rim ragged, and the gaps
/// between rim strands show on the surface as dark slits.
fn place(k: usize, n: usize) -> Vec2 {
    let spacing = (PI / n as f32).sqrt();
    let rim = if n >= MIN_STRANDS_FOR_RIM { ((TAU / spacing).round() as usize).min(n) } else { 0 };
    if k < rim {
        return Vec2::from_angle(TAU * k as f32 / rim as f32);
    }
    let (j, inner) = (k - rim, n - rim);
    let outer = if rim > 0 { 1.0 - RIM_GAP * spacing } else { 1.0 };
    Vec2::from_angle(j as f32 * GOLDEN_ANGLE) * outer * ((j as f32 + 0.5) / inner as f32).sqrt()
}

/// A position in the unit disk that went past the rim is folded back in, so a strand pushed
/// out bounces off the wall of the tube instead of piling up on it
fn fold_into_disk(p: Vec2) -> Vec2 {
    let length = p.length();
    if length <= 1.0 {
        p
    } else {
        p * ((2.0 - length).max(0.0) / length)
    }
}

/// The guide sampled at `u`, a position between its points (0 the first, `n - 1` the last)
struct Guide {
    centre: Vec<Vec3>,
    radius: Vec<f32>,
    frame: Vec<(Vec3, Vec3)>,
    alpha: Vec<f32>,
    along: Vec<f32>,
}

impl Guide {
    fn at(&self, u: f32) -> (Vec3, f32, Vec3, Vec3, f32, f32) {
        let last = self.centre.len() - 1;
        let i = (u.floor() as usize).min(last.saturating_sub(1));
        let j = (i + 1).min(last);
        let t = (u - i as f32).clamp(0.0, 1.0);
        let normal = self.frame[i].0.lerp(self.frame[j].0, t).normalize_or_zero();
        let binormal = self.frame[i].1.lerp(self.frame[j].1, t).normalize_or_zero();
        let lerp = |v: &[f32]| v[i] + (v[j] - v[i]) * t;
        (self.centre[i].lerp(self.centre[j], t), lerp(&self.radius), normal, binormal, lerp(&self.alpha), lerp(&self.along))
    }
}

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let mut ctx = Ctx::default();
    let paths = split_args(args[0]);
    let seed = int(args[8]);
    // the graph node that makes the strands, it is the last input
    let creator = int(args[9]);

    let guides = DeepestPaths::new(&paths, 1);
    let guide_count = guides.count(&paths);
    let mut rank = 0;
    let mut output: Vec<Vec<i32>> = Vec::new();

    for path_data in paths.iter() {
        if !guides.contains(path_data) {
            output.push(path_data.to_vec());
            continue;
        }
        ctx.set_path(path_data, rank, guide_count);
        rank += 1;

        let path = wrap_path(path_data);
        let n = path.length;
        let centre: Vec<Vec3> = path.points.chunks_exact(4).map(|p| Vec3::new(p[0], p[1], p[2])).collect();
        let radius: Vec<f32> = path.points.chunks_exact(4).map(|p| p[3].abs()).collect();
        let widest = radius.iter().copied().fold(0.0, f32::max);
        let mut along = vec![0.0f32; n];
        for i in 1..n {
            along[i] = along[i - 1] + centre[i].distance(centre[i - 1]);
        }
        let length = along[n - 1];
        let alpha = along.iter().enumerate().map(|(i, &l)| if length > 0.0 { l / length } else { i as f32 / (n - 1).max(1) as f32 }).collect();
        let guide = Guide { frame: frames(&centre), centre, radius, alpha, along };

        let amount = ctx.int(args[1]).max(0);
        let thickness_variation = ctx.float(args[3]).clamp(0.0, 1.0);
        let spread = ctx.float(args[4]).clamp(0.0, 1.0);
        let twist = ctx.float(args[5]);
        let wobble = ctx.float(args[6]).clamp(0.0, 1.0);
        let length_variation = ctx.float(args[7]).clamp(0.0, 1.0);

        let noise_seed = seed.wrapping_add(path.link.id) as u32;
        let (noise_x, noise_y) = (OpenSimplex::new(noise_seed), OpenSimplex::new(noise_seed.wrapping_add(1)));
        let wavelength = if widest > 0.0 { WOBBLE_WAVELENGTH * widest } else { length.max(f32::EPSILON) };
        let turn = TAU * path_random(seed, path.link.id, 1);

        for k in 0..amount {
            let id = path_id(creator, path.link.id, k);
            let random = |salt: i32| path_random(seed, id, salt);
            let (start, end) = (
                length_variation * MAX_SHORTENING * random(2).powi(3),
                1.0 - length_variation * MAX_SHORTENING * random(3).powi(3),
            );
            let span = (n - 1) as f32;
            let u: Vec<f32> = (0..n).map(|j| span * (start + (end - start) * j as f32 / span.max(1.0))).collect();
            let samples: Vec<_> = u.iter().map(|&u| guide.at(u)).collect();

            let link = PathLink::child(creator, path.link.id, samples[0].4, k);
            ctx.set_path_info(PathInfo {
                index: k as usize,
                rank: k as usize,
                count: amount as usize,
                depth: path.depth + 1,
                parent_position: samples[0].4,
                length,
            });
            ctx.seed_path(link.id);
            let thickness = ctx.float_along(args[2], samples.iter().map(|s| s.4));
            let thinner = 1.0 - thickness_variation * random(4);

            let home = place(k as usize, amount as usize) * spread;
            let mut strand_data = create_path(n, path.depth + 1);
            set_path_link(&mut strand_data, link);
            let strand = wrap_path_mut(&mut strand_data);
            for (j, &(c, tube, normal, binormal, a, s)) in samples.iter().enumerate() {
                let profile = if widest > 0.0 { tube / widest } else { 0.0 };
                let r = (thickness[j].max(0.0) * thinner * profile).min(tube);
                // sampled at the strand's home, so neighbours get about the same push
                let q = [(home.x / WOBBLE_COHERENCE) as f64, (home.y / WOBBLE_COHERENCE) as f64, (s / wavelength) as f64];
                let push = (Vec2::new(noise_x.get(q) as f32, noise_y.get(q) as f32) * WOBBLE_GAIN).clamp_length_max(1.0);
                // strands near the surface mostly slide along it: moving in and out together,
                // a whole side of them would dent the stipe in one broad crease
                let outward = home.normalize_or_zero();
                let radial = outward * push.dot(outward);
                let push = push - radial * home.length_squared();
                let p = fold_into_disk(home + push * wobble * spread);
                let p = Vec2::from_angle(turn + TAU * twist * a).rotate(p);
                let point = c + (normal * p.x + binormal * p.y) * (tube - r);
                strand.points[j * 4..j * 4 + 4].copy_from_slice(&[point.x, point.y, point.z, r]);
            }
            output.push(strand_data);
        }
    }

    concat_arg_vecs(output)
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use glam::Vec3;
    use std::f32::consts::TAU;
    use nodarium_utils::{
        encode_float,
        geometry::{create_multiple_paths, path_link, wrap_multiple_paths, wrap_path, PathLink},
        split_args,
    };

    const CREATOR: i32 = 9;

    /// A tapering stem bent to the side, 5 cm tall and 7 mm thick at the base
    fn guide() -> Vec<i32> {
        let mut data = create_multiple_paths(1, 24, 1, 3);
        for path in wrap_multiple_paths(&mut data).iter_mut() {
            for i in 0..24 {
                let a = i as f32 / 23.0;
                path.points[i * 4..i * 4 + 4].copy_from_slice(&[0.01 * a * a, 0.05 * a, 0.0, 0.007 * (1.0 - 0.5 * a)]);
            }
        }
        data
    }

    /// A straight stem from 0 to 5 cm, 7 mm thick at the base and 3.5 mm at the tip, so the
    /// tube at a height is known without knowing where on the guide a strand point came from
    fn straight_guide() -> Vec<i32> {
        let mut data = create_multiple_paths(1, 24, 1, 3);
        for path in wrap_multiple_paths(&mut data).iter_mut() {
            for i in 0..24 {
                let a = i as f32 / 23.0;
                path.points[i * 4..i * 4 + 4].copy_from_slice(&[0.0, 0.05 * a, 0.0, tube_at(0.05 * a)]);
            }
        }
        data
    }

    fn tube_at(y: f32) -> f32 {
        0.007 * (1.0 - 0.5 * y / 0.05)
    }

    #[derive(Clone, Copy)]
    struct Params {
        amount: i32,
        thickness_variation: f32,
        spread: f32,
        twist: f32,
        wobble: f32,
        length_variation: f32,
    }

    const PLAIN: Params =
        Params { amount: 30, thickness_variation: 0.0, spread: 1.0, twist: 0.0, wobble: 0.0, length_variation: 0.0 };

    fn grow(guide: &[i32], p: Params) -> Vec<Vec<i32>> {
        let out = __nodarium_execute_inner(&[
            guide,
            &[p.amount],
            &[encode_float(0.0006)],
            &[encode_float(p.thickness_variation)],
            &[encode_float(p.spread)],
            &[encode_float(p.twist)],
            &[encode_float(p.wobble)],
            &[encode_float(p.length_variation)],
            &[4],
            &[CREATOR],
        ]);
        split_args(&out).iter().map(|s| s.to_vec()).collect()
    }

    fn strands(guide: &[i32], amount: i32, spread: f32, twist: f32) -> Vec<Vec<i32>> {
        grow(guide, Params { amount, spread, twist, ..PLAIN })
    }

    #[test]
    fn wobbling_varied_strands_stay_inside_the_tube() {
        let g = straight_guide();
        for wobble in [0.0, 0.5, 1.0] {
            let p = Params { amount: 200, thickness_variation: 1.0, twist: 0.7, wobble, length_variation: 1.0, ..PLAIN };
            for s in grow(&g, p) {
                for q in points(&s) {
                    let offset = Vec3::new(q[0], 0.0, q[2]).length();
                    let tube = tube_at(q[1]);
                    assert!(offset + q[3] <= tube + 1e-6, "strand reaches {} out of a tube of {}", offset + q[3], tube);
                }
            }
        }
    }

    #[test]
    fn neighbouring_strands_wobble_together() {
        let g = straight_guide();
        let still = grow(&g, Params { amount: 200, ..PLAIN });
        let moving = grow(&g, Params { amount: 200, wobble: 0.6, ..PLAIN });
        let push = |k: usize, i: usize| {
            let (a, b) = (points(&still[k])[i], points(&moving[k])[i]);
            Vec3::new(b[0] - a[0], 0.0, b[2] - a[2])
        };
        let home = |k: usize| {
            let a = points(&still[k])[0];
            Vec3::new(a[0], 0.0, a[2])
        };
        let (mut near, mut far) = ((0.0, 0), (0.0, 0));
        for k in 60..200 {
            for l in 60..200 {
                let d = home(k).distance(home(l));
                if k == l || (d > 0.0015 && d < 0.006) {
                    continue;
                }
                let difference = (0..24).map(|i| push(k, i).distance(push(l, i))).sum::<f32>();
                let sums = if d <= 0.0015 { &mut near } else { &mut far };
                sums.0 += difference;
                sums.1 += 1;
            }
        }
        let (near, far) = (near.0 / near.1 as f32, far.0 / far.1 as f32);
        assert!(moving.iter().zip(&still).any(|(m, s)| m != s));
        assert!(near < 0.5 * far, "neighbours differ by {} in their push, distant strands by {}", near, far);
    }

    #[test]
    fn thickness_and_length_vary_within_bounds() {
        let g = straight_guide();
        let strands = grow(&g, Params { amount: 200, thickness_variation: 0.5, length_variation: 1.0, ..PLAIN });
        let (mut thinnest, mut shortened) = (f32::MAX, 0);
        for s in &strands {
            let p = points(s);
            let base = p[0];
            // the base thickness relative to the full thickness there
            let share = base[3] / (0.0006 * tube_at(base[1]) / 0.007);
            assert!((0.5 - 1e-4..=1.0 + 1e-4).contains(&share), "thickness share {}", share);
            thinnest = thinnest.min(share);
            let (start, end) = (p[0][1], p[p.len() - 1][1]);
            assert!(start >= -1e-6 && start <= 0.025 + 1e-6, "starts at {}", start);
            assert!(end <= 0.05 + 1e-6 && end >= 0.025 - 1e-6, "ends at {}", end);
            if start > 0.001 || end < 0.049 {
                shortened += 1;
            }
            let link = path_link(s);
            assert!((link.parent_alpha - start / 0.05).abs() < 1e-4);
        }
        assert!(thinnest < 0.6, "the thinnest strand keeps {} of the thickness", thinnest);
        assert!(shortened > 50, "only {} of 200 strands are shorter", shortened);
        // most strands still run close to the whole length, so the silhouette stays
        let full = strands.iter().filter(|s| { let p = points(s); p[0][1] < 0.005 && p[p.len() - 1][1] > 0.045 }).count();
        assert!(full > 50, "only {} of 200 strands run nearly the whole length", full);
    }

    #[test]
    fn a_large_bundle_has_an_even_outer_ring() {
        let g = straight_guide();
        let base = points(split_args(&g)[0])[0];
        let room = base[3] - 0.0006;
        let mut rim: Vec<f32> = grow(&g, Params { amount: 200, ..PLAIN })
            .iter()
            .map(|s| points(s)[0])
            .filter(|p| Vec3::new(p[0], 0.0, p[2]).length() > 0.99 * room)
            .map(|p| p[2].atan2(p[0]))
            .collect();
        assert!(rim.len() > 30, "{} strands on the rim", rim.len());
        rim.sort_by(f32::total_cmp);
        let gaps: Vec<f32> = rim.windows(2).map(|w| w[1] - w[0]).chain([rim[0] + TAU - rim[rim.len() - 1]]).collect();
        let (smallest, largest) = gaps.iter().fold((f32::MAX, 0.0f32), |(a, b), &g| (a.min(g), b.max(g)));
        assert!(largest < 1.1 * smallest, "gaps between rim strands from {} to {}", smallest, largest);
    }

    fn points(path: &[i32]) -> Vec<[f32; 4]> {
        wrap_path(path).points.chunks_exact(4).map(|p| [p[0], p[1], p[2], p[3]]).collect()
    }

    #[test]
    fn every_strand_is_a_child_of_its_guide() {
        let g = guide();
        let parent = wrap_path(split_args(&g)[0]).link;
        let out = strands(&g, 30, 1.0, 0.5);
        assert_eq!(out.len(), 30);
        for (k, s) in out.iter().enumerate() {
            let link = path_link(s);
            assert_eq!(link, PathLink::child(CREATOR, parent.id, 0.0, k as i32));
            assert_eq!(s[3], 2);
        }
    }

    #[test]
    fn strands_stay_inside_the_tube_and_keep_their_place_across_it() {
        let g = guide();
        let guide_points = points(split_args(&g)[0]);
        for twist in [0.0, 1.5] {
            for s in strands(&g, 50, 1.0, twist) {
                let p = points(&s);
                let mut place = None;
                for (i, q) in p.iter().enumerate() {
                    let c = Vec3::new(guide_points[i][0], guide_points[i][1], guide_points[i][2]);
                    let offset = Vec3::new(q[0], q[1], q[2]).distance(c);
                    let r = guide_points[i][3];
                    assert!(offset + q[3] <= r + 1e-6, "strand reaches {} out of a tube of {}", offset + q[3], r);
                    // where across the tube the strand sits, relative to the room it has
                    let share = offset / (r - q[3]);
                    let first = *place.get_or_insert(share);
                    assert!((share - first).abs() < 1e-3, "strand moves from {} to {} across the tube", first, share);
                }
            }
        }
    }

    #[test]
    fn strands_thin_with_the_guide() {
        let g = guide();
        for s in strands(&g, 10, 1.0, 0.0) {
            let p = points(&s);
            assert!((p[0][3] - 0.0006).abs() < 1e-7);
            assert!((p[p.len() - 1][3] - 0.0003).abs() < 1e-6);
        }
    }

    #[test]
    fn no_spread_packs_the_strands_on_the_axis() {
        let g = guide();
        let guide_points = points(split_args(&g)[0]);
        for s in strands(&g, 10, 0.0, 0.0) {
            for (q, c) in points(&s).iter().zip(&guide_points) {
                assert!(Vec3::new(q[0], q[1], q[2]).distance(Vec3::new(c[0], c[1], c[2])) < 1e-6);
            }
        }
    }

    #[test]
    fn full_spread_fills_the_tube() {
        let g = guide();
        let base = points(split_args(&g)[0])[0];
        let offsets: Vec<f32> =
            strands(&g, 200, 1.0, 0.0).iter().map(|s| { let p = points(s)[0]; Vec3::new(p[0] - base[0], p[1] - base[1], p[2] - base[2]).length() }).collect();
        let room = base[3] - 0.0006;
        assert!(offsets.iter().any(|&o| o > 0.9 * room));
        assert!(offsets.iter().any(|&o| o < 0.2 * room));
        // an even fill puts half the strands inside the inner 1/sqrt(2) of the radius
        let inner = offsets.iter().filter(|&&o| o < room / 2f32.sqrt()).count();
        assert!((80..120).contains(&inner), "{} of 200 strands in the inner half", inner);
    }

    #[test]
    fn twist_turns_the_strands_around_the_guide() {
        let g = guide();
        let straight = strands(&g, 5, 1.0, 0.0);
        let twisted = strands(&g, 5, 1.0, 0.5);
        let (a, b) = (points(&straight[3]), points(&twisted[3]));
        assert_eq!(a[0], b[0]);
        let last = a.len() - 1;
        let guide_tip = points(split_args(&g)[0])[last];
        let side = |p: [f32; 4]| Vec3::new(p[0] - guide_tip[0], p[1] - guide_tip[1], p[2] - guide_tip[2]);
        // half a turn puts the strand on the other side of the guide
        assert!(side(a[last]).dot(side(b[last])) < 0.0);
    }
}
