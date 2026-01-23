use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;
use nodarium_utils::encode_float;
use nodarium_utils::evaluate_float;
use nodarium_utils::evaluate_vec3;
use nodarium_utils::read_i32;
use nodarium_utils::read_i32_slice;

nodarium_definition_file!("src/input.json");

#[nodarium_execute]
pub fn execute(input: (i32, i32), input_type: (i32, i32)) -> Vec<i32> {
    let inp = read_i32_slice(input);
    let t = read_i32(input_type.0);
    if t == 0 {
        let f = evaluate_float(inp.as_slice());
        return vec![encode_float(f)];
    }
    if t == 1 {
        let f = evaluate_vec3(inp.as_slice());
        return vec![encode_float(f[0]), encode_float(f[1]), encode_float(f[2])];
    }

    return inp;
}
