use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;
use nodarium_utils::concat_arg_vecs;
use nodarium_utils::read_i32_slice;
use nodarium_utils::{concat_args, log, split_args};

nodarium_definition_file!("src/input.json");

#[nodarium_execute]
pub fn execute(x: (i32, i32), y: (i32, i32), z: (i32, i32)) -> Vec<i32> {
    concat_arg_vecs(vec![
        read_i32_slice(x),
        read_i32_slice(y),
        read_i32_slice(z),
    ])
}
