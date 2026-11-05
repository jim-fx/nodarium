use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use myzel_core::{read_field, read_float, read_int};

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let field = read_field(args[0]);
    let amplitude = read_float(args[1]);
    let frequency = read_float(args[2]);
    let seed = read_int(args[3]) as u32;
    field.warp(amplitude, frequency, seed).encode()
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
    fn displaces_depending_on_seed() {
        let field = Field::sphere(1.0).encode();
        let run = |seed: i32| __nodarium_execute_inner(&[&field, &[encode_float(0.2)], &[encode_float(3.0)], &[seed]]);
        let p = Vec3::new(0.9, 0.3, 0.1);
        assert_eq!(eval(&run(1), p), eval(&run(1), p));
        assert_ne!(eval(&run(1), p), eval(&run(2), p));
    }
}
