use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;
use nodarium_utils::{concat_args, int};

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    // the second word keeps the expression from being read as a plain value, 1 reads across
    let across = args.first().is_some_and(|arg| int(arg) == 1);
    concat_args(vec![&[2], &[across as i32]])
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use nodarium_utils::Ctx;

    #[test]
    fn reads_along_or_across() {
        let mut ctx = Ctx::default();
        ctx.alpha = 0.25;
        ctx.across = 0.75;
        assert_eq!(ctx.float(&__nodarium_execute_inner(&[&[0]])), 0.25);
        assert_eq!(ctx.float(&__nodarium_execute_inner(&[&[1]])), 0.75);
        // a graph from before the option
        assert_eq!(__nodarium_execute_inner(&[]), __nodarium_execute_inner(&[&[0]]));
    }
}
