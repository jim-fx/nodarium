use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use glam::Vec3;
use myzel_core::{read_float, read_vec3, Field};

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let position = read_vec3(args[0]);
    let height = read_float(args[1]);
    let radius = read_float(args[2]);
    Field::capsule(position, position + Vec3::Y * height, radius).encode()
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
    fn capsule_goes_up_from_position() {
        let words = __nodarium_execute_inner(&[&vec3(0.0, 1.0, 0.0), &[encode_float(2.0)], &[encode_float(0.25)]]);
        assert_eq!(eval(&words, Vec3::new(0.0, 4.0, 0.0)), 0.75);
        assert_eq!(eval(&words, Vec3::new(0.5, 2.0, 0.0)), 0.25);
        assert!(eval(&words, Vec3::new(0.0, 0.9, 0.0)) < 0.0);
    }
}
