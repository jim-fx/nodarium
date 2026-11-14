use glam::Vec2;

use crate::field::{Cuts, Field};
use crate::mesh::Density;
use crate::polygon::{area2, distance, nearest, Bands};
use crate::veins::{closest, Vein};

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

/// The line the mesh fills: the margin of the cut blade, counter-clockwise, with the thin ends of
/// the veins the grid misses put back, simplified along its curvature.
pub struct Outline {
    pub points: Vec<Vec2>,
    /// Where the end of every vein meets the margin, for the veins whose end was put back.
    pub tips: Vec<Option<Vec2>>,
}

impl Outline {
    pub fn of(field: &Field, veins: &[Vein], cuts: &Cuts, density: &Density) -> Outline {
        let mut points = longest(field.contour_loops());
        if area2(&points) < 0.0 {
            points.reverse();
        }
        let mut fixed = vec![false; points.len()];
        // made again only when a tip changes the margin
        let mut bands = None;
        let tips = veins
            .iter()
            .map(|vein| {
                let cut = cuts.at(vein.end());
                let tip = put_back_tip(&mut points, &mut fixed, &mut bands, vein, cut, field.cell);
                if tip.is_some() {
                    bands = None;
                }
                tip
            })
            .collect();
        let points = simplify(&points, &fixed, density.tolerance, density.margin);
        Outline { points, tips }
    }
}

/// A vein that tapers to nothing ends in a spike thinner than a cell, which the grid loses. The
/// spike is rebuilt from the reach of the vein, less the cut there, from its tip back to where the
/// margin of the grid takes it in, and put into the margin in place of the stump.
fn put_back_tip(
    points: &mut Vec<Vec2>,
    fixed: &mut Vec<bool>,
    bands: &mut Option<Bands>,
    vein: &Vein,
    cut: f32,
    cell: f32,
) -> Option<Vec2> {
    let step = 0.25 * cell;
    let length = vein.length();
    let count = ((length / step) as usize).min(200);
    let sample = |i: usize| {
        let (p, direction, reach) = vein.at(length - i as f32 * step);
        (p, direction, reach - cut)
    };
    let first = (0..=count).find(|&i| sample(i).2 > 0.0)?;
    let (p, direction, width) = sample(first);
    let tip = if first == 0 {
        p + direction * width
    } else {
        let (q, _, w) = sample(first - 1);
        q.lerp(p, w / (w - width))
    };
    if points.len() < 3 {
        return None;
    }
    let bands = bands.get_or_insert_with(|| Bands::new(points));
    let contains = |p: Vec2| bands.contains(points, p);
    if contains(tip) || distance(points, tip) < 0.25 * cell {
        return None;
    }

    let (mut left, mut right) = (Vec::new(), Vec::new());
    let base = (first..=count).find(|&i| {
        let (p, direction, width) = sample(i);
        let side = direction.perp() * width.max(0.0);
        left.push(p + side);
        right.push(p - side);
        contains(p) && contains(p + side) && contains(p - side)
    });
    base?;
    let mut spike: Vec<Vec2> = right.into_iter().rev().collect();
    spike.push(tip);
    spike.extend(left);
    splice(points, fixed, &spike).then_some(tip)
}

/// Puts `spike`, which leaves the margin and comes back to it, in place of the piece of the margin
/// between the points closest to its ends. Refuses when that would cross the margin.
fn splice(points: &mut Vec<Vec2>, fixed: &mut Vec<bool>, spike: &[Vec2]) -> bool {
    let n = points.len();
    let from = nearest(points, spike[0]);
    let to = nearest(points, spike[spike.len() - 1]);
    let stump = (to + n - from) % n;
    if stump > n / 4 || (0..=stump).any(|k| fixed[(from + k) % n]) {
        return false;
    }
    let kept: Vec<usize> = (1..n - stump).map(|k| (to + k) % n).collect();
    let mut chain = vec![points[kept[kept.len() - 1]]];
    chain.extend_from_slice(spike);
    chain.push(points[kept[0]]);
    let side = |p: Vec2, q: Vec2, r: Vec2| (q - p).perp_dot(r - p);
    let mut sides = vec![0.0; kept.len()];
    for w in chain.windows(2) {
        for (k, &i) in kept.iter().enumerate() {
            sides[k] = side(w[0], w[1], points[i]);
        }
        // whether the edges cross in their insides, with the side of every point of the margin
        // to the chain worked out once
        for k in 0..kept.len() - 1 {
            if sides[k] * sides[k + 1] < 0.0 {
                let (a, b) = (points[kept[k]], points[kept[k + 1]]);
                if side(a, b, w[0]) * side(a, b, w[1]) < 0.0 {
                    return false;
                }
            }
        }
    }
    let tip = spike.len() / 2;
    let mut new_points: Vec<Vec2> = kept.iter().map(|&i| points[i]).collect();
    let mut new_fixed: Vec<bool> = kept.iter().map(|&i| fixed[i]).collect();
    new_points.extend_from_slice(spike);
    new_fixed.extend((0..spike.len()).map(|i| i == tip));
    *points = new_points;
    *fixed = new_fixed;
    true
}

/// Drops the points of a closed line that lie within `tolerance` of the line through the others,
/// keeping the `fixed` ones and no piece longer than `longest`. Sharp teeth keep their points.
fn simplify(points: &[Vec2], fixed: &[bool], tolerance: f32, longest: f32) -> Vec<Vec2> {
    let n = points.len();
    if n < 4 {
        return points.to_vec();
    }
    let mut keep = fixed.to_vec();
    if !keep.iter().any(|&k| k) {
        keep[0] = true;
    }
    let anchors: Vec<usize> = (0..n).filter(|&i| keep[i]).collect();
    if anchors.len() == 1 {
        keep[(0..n).max_by(|&a, &b| {
            let o = points[anchors[0]];
            points[a].distance_squared(o).total_cmp(&points[b].distance_squared(o))
        }).unwrap()] = true;
    }
    let anchors: Vec<usize> = (0..n).filter(|&i| keep[i]).collect();
    for k in 0..anchors.len() {
        split(points, &mut keep, anchors[k], anchors[(k + 1) % anchors.len()], tolerance, longest);
    }
    (0..n).filter(|&i| keep[i]).map(|i| points[i]).collect()
}

fn split(points: &[Vec2], keep: &mut [bool], a: usize, b: usize, tolerance: f32, longest: f32) {
    let n = points.len();
    let count = (b + n - a) % n;
    if count < 2 {
        return;
    }
    let (pa, pb) = (points[a], points[b]);
    let (mut worst, mut at, mut length) = (0.0, a, 0.0);
    let mut lengths = Vec::with_capacity(count);
    for k in 1..=count {
        let (i, previous) = ((a + k) % n, (a + k - 1) % n);
        length += (points[i] - points[previous]).length();
        lengths.push(length);
        if k < count {
            let d = closest(pa, pb, points[i]).1;
            if d > worst {
                (worst, at) = (d, i);
            }
        }
    }
    if worst <= tolerance {
        if length <= longest {
            return;
        }
        let k = (lengths.iter().position(|&l| l >= 0.5 * length).unwrap() + 1).clamp(1, count - 1);
        at = (a + k) % n;
    }
    keep[at] = true;
    split(points, keep, a, at, tolerance, longest);
    split(points, keep, at, b, tolerance, longest);
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
