use glam::Vec2;

use crate::field::Field;

/// The margin of the leaf: the longest closed line of the field, and where every point of it sits
/// between the base of the leaf (0) and its tip (1).
pub struct Margin {
    pub points: Vec<Vec2>,
    pub alphas: Vec<f32>,
}

impl Margin {
    pub fn of(field: &Field) -> Margin {
        let points = longest(field.contour_loops());
        let alphas = alphas(&points);
        Margin { points, alphas }
    }
}

fn longest(loops: Vec<Vec<Vec2>>) -> Vec<Vec2> {
    let length = |l: &Vec<Vec2>| -> f32 { (0..l.len()).map(|i| (l[(i + 1) % l.len()] - l[i]).length()).sum() };
    loops
        .into_iter()
        .max_by(|a, b| length(a).total_cmp(&length(b)))
        .unwrap_or_default()
}

/// The base is the point closest to the base of the veins, the tip the one farthest along the
/// leaf. Alpha goes by the length of the margin from one to the other, on both sides.
fn alphas(margin: &[Vec2]) -> Vec<f32> {
    let n = margin.len();
    let mut alphas = vec![0.0; n];
    if n < 3 {
        return alphas;
    }
    let base = (0..n)
        .min_by(|&a, &b| margin[a].length().total_cmp(&margin[b].length()))
        .unwrap();
    let tip = (0..n).max_by(|&a, &b| margin[a].y.total_cmp(&margin[b].y)).unwrap();
    for step in [1, n - 1] {
        let mut walked = vec![(base, 0.0)];
        let (mut i, mut length) = (base, 0.0);
        while i != tip {
            let next = (i + step) % n;
            length += (margin[next] - margin[i]).length();
            i = next;
            walked.push((i, length));
        }
        for (i, l) in walked {
            alphas[i] = if length > 0.0 { l / length } else { 0.0 };
        }
    }
    alphas
}

#[cfg(test)]
mod tests {
    use super::alphas;
    use glam::Vec2;

    #[test]
    fn alpha_goes_from_the_base_to_the_tip_on_both_sides() {
        let square = [
            Vec2::new(0.0, 0.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(0.0, 2.0),
            Vec2::new(-1.0, 1.0),
        ];
        assert_eq!(alphas(&square), vec![0.0, 0.5, 1.0, 0.5]);
    }
}
