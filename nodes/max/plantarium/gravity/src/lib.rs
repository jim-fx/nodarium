use glam::Vec3;
use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;
use nodarium_utils::{
    concat_args, int,
    geometry::{wrap_path, wrap_path_mut},
    log, split_args, Ctx,
};

nodarium_definition_file!("src/definition.json");

fn lerp_vec3(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    a + (b - a) * t
}

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let mut ctx = Ctx::default();


    let plants = split_args(args[0]);
    let depth = ctx.int(args[2]);
    let mode = int(args[4]); // 0 = closed-form, 1 = verlet
    let mut max_depth = 0;
    for path_data in plants.iter() {
        if path_data[2] != 0 {
            continue;
        }
        max_depth = max_depth.max(path_data[3]);
    }

    let path_count = plants.iter().filter(|p| p[2] == 0).count();
    let mut path_index = 0;

    let output: Vec<Vec<i32>> = plants
        .iter()
        .map(|_path_data| {
            let path_data = _path_data.to_vec();
            if path_data[2] != 0 || path_data[3] < (max_depth - depth + 1) {
                return path_data;
            }

            // alpha is where this path sits among all paths, from 0 at the first to 1 at the last
            ctx.set_path(&path_data, path_index, path_count);
            path_index += 1;
            let elasticity = ctx.float(args[3]).clamp(0.0, 1.0);
            // 0 → sqrt (rope), 1 → ~4.5 (only the tip droops)
            let bend_exponent = 0.5 + elasticity * 4.0;

            let path = wrap_path(&path_data);
            let mut output_data = path_data.clone();

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

                let gravity_dir = Vec3::new(0.0, -1.0, 0.0);

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
                        if l > 0.0001 { d / l } else { Vec3::Y }
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

                    let rest_dir = rest_dirs[seg_idx];
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
                    let bent_dir = rest_dir * cos_a
                        + bend_axis.cross(rest_dir) * sin_a
                        + bend_axis * bend_axis.dot(rest_dir) * (1.0 - cos_a);

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
        encode_float,
        geometry::{create_multiple_paths, wrap_multiple_paths, wrap_path},
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

    fn tip(resolution: usize, mode: i32) -> Vec3 {
        let out = __nodarium_execute_inner(&[
            &stem(resolution),
            &[encode_float(0.6)],
            &[1],
            &[encode_float(0.3)],
            &[mode],
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
}
