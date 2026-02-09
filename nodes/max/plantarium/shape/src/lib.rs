use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;
use nodarium_utils::{concat_args, split_args};

nodarium_definition_file!("src/input.json");

#[nodarium_execute]
pub fn execute(input: &[i32]) -> Vec<i32> {
    concat_args(split_args(input))
}
