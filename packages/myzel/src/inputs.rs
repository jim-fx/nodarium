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

fn is_frame(arg: &[i32]) -> bool {
    let n = arg.len();
    n >= 4 && arg[0] == 0 && arg[1] as usize == n - 3 && arg[n - 2] == 1 && arg[n - 1] == 1
}

/// An array input (vec3, curve, shape) without the brackets `[0, n + 1, ...values, 1, 1]` of the
/// host. A value set on the node arrives as one such frame, one from a connected node wrapped in a
/// list of its own, `concat_args([frame])` = `[0, 1, ...frame, 1, 1]`.
pub fn read_floats(arg: &[i32]) -> Vec<f32> {
    let n = arg.len();
    let connected = n >= 8 && arg[..2] == [0, 1] && arg[n - 2..] == [1, 1] && is_frame(&arg[2..n - 2]);
    let frame = if connected { &arg[2..n - 2] } else { arg };
    assert!(is_frame(frame), "expected a list of floats [0, n + 1, ...values, 1, 1], got {} words: {:?}", n, arg);
    frame[2..frame.len() - 2].iter().map(|&w| decode_float(w)).collect()
}

pub fn read_vec3(arg: &[i32]) -> Vec3 {
    let values = read_floats(arg);
    assert!(values.len() == 3, "expected a vec3, got {} values", values.len());
    Vec3::new(values[0], values[1], values[2])
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
    }

    #[test]
    fn reads_a_list_from_a_connected_node() {
        let curve = [0, 4, encode_float(0.0), encode_float(0.5), encode_float(1.0), 1, 1];
        let connected = nodarium_utils::concat_args(vec![&curve]);
        assert_eq!(read_floats(&connected), vec![0.0, 0.5, 1.0]);
        // a point at 0 is not mistaken for a bracket
        let zeros = [0, 3, 0, 0, 1, 1];
        assert_eq!(read_floats(&nodarium_utils::concat_args(vec![&zeros])), vec![0.0, 0.0]);
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
