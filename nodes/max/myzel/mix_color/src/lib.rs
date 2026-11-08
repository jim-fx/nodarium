use myzel_core::{encode_color, mix_colors, read_color};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::Ctx;

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    // a curve on time sends an expression
    let factor = Ctx::default().float(args[2]);
    encode_color(mix_colors(read_color(args[0]), read_color(args[1]), factor))
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use myzel_core::{encode_color, read_color};
    use nodarium_utils::{concat_args, encode_float};

    fn mix(a: [u8; 3], b: [u8; 3], factor: &[i32]) -> [u8; 3] {
        read_color(&__nodarium_execute_inner(&[&encode_color(a), &encode_color(b), factor]))
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
    fn the_output_is_a_color_input() {
        let color = encode_color([1, 2, 3]);
        assert_eq!(__nodarium_execute_inner(&[&color, &color, &[encode_float(0.3)]]), color);
    }
}
