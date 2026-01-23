use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;
use nodarium_utils::read_i32_slice;
use nodarium_utils::{
    concat_args, evaluate_float, evaluate_int, evaluate_vec3, geometry::wrap_path_mut, read_i32,
    reset_call_count, split_args,
};
use noise::{HybridMulti, MultiFractal, NoiseFn, OpenSimplex};

nodarium_definition_file!("src/input.json");

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + t * (b - a)
}

#[nodarium_execute]
pub fn execute(
    plant: (i32, i32),
    scale: (i32, i32),
    strength: (i32, i32),
    fix_bottom: (i32, i32),
    seed: (i32, i32),
    directional_strength: (i32, i32),
    depth: (i32, i32),
    octaves: (i32, i32),
) -> Vec<i32> {
    reset_call_count();

    let arg = read_i32_slice(plant);
    let plants = split_args(arg.as_slice());
    let scale = (evaluate_float(read_i32_slice(scale).as_slice()) * 0.1) as f64;
    let strength = evaluate_float(read_i32_slice(strength).as_slice());
    let fix_bottom = evaluate_float(read_i32_slice(fix_bottom).as_slice());

    let seed = read_i32(seed.0);

    let directional_strength = evaluate_vec3(read_i32_slice(directional_strength).as_slice());

    let depth = evaluate_int(read_i32_slice(depth).as_slice());

    let octaves = evaluate_int(read_i32_slice(octaves).as_slice());

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

            for i in 0..path.length {
                let a = i as f64 / (path.length - 1) as f64;

                let px = j as f64 + a * length * scale;
                let py = a * scale as f64;

                path.points[i * 4] += noise_x.get([px, py]) as f32
                    * directional_strength[0]
                    * strength
                    * lerp(1.0, a as f32, fix_bottom);
                path.points[i * 4 + 1] += noise_y.get([px, py]) as f32
                    * directional_strength[1]
                    * strength
                    * lerp(1.0, a as f32, fix_bottom);
                path.points[i * 4 + 2] += noise_z.get([px, py]) as f32
                    * directional_strength[2]
                    * strength
                    * lerp(1.0, a as f32, fix_bottom);
            }
            path_data
        })
        .collect();

    concat_args(output.iter().map(|x| x.as_slice()).collect())
}
