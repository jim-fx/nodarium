use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use myzel_core::{read_float, read_vec3, Field};

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let position = read_vec3(args[0]);
    let radius = read_float(args[1]);
    Field::sphere(radius).translate(position).encode()
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
    fn sphere_at_position() {
        let words = __nodarium_execute_inner(&[&vec3(1.0, 2.0, 0.0), &[encode_float(0.5)]]);
        assert_eq!(eval(&words, Vec3::new(1.0, 2.0, 0.0)), -0.5);
        assert_eq!(eval(&words, Vec3::new(1.0, 3.0, 0.0)), 0.5);
    }
}
