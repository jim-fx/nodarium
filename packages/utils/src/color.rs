//! Colors: what a color input holds, and how colors mix.
//!
//! A color input is the sRGB bytes `[0, 4, r, g, b, 1, 1]`, the way the host sends a color set on
//! a node, or the expression of the `mix_color` node `[7, a, b, factor]`, where `a` and `b` are
//! colors and `factor` a float, see [`crate::Ctx::color`]. A color on a path, a geometry or a mesh
//! is an RGBA8 word, red in the lowest byte, see [`opaque`] and [`NO_COLOR`].

use std::sync::OnceLock;

/// The first word of the expression of the `mix_color` node
pub const MIX_COLOR: i32 = 7;

/// A vertex without a color of its own: the viewer shows its material there. White, so programs
/// that multiply the vertex color with their material see the material.
pub const NO_COLOR: u32 = 0x00ff_ffff;

/// The sRGB bytes of a color input that is no expression, `None` for an expression
pub fn plain_color(arg: &[i32]) -> Option<[u8; 3]> {
    match arg {
        [0, 4, r, g, b, 1, 1] | [0, 1, 0, 4, r, g, b, 1, 1, 1, 1] => Some([*r, *g, *b].map(|v| v.clamp(0, 255) as u8)),
        _ => None,
    }
}

/// A color as a node sends it, see [`plain_color`]
pub fn encode_color([r, g, b]: [u8; 3]) -> Vec<i32> {
    vec![0, 4, r as i32, g as i32, b as i32, 1, 1]
}

fn to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn to_srgb(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

fn unit_byte(v: f32) -> u32 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u32
}

/// The sRGB byte of a linear channel, rounded from the `powf` of the channel
fn srgb_byte_by_powf(c: f32) -> u32 {
    unit_byte(to_srgb(c))
}

/// [`to_linear`] of every byte. On wasm `powf` is the software one, and it ran three times for
/// every color mixed.
fn linear_bytes() -> &'static [f32; 256] {
    static TABLE: OnceLock<[f32; 256]> = OnceLock::new();
    TABLE.get_or_init(|| std::array::from_fn(|b| to_linear(b as f32 / 255.0)))
}

/// Per sRGB byte from 1 on, the least linear channel that rounds to it or above, see
/// [`srgb_byte_by_powf`]. Its bytes rise with the channel, so the byte of a channel is the number
/// of thresholds at or below it, the same byte without a `powf`.
fn srgb_thresholds() -> &'static [f32; 255] {
    static TABLE: OnceLock<[f32; 255]> = OnceLock::new();
    TABLE.get_or_init(|| {
        std::array::from_fn(|i| {
            let byte = i as u32 + 1;
            // the positive floats are ordered as their bits, 1.0 gives 255
            let (mut lo, mut hi) = (0u32, 1.0f32.to_bits());
            while lo < hi {
                let mid = lo + (hi - lo) / 2;
                if srgb_byte_by_powf(f32::from_bits(mid)) >= byte {
                    hi = mid;
                } else {
                    lo = mid + 1;
                }
            }
            f32::from_bits(lo)
        })
    })
}

fn srgb_byte(c: f32) -> u32 {
    srgb_thresholds().partition_point(|&t| t <= c) as u32
}

/// An RGBA8 word as linear RGB and alpha, which mix like light does
pub fn linear_rgba(word: u32) -> [f32; 4] {
    let table = linear_bytes();
    let byte = |i: u32| ((word >> (i * 8)) & 0xff) as usize;
    [table[byte(0)], table[byte(1)], table[byte(2)], byte(3) as f32 / 255.0]
}

/// The RGBA8 word of a linear color, see [`linear_rgba`]
pub fn pack_linear([r, g, b, a]: [f32; 4]) -> u32 {
    srgb_byte(r) | srgb_byte(g) << 8 | srgb_byte(b) << 16 | unit_byte(a) << 24
}

/// The opaque RGBA8 word of sRGB bytes
pub fn opaque([r, g, b]: [u8; 3]) -> u32 {
    r as u32 | (g as u32) << 8 | (b as u32) << 16 | 0xff00_0000
}

/// `a` at 0 to `b` at 1, mixed in linear light like paint under the same light
pub fn mix_colors(a: [u8; 3], b: [u8; 3], factor: f32) -> [u8; 3] {
    let t = factor.clamp(0.0, 1.0);
    let (a, b) = (linear_rgba(opaque(a)), linear_rgba(opaque(b)));
    let mixed = pack_linear([0, 1, 2, 3].map(|i| a[i] + (b[i] - a[i]) * t));
    [0, 8, 16].map(|s| (mixed >> s) as u8)
}

