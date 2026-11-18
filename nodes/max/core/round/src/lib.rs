use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;
use nodarium_utils::concat_args;

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    concat_args(vec![&[6], args[0]])
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use nodarium_utils::{concat_args, encode_float, Ctx};

    #[test]
    fn rounds_what_it_is_given_where_it_is_read() {
        let mut ctx = Ctx::default();
        assert_eq!(ctx.int(&__nodarium_execute_inner(&[&[encode_float(4.6)]])), 5);
        let quotient = concat_args(vec![&[0], &[3], &[encode_float(9.0)], &[encode_float(2.0)]]);
        assert_eq!(ctx.int(&__nodarium_execute_inner(&[&quotient])), 5);
    }
}
