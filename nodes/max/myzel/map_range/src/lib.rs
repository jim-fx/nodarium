use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use myzel_core::{read_float, read_int};
use nodarium_utils::encode_float;

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let value = read_float(args[0]);
    let (from_min, from_max) = (read_float(args[1]), read_float(args[2]));
    let (to_min, to_max) = (read_float(args[3]), read_float(args[4]));
    let smooth = read_int(args[5]) != 0;

    let range = from_max - from_min;
    let mut t = if range == 0.0 {
        if value >= from_max { 1.0 } else { 0.0 }
    } else {
        ((value - from_min) / range).clamp(0.0, 1.0)
    };
    if smooth {
        t = t * t * (3.0 - 2.0 * t);
    }
    vec![encode_float(to_min + (to_max - to_min) * t)]
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

    fn map(value: f32, from: (f32, f32), to: (f32, f32), smooth: bool) -> f32 {
        let f = |v: f32| vec![encode_float(v)];
        let result = __nodarium_execute_inner(&[
            &f(value),
            &f(from.0),
            &f(from.1),
            &f(to.0),
            &f(to.1),
            &[smooth as i32],
        ]);
        decode_float(result[0])
    }

    #[test]
    fn maps_and_clamps() {
        assert_eq!(map(0.5, (0.0, 1.0), (2.0, 4.0), false), 3.0);
        assert_eq!(map(0.25, (0.0, 0.5), (0.0, 1.0), false), 0.5);
        assert_eq!(map(2.0, (0.0, 1.0), (0.0, 10.0), false), 10.0);
        assert_eq!(map(-1.0, (0.0, 1.0), (0.0, 10.0), false), 0.0);
        // reversed target ranges count down
        assert_eq!(map(0.25, (0.0, 1.0), (1.0, 0.0), false), 0.75);
    }

    #[test]
    fn smooth_eases_in_and_out() {
        assert_eq!(map(0.5, (0.0, 1.0), (0.0, 1.0), true), 0.5);
        assert!(map(0.1, (0.0, 1.0), (0.0, 1.0), true) < 0.1);
        assert!(map(0.9, (0.0, 1.0), (0.0, 1.0), true) > 0.9);
    }

    #[test]
    fn empty_source_range_steps() {
        assert_eq!(map(0.4, (0.5, 0.5), (0.0, 1.0), false), 0.0);
        assert_eq!(map(0.5, (0.5, 0.5), (0.0, 1.0), false), 1.0);
    }
}
