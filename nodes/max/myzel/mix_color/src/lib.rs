use myzel_core::{encode_color, mix_colors};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{concat_args, plain_color, split_args, Ctx, MIX_COLOR};

nodarium_definition_file!("src/definition.json");

/// A factor that is a number and no expression, plain or in brackets
fn plain_factor(arg: &[i32]) -> bool {
    arg.len() == 1 || matches!(split_args(arg).as_slice(), [[_]])
}

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    match (plain_color(args[0]), plain_color(args[1])) {
        (Some(a), Some(b)) if plain_factor(args[2]) => encode_color(mix_colors(a, b, Ctx::default().float(args[2]))),
        // the node that reads the color evaluates it, with alpha and the path it works on
        _ => concat_args(vec![&[MIX_COLOR], args[0], args[1], args[2]]),
    }
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use myzel_core::{encode_color, mix_colors, read_color};
    use nodarium_utils::{concat_args, encode_float, Ctx};

    fn mix(a: [u8; 3], b: [u8; 3], factor: &[i32]) -> [u8; 3] {
        read_color(&__nodarium_execute_inner(&[&encode_color(a), &encode_color(b), factor]))
    }

    fn alpha() -> Vec<i32> {
        concat_args(vec![&[2], &[0]])
    }

    #[test]
    fn mixes_from_a_to_b() {
        let (pink, brown) = ([222, 170, 160], [70, 46, 36]);
        assert_eq!(mix(pink, brown, &[encode_float(0.0)]), pink);
        assert_eq!(mix(pink, brown, &[encode_float(1.0)]), brown);
        // a curve or time node sends its value in brackets
        let half = mix(pink, brown, &concat_args(vec![&[encode_float(0.5)]]));
        for c in 0..3 {
            assert!(half[c] < pink[c] && half[c] > brown[c], "{:?}", half);
        }
        let lightness = |c: [u8; 3]| c.iter().map(|&v| v as u32).sum::<u32>();
        let steps: Vec<u32> = (0..=10).map(|i| lightness(mix(pink, brown, &[encode_float(i as f32 / 10.0)]))).collect();
        assert!(steps.windows(2).all(|w| w[1] <= w[0]), "darkens step by step: {:?}", steps);
    }

    #[test]
    fn a_plain_factor_gives_a_color() {
        let color = encode_color([1, 2, 3]);
        assert_eq!(__nodarium_execute_inner(&[&color, &color, &[encode_float(0.3)]]), color);
    }

    #[test]
    fn an_expression_is_read_where_the_color_is_used() {
        let (green, brown) = ([60, 140, 50], [110, 80, 50]);
        let out = __nodarium_execute_inner(&[&encode_color(green), &encode_color(brown), &alpha()]);
        let at = |a: f32| {
            let mut ctx = Ctx::default();
            ctx.alpha = a;
            ctx.color(&out)
        };
        assert_eq!(at(0.0), green);
        assert_eq!(at(1.0), brown);
        assert_eq!(at(0.5), mix_colors(green, brown, 0.5));
        // Myzel nodes read it without a path, at alpha 0
        assert_eq!(read_color(&out), green);
    }

    #[test]
    fn mixes_chain() {
        let (a, b, c) = ([255, 255, 255], [0, 0, 0], [255, 0, 0]);
        let inner = __nodarium_execute_inner(&[&encode_color(a), &encode_color(b), &alpha()]);
        let outer = __nodarium_execute_inner(&[&inner, &encode_color(c), &[encode_float(0.5)]]);
        let mut ctx = Ctx::default();
        ctx.alpha = 1.0;
        assert_eq!(ctx.color(&outer), mix_colors(b, c, 0.5));
    }
}
