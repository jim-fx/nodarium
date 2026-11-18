use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;
use nodarium_utils::concat_args;

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    // nothing connected to the value gives a plain number, then the wave runs along alpha
    let alpha = concat_args(vec![&[2], &[0]]);
    let value = if args[1].len() == 1 { &alpha[..] } else { args[1] };
    concat_args(vec![&[8], value, args[2], args[0]])
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use nodarium_utils::{concat_args, encode_float, Ctx};

    fn wave(shape: i32, value: &[i32], count: f32) -> Vec<i32> {
        __nodarium_execute_inner(&[&[shape], value, &[encode_float(count)]])
    }

    #[test]
    fn runs_along_alpha_when_no_value_is_connected() {
        let mut ctx = Ctx::default();
        let teeth = wave(1, &[encode_float(0.0)], 2.0);
        assert_eq!(ctx.float_along(&teeth, [0.0, 0.25, 0.5]), vec![0.0, 1.0, 0.0]);
    }

    #[test]
    fn reads_a_connected_value() {
        let mut ctx = Ctx::default();
        let quarter = concat_args(vec![&[0], &[0], &[encode_float(0.25)], &[encode_float(0.0)]]);
        assert_eq!(ctx.float(&wave(1, &quarter, 2.0)), 1.0);
        assert_eq!(ctx.float(&wave(0, &quarter, 1.0)), 0.5);
    }
}
