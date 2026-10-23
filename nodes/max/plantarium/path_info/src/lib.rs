use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;
use nodarium_utils::{concat_args, int};

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    // which value to read is a select, the node that reads the expression looks it up
    concat_args(vec![&[5], &[int(args[0])]])
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use nodarium_utils::{Ctx, PathInfo};

    #[test]
    fn reads_the_selected_value_of_the_path() {
        let mut ctx = Ctx::default();
        ctx.set_path_info(PathInfo {
            index: 7,
            rank: 2,
            count: 5,
            depth: 3,
            parent_position: 0.4,
            length: 1.5,
        });

        // position, index, count, depth, parent position, length
        for (field, expected) in [(0, 0.5), (1, 7.0), (2, 5.0), (3, 3.0), (4, 0.4), (5, 1.5)] {
            let expression = __nodarium_execute_inner(&[&[field]]);
            assert_eq!(ctx.float(&expression), expected, "field {}", field);
        }
    }
}
