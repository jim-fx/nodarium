use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{concat_args, decode_float, encode_float, split_args, wrap_arg, Ctx};

nodarium_definition_file!("src/definition.json");

/// The points of a shape input, connected (wrapped by the shape node) or not
fn points(arg: &[i32]) -> Vec<[f32; 2]> {
    let shape = match split_args(arg).first() {
        Some(inner) if inner.len() > 1 => *inner,
        _ => arg,
    };
    let count = shape.len().saturating_sub(4) / 2;
    (0..count)
        .map(|i| [decode_float(shape[2 + i * 2]), decode_float(shape[3 + i * 2])])
        .collect()
}

/// `count` points spread evenly along the outline through `points`
fn resample(points: &[[f32; 2]], count: usize) -> Vec<[f32; 2]> {
    let mut lengths = vec![0.0f32];
    for w in points.windows(2) {
        let step = (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]);
        lengths.push(lengths.last().unwrap() + step);
    }
    let total = *lengths.last().unwrap();
    let mut segment = 0;
    (0..count)
        .map(|k| {
            let at = total * k as f32 / (count - 1).max(1) as f32;
            while segment + 2 < points.len() && lengths[segment + 1] < at {
                segment += 1;
            }
            let span = lengths[segment + 1] - lengths[segment];
            let t = if span > 0.0 { ((at - lengths[segment]) / span).clamp(0.0, 1.0) } else { 0.0 };
            let (p, q) = (points[segment], points[segment + 1]);
            [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t]
        })
        .collect()
}

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let mut ctx = Ctx::default();
    let (mut a, mut b) = (points(args[0]), points(args[1]));
    let factor = ctx.float(args[2]);
    if a.len() < 2 || b.len() < 2 {
        let only = if a.len() >= 2 { a } else { b };
        let words: Vec<i32> = only.iter().flat_map(|p| p.map(encode_float)).collect();
        return concat_args(vec![&wrap_arg(&words)]);
    }
    if a.len() != b.len() {
        let count = a.len().max(b.len());
        a = resample(&a, count);
        b = resample(&b, count);
    }
    let words: Vec<i32> = a
        .iter()
        .zip(&b)
        .flat_map(|(p, q)| [0, 1].map(|c| encode_float(p[c] + (q[c] - p[c]) * factor)))
        .collect();
    concat_args(vec![&wrap_arg(&words)])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shape(points: &[[f32; 2]]) -> Vec<i32> {
        let words: Vec<i32> = points.iter().flat_map(|p| p.map(encode_float)).collect();
        concat_args(vec![&wrap_arg(&words)])
    }

    fn run(a: &[[f32; 2]], b: &[[f32; 2]], factor: f32) -> Vec<[f32; 2]> {
        points(&__nodarium_execute_inner(&[&shape(a), &shape(b), &[encode_float(factor)]]))
    }

    #[test]
    fn blends_point_by_point() {
        let a = [[50.0, 100.0], [10.0, 50.0], [50.0, 0.0]];
        let b = [[50.0, 100.0], [40.0, 50.0], [50.0, 0.0]];
        assert_eq!(run(&a, &b, 0.0), a.to_vec());
        assert_eq!(run(&a, &b, 1.0), b.to_vec());
        assert_eq!(run(&a, &b, 0.5)[1], [25.0, 50.0]);
    }

    #[test]
    fn shapes_of_other_lengths_are_spread_along_their_outline() {
        let a = [[50.0, 100.0], [50.0, 0.0]];
        let b = [[50.0, 100.0], [50.0, 50.0], [50.0, 0.0]];
        assert_eq!(run(&a, &b, 0.0), vec![[50.0, 100.0], [50.0, 50.0], [50.0, 0.0]]);
    }

    #[test]
    fn an_unconnected_input_reads_like_a_connected_one() {
        let a = [[50.0, 100.0], [10.0, 50.0], [50.0, 0.0]];
        let words: Vec<i32> = a.iter().flat_map(|p| p.map(encode_float)).collect();
        let out = __nodarium_execute_inner(&[&wrap_arg(&words), &shape(&a), &[encode_float(0.3)]]);
        assert_eq!(points(&out), a.to_vec());
    }
}
