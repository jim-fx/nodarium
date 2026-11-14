use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;
use nodarium_utils::{concat_args, int};

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    // the second word keeps the expression from being read as a plain value, 1 reads across and
    // 2 the vein
    let direction = args.first().map_or(0, |arg| int(arg)).clamp(0, 2);
    concat_args(vec![&[2], &[direction]])
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use nodarium_utils::Ctx;

    #[test]
    fn reads_along_across_or_the_vein() {
        let mut ctx = Ctx::default();
        ctx.alpha = 0.25;
        ctx.across = 0.75;
        ctx.vein = 0.5;
        assert_eq!(ctx.float(&__nodarium_execute_inner(&[&[0]])), 0.25);
        assert_eq!(ctx.float(&__nodarium_execute_inner(&[&[1]])), 0.75);
        assert_eq!(ctx.float(&__nodarium_execute_inner(&[&[2]])), 0.5);
        // a graph from before the option
        assert_eq!(__nodarium_execute_inner(&[]), __nodarium_execute_inner(&[&[0]]));
    }
}
