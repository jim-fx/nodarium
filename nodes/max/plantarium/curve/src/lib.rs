use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;
use nodarium_utils::concat_args;

nodarium_definition_file!("src/input.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    // the points arrive as [0, n + 1, ...points, 1, 1]. They go into the expression
    // one word each, a slice of several words would be read as a nested list
    let points = &args[0][2..args[0].len() - 2];

    // nothing connected to the value gives a plain number, then the curve shapes alpha
    let alpha = concat_args(vec![&[2], &[0]]);
    let value = if args[1].len() == 1 { &alpha[..] } else { args[1] };
    let mut parts: Vec<&[i32]> = vec![&[3], value, args[2], args[3]];
    parts.extend(points.chunks(1));
    concat_args(parts)
}
