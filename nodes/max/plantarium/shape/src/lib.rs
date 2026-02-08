use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;
use nodarium_utils::{concat_args, log, split_args};

nodarium_definition_file!("src/input.json");

#[nodarium_execute]
pub fn execute(input: &[i32]) -> Vec<i32> {
    let args = split_args(input);
    log!("vec3 input: {:?}", input);
    log!("vec3 args: {:?}", args);
    concat_args(args)
}
