/// Splits a framed list like `[0, 1, ...parts, 1, 1]` into its parts. A part is
/// either a single value (one word) or a framed group that keeps its brackets.
///
/// A framed group starts with `[0, count, ...]` and ends with `[1, ...]`, where
/// `count` is the distance from the group's first word to the next bracket
/// word, minus one. It points at the group's own closing bracket when the
/// payload holds only values (a path, or the arguments of a math node), and at
/// a nested group's opening when single values precede it. The second word of
/// a closing bracket counts the single values that follow the group at the
/// same level, so the scanner can skip past them. Because a value of 0 is
/// indistinguishable from an opening bracket, the scan is driven by these
/// counts, not by the words themselves.
pub fn split_args(args: &[i32]) -> Vec<&[i32]> {
    let mut out_args: Vec<&[i32]> = Vec::new();
    let mut depth = 0;
    let mut i = 0;
    let mut start_index = 0;
    let mut next_bracket_index = 0;
    let len = args.len();

    while i < len {
        // check if we are at a bracket
        if i == next_bracket_index {
            next_bracket_index = i + args[i + 1] as usize + 1;
            // check if the bracket is opening
            if args[i] == 0 {
                if depth == 1 {
                    start_index = i;
                }
                depth += 1;
            } else {
                depth -= 1;
                if depth == 1 {
                    out_args.push(&args[start_index..i + 2]);
                    start_index = i + 2;
                }
            }
            i += 2;
            continue;
        } else if depth == 1 {
            out_args.push(&args[i..i + 1]);
            start_index = i + 1;
        }
        i += 1;
    }

    out_args
}

pub fn concat_arg_vecs(data: Vec<Vec<i32>>) -> Vec<i32> {
    let mut total_length = 4; // Start with 4 to account for [0, 1] at the start and [1, 1] at the end

    // Calculate the total length first to avoid reallocations
    for vec in &data {
        if vec.len() == 1 {
            total_length += 1;
        } else {
            total_length += vec.len(); // +4 for [0, 1] and [1, 1] per inner vec
        }
    }

    let mut result = Vec::with_capacity(total_length);

    // Add [0, 1] initially
    result.push(0);
    result.push(1);

    let mut last_closing_bracket = 1;

    // Process each vector
    for vec in data {
        if vec.len() == 1 {
            result.push(vec[0]);
            result[last_closing_bracket] += 1;
            continue;
        } else {
            result.extend(vec);
            last_closing_bracket = result.len() - 1;
            result[last_closing_bracket] = 1;
        }
    }

    // Add [1, 1] at the end
    result.push(1);
    result.push(1);

    result
}

pub fn concat_args(mut data: Vec<&[i32]>) -> Vec<i32> {
    let mut total_length = 4; // Start with 4 to account for [0, 1] at the start and [1, 1] at the end

    // Calculate the total length first to avoid reallocations
    for vec in &data {
        if vec.len() == 1 {
            total_length += 1;
        } else {
            total_length += vec.len(); // +4 for [0, 1] and [1, 1] per inner vec
        }
    }

    let mut result = Vec::with_capacity(total_length);

    // Add [0, 1] initially
    result.push(0);
    result.push(1);

    let mut last_closing_bracket = 1;

    // Process each vector
    for vec in data.iter_mut() {
        if vec.len() == 1 {
            result.push(vec[0]);
            result[last_closing_bracket] += 1;
            continue;
        } else {
            result.extend_from_slice(vec);
            last_closing_bracket = result.len() - 1;
            result[last_closing_bracket] = 1;
        }
    }

    // Add [1, 1] at the end
    result.push(1);
    result.push(1);

    result
}

pub fn wrap_arg(arg: &[i32]) -> Vec<i32> {
    let mut out_args = Vec::with_capacity(arg.len() + 4);
    out_args.push(0);
    out_args.push(arg.len() as i32 + 1);
    out_args.extend_from_slice(arg);
    out_args.push(1);
    out_args.push(1);
    out_args
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::{concat_args, encode_float};

    #[test]
    fn test_split_args_bare_values() {
        // like the expression of a random node: four single values
        let input = concat_args(vec![&[1], &[0], &[encode_float(1.0)], &[42]]);

        let args = split_args(&input);

        assert_eq!(args, vec![&[1][..], &[0], &[encode_float(1.0)], &[42]]);
    }

    #[test]
    fn test_split_args_groups() {
        // like the output of a node that produces a list of paths
        let path = [0, 11, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1];
        let input = concat_arg_vecs(vec![path.to_vec(), path.to_vec()]);

        let args = split_args(&input);

        assert_eq!(args, vec![&path[..], &path]);
    }

    #[test]
    fn test_split_args_values_then_group() {
        let group = [0, 4, 4, 2, 4, 1, 1];
        let input = concat_args(vec![&[1], &[2], &[3], &group]);

        let args = split_args(&input);

        assert_eq!(args, vec![&[1][..], &[2], &[3], &group]);
    }

    #[test]
    fn test_split_args_group_then_value_then_group() {
        let first = [0, 4, 4, 2, 4, 1, 1];
        let second = [0, 3, 2, 3, 1, 1];
        // values after a group raise the group's closing count, which tells
        // split_args how far to skip past them
        let input = concat_args(vec![&first, &[7], &second, &[2]]);

        let args = split_args(&input);

        assert_eq!(
            args,
            vec![&[0, 4, 4, 2, 4, 1, 2][..], &[7], &[0, 3, 2, 3, 1, 2], &[2]]
        );
    }

    #[test]
    fn test_split_args_nested_groups() {
        // a math node whose second operand is another math node: the inner
        // group is framed exactly like a top level one
        let inner = concat_args(vec![&[0], &[2], &[encode_float(2.0)], &[encode_float(2.0)]]);
        // three single values lead to the nested group, so the outer count is 4
        let mut input = vec![0, 4, 0, 0, encode_float(2.0)];
        input.extend_from_slice(&inner);
        input.extend_from_slice(&[1, 1]);

        let args = split_args(&input);

        assert_eq!(args.len(), 4);
        assert_eq!(args[0], [0]);
        assert_eq!(args[1], [0]);
        assert_eq!(args[2], [encode_float(2.0)]);
        assert_eq!(args[3], inner);

        // the nested group splits into the math node's own operands
        let inner_args = split_args(args[3]);
        assert_eq!(
            inner_args,
            vec![&[0][..], &[2], &[encode_float(2.0)], &[encode_float(2.0)]]
        );
    }
}