/// Mixes two RGBA8 words in linear light, `a` at 0 to `b` at 1, alpha included
pub fn mix_words(a: u32, b: u32, factor: f32) -> u32 {
    if a == b {
        return a;
    }
    let (a, b) = (linear_rgba(a), linear_rgba(b));
    pack_linear([0, 1, 2, 3].map(|i| a[i] + (b[i] - a[i]) * factor))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_color_set_on_the_node_or_connected() {
        assert_eq!(plain_color(&[0, 4, 250, 128, 0, 1, 1]), Some([250, 128, 0]));
        assert_eq!(plain_color(&[0, 1, 0, 4, 1, 2, 3, 1, 1, 1, 1]), Some([1, 2, 3]));
        assert_eq!(plain_color(&encode_color([9, 8, 7])), Some([9, 8, 7]));
        assert_eq!(plain_color(&[0, 4, 300, -2, 7, 1, 1]), Some([255, 0, 7]));
        assert_eq!(plain_color(&[1]), None);
    }

    #[test]
    fn the_tables_give_what_powf_gives() {
        for b in 0..=255u32 {
            let c = b as f32 / 255.0;
            assert_eq!(linear_bytes()[b as usize].to_bits(), to_linear(c).to_bits());
            let word = b | (255 - b) << 8 | (b / 2) << 16 | b << 24;
            let by_powf = [to_linear(c), to_linear((255 - b) as f32 / 255.0), to_linear((b / 2) as f32 / 255.0), c];
            assert_eq!(linear_rgba(word).map(f32::to_bits), by_powf.map(f32::to_bits));
        }
        // around every threshold, and a fine sweep of the channel with what lies outside it
        let mut channels: Vec<f32> = vec![f32::NAN, f32::NEG_INFINITY, -1.0, -0.0, 0.0, 1.0, 1.5, f32::INFINITY, f32::MIN_POSITIVE];
        for &t in srgb_thresholds() {
            channels.extend((-3..=3).map(|d| f32::from_bits((t.to_bits() as i32 + d) as u32)));
        }
        channels.extend((0..=200_000).map(|i| i as f32 / 200_000.0));
        channels.extend((0..=1000).map(|i| 0.004 * i as f32 / 1000.0));
        for c in channels {
            assert_eq!(srgb_byte(c), srgb_byte_by_powf(c), "channel {}", c);
            let packed = pack_linear([c, 0.5, c, c]);
            let expected = srgb_byte_by_powf(c) | srgb_byte_by_powf(0.5) << 8 | srgb_byte_by_powf(c) << 16 | unit_byte(c) << 24;
            assert_eq!(packed, expected, "channel {}", c);
        }
    }

    /// Every float of the channel from 0 to 1, a few seconds in release: `cargo test --release -- --ignored`
    #[test]
    #[ignore]
    fn the_thresholds_give_what_powf_gives_for_every_float() {
        for bits in 0..=1.0f32.to_bits() {
            let c = f32::from_bits(bits);
            assert_eq!(srgb_byte(c), srgb_byte_by_powf(c), "channel {}", c);
        }
    }

    #[test]
    fn mixes_in_linear_light() {
        assert_eq!(mix_colors([255, 0, 0], [0, 0, 255], 0.0), [255, 0, 0]);
        assert_eq!(mix_colors([255, 0, 0], [0, 0, 255], 1.0), [0, 0, 255]);
        assert_eq!(mix_colors([255, 0, 0], [0, 0, 255], 7.0), [0, 0, 255]);
        // half of white light is lighter than the middle byte
        let [g, _, _] = mix_colors([0, 0, 0], [255, 255, 255], 0.5);
        assert!((186..=189).contains(&g), "{}", g);
        for c in [0u8, 1, 50, 128, 200, 255] {
            assert_eq!(pack_linear(linear_rgba(opaque([c, c, c]))), opaque([c, c, c]));
        }
    }

    #[test]
    fn words_mix_with_their_alpha() {
        let red = opaque([255, 0, 0]);
        assert_eq!(mix_words(red, red, 0.3), red);
        assert_eq!(mix_words(red, NO_COLOR, 0.0), red);
        assert_eq!(mix_words(red, NO_COLOR, 1.0), NO_COLOR);
        assert_eq!(mix_words(red, NO_COLOR, 0.5) >> 24, 128);
    }
}
