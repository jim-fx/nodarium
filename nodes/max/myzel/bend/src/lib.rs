use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use myzel_core::{read_field, read_float};

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let field = read_field(args[0]);
    let amount = read_float(args[1]);
    field.bend(amount).encode()
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
    fn bends_the_top_away() {
        let stem = Field::capsule(Vec3::ZERO, Vec3::new(0.0, 2.0, 0.0), 0.1).encode();
        let words = __nodarium_execute_inner(&[&stem, &[encode_float(0.3)]]);
        assert!(eval(&words, Vec3::ZERO) < 0.0);
        assert!(eval(&words, Vec3::new(0.0, 2.0, 0.0)) > 0.0);
    }
}
