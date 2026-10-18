use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;
use nodarium_utils::concat_args;

nodarium_definition_file!("src/input.json");

#[nodarium_execute]
pub fn execute(_args: &[&[i32]]) -> Vec<i32> {
    // the second word keeps the expression from being read as a plain value
    concat_args(vec![&[2], &[0]])
}
