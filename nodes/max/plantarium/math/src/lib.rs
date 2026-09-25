use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;
use nodarium_utils::{
    concat_args
};

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    concat_args(vec![&[0], args[0], args[1], args[2]])
}

nodarium_definition_file!("src/input.json");
