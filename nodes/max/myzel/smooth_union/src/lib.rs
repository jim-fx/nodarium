use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use myzel_core::{read_field, read_float};

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let a = read_field(args[0]);
    let b = read_field(args[1]);
    let k = read_float(args[2]);
    a.smooth_union(b, k).encode()
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
    fn joins_both_fields() {
        let a = Field::sphere(1.0).encode();
        let b = Field::sphere(1.0).translate(Vec3::new(3.0, 0.0, 0.0)).encode();
        let words = __nodarium_execute_inner(&[&a, &b, &[encode_float(0.2)]]);
        assert!(eval(&words, Vec3::ZERO) < 0.0);
        assert!(eval(&words, Vec3::new(3.0, 0.0, 0.0)) < 0.0);
        assert!(eval(&words, Vec3::new(1.5, 0.0, 0.0)) > 0.0);
    }
}
