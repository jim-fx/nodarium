use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;
use nodarium_utils::concat_args;

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    // like the curve node, an unconnected value reads alpha
    let alpha = concat_args(vec![&[2], &[0]]);
    let value = if args[0].len() == 1 { &alpha[..] } else { args[0] };
    concat_args(vec![&[8], value, args[1], args[2]])
}
