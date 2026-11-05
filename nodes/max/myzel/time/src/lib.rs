use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use myzel_core::read_float;
use nodarium_utils::encode_float;

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    vec![encode_float(read_float(args[0]).clamp(0.0, 1.0))]
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    #[allow(unused_imports)]
    use glam::Vec3;
    #[allow(unused_imports)]
    use myzel_core::{eval, Field};
    #[allow(unused_imports)]
    use nodarium_utils::{decode_float, encode_float};

    #[allow(dead_code)]
    fn vec3(x: f32, y: f32, z: f32) -> Vec<i32> {
        vec![0, 4, encode_float(x), encode_float(y), encode_float(z), 1, 1]
    }

    #[test]
    fn passes_the_time_on() {
        assert_eq!(__nodarium_execute_inner(&[&[encode_float(0.25)]]), vec![encode_float(0.25)]);
        assert_eq!(__nodarium_execute_inner(&[&[encode_float(1.5)]]), vec![encode_float(1.0)]);
    }
}
