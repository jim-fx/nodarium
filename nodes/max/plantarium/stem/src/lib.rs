use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;
use nodarium_utils::{
    evaluate_float, evaluate_int, evaluate_vec3,
    geometry::{create_multiple_paths, wrap_multiple_paths},
    log, reset_call_count,
    read_i32_slice, read_i32, 
};

nodarium_definition_file!("src/input.json");

#[nodarium_execute]
pub fn execute(origin: (i32, i32), _amount: (i32,i32), length: (i32, i32), thickness: (i32, i32), resolution_curve: (i32, i32)) -> Vec<i32> {
    reset_call_count();

    let amount = evaluate_int(read_i32_slice(_amount).as_slice()) as usize;
    let path_resolution = read_i32(resolution_curve.0) as usize;

    log!("stem args: amount={:?}", amount);

    let mut stem_data = create_multiple_paths(amount, path_resolution, 1);

    let mut stems = wrap_multiple_paths(&mut stem_data);

    for stem in stems.iter_mut() {
        let origin = evaluate_vec3(read_i32_slice(origin).as_slice());
        let length = evaluate_float(read_i32_slice(length).as_slice());
        let thickness = evaluate_float(read_i32_slice(thickness).as_slice());
        let amount_points = stem.points.len() / 4;

        for i in 0..amount_points {
            let a = i as f32 / (path_resolution - 1) as f32;
            stem.points[i * 4] = origin[0];
            stem.points[i * 4 + 1] = origin[1] + a * length;
            stem.points[i * 4 + 2] = origin[2];
            stem.points[i * 4 + 3] = thickness * (1.0 - a);
        }
    }

    log!("stem_data: {:?}", stem_data);

    stem_data
}
