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
            [[3], value, min, max, points @ ..] => {
                let x = self.float(value).clamp(0.0, 1.0);
                let (min, max) = (self.float(min), self.float(max));
                let points: Vec<f32> = points.iter().map(|p| decode_float(p[0])).collect();
                min + curve(&points, x) * (max - min)
            }
            _ => 0.0,
        };

        if v.is_nan() {
            0.0
        } else {
            v
        }
    }

    /// Evaluates `arg` at each of `alphas`, e.g. at every point of a path. Random values
    /// are drawn once, so only what depends on alpha changes from one alpha to the next.
    pub fn float_along(&mut self, arg: &[i32], alphas: impl IntoIterator<Item = f32>) -> Vec<f32> {
        let (alpha, calls) = (self.alpha, self.calls);
        let mut end = calls;

        let values = alphas
            .into_iter()
            .map(|a| {
                self.alpha = a;
                self.calls = calls;
                let v = self.float(arg);
                end = self.calls;
                v
            })
            .collect();

        self.alpha = alpha;
        self.calls = end;
        values
    }

    /// Reads an integer input, a plain value or the expression of a `random_int` node.
    /// Like `float`, every call draws a new random value.
    pub fn int(&mut self, arg: &[i32]) -> i32 {
        if arg.len() == 1 {
            return arg[0];
        }

        match split_args(arg).as_slice() {
            [v] => self.int(v),
            [[4], min, max, [seed]] => {
                let (min, max) = (self.int(min), self.int(max));
                let (min, max) = (min.min(max), min.max(max));
                let range = (max as i64 - min as i64 + 1) as u64;
                (min as i64 + (self.random_u32(*seed) as u64 % range) as i64) as i32
            }
            _ => 0,
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

    /// A uniformly distributed random number. Unlike `random`, which reads a noise
    /// and gives values near the middle of the range more often, every value is
    /// equally likely, so a `random_int` reaches its min and max as often as the
    /// values between them.
    fn random_u32(&mut self, seed: i32) -> u32 {
        // lowbias32 by Chris Wellons
        let mut x = (seed as u32) ^ (self.calls as u32).wrapping_mul(0x9e37_79b9);
        x ^= x >> 16;
        x = x.wrapping_mul(0x21f0_aaad);
        x ^= x >> 15;
        x = x.wrapping_mul(0x735a_2d97);
        x ^= x >> 15;

        self.calls = (self.calls + 512) % i32::MAX;

        x
    }
}

/// Reads the curve through `points` (`x0, y0, x1, y1, ...`, sorted by x) at `x`. Between two
/// points it is a monotone cubic (Steffen), so it never overshoots the points.
fn curve(points: &[f32], x: f32) -> f32 {
    let n = points.len() / 2;
    if n < 2 {
        return x;
    }

    let px = |i: usize| points[i * 2];
    let py = |i: usize| points[i * 2 + 1];
    let slope = |i: usize| {
        let h = px(i + 1) - px(i);
        if h > 0.0 {
            (py(i + 1) - py(i)) / h
        } else {
            0.0
        }
    };
    let tangent = |i: usize| {
        if i == 0 {
            return slope(0);
        }
        if i == n - 1 {
            return slope(n - 2);
        }
        let (s0, s1) = (slope(i - 1), slope(i));
        if s0 * s1 <= 0.0 {
            return 0.0;
        }
        let (h0, h1) = (px(i) - px(i - 1), px(i + 1) - px(i));
        let p = (s0 * h1 + s1 * h0) / (h0 + h1);
        s0.signum() * 2.0 * s0.abs().min(s1.abs()).min(0.5 * p.abs())
    };

    let i = (0..n - 1).find(|&i| x <= px(i + 1)).unwrap_or(n - 2);
    let h = px(i + 1) - px(i);
    if h <= 0.0 {
        return py(i + 1);
    }

    let t = ((x - px(i)) / h).clamp(0.0, 1.0);
    let (t2, t3) = (t * t, t * t * t);
    (2.0 * t3 - 3.0 * t2 + 1.0) * py(i)
        + (t3 - 2.0 * t2 + t) * h * tangent(i)
        + (-2.0 * t3 + 3.0 * t2) * py(i + 1)
        + (t3 - t2) * h * tangent(i + 1)
}

/// Reads a select, boolean or seed input, or an integer input that is set by a
/// setting. These are never connected to an expression, so they always hold a
/// single value.
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

    fn curve_expr(value: f32, min: f32, max: f32, points: &[f32]) -> Vec<i32> {
        let words: Vec<[i32; 1]> = points.iter().map(|p| [encode_float(*p)]).collect();
        let mut parts: Vec<&[i32]> = vec![&[3]];
        let (value, min, max) = ([encode_float(value)], [encode_float(min)], [encode_float(max)]);
        parts.extend([&value[..], &min[..], &max[..]]);
        parts.extend(words.iter().map(|w| &w[..]));
        concat_args(parts)
    }

    #[test]
    fn test_curve_line() {
        let line = [0.0, 0.0, 1.0, 1.0];
        let mut ctx = Ctx::default();

        assert_eq!(ctx.float(&curve_expr(0.25, 0.0, 1.0, &line)), 0.25);
        assert_eq!(ctx.float(&curve_expr(0.5, 2.0, 4.0, &line)), 3.0);
        // the input is clamped to 0-1
        assert_eq!(ctx.float(&curve_expr(-1.0, 0.0, 1.0, &line)), 0.0);
        assert_eq!(ctx.float(&curve_expr(2.0, 0.0, 1.0, &line)), 1.0);
    }

    #[test]
    fn test_curve_bell() {
        let bell = [0.0, 0.0, 0.5, 1.0, 1.0, 0.0];
        let mut ctx = Ctx::default();

        assert_eq!(ctx.float(&curve_expr(0.5, 0.0, 1.0, &bell)), 1.0);
        assert_eq!(ctx.float(&curve_expr(0.25, 0.0, 1.0, &bell)), 0.625);

        // the peak is the highest value, the curve does not overshoot it
        for i in 0..=100 {
            let y = ctx.float(&curve_expr(i as f32 / 100.0, 0.0, 1.0, &bell));
            assert!((0.0..=1.0).contains(&y), "{} at {}", y, i);
        }
    }

    #[test]
    fn test_curve_plateau_stays_flat() {
        let plateau = [0.0, 0.0, 0.3, 1.0, 0.7, 1.0, 1.0, 0.0];
        let mut ctx = Ctx::default();

        for i in 30..=70 {
            assert_eq!(ctx.float(&curve_expr(i as f32 / 100.0, 0.0, 1.0, &plateau)), 1.0);
        }
    }

    #[test]
    fn test_float_along_draws_random_once() {
        let alpha = concat_args(vec![&[2], &[0]]);
        let random = concat_args(vec![&[1], &[encode_float(1.0)], &[encode_float(2.0)], &[42]]);
        // alpha + random
        let sum = concat_args(vec![&[0], &[0], &alpha, &random]);

        let mut ctx = Ctx::default();
        ctx.alpha = 0.7;
        let values = ctx.float_along(&sum, [0.0, 0.5, 1.0]);
        let r = values[0];
        assert!((1.0..=2.0).contains(&r));
        assert_eq!(values, vec![r, r + 0.5, r + 1.0]);
        assert_eq!(ctx.alpha, 0.7);

        // the next random value is the same as after evaluating it once
        let mut once = Ctx::default();
        once.float(&random);
        assert_eq!(ctx.float(&random), once.float(&random));
    }

    fn random_int_expr(min: i32, max: i32, seed: i32) -> Vec<i32> {
        concat_args(vec![&[4], &[min], &[max], &[seed]])
    }

    #[test]
    fn test_int_plain_and_wrapped() {
        let mut ctx = Ctx::default();
        assert_eq!(ctx.int(&[7]), 7);
        assert_eq!(ctx.int(&concat_args(vec![&[7]])), 7);
    }

    #[test]
    fn test_random_int_covers_its_range() {
        let random = random_int_expr(3, 6, 42);
        let mut ctx = Ctx::default();

        let mut counts = [0; 4];
        for _ in 0..1000 {
            let v = ctx.int(&random);
            assert!((3..=6).contains(&v), "{}", v);
            counts[(v - 3) as usize] += 1;
        }
        // every value is about equally likely
        assert!(counts.iter().all(|&c| (200..300).contains(&c)), "{:?}", counts);
    }

    #[test]
    fn test_random_int_is_the_same_every_run() {
        let random = random_int_expr(0, 100, 7);

        let run = || {
            let mut ctx = Ctx::default();
            (0..8).map(|_| ctx.int(&random)).collect::<Vec<_>>()
        };

        assert_eq!(run(), run());
    }

    #[test]
    fn test_random_int_swapped_and_equal_bounds() {
        let mut ctx = Ctx::default();
        for _ in 0..50 {
            assert!((2..=5).contains(&ctx.int(&random_int_expr(5, 2, 1))));
            assert_eq!(ctx.int(&random_int_expr(4, 4, 1)), 4);
        }
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
