use crate::geometry::{path_link, wrap_path};
use crate::{decode_float, log, mix_colors, plain_color, split_args, MIX_COLOR};
use noise::{core::open_simplex::open_simplex_2d, permutationtable::PermutationTable, Vector2};

/// What the `path_info` node can read about the path a node is working on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PathField {
    /// From 0 at the first path to 1 at the last.
    Position,
    /// The number of the path among the paths its creator made on the same parent, the first is 0.
    /// It stays the same when other paths come or go.
    Index,
    /// How many paths the node works on.
    Count,
    /// The layer of the path: 1 for a stem, one more for every branching.
    Depth,
    /// Where on its parent the path grows, from 0 at the base to 1 at the tip.
    /// Paths that grow on their own (stems) have 0.
    ParentPosition,
    /// How long the path is.
    Length,
}

impl PathField {
    /// The order of the options of the `path_info` node.
    pub fn from_index(index: i32) -> Option<Self> {
        Some(match index {
            0 => Self::Position,
            1 => Self::Index,
            2 => Self::Count,
            3 => Self::Depth,
            4 => Self::ParentPosition,
            5 => Self::Length,
            _ => return None,
        })
    }
}

/// The path a node is working on, set by `Ctx::set_path` before the inputs of that path are read.
/// A `branch` or `instance` works on the path it places its branches or instances on.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PathInfo {
    /// The number of the path among the paths its creator made on the same parent.
    pub index: usize,
    /// The place of the path among the paths the node works on, `position` is derived from it.
    pub rank: usize,
    pub count: usize,
    pub depth: i32,
    pub parent_position: f32,
    pub length: f32,
}

impl PathInfo {
    pub fn position(&self) -> f32 {
        self.rank as f32 / self.count.saturating_sub(1).max(1) as f32
    }

    pub fn read(&self, field: PathField) -> f32 {
        match field {
            PathField::Position => self.position(),
            PathField::Index => self.index as f32,
            PathField::Count => self.count as f32,
            PathField::Depth => self.depth as f32,
            PathField::ParentPosition => self.parent_position,
            PathField::Length => self.length,
        }
    }
}

/// Evaluates the float expressions produced by the `math` and `random` nodes.
///
/// Create one per execution, so a `random` node gives the same values every
/// time the node runs.
#[derive(Default)]
pub struct Ctx {
    /// Where the branch or instance a node is placing sits on its parent path,
    /// from 0 at the base to 1 at the tip. The `alpha` node reads it.
    pub alpha: f32,
    /// Where a vertex sits across a geometry, from 0 on its middle to 1 at its edge. Only
    /// `set_color` sets it, the `alpha` node with `across` reads it.
    pub across: f32,
    /// The path the node is working on. The `path_info` node reads it.
    pub path: PathInfo,
    calls: i32,
    /// The path the random values are drawn for, set by `seed_path`
    path_id: i32,
}

impl Ctx {
    /// Starts working on `path`, the `rank`-th of `count` paths. Alpha is the position of the
    /// path among all paths, so `alpha` reads the same as `path_info` with position until a node
    /// sets it to something else, for example the place of a branch on its parent.
    ///
    /// The random values a path draws start from its id, so they stay the same when paths
    /// before it come or go.
    pub fn set_path(&mut self, path: &[i32], rank: usize, count: usize) {
        let link = path_link(path);
        self.seed_path(link.id);
        self.set_path_info(PathInfo {
            index: link.index.max(0) as usize,
            rank,
            count,
            depth: path[3],
            parent_position: if link.has_parent() {
                link.parent_alpha
            } else {
                0.0
            },
            length: wrap_path(path).get_length(),
        });
    }

    /// Makes the random values that follow depend on the path with this id.
    pub fn seed_path(&mut self, id: i32) {
        self.calls = (id as u32 % 100_000) as i32;
        self.path_id = id;
    }

