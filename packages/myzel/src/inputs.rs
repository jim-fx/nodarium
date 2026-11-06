use glam::Vec3;
use nodarium_utils::decode_float;

use crate::field::Field;
use crate::hyphae::Hyphae;
use crate::mesh::Mesh;

/// A float input, which the host sends as one f32 word
pub fn read_float(arg: &[i32]) -> f32 {
    assert!(
        arg.len() == 1,
        "expected a plain float (one word), got {} words. Myzel nodes take no expressions, \
         connect a value or set it on the node",
        arg.len()
    );
    decode_float(arg[0])
}

/// An integer, select or seed input, which the host sends as one word
pub fn read_int(arg: &[i32]) -> i32 {
    assert!(arg.len() == 1, "expected a plain integer (one word), got {} words", arg.len());
    arg[0]
}

/// An array input (vec3, curve, shape) without the brackets `[0, n + 1, ...values, 1, 1]` of the host
pub fn read_floats(arg: &[i32]) -> Vec<f32> {
    let framed = arg.len() >= 4
        && arg[0] == 0
        && arg[1] as usize == arg.len() - 3
        && arg[arg.len() - 2] == 1
        && arg[arg.len() - 1] == 1;
    assert!(framed, "expected a list of floats [0, n + 1, ...values, 1, 1], got {} words: {:?}", arg.len(), arg);
    arg[2..arg.len() - 2].iter().map(|&w| decode_float(w)).collect()
}

pub fn read_vec3(arg: &[i32]) -> Vec3 {
    let values = read_floats(arg);
    assert!(values.len() == 3, "expected a vec3, got {} values", values.len());
    Vec3::new(values[0], values[1], values[2])
}

/// A color input `[0, 4, r, g, b, 1, 1]` with integer components in [0, 255], as sRGB in [0, 1]
pub fn read_color(arg: &[i32]) -> [f32; 3] {
    let framed = arg.len() == 7 && arg[0] == 0 && arg[1] == 4 && arg[5] == 1 && arg[6] == 1;
    assert!(framed, "expected a color [0, 4, r, g, b, 1, 1], got {} words: {:?}", arg.len(), arg);
    [arg[2], arg[3], arg[4]].map(|c| c.clamp(0, 255) as f32 / 255.0)
}

pub fn read_field(arg: &[i32]) -> Field {
    Field::decode(arg)
}

pub fn read_mesh(arg: &[i32]) -> Mesh {
    Mesh::decode(arg)
}

pub fn read_hyphae(arg: &[i32]) -> Hyphae {
    Hyphae::decode(arg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodarium_utils::encode_float;

    #[test]
    fn reads_plain_values() {
        assert_eq!(read_float(&[encode_float(1.5)]), 1.5);
        assert_eq!(read_int(&[7]), 7);
        let curve = [0, 4, encode_float(0.0), encode_float(0.5), encode_float(1.0), 1, 1];
        assert_eq!(read_floats(&curve), vec![0.0, 0.5, 1.0]);
        let v = [0, 4, encode_float(1.0), encode_float(2.0), encode_float(3.0), 1, 1];
        assert_eq!(read_vec3(&v), Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(read_color(&[0, 4, 255, 0, 51, 1, 1]), [1.0, 0.0, 0.2]);
    }

    #[test]
    #[should_panic(expected = "take no expressions")]
    fn rejects_expressions_as_floats() {
        // a math node output: [0, count, op, a, b, 1, 1]
        read_float(&[0, 4, 1, encode_float(1.0), encode_float(2.0), 1, 1]);
    }

    #[test]
    #[should_panic(expected = "expected a list of floats")]
    fn rejects_unframed_lists() {
        read_floats(&[encode_float(1.0), encode_float(2.0)]);
    }
}
