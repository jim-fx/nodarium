use glam::Vec3;
use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;
use nodarium_utils::{
    concat_args, evaluate_float, evaluate_int,
    geometry::{wrap_path, wrap_path_mut},
    log, reset_call_count, split_args,
};

nodarium_definition_file!("src/input.json");

fn lerp_vec3(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    a + (b - a) * t
}

#[nodarium_execute]
pub fn execute(input: &[i32]) -> Vec<i32> {
    reset_call_count();

    let args = split_args(input);

    let plants = split_args(args[0]);
    let depth = evaluate_int(args[2]);
    let elasticity = evaluate_float(args[3]).clamp(0.0, 1.0);
    let mode = evaluate_int(args[4]); // 0 = closed-form, 1 = verlet
    // 0 → sqrt (rope), 1 → ~4.5 (only the tip droops)
    let bend_exponent = 0.5 + elasticity * 4.0;

    let mut max_depth = 0;
    for path_data in plants.iter() {
        if path_data[2] != 0 {
            continue;
        }
        max_depth = max_depth.max(path_data[3]);
    }

    let output: Vec<Vec<i32>> = plants
        .iter()
        .map(|_path_data| {
            let path_data = _path_data.to_vec();
            if path_data[2] != 0 || path_data[3] < (max_depth - depth + 1) {
                return path_data;
            }

            let path = wrap_path(&path_data);
            let mut output_data = path_data.clone();
            let output = wrap_path_mut(&mut output_data);

            if mode == 1 {
                // Forward-kinematic cantilever chain. Each segment rotates around
                // an axis perpendicular to (rest_dir, gravity) by an angle that
                // grows with alpha along the stem. Positions are built from the
                // anchored base outward, so segment lengths are preserved by
                // construction (no iteration, no rescaling, no oscillation).

                let raw_strength = evaluate_float(args[1]);
                let gravity_dir = Vec3::new(0.0, -1.0, 0.0);

                // Tip bend angle in radians. PI/2 = horizontal tip at strength=1.
                let max_angle = raw_strength * std::f32::consts::FRAC_PI_2;

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
                    let alpha = if path.length > 2 {
                        seg_idx as f32 / (path.length - 2) as f32
                    } else {
                        1.0
                    };
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
                    let alpha = i as f32 / (path.length - 1) as f32;
                    let start_index = i * 4;

                    let start_point = Vec3::from_slice(&path.points[start_index..start_index + 3]);
                    let end_point =
                        Vec3::from_slice(&path.points[start_index + 4..start_index + 7]);

                    let direction = end_point - start_point;

                    let length = direction.length();

                    let curviness = elasticity.max(0.0001);
                    let strength_arg = evaluate_float(args[1]) * 10.0;
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
                    let offset_end_point = end_point + offset_vec;

                    output.points[start_index + 4] = offset_end_point[0];
                    output.points[start_index + 5] = offset_end_point[1];
                    output.points[start_index + 6] = offset_end_point[2];

                    offset_vec += final_end_point - end_point;
                }
            }
            output_data
        })
        .collect();

    concat_args(output.iter().map(|x| x.as_slice()).collect())
}
