use glam::Vec3;
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{
    concat_arg_vecs,
    geometry::{create_path, orthogonal_to, path_random, set_path_link, wrap_path, wrap_path_mut, DeepestPaths, PathLink},
    int, split_args, Ctx, PathInfo,
};
use std::f32::consts::{PI, TAU};

nodarium_definition_file!("src/definition.json");

/// Neighbouring strands are this far apart in angle, so no two line up across the tube
const GOLDEN_ANGLE: f32 = PI * (3.0 - 2.236_068);

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

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let mut ctx = Ctx::default();
    let paths = split_args(args[0]);
    let seed = int(args[5]);
    // the graph node that makes the strands, it is the last input
    let creator = int(args[6]);

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

        let guide = wrap_path(path_data);
        let n = guide.length;
        let centre: Vec<Vec3> = guide.points.chunks_exact(4).map(|p| Vec3::new(p[0], p[1], p[2])).collect();
        let radius: Vec<f32> = guide.points.chunks_exact(4).map(|p| p[3].abs()).collect();
        let widest = radius.iter().copied().fold(0.0, f32::max);
        let frame = frames(&centre);
        let mut along = vec![0.0f32; n];
        for i in 1..n {
            along[i] = along[i - 1] + centre[i].distance(centre[i - 1]);
        }
        let length = along[n - 1];
        let alphas: Vec<f32> =
            along.iter().enumerate().map(|(i, &l)| if length > 0.0 { l / length } else { i as f32 / (n - 1).max(1) as f32 }).collect();

        let amount = ctx.int(args[1]).max(0);
        let spread = ctx.float(args[3]).clamp(0.0, 1.0);
        let twist = ctx.float(args[4]);

        for k in 0..amount {
            let link = PathLink::child(creator, guide.link.id, 0.0, k);
            ctx.set_path_info(PathInfo {
                index: k as usize,
                rank: k as usize,
                count: amount as usize,
                depth: guide.depth + 1,
                parent_position: 0.0,
                length,
            });
            ctx.seed_path(link.id);
            let thickness = ctx.float_along(args[2], alphas.iter().copied());

            // stratified sunflower: every strand gets an equal share of the cross-section
            let across = spread * ((k as f32 + path_random(seed, link.id, 0)) / amount as f32).sqrt();
            let angle = k as f32 * GOLDEN_ANGLE + TAU * path_random(seed, guide.link.id, 1);

            let mut strand_data = create_path(n, guide.depth + 1);
            set_path_link(&mut strand_data, link);
            let strand = wrap_path_mut(&mut strand_data);
            for i in 0..n {
                let profile = if widest > 0.0 { radius[i] / widest } else { 0.0 };
                let r = (thickness[i].max(0.0) * profile).min(radius[i]);
                let a = angle + TAU * twist * alphas[i];
                let (normal, binormal) = frame[i];
                let p = centre[i] + (normal * a.cos() + binormal * a.sin()) * across * (radius[i] - r);
                strand.points[i * 4..i * 4 + 4].copy_from_slice(&[p.x, p.y, p.z, r]);
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

    fn strands(guide: &[i32], amount: i32, spread: f32, twist: f32) -> Vec<Vec<i32>> {
        let out = __nodarium_execute_inner(&[
            guide,
            &[amount],
            &[encode_float(0.0006)],
            &[encode_float(spread)],
            &[encode_float(twist)],
            &[4],
            &[CREATOR],
        ]);
        split_args(&out).iter().map(|s| s.to_vec()).collect()
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
