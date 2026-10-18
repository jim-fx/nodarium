use crate::{decode_float, log, split_args};
use noise::{core::open_simplex::open_simplex_2d, permutationtable::PermutationTable, Vector2};

/// Evaluates the float expressions produced by the `math` and `random` nodes.
///
/// Create one per execution, so a `random` node gives the same values every
/// time the node runs.
#[derive(Default)]
pub struct Ctx {
    /// Where the branch or instance a node is placing sits on its parent path,
    /// from 0 at the base to 1 at the tip. The `alpha` node reads it.
    pub alpha: f32,
    calls: i32,
}

impl Ctx {
    pub fn float(&mut self, arg: &[i32]) -> f32 {
        if arg.len() == 1 {
            return decode_float(arg[0]);
        }

        let v = match split_args(arg).as_slice() {
            // a single value wrapped in brackets, e.g. the output of the float node
            [v] => self.float(v),
            [[0], [op], a, b] => {
                let (a, b) = (self.float(a), self.float(b));
                match op {
                    0 => a + b,
                    1 => a - b,
                    2 => a * b,
                    3 => a / b,
                    _ => 0.0,
                }
            }
            [[1], min, max, [seed]] => {
                let (min, max) = (self.float(min), self.float(max));
                self.random(min, max, *seed)
            }
            [[2], ..] => self.alpha,
            _ => 0.0,
        };

        if v.is_nan() {
            0.0
        } else {
            v
        }
    }

    pub fn vec3(&mut self, arg: &[i32]) -> [f32; 3] {
        let args = split_args(arg);

        assert!(
            args.len() == 3,
            "Failed to evaluate Vec3 - Expected 3 arguments, got {} \n {:?}",
            args.len(),
            args
        );

        [
            self.float(args[0]),
            self.float(args[1]),
            self.float(args[2]),
        ]
    }

    fn random(&mut self, min: f32, max: f32, seed: i32) -> f32 {
        let seed = (seed + self.calls) % 100_000;
        let hasher = PermutationTable::new(seed as u32);

        let value = open_simplex_2d(Vector2::new(seed as f64, 5.0), &hasher) as f32 + 0.5;

        log!(
            "Random node: min: {}, max: {}, seed: {}, v: {}",
            min,
            max,
            seed,
            value
        );

        self.calls = (self.calls + 512) % i32::MAX;

        min + value * (max - min)
    }
}

/// Reads an integer, select, boolean or seed input. These are never
/// connected to an expression, so they always hold a single value.
pub fn int(arg: &[i32]) -> i32 {
    arg[0]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{concat_args, encode_float};

    #[test]
    fn test_recursive_evaluation() {
        let input = vec![
            0, 3, 0, 0, 0, 5, 0, 2, 1073741824, 1073741824, 1, 5, 1073741824,
        ];
        // this is an encoded version of a math node that multiplies 2 * 2
        // and another math node that adds 2 to that result

        assert_eq!(Ctx::default().float(&input), 6.0);
    }

    #[test]
    fn test_wrapped_value() {
        let input = concat_args(vec![&[encode_float(1.5)]]);

        assert_eq!(Ctx::default().float(&input), 1.5);
    }

    #[test]
    fn test_random_is_the_same_every_run() {
        let random = concat_args(vec![&[1], &[encode_float(1.0)], &[encode_float(2.0)], &[42]]);

        let run = || {
            let mut ctx = Ctx::default();
            [ctx.float(&random), ctx.float(&random)]
        };

        let first = run();
        assert_ne!(first[0], first[1]);
        assert_eq!(first, run());
    }

    #[test]
    fn test_alpha() {
        let alpha = concat_args(vec![&[2], &[0]]);
        let doubled = concat_args(vec![&[0], &[2], &alpha, &[encode_float(2.0)]]);

        let mut ctx = Ctx::default();
        assert_eq!(ctx.float(&alpha), 0.0);

        ctx.alpha = 0.25;
        assert_eq!(ctx.float(&alpha), 0.25);
        assert_eq!(ctx.float(&doubled), 0.5);
    }

    #[test]
    fn test_vec3() {
        let input = concat_args(vec![
            &[encode_float(1.0)],
            &[encode_float(2.0)],
            &[encode_float(3.0)],
        ]);

        assert_eq!(Ctx::default().vec3(&input), [1.0, 2.0, 3.0]);
    }
}
