use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;
use nodarium_utils::{concat_args, log};

nodarium_definition_file!("src/input.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    log!("vec3 args: {:?}", args);
    concat_args(args.to_vec())
}