    /// Like `set_path`, for a path that doesn't exist yet, e.g. a stem that is about to be made.
    pub fn set_path_info(&mut self, info: PathInfo) {
        self.path = info;
        self.alpha = info.position();
    }

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
            [[1], min, max, [seed], options @ ..] => {
                let (min, max) = (self.float(min), self.float(max));
                let flag = |i: usize| options.get(i).is_some_and(|o| o.first() == Some(&1));
                let uniform = flag(0);
                let once = flag(3);
                if flag(1) || once {
                    let salt = options.get(2).and_then(|o| o.first()).copied().unwrap_or(0);
                    // drawn for no path at all, so a node without a path reads the same value
                    let path = if once { 0 } else { self.path_id };
                    min + self.per_path(*seed, path, salt, uniform) * (max - min)
                } else if uniform {
                    min + self.random_u32(*seed) as f32 / u32::MAX as f32 * (max - min)
                } else {
                    self.random(min, max, *seed)
                }
            }
            [[2], [1]] => self.across,
            [[2], ..] => self.alpha,
            [[5], [field]] => PathField::from_index(*field)
                .map(|field| self.path.read(field))
                .unwrap_or(0.0),
            [[3], value, min, max, points @ ..] => {
                let x = self.float(value).clamp(0.0, 1.0);
                let (min, max) = (self.float(min), self.float(max));
                let points: Vec<f32> = points.iter().map(|p| decode_float(p[0])).collect();
                min + curve(&points, x) * (max - min)
            }
            [[6], value] => self.float(value).round(),
            [[8], value, count, [shape]] => {
                let x = self.float(value) * self.float(count);
                wave(x - x.floor(), *shape)
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

    /// Reads a color input: a color set on the node or the expression of `mix_color`, whose
    /// factor reads alpha and the path like any float input.
    pub fn color(&mut self, arg: &[i32]) -> [u8; 3] {
        if let Some(color) = plain_color(arg) {
            return color;
        }
        match split_args(arg).as_slice() {
            [[r], [g], [b]] => [*r, *g, *b].map(|v| v.clamp(0, 255) as u8),
            [inner] if inner.len() > 1 => self.color(inner),
            [[MIX_COLOR], a, b, factor] => {
                let (a, b) = (self.color(a), self.color(b));
                mix_colors(a, b, self.float(factor))
            }
            _ => panic!("expected a color [0, 4, r, g, b, 1, 1] or a mix of two, got {} words: {:?}", arg.len(), arg),
        }
    }

    /// Reads a color at every `(alpha, across)`, e.g. at every point of a path or vertex of a
    /// geometry. Like `float_along`, random values are drawn once.
    pub fn colors_at(&mut self, arg: &[i32], at: impl IntoIterator<Item = (f32, f32)>) -> Vec<[u8; 3]> {
        let at = at.into_iter();
        if let Some(color) = plain_color(arg) {
            return at.map(|_| color).collect();
        }
        let (alpha, across, calls) = (self.alpha, self.across, self.calls);
        let mut end = calls;
        let colors = at
            .map(|(a, c)| {
                self.alpha = a;
                self.across = c;
                self.calls = calls;
                let color = self.color(arg);
                end = self.calls;
                color
            })
            .collect();
        self.alpha = alpha;
        self.across = across;
        self.calls = end;
        colors
    }

    /// Reads an integer input, a plain value or the expression of a `random_int` or `round`
    /// node. Like `float`, every call draws a new random value.
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
            [[6], value] => self.float(value).round() as i32,
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

    /// A number from 0 to 1 that only depends on the seed, the path and `salt` (the random
    /// node), so every node that reads it for the same path gets the same value. It draws
    /// nothing, the values after it stay as they were.
    fn per_path(&self, seed: i32, path: i32, salt: i32, uniform: bool) -> f32 {
        let draw = |k: i32| crate::geometry::path_random(seed, path, salt.wrapping_mul(2).wrapping_add(k));
        if uniform {
            draw(0)
        } else {
            // values near the middle come more often, as with the noise of `random`
            0.5 * (draw(0) + draw(1))
        }
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

/// One period of the `wave` node at `u` (0 to 1), 0 on a crest and 1 in a trough. Sine, tooth and
/// round start on a crest and reach the trough halfway. Saw starts in a trough and climbs to the
/// crest at the end of the period, so along a margin its teeth lean toward higher values, the tip.
fn wave(u: f32, shape: i32) -> f32 {
    use std::f32::consts::PI;
    match shape {
        1 => 1.0 - (2.0 * u - 1.0).abs(),
        2 => 1.0 - u,
        3 => 1.0 - (PI * u).cos().abs(),
        _ => 0.5 - 0.5 * (2.0 * PI * u).cos(),
    }
}

/// Reads the curve through `points` (`x0, y0, x1, y1, ...`, sorted by x) at `x`. Between two
/// points it is a monotone cubic (Steffen), so it never overshoots the points.
pub fn curve(points: &[f32], x: f32) -> f32 {
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
/// single value. An empty input (nothing connected) reads as 0 instead of
/// panicking.
pub fn int(arg: &[i32]) -> i32 {
    arg.first().copied().unwrap_or(0)
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
        let random = concat_args(vec![
            &[1],
            &[encode_float(1.0)],
            &[encode_float(2.0)],
            &[42],
        ]);

        let run = || {
            let mut ctx = Ctx::default();
            [ctx.float(&random), ctx.float(&random)]
        };

        let first = run();
        assert_ne!(first[0], first[1]);
        assert_eq!(first, run());
    }

    #[test]
    fn a_random_per_path_is_the_same_wherever_the_path_reads_it() {
        let random = |uniform: i32, salt: i32| {
            concat_args(vec![&[1], &[encode_float(0.0)], &[encode_float(10.0)], &[7], &[uniform], &[1], &[salt]])
        };
        let other = concat_args(vec![&[1], &[encode_float(0.0)], &[encode_float(10.0)], &[7]]);
        let read = |path: i32, first: &[i32], salt: i32| {
            let mut ctx = Ctx::default();
            ctx.seed_path(path);
            ctx.float(first);
            ctx.float(&random(1, salt))
        };
        // read first or after other random values, it stays the same
        let age = read(3, &random(1, 5), 5);
        assert_eq!(age, read(3, &other, 5));
        assert_eq!(age, read(3, &concat_args(vec![&[encode_float(1.0)]]), 5));
        assert!((0.0..=10.0).contains(&age));
        // every path and every random node has its own
        assert_ne!(age, read(4, &other, 5));
        assert_ne!(age, read(3, &other, 6));

        // and it draws nothing, the values after it stay as they were
        let mut a = Ctx::default();
        a.seed_path(3);
        a.float(&random(0, 5));
        let mut b = Ctx::default();
        b.seed_path(3);
        assert_eq!(a.float(&other), b.float(&other));
    }

    #[test]
    fn a_random_once_is_the_same_for_every_path_and_without_one() {
        let random = |seed: i32, salt: i32, per_path: i32| {
            concat_args(vec![&[1], &[encode_float(0.0)], &[encode_float(10.0)], &[seed], &[0], &[per_path], &[salt], &[1]])
        };
        let read = |path: Option<i32>, expr: &[i32]| {
            let mut ctx = Ctx::default();
            if let Some(path) = path {
                ctx.seed_path(path);
            }
            [ctx.float(expr), ctx.float(expr)]
        };
        let size = read(None, &random(7, 5, 0));
        assert_eq!(size[0], size[1]);
        assert!((0.0..=10.0).contains(&size[0]));
        for path in [3, 4, 1234] {
            assert_eq!(size, read(Some(path), &random(7, 5, 0)));
            // it wins over once per path
            assert_eq!(size, read(Some(path), &random(7, 5, 1)));
        }
        // every seed and every random node has its own
        assert_ne!(size, read(None, &random(8, 5, 0)));
        assert_ne!(size, read(None, &random(7, 6, 0)));
    }

    #[test]
    fn test_path_info() {
        let info = |field: i32| concat_args(vec![&[5], &[field]]);

        let mut ctx = Ctx::default();
        // nothing set yet
        assert_eq!(ctx.float(&info(2)), 0.0);

        ctx.set_path_info(PathInfo {
            index: 4,
            rank: 1,
            count: 3,
            depth: 2,
            parent_position: 0.4,
            length: 2.5,
        });
        assert_eq!(ctx.float(&info(0)), 0.5);
        assert_eq!(ctx.float(&info(1)), 4.0);
        assert_eq!(ctx.float(&info(2)), 3.0);
        assert_eq!(ctx.float(&info(3)), 2.0);
        assert_eq!(ctx.float(&info(4)), 0.4);
        assert_eq!(ctx.float(&info(5)), 2.5);
        // an unknown field reads as 0 instead of panicking
        assert_eq!(ctx.float(&info(99)), 0.0);
        // and alpha starts out as the position of the path
        assert_eq!(ctx.float(&concat_args(vec![&[2], &[0]])), 0.5);

        // the info is an expression like the others, a curve can read it
        let curve = concat_args(vec![
            &[3],
            &info(0),
            &[encode_float(0.0)],
            &[encode_float(10.0)],
            &[encode_float(0.0)],
            &[encode_float(0.0)],
            &[encode_float(1.0)],
            &[encode_float(1.0)],
        ]);
        assert_eq!(ctx.float(&curve), 5.0);
    }

    #[test]
    fn test_set_path_reads_the_path() {
        use crate::geometry::{create_path, set_path_link, wrap_path_mut, PathLink};

        // a branch of depth 2 that grows at 0.4 of its parent, 3 units long
        let mut path = create_path(4, 2);
        set_path_link(
            &mut path,
            PathLink {
                id: 7,
                parent_id: 3,
                parent_alpha: 0.4,
                creator: 2,
                index: 6,
            },
        );
        for (i, point) in wrap_path_mut(&mut path).points.chunks_exact_mut(4).enumerate() {
            point.copy_from_slice(&[0.0, i as f32, 0.0, 1.0]);
        }

        let info = |field: i32| concat_args(vec![&[5], &[field]]);
        let mut ctx = Ctx::default();
        ctx.set_path(&path, 1, 3);

        // the position is the place among the paths of the node, the index belongs to the path
        assert_eq!(ctx.float(&info(0)), 0.5);
        assert_eq!(ctx.float(&info(1)), 6.0);
        assert_eq!(ctx.float(&info(2)), 3.0);
        assert_eq!(ctx.float(&info(3)), 2.0);
        assert_eq!(ctx.float(&info(4)), 0.4);
        assert_eq!(ctx.float(&info(5)), 3.0);

        // a path that grows on its own has no parent position
        let stem = create_path(4, 1);
        ctx.set_path(&stem, 0, 1);
        assert_eq!(ctx.float(&info(4)), 0.0);
        assert_eq!(ctx.float(&info(3)), 1.0);
    }

    #[test]
    fn a_path_draws_the_same_random_values_wherever_it_sits_in_the_list() {
        use crate::geometry::{create_path, set_path_link, PathLink};

        let draw = |ctx: &mut Ctx| {
            let expr = concat_args(vec![
                &[1],
                &[encode_float(0.0)],
                &[encode_float(1.0)],
                &[1234],
            ]);
            (ctx.float(&expr), ctx.float(&expr))
        };
        let path = |creator: i32, index: i32| {
            let mut data = create_path(3, 1);
            set_path_link(&mut data, PathLink::root(creator, index));
            data
        };
        let (a, b) = (path(1, 0), path(1, 1));

        // b first or after a, with other draws in between: b sees the same values
        let mut ctx = Ctx::default();
        ctx.set_path(&b, 0, 1);
        let alone = draw(&mut ctx);

        let mut ctx = Ctx::default();
        ctx.set_path(&a, 0, 2);
        let of_a = draw(&mut ctx);
        ctx.set_path(&b, 1, 2);
        let after_a = draw(&mut ctx);

        assert_eq!(alone, after_a);
        assert_ne!(of_a, after_a);
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
        let (value, min, max) = (
            [encode_float(value)],
            [encode_float(min)],
            [encode_float(max)],
        );
        parts.extend([&value[..], &min[..], &max[..]]);
        parts.extend(words.iter().map(|w| &w[..]));
        concat_args(parts)
    }

    fn wave_expr(count: f32, shape: i32) -> Vec<i32> {
        let alpha = concat_args(vec![&[2], &[0]]);
        concat_args(vec![&[8], &alpha, &[encode_float(count)], &[shape]])
    }

    fn assert_close(values: &[f32], expected: &[f32]) {
        for (v, e) in values.iter().zip(expected) {
            assert!((v - e).abs() < 1e-5, "{values:?} != {expected:?}");
        }
    }

    #[test]
    fn a_sine_wave_rises_smoothly_from_a_crest_to_a_trough() {
        let mut ctx = Ctx::default();
        let values = ctx.float_along(&wave_expr(1.0, 0), [0.0, 0.25, 0.5, 0.75]);
        assert_close(&values, &[0.0, 0.5, 1.0, 0.5]);
    }

    #[test]
    fn a_tooth_wave_has_straight_flanks() {
        let mut ctx = Ctx::default();
        let values = ctx.float_along(&wave_expr(1.0, 1), [0.0, 0.25, 0.5, 0.75, 0.875]);
        assert_close(&values, &[0.0, 0.5, 1.0, 0.5, 0.25]);
    }

    #[test]
    fn a_saw_wave_climbs_to_its_crest_and_falls_back_at_once() {
        let mut ctx = Ctx::default();
        let values = ctx.float_along(&wave_expr(1.0, 2), [0.0, 0.25, 0.5, 0.999, 1.0]);
        assert_close(&values, &[1.0, 0.75, 0.5, 0.001, 1.0]);
    }

    #[test]
    fn a_round_wave_has_round_crests_and_sharp_troughs() {
        let mut ctx = Ctx::default();
        let values = ctx.float_along(&wave_expr(1.0, 3), [0.0, 0.5, 0.01, 0.48, 0.52]);
        assert_close(&values[..2], &[0.0, 1.0]);
        // flat next to the crest, steep on both sides of the trough
        assert!(values[2] < 0.001, "{values:?}");
        assert!(1.0 - values[3] > 0.05 && 1.0 - values[4] > 0.05, "{values:?}");
    }

    #[test]
    fn a_wave_repeats_count_times_from_0_to_1() {
        let mut ctx = Ctx::default();
        let at = [0.0, 0.125, 0.25, 0.375, 0.5, 0.625, 0.75, 0.875, 1.0];
        let values = ctx.float_along(&wave_expr(4.0, 1), at);
        assert_close(&values, &[0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0]);
        // no crests at all, the value stays on the first one
        let flat = ctx.float_along(&wave_expr(0.0, 0), at);
        assert_close(&flat, &[0.0; 9]);
    }

    #[test]
    fn a_wave_reads_its_count_like_any_float() {
        let alpha = concat_args(vec![&[2], &[0]]);
        let count = concat_args(vec![&[0], &[2], &[encode_float(2.0)], &[encode_float(3.0)]]);
        let teeth = concat_args(vec![&[8], &alpha, &count, &[1]]);
        let mut ctx = Ctx::default();
        let values = ctx.float_along(&teeth, [0.0, 1.0 / 12.0, 1.0 / 6.0]);
        assert_close(&values, &[0.0, 1.0, 0.0]);
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
            assert_eq!(
                ctx.float(&curve_expr(i as f32 / 100.0, 0.0, 1.0, &plateau)),
                1.0
            );
        }
    }

    #[test]
    fn test_float_along_draws_random_once() {
        let alpha = concat_args(vec![&[2], &[0]]);
        let random = concat_args(vec![
            &[1],
            &[encode_float(1.0)],
            &[encode_float(2.0)],
            &[42],
        ]);
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

    #[test]
    fn test_uniform_random_covers_its_range_evenly() {
        let random = concat_args(vec![&[1], &[encode_float(2.0)], &[encode_float(4.0)], &[42], &[1]]);
        let mut ctx = Ctx::default();

        let mut counts = [0; 4];
        for _ in 0..2000 {
            let v = ctx.float(&random);
            assert!((2.0..=4.0).contains(&v), "{}", v);
            counts[(((v - 2.0) * 2.0) as usize).min(3)] += 1;
        }
        assert!(counts.iter().all(|&c| (420..580).contains(&c)), "{:?}", counts);

        // the same every run, and a draw like any other random
        let run = || {
            let mut ctx = Ctx::default();
            [ctx.float(&random), ctx.float(&random)]
        };
        assert_eq!(run(), run());
        assert_ne!(run()[0], run()[1]);
    }

    #[test]
    fn test_random_without_uniform_reads_the_noise() {
        let plain = concat_args(vec![&[1], &[encode_float(0.0)], &[encode_float(1.0)], &[42]]);
        let off = concat_args(vec![&[1], &[encode_float(0.0)], &[encode_float(1.0)], &[42], &[0]]);
        let (mut a, mut b) = (Ctx::default(), Ctx::default());
        for _ in 0..8 {
            assert_eq!(a.float(&plain), b.float(&off));
        }
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
        assert!(
            counts.iter().all(|&c| (200..300).contains(&c)),
            "{:?}",
            counts
        );
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
    fn test_round_turns_a_float_expression_into_an_int() {
        let product = concat_args(vec![&[0], &[2], &[encode_float(2.6)], &[encode_float(3.0)]]);
        let round = concat_args(vec![&[6], &product]);
        let mut ctx = Ctx::default();
        assert_eq!(ctx.int(&round), 8);
        assert_eq!(ctx.float(&round), 8.0);
        assert_eq!(ctx.int(&concat_args(vec![&[6], &[encode_float(-1.5)]])), -2);
        let bounded = concat_args(vec![&[4], &round, &round, &[1]]);
        assert_eq!(ctx.int(&bounded), 8);
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

    #[test]
    fn a_color_is_plain_or_a_mix_read_with_alpha() {
        use crate::{encode_color, mix_colors, MIX_COLOR};
        let (a, b) = ([200, 40, 10], [20, 60, 220]);
        let mut ctx = Ctx::default();
        assert_eq!(ctx.color(&encode_color(a)), a);
        assert_eq!(ctx.color(&concat_args(vec![&encode_color(a)])), a);

        let along = concat_args(vec![&[MIX_COLOR], &encode_color(a), &encode_color(b), &concat_args(vec![&[2], &[0]])]);
        let across = concat_args(vec![&[MIX_COLOR], &encode_color(a), &encode_color(b), &concat_args(vec![&[2], &[1]])]);
        let colors = ctx.colors_at(&along, [(0.0, 1.0), (0.5, 1.0), (1.0, 0.0)]);
        assert_eq!(colors, vec![a, mix_colors(a, b, 0.5), b]);
        assert_eq!(ctx.colors_at(&across, [(0.0, 1.0), (1.0, 0.0)]), vec![b, a]);
        // alpha and across are as they were
        assert_eq!((ctx.alpha, ctx.across), (0.0, 0.0));

        // a mix of a mix
        let twice = concat_args(vec![&[MIX_COLOR], &along, &encode_color(a), &[encode_float(0.0)]]);
        ctx.alpha = 1.0;
        assert_eq!(ctx.color(&twice), b);
    }

    #[test]
    fn a_random_color_is_the_same_along_the_path() {
        use crate::{encode_color, MIX_COLOR};
        let random = concat_args(vec![&[1], &[encode_float(0.0)], &[encode_float(1.0)], &[3]]);
        let mix = concat_args(vec![&[MIX_COLOR], &encode_color([0, 0, 0]), &encode_color([255, 255, 255]), &random]);
        let mut ctx = Ctx::default();
        let colors = ctx.colors_at(&mix, (0..4).map(|i| (i as f32 / 3.0, 0.0)));
        assert!(colors.iter().all(|&c| c == colors[0]));
        // and the next read draws a new one
        let next = ctx.colors_at(&mix, [(0.0, 0.0)]);
        assert_ne!(next[0], colors[0]);
    }
}
