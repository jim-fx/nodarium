pub use nodarium_utils::{encode_color, linear_rgba, mix_colors, opaque, pack_linear, NO_COLOR};
use nodarium_utils::{plain_color, Ctx};

/// A color input: the sRGB bytes `[0, 4, r, g, b, 1, 1]` set on the node or sent by a color
/// node, a connected node may wrap it in a list of its own, or the expression of `mix_color`.
/// Myzel nodes read an expression once, without a path and with alpha 0.
pub fn read_color(arg: &[i32]) -> [u8; 3] {
    plain_color(arg).unwrap_or_else(|| Ctx::default().color(arg))
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodarium_utils::{concat_args, encode_float, MIX_COLOR};

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
