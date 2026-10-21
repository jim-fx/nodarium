use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;
use nodarium_utils::{concat_args, split_args};

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let mut output: Vec<&[i32]> = Vec::new();

    for arg in args.iter() {
        // an unconnected input is empty, a wrapped list is at least [0, n, 1, n]
        if arg.len() < 4 {
            continue;
        }
        output.extend(split_args(arg));
    }

    concat_args(output)
}
