//! Inputs read as plain values, for nodes that take no expressions: a value set on the node or
//! one a connected value node sends. A node that reads expressions uses [`crate::Ctx`].

use crate::{decode_float, plain_color, Ctx};

/// A float input. The host sends a value set on the node as one f32 word, a connected float or
/// time node sends it in brackets of its own, `[0, 2, value, 1, 1]`.
pub fn read_float(arg: &[i32]) -> f32 {
    let word = match arg {
        [word] | [0, 2, word, 1, 1] => *word,
        _ => panic!(
            "expected a plain float (one word), got {} words. This input takes no expressions, \
             connect a value or set it on the node",
            arg.len()
        ),
    };
    decode_float(word)
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

/// A color input: the sRGB bytes `[0, 4, r, g, b, 1, 1]` set on the node or sent by a color
/// node, a connected node may wrap it in a list of its own, or the expression of `mix_color`,
/// read once, without a path and with alpha 0.
pub fn read_color(arg: &[i32]) -> [u8; 3] {
    plain_color(arg).unwrap_or_else(|| Ctx::default().color(arg))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{concat_args, encode_color, encode_float, mix_colors, MIX_COLOR};

    #[test]
    fn reads_plain_values() {
        assert_eq!(read_float(&[encode_float(1.5)]), 1.5);
        assert_eq!(read_int(&[7]), 7);
        let curve = [0, 4, encode_float(0.0), encode_float(0.5), encode_float(1.0), 1, 1];
        assert_eq!(read_floats(&curve), vec![0.0, 0.5, 1.0]);
    }

    #[test]
    fn reads_a_list_from_a_connected_node() {
        let curve = [0, 4, encode_float(0.0), encode_float(0.5), encode_float(1.0), 1, 1];
        let connected = concat_args(vec![&curve]);
        assert_eq!(read_floats(&connected), vec![0.0, 0.5, 1.0]);
        // a point at 0 is not mistaken for a bracket
        let zeros = [0, 3, 0, 0, 1, 1];
        assert_eq!(read_floats(&concat_args(vec![&zeros])), vec![0.0, 0.0]);
    }

    #[test]
    fn reads_a_float_in_brackets_from_a_connected_node() {
        // the float and time nodes send their value as [0, 2, value, 1, 1]
        let connected = concat_args(vec![&[encode_float(0.25)]]);
        assert_eq!(connected, vec![0, 2, encode_float(0.25), 1, 1]);
        assert_eq!(read_float(&connected), 0.25);
    }

    #[test]
    #[should_panic(expected = "takes no expressions")]
    fn rejects_expressions_as_floats() {
        // a math node output: [0, count, op, a, b, 1, 1]
        read_float(&[0, 4, 1, encode_float(1.0), encode_float(2.0), 1, 1]);
    }

    #[test]
    #[should_panic(expected = "expected a list of floats")]
    fn rejects_unframed_lists() {
        read_floats(&[encode_float(1.0), encode_float(2.0)]);
    }

    #[test]
    fn reads_a_color_set_on_the_node_or_connected() {
        assert_eq!(read_color(&[0, 4, 250, 128, 0, 1, 1]), [250, 128, 0]);
        assert_eq!(read_color(&[0, 1, 0, 4, 1, 2, 3, 1, 1, 1, 1]), [1, 2, 3]);
        assert_eq!(read_color(&encode_color([9, 8, 7])), [9, 8, 7]);
    }

    #[test]
    fn reads_a_mix_as_mix_color_did() {
        let (a, b) = (encode_color([222, 170, 160]), encode_color([70, 46, 36]));
        let half = concat_args(vec![&[encode_float(0.5)]]);
        let mix = concat_args(vec![&[MIX_COLOR], &a, &b, &half]);
        assert_eq!(read_color(&mix), mix_colors([222, 170, 160], [70, 46, 36], 0.5));
    }
}
