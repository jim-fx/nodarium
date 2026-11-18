use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use myzel_core::read_float;
use nodarium_utils::{concat_args, encode_float};

nodarium_definition_file!("src/definition.json");

/// The time in brackets, `[0, 2, t, 1, 1]`, as the float node sends its value. A curve node
/// reads a bare word on its `value` as nothing connected and would shape alpha instead.
#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    concat_args(vec![&[encode_float(read_float(args[0]).clamp(0.0, 1.0))]])
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use nodarium_utils::encode_float;

    #[test]
    fn passes_the_time_on() {
        assert_eq!(__nodarium_execute_inner(&[&[encode_float(0.25)]]), vec![0, 2, encode_float(0.25), 1, 1]);
        assert_eq!(__nodarium_execute_inner(&[&[encode_float(1.5)]]), vec![0, 2, encode_float(1.0), 1, 1]);
    }

    #[test]
    fn sends_the_time_like_the_float_node() {
        let t = __nodarium_execute_inner(&[&[encode_float(0.25)]]);
        // the curve node takes a single word on its value for nothing connected and shapes alpha
        // then, so the time comes in brackets like the value of the float node
        assert_eq!(t, nodarium_utils::concat_args(vec![&[encode_float(0.25)]]));
        assert!(t.len() > 1);
        // expressions and Myzel nodes read it as the time
        assert_eq!(nodarium_utils::Ctx::default().float(&t), 0.25);
        assert_eq!(myzel_core::read_float(&t), 0.25);
    }
}
