use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use myzel_core::{read_field, read_vec3};

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let field = read_field(args[0]);
    let offset = read_vec3(args[1]);
    field.translate(offset).encode()
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
    fn moves_the_field() {
        let field = Field::sphere(1.0).encode();
        let words = __nodarium_execute_inner(&[&field, &vec3(0.0, 2.0, 0.0)]);
        assert_eq!(eval(&words, Vec3::new(0.0, 2.0, 0.0)), -1.0);
        assert_eq!(eval(&words, Vec3::ZERO), 1.0);
    }
}
