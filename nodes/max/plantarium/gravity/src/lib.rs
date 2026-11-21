use glam::Vec3;
use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;
use nodarium_utils::{
    concat_arg_vecs,
    geometry::{follow_parents, is_path, wrap_path, wrap_path_mut, DeepestPaths},
    int, split_args, Ctx,
};

nodarium_definition_file!("src/definition.json");

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

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let mut ctx = Ctx::default();

    let plants = split_args(args[0]);
    let selected = DeepestPaths::new(&plants, int(args[3]).max(1));

    let path_count = plants.iter().filter(|p| is_path(p)).count();
    let mut path_index = 0;

    let mut output: Vec<Vec<i32>> = plants
        .iter()
        .map(|path_data| {
            if !selected.contains(path_data) {
                return path_data.to_vec();
            }

            // alpha is where this path sits among all paths, from 0 at the first to 1 at the last
            ctx.set_path(path_data, path_index, path_count);
            path_index += 1;
            let elasticity = ctx.float(args[2]).clamp(0.0, 1.0);
            // 0 → sqrt (rope), 1 → ~4.5 (only the tip droops)
            let bend_exponent = 0.5 + elasticity * 4.0;

            let path = wrap_path(path_data);
            let mut output_data = path_data.to_vec();

            // a path that has not finished growing bends like the lower part of the full-grown
            // one, so a spot keeps its bend while the tip grows on
            let share = path.link.share();
            // alpha goes along the path, so the strength can change from base to tip
            let alphas = (0..path.length).map(|i| i as f32 / (path.length - 1).max(1) as f32 * share);
            let strengths = ctx.float_along(args[1], alphas);
            let output = wrap_path_mut(&mut output_data);

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
                let alpha = (seg_idx as f32 + 0.5) / (path.length - 1) as f32 * share;
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
            output_data
        })
        .collect();

    // paths on the layers that bent carry what grows on them
    let original: Vec<Vec<i32>> = plants.iter().map(|p| p.to_vec()).collect();
    follow_parents(&original, &mut output);

    concat_arg_vecs(output)
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use glam::Vec3;
    use nodarium_utils::{
        concat_arg_vecs, encode_float,
        geometry::{
            create_multiple_paths, create_path, path_link, set_path_link, wrap_multiple_paths,
            wrap_path, wrap_path_mut, PathLink,
        },
        split_args,
    };

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

    fn tip(resolution: usize) -> Vec3 {
        let out = __nodarium_execute_inner(&[
            &stem(resolution),
            &[encode_float(0.6)],
            &[encode_float(0.3)],
            &[1],
        ]);
        let path = split_args(&out)[0].to_vec();
        let p = wrap_path(&path).points;
        let s = (resolution - 1) * 4;
        Vec3::new(p[s], p[s + 1], p[s + 2])
    }

    #[test]
    fn the_shape_does_not_depend_on_the_resolution() {
        let reference = tip(32);
        for resolution in [8, 16, 64] {
            let t = tip(resolution);
            assert!(
                t.distance(reference) < 0.05,
                "tip at resolution {} is {:?}, at 32 it is {:?}",
                resolution,
                t,
                reference
            );
        }
    }

    #[test]
    fn with_depth_the_parents_bend_too_and_carry_their_branches() {
        let stem = split_args(&stem(8))[0].to_vec();
        let mut branch = create_path(4, 2);
        set_path_link(&mut branch, PathLink::child(4, path_link(&stem).id, 1.0, 0));
        for (i, point) in wrap_path_mut(&mut branch).points.chunks_exact_mut(4).enumerate() {
            point.copy_from_slice(&[0.5 + 0.2 * i as f32, 5.0, 0.0, 0.05]);
        }
        let plant = concat_arg_vecs(vec![stem.clone(), branch]);
        let run = |depth: i32| {
            let out = __nodarium_execute_inner(&[&plant, &[encode_float(0.6)], &[encode_float(0.3)], &[depth]]);
            split_args(&out).iter().map(|p| wrap_path(p).points.to_vec()).collect::<Vec<_>>()
        };

        let deepest = run(1);
        assert_eq!(deepest[0], wrap_path(&stem).points);
        let both = run(2);
        assert_ne!(both[0], deepest[0]);
        let tip = Vec3::from_slice(&both[0][both[0].len() - 4..]);
        assert!(tip.distance(Vec3::from_slice(&both[1][..3])) < 1e-4);
    }
}
