use glam::Vec3;
use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;
use nodarium_utils::read_i32_slice;
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
pub fn execute(
    plant: (i32, i32),
    strength: (i32, i32),
    curviness: (i32, i32),
    depth: (i32, i32),
) -> Vec<i32> {
    reset_call_count();

    let arg = read_i32_slice(plant);
    let plants = split_args(arg.as_slice());
    let depth = evaluate_int(read_i32_slice(depth).as_slice());

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

            let mut offset_vec = Vec3::ZERO;

            for i in 0..path.length - 1 {
                let alpha = i as f32 / (path.length - 1) as f32;
                let start_index = i * 4;

                let start_point = Vec3::from_slice(&path.points[start_index..start_index + 3]);
                let end_point = Vec3::from_slice(&path.points[start_index + 4..start_index + 7]);

                let direction = end_point - start_point;

                let length = direction.length();

                let str = evaluate_float(read_i32_slice(strength).as_slice());
                let curviness = evaluate_float(read_i32_slice(curviness).as_slice());
                let strength = str / curviness.max(0.0001) * str;

                log!(
                    "length: {}, curviness: {}, strength: {}",
                    length,
                    curviness,
                    strength
                );

                let down_point = Vec3::new(0.0, -length * strength, 0.0);

                let mut mid_point = lerp_vec3(direction, down_point, curviness * alpha.sqrt());

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
            output_data
        })
        .collect();

    concat_args(output.iter().map(|x| x.as_slice()).collect())
}
