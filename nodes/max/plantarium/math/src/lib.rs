use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;
use nodarium_utils::{concat_arg_vecs, encode_float, log, read_i32_slice};

nodarium_definition_file!("src/input.json");

#[nodarium_execute]
pub fn execute(op_type: (i32, i32), a: (i32, i32), b: (i32, i32)) -> Vec<i32> {
    let op = read_i32_slice(op_type);
    let a_val = read_i32_slice(a);
    let b_val = read_i32_slice(b);
    concat_arg_vecs(vec![vec![0], op, a_val, b_val])
}
