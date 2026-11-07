use glam::Vec3;
use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;
use nodarium_utils::{
    concat_arg_vecs, concat_args, decode_points,
    geometry::{
        is_path, place_root, translate_path, wrap_path, wrap_path_mut, DeepestPaths, PathStructure,
        PlaceChain,
    },
    int, log, split_args, Ctx,
};

nodarium_definition_file!("src/definition.json");

/// The `mode` that builds the path as a chain of turned segments
const CHAIN: i32 = 1;
/// The `weight` option that bends by how far a segment is from the root of its path. The
/// other one, 0, bends by the place of a segment along its own path.
const BY_PLACE: i32 = 1;

fn lerp_vec3(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    a + (b - a) * t
}

/// Turns a segment that points along `rest_dir` down towards gravity by `bend_angle` radians
fn droop(rest_dir: Vec3, bend_angle: f32) -> Vec3 {
    let gravity_dir = Vec3::new(0.0, -1.0, 0.0);
    let mut bend_axis = rest_dir.cross(gravity_dir);
    let axis_len = bend_axis.length();
    bend_axis = if axis_len > 0.0001 {
        bend_axis / axis_len
    } else {
        // rest_dir parallel to gravity — pick an arbitrary
        // perpendicular axis to break symmetry.
        Vec3::X
    };

    // Rodrigues' rotation formula
    let (sin_a, cos_a) = bend_angle.sin_cos();
    rest_dir * cos_a
        + bend_axis.cross(rest_dir) * sin_a
        + bend_axis * bend_axis.dot(rest_dir) * (1.0 - cos_a)
}

