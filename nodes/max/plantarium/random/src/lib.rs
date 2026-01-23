use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;
use nodarium_utils::concat_arg_vecs;
use nodarium_utils::read_i32_slice;

nodarium_definition_file!("src/input.json");

#[nodarium_execute]
pub fn execute(min: (i32, i32), max: (i32, i32), seed: (i32, i32)) -> Vec<i32> {
    concat_arg_vecs(vec![
        vec![1],
        read_i32_slice(min),
        read_i32_slice(max),
        read_i32_slice(seed),
    ])
}
