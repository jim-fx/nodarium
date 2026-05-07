use glam::Vec3;
use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;
use nodarium_utils::{
    concat_args, evaluate_float, evaluate_int, evaluate_vec3, geometry::wrap_path_mut,
    reset_call_count, split_args,
};
use noise::{HybridMulti, MultiFractal, NoiseFn, OpenSimplex};

nodarium_definition_file!("src/input.json");

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + t * (b - a)
}

#[nodarium_execute]
pub fn execute(input: &[i32]) -> Vec<i32> {
    reset_call_count();

    let args = split_args(input);

    let plants = split_args(args[0]);
    let scale = (evaluate_float(args[1]) * 0.1) as f64;
    let strength = evaluate_float(args[2]);
    let fix_bottom = evaluate_float(args[3]);

    let seed = args[4][0];

    let directional_strength = evaluate_vec3(args[5]);

    let depth = evaluate_int(args[6]);

    let octaves = evaluate_int(args[7]);

    let noise_x: HybridMulti<OpenSimplex> =
        HybridMulti::new(seed as u32 + 1).set_octaves(octaves as usize);
    let noise_y: HybridMulti<OpenSimplex> =
        HybridMulti::new(seed as u32 + 2).set_octaves(octaves as usize);
    let noise_z: HybridMulti<OpenSimplex> =
        HybridMulti::new(seed as u32 + 3).set_octaves(octaves as usize);

    let mut max_depth = 0;
    for path_data in plants.iter() {
        if path_data[2] != 0 {
            continue;
        }
        max_depth = max_depth.max(path_data[3]);
    }

    let output: Vec<Vec<i32>> = plants
        .iter()
        .enumerate()
        .map(|(j, _path_data)| {
            let mut path_data = _path_data.to_vec();

            // if this is not a path don't modify it
            if path_data[2] != 0 {
                return path_data;
            }

            if path_data[3] < (max_depth - depth + 1) {
                return path_data;
            }

            let path = wrap_path_mut(&mut path_data);

            let length = path.get_length() as f64;

            // Record original segment lengths so we can re-project after displacement
            let seg_lens: Vec<f32> = (0..path.length - 1)
                .map(|k| {
                    let p0 = Vec3::new(
                        path.points[k * 4],
                        path.points[k * 4 + 1],
                        path.points[k * 4 + 2],
                    );
                    let p1 = Vec3::new(
                        path.points[(k + 1) * 4],
                        path.points[(k + 1) * 4 + 1],
                        path.points[(k + 1) * 4 + 2],
                    );
                    (p1 - p0).length()
                })
                .collect();

            // Displace the first point (fix_bottom=1 → scale=0 here, anchoring the base)
            let scale0 = lerp(1.0, 0.0, fix_bottom);
            path.points[0] += noise_x.get([j as f64, 0.0]) as f32
                * directional_strength[0]
                * strength
                * scale0;
            path.points[1] += noise_y.get([j as f64, 0.0]) as f32
                * directional_strength[1]
                * strength
                * scale0;
            path.points[2] += noise_z.get([j as f64, 0.0]) as f32
                * directional_strength[2]
                * strength
                * scale0;
            let mut prev = Vec3::new(path.points[0], path.points[1], path.points[2]);

            for i in 1..path.length {
                let a = i as f64 / (path.length - 1) as f64;

                let px = j as f64 + a * length * scale;
                let py = a * scale as f64;

                let sf = lerp(1.0, a as f32, fix_bottom);
                path.points[i * 4] +=
                    noise_x.get([px, py]) as f32 * directional_strength[0] * strength * sf;
                path.points[i * 4 + 1] +=
                    noise_y.get([px, py]) as f32 * directional_strength[1] * strength * sf;
                path.points[i * 4 + 2] +=
                    noise_z.get([px, py]) as f32 * directional_strength[2] * strength * sf;

                // Re-project onto sphere of radius seg_lens[i-1] centered at prev
                let cur = Vec3::new(
                    path.points[i * 4],
                    path.points[i * 4 + 1],
                    path.points[i * 4 + 2],
                );
                let dir = cur - prev;
                let dir_len = dir.length();
                if dir_len > 0.0001 {
                    let corrected = prev + dir * (seg_lens[i - 1] / dir_len);
                    path.points[i * 4] = corrected.x;
                    path.points[i * 4 + 1] = corrected.y;
                    path.points[i * 4 + 2] = corrected.z;
                    prev = corrected;
                } else {
                    prev = cur;
                }
            }
            path_data
        })
        .collect();

    concat_args(output.iter().map(|x| x.as_slice()).collect())
}