/// The chain mode with the bend weighted by place: every segment turns by how far it is from
/// the root of its path, and a path moves with the spot on its parent it grows from.
fn bend_by_place(args: &[&[i32]], ctx: &mut Ctx, depth: i32) -> Vec<i32> {
    let original: Vec<Vec<i32>> = split_args(args[0]).iter().map(|p| p.to_vec()).collect();
    let selected = DeepestPaths::new(&original, depth);
    let structure = PathStructure::new(&original);
    let roots = decode_points(args[7]);
    let path_count = original.iter().filter(|p| is_path(p)).count();

    // the inputs are read in the order of the list, as in the other modes
    let mut path_index = 0;
    let settings: Vec<Option<(PlaceChain, Vec<f32>)>> = original
        .iter()
        .enumerate()
        .map(|(index, path_data)| {
            if !selected.contains(path_data) {
                return None;
            }
            ctx.set_path(path_data, path_index, path_count);
            path_index += 1;
            let elasticity = ctx.float(args[3]).clamp(0.0, 1.0);
            let bend_exponent = 0.5 + elasticity * 4.0;
            let reach = ctx.float(args[6]).max(0.0);

            let path = wrap_path(path_data);
            if path.length < 2 {
                return None;
            }
            let points: Vec<Vec3> = path
                .points
                .chunks_exact(4)
                .map(|p| Vec3::new(p[0], p[1], p[2]))
                .collect();
            let root = place_root(&structure, &original, index, &roots)?;
            let chain = PlaceChain::new(&points, root, reach);
            // alpha is how far a segment is into the reach, the same for every path there
            let strengths = ctx.float_along(args[1], chain.alphas.iter().copied());
            let angles = strengths
                .iter()
                .zip(&chain.alphas)
                .map(|(strength, alpha)| {
                    strength * std::f32::consts::FRAC_PI_2 * alpha.powf(bend_exponent)
                })
                .collect();
            Some((chain, angles))
        })
        .collect();

    let mut current = original.clone();
    for &index in structure.order() {
        let shift = structure.shift(index, &original, &current);
        if let Some((chain, angles)) = &settings[index] {
            let bent = chain.bend(chain.points[0] + shift, |k, rest, _| droop(rest, angles[k]));
            let points = wrap_path_mut(&mut current[index]).points;
            for (point, position) in points.chunks_exact_mut(4).zip(&bent) {
                point[..3].copy_from_slice(&position.to_array());
            }
        } else {
            translate_path(&mut current[index], shift);
        }
    }
    concat_arg_vecs(current)
}

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let mut ctx = Ctx::default();

    let plants = split_args(args[0]);
    let depth = ctx.int(args[2]);
    let mode = int(args[4]); // 0 = closed-form, 1 = verlet
    if mode == CHAIN && int(args[5]) == BY_PLACE {
        return bend_by_place(args, &mut ctx, depth);
    }
    let selected = DeepestPaths::new(&plants, depth);

    let path_count = plants.iter().filter(|p| is_path(p)).count();
    let mut path_index = 0;

    let output: Vec<Vec<i32>> = plants
        .iter()
        .map(|path_data| {
            if !selected.contains(path_data) {
                return path_data.to_vec();
            }

            // alpha is where this path sits among all paths, from 0 at the first to 1 at the last
            ctx.set_path(path_data, path_index, path_count);
            path_index += 1;
            let elasticity = ctx.float(args[3]).clamp(0.0, 1.0);
            // 0 → sqrt (rope), 1 → ~4.5 (only the tip droops)
            let bend_exponent = 0.5 + elasticity * 4.0;

            let path = wrap_path(path_data);
            let mut output_data = path_data.to_vec();

            // alpha goes along the path, so the strength can change from base to tip
            let alphas = (0..path.length).map(|i| i as f32 / (path.length - 1).max(1) as f32);
            let strengths = ctx.float_along(args[1], alphas);
            let output = wrap_path_mut(&mut output_data);

            if mode == 1 {
                // Forward-kinematic cantilever chain. Each segment rotates around
                // an axis perpendicular to (rest_dir, gravity) by an angle that
                // grows with alpha along the stem. Positions are built from the
                // anchored base outward, so segment lengths are preserved by
                // construction (no iteration, no rescaling, no oscillation).

                let original: Vec<Vec3> = (0..path.length)
                    .map(|i| {
                        let s = i * 4;
                        Vec3::from_slice(&path.points[s..s + 3])
                    })
                    .collect();

                let seg_lens: Vec<f32> = (0..path.length - 1)
                    .map(|i| (original[i + 1] - original[i]).length())
                    .collect();
                let rest_dirs: Vec<Vec3> = (0..path.length - 1)
                    .map(|i| {
                        let d = original[i + 1] - original[i];
                        let l = d.length();
                        if l > 0.0001 {
                            d / l
                        } else {
                            Vec3::Y
                        }
                    })
                    .collect();

                let mut cur = vec![Vec3::ZERO; path.length];
                cur[0] = original[0];

                for i in 1..path.length {
                    let seg_idx = i - 1;
                    // the middle of the segment, so coarse paths bend like fine ones
                    let alpha = (seg_idx as f32 + 0.5) / (path.length - 1) as f32;
                    let strength = (strengths[seg_idx] + strengths[i]) * 0.5;
                    // Tip bend angle in radians. PI/2 = horizontal tip at strength=1.
                    let max_angle = strength * std::f32::consts::FRAC_PI_2;
                    let bend_angle = max_angle * alpha.powf(bend_exponent);

                    let bent_dir = droop(rest_dirs[seg_idx], bend_angle);

                    cur[i] = cur[i - 1] + bent_dir * seg_lens[seg_idx];
                }

                for i in 0..path.length {
                    let s = i * 4;
                    output.points[s] = cur[i].x;
                    output.points[s + 1] = cur[i].y;
                    output.points[s + 2] = cur[i].z;
                }
            } else {
                // Closed-form: per-segment lerp toward a downward vector
                let mut offset_vec = Vec3::ZERO;

                for i in 0..path.length - 1 {
                    // the middle of the segment, so coarse paths bend like fine ones
                    let alpha = (i as f32 + 0.5) / (path.length - 1) as f32;
                    let start_index = i * 4;

                    let start_point = Vec3::from_slice(&path.points[start_index..start_index + 3]);
                    let end_point =
                        Vec3::from_slice(&path.points[start_index + 4..start_index + 7]);

                    let direction = end_point - start_point;

                    let length = direction.length();

                    let curviness = elasticity.max(0.0001);
                    let strength_arg = (strengths[i] + strengths[i + 1]) * 0.5 * 10.0;
                    let strength = strength_arg / curviness * strength_arg;

                    log!(
                        "length: {}, curviness: {}, strength: {}",
                        length,
                        curviness,
                        strength
                    );

                    let down_point = Vec3::new(0.0, -length * strength, 0.0);

                    let mut mid_point =
                        lerp_vec3(direction, down_point, curviness * alpha.powf(bend_exponent));

                    if mid_point[0] == 0.0 && mid_point[2] == 0.0 {
                        mid_point[0] += 0.0001;
                        mid_point[2] += 0.0001;
                    }

                    // Correct midpoint length
                    mid_point *= length / mid_point.length();

                    let final_end_point = start_point + mid_point;
                    // the end of this segment moves by its own bend and by all bends below it
                    offset_vec += final_end_point - end_point;
                    let offset_end_point = end_point + offset_vec;

                    output.points[start_index + 4] = offset_end_point[0];
                    output.points[start_index + 5] = offset_end_point[1];
                    output.points[start_index + 6] = offset_end_point[2];
                }
            }
            output_data
        })
        .collect();

    concat_args(output.iter().map(|x| x.as_slice()).collect())
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use glam::Vec3;
    use nodarium_utils::{
        concat_arg_vecs, encode_float,
        geometry::{
            create_multiple_paths, create_path, interpolate_along_path, set_path_link,
            wrap_multiple_paths, wrap_path, wrap_path_mut, PathLink,
        },
        split_args,
    };

    const CHAIN: i32 = 1;
    const BY_PLACE: i32 = 1;

    fn path(id: i32, depth: i32, parent: Option<(i32, f32)>, points: &[Vec3]) -> Vec<i32> {
        let mut data = create_path(points.len(), depth);
        let (parent_id, parent_alpha) = parent.unwrap_or((-1, 0.0));
        set_path_link(
            &mut data,
            PathLink {
                id,
                parent_id,
                parent_alpha,
                ..PathLink::root(0, 0)
            },
        );
        let path = wrap_path_mut(&mut data);
        for (i, p) in points.iter().enumerate() {
            path.points[i * 4..i * 4 + 4].copy_from_slice(&[p.x, p.y, p.z, 0.1]);
        }
        data
    }

    fn points(path_data: &[i32]) -> Vec<Vec3> {
        wrap_path(path_data)
            .points
            .chunks_exact(4)
            .map(|p| Vec3::new(p[0], p[1], p[2]))
            .collect()
    }

    fn run(paths: Vec<Vec<i32>>, depth: i32, weight: i32, reach: f32) -> Vec<Vec<i32>> {
        let out = __nodarium_execute_inner(&[
            &concat_arg_vecs(paths),
            &[encode_float(0.5)],
            &[depth],
            &[encode_float(0.3)],
            &[CHAIN],
            &[weight],
            &[encode_float(reach)],
            &[0, 1, 1, 1],
        ]);
        split_args(&out).iter().map(|p| p.to_vec()).collect()
    }

    /// A stipe up to the origin, a long path from its tip that rises along x and a short one
    /// that is the outer part of the long one
    fn long_and_short() -> Vec<Vec<i32>> {
        let stipe = path(
            0,
            1,
            None,
            &[
                Vec3::new(0.0, -2.0, 0.0),
                Vec3::new(0.0, -1.0, 0.0),
                Vec3::ZERO,
            ],
        );
        let rising = |from: usize| {
            (from..9)
                .map(|i| Vec3::new(i as f32, i as f32 * 0.3, 0.0))
                .collect::<Vec<_>>()
        };
        vec![
            stipe,
            path(1, 2, Some((0, 1.0)), &rising(0)),
            path(2, 2, Some((0, 1.0)), &rising(4)),
        ]
    }

    fn gap(result: &[Vec<i32>]) -> f32 {
        let (long, short) = (points(&result[1]), points(&result[2]));
        long[4..]
            .iter()
            .zip(&short)
            .map(|(a, b)| a.distance(*b))
            .fold(0.0, f32::max)
    }

    /// A slightly leaning stem from (0, 0, 0) to (0.5, 5, 0) with `resolution` points.
    fn stem(resolution: usize) -> Vec<i32> {
        let mut stems = create_multiple_paths(1, resolution, 1, 3);
        for stem in wrap_multiple_paths(&mut stems).iter_mut() {
            for i in 0..resolution {
                let a = i as f32 / (resolution - 1) as f32;
                stem.points[i * 4..i * 4 + 4].copy_from_slice(&[0.5 * a, 5.0 * a, 0.0, 0.1]);
            }
        }
        stems
    }

    fn tip(resolution: usize, mode: i32) -> Vec3 {
        let out = __nodarium_execute_inner(&[
            &stem(resolution),
            &[encode_float(0.6)],
            &[1],
            &[encode_float(0.3)],
            &[mode],
            &[0],
            &[encode_float(0.0)],
            &[0, 1, 1, 1],
        ]);
        let path = split_args(&out)[0].to_vec();
        let p = wrap_path(&path).points;
        let s = (resolution - 1) * 4;
        Vec3::new(p[s], p[s + 1], p[s + 2])
    }

    #[test]
    fn the_shape_does_not_depend_on_the_resolution() {
        // the closed-form mode only gets close to the shape as the resolution grows
        for (mode, tolerance) in [(1, 0.05), (0, 0.25)] {
            let reference = tip(32, mode);
            for resolution in [8, 16, 64] {
                let t = tip(resolution, mode);
                assert!(
                    t.distance(reference) < tolerance,
                    "mode {}: tip at resolution {} is {:?}, at 32 it is {:?}",
                    mode,
                    resolution,
                    t,
                    reference
                );
            }
        }
    }

    #[test]
    fn by_place_a_short_path_bends_like_the_long_path_it_lies_on() {
        let along = run(long_and_short(), 1, 0, 0.0);
        assert!(gap(&along) > 0.5, "gap along each path {}", gap(&along));

        for reach in [0.0, 4.0, 12.0] {
            let result = run(long_and_short(), 1, BY_PLACE, reach);
            assert!(points(&result[1])[8].y < 2.0, "the long path droops");
            assert!(
                gap(&result) < 1e-5,
                "gap {} with reach {}",
                gap(&result),
                reach
            );
        }
    }

    #[test]
    fn by_place_the_segments_keep_their_length() {
        let original = long_and_short();
        let result = run(original.clone(), 1, BY_PLACE, 4.0);
        for (before, after) in original.iter().zip(&result) {
            let (before, after) = (points(before), points(after));
            for (a, b) in before.windows(2).zip(after.windows(2)) {
                assert!((a[0].distance(a[1]) - b[0].distance(b[1])).abs() < 1e-4);
            }
        }
    }

    #[test]
    fn by_place_a_path_moves_with_its_parent() {
        let trunk = path(
            0,
            1,
            None,
            &(0..5)
                .map(|i| Vec3::new(i as f32, 0.5 * i as f32, 0.0))
                .collect::<Vec<_>>(),
        );
        let attach = 0.5;
        let on_trunk = interpolate_along_path(wrap_path(&trunk).points, attach)
            .0
            .truncate();
        let branch = path(
            1,
            2,
            Some((0, attach)),
            &[on_trunk, on_trunk + Vec3::Z, on_trunk + Vec3::Z * 2.0],
        );

        let result = run(vec![branch, trunk], 2, BY_PLACE, 3.0);

        let spot = interpolate_along_path(wrap_path(&result[1]).points, attach)
            .0
            .truncate();
        assert!(spot.distance(on_trunk) > 0.1, "the trunk droops");
        assert!(points(&result[0])[0].distance(spot) < 1e-4);
    }
}
