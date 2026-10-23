use glam::Vec3;
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{
    concat_args,
    geometry::{wrap_path, wrap_path_mut},
    int, split_args, Ctx,
};

use std::f32::consts::TAU;

nodarium_definition_file!("src/definition.json");

const MAX_SMOOTHING: f32 = 10.0;

fn smoothstep(value: f32) -> f32 {
    let t = value.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The layer of paths the node works on, the same way `gravity` and `branch`
/// pick their paths. The outermost paths are depth 0, a branch is one deeper.
fn paths_at_depth(plants: &[&[i32]], depth: i32) -> Vec<usize> {
    let max_depth = plants
        .iter()
        .filter(|p| p.len() >= 4 && p[2] == 0)
        .map(|p| p[3])
        .max()
        .unwrap_or(0);
    let min_depth = max_depth - depth + 1;

    plants
        .iter()
        .enumerate()
        .filter(|(_, p)| p.len() >= 4 && p[2] == 0 && p[3] >= min_depth)
        .map(|(index, _)| index)
        .collect()
}

fn point_at(points: &[f32], index: usize) -> Vec3 {
    Vec3::new(
        points[index * 4],
        points[index * 4 + 1],
        points[index * 4 + 2],
    )
}

/// Box around a path, grown by its thickest point, so paths that are far
/// apart can be skipped without looking at their segments.
fn bounds(points: &[f32]) -> (Vec3, Vec3) {
    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);
    let mut thickness = 0.0f32;

    for i in 0..points.len() / 4 {
        let point = point_at(points, i);
        min = min.min(point);
        max = max.max(point);
        thickness = thickness.max(points[i * 4 + 3]);
    }

    (min - Vec3::splat(thickness), max + Vec3::splat(thickness))
}

/// Parameters `(s, t)` in 0..=1 of the closest points between the segments
/// `p1..q1` and `p2..q2`.
fn closest_segment_params(p1: Vec3, q1: Vec3, p2: Vec3, q2: Vec3) -> (f32, f32) {
    const EPSILON: f32 = 1e-8;

    let d1 = q1 - p1;
    let d2 = q2 - p2;
    let r = p1 - p2;
    let a = d1.dot(d1);
    let e = d2.dot(d2);
    let f = d2.dot(r);

    if a <= EPSILON && e <= EPSILON {
        return (0.0, 0.0);
    }
    if a <= EPSILON {
        return (0.0, (f / e).clamp(0.0, 1.0));
    }

    let c = d1.dot(r);
    if e <= EPSILON {
        return ((-c / a).clamp(0.0, 1.0), 0.0);
    }

    let b = d1.dot(d2);
    let denominator = a * e - b * b;
    let mut s = if denominator > EPSILON {
        ((b * f - c * e) / denominator).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let mut t = (b * s + f) / e;

    if t < 0.0 {
        t = 0.0;
        s = (-c / a).clamp(0.0, 1.0);
    } else if t > 1.0 {
        t = 1.0;
        s = ((b - c) / a).clamp(0.0, 1.0);
    }

    (s, t)
}

/// The direction to separate two segments that touch or cross each other. A
/// crossing is resolved perpendicular to both segments, parallel segments on
/// top of each other are spread around their axis by the order of the path.
fn fallback_direction(da: Vec3, db: Vec3, order: usize, path_count: usize) -> Vec3 {
    if let Some(normal) = da.cross(db).try_normalize() {
        return normal;
    }

    let axis = da.try_normalize().unwrap_or(Vec3::Y);
    let u = axis.any_orthonormal_vector();
    let w = axis.cross(u);
    let angle = TAU * order as f32 / path_count.max(1) as f32;
    u * angle.cos() + w * angle.sin()
}

/// One side of a pair of paths: its points, the push collected for every
/// point and how many contacts contributed to that push.
struct Side<'a> {
    points: &'a [f32],
    deltas: &'a mut [Vec3],
    weights: &'a mut [f32],
    order: usize,
}

impl Side<'_> {
    /// Pushes the two points of segment `i` so that the spot at `s` moves by
    /// `push`. The push fades in along the stem, so the base never gets a kink.
    fn push(&mut self, i: usize, s: f32, push: Vec3, alpha: f32) {
        let share = 1.0 / ((1.0 - s) * (1.0 - s) + s * s);
        let push = push * smoothstep(alpha) * share;

        self.deltas[i] += push * (1.0 - s);
        self.deltas[i + 1] += push * s;
        self.weights[i] += 1.0 - s;
        self.weights[i + 1] += s;
    }

    fn thickness(&self, i: usize, s: f32) -> f32 {
        let start = self.points[i * 4 + 3];
        let end = self.points[i * 4 + 7];
        (start + (end - start) * s).max(0.0)
    }
}

/// Finds where two paths come closer than `radius` plus their thickness and
/// pushes them apart. Every segment of one path is tested against every
/// segment of the other in 3D, so crossings are found wherever they happen.
fn repel_pair(a: &mut Side, b: &mut Side, path_count: usize, radius: f32, strength: f32) {
    let length_a = a.points.len() / 4;
    let length_b = b.points.len() / 4;
    if length_a < 2 || length_b < 2 {
        return;
    }

    for i in 0..length_a - 1 {
        let pa = point_at(a.points, i);
        let qa = point_at(a.points, i + 1);

        for j in 0..length_b - 1 {
            let pb = point_at(b.points, j);
            let qb = point_at(b.points, j + 1);

            let (s, t) = closest_segment_params(pa, qa, pb, qb);
            let offset = pa.lerp(qa, s) - pb.lerp(qb, t);
            let distance = offset.length();

            // radius is the free space between the surfaces of the two stems
            let required = radius + a.thickness(i, s) + b.thickness(j, t);
            if distance >= required || required <= 0.0 {
                continue;
            }

            let direction = if distance > 0.0001 {
                offset / distance
            } else {
                fallback_direction(qa - pa, qb - pb, a.order, path_count)
            };

            let push = direction * ((required - distance) * 0.5 * strength);
            a.push(i, s, push, (i as f32 + s) / (length_a - 1) as f32);
            b.push(j, t, -push, (j as f32 + t) / (length_b - 1) as f32);
        }
    }
}

/// Spreads the displacement of a point over its two neighbours, so a stem
/// bends as a whole instead of getting a sharp dent where it touches another.
fn smooth_deltas(deltas: &mut [Vec3], smoothing: f32) {
    if deltas.is_empty() {
        return;
    }

    // the stem is rooted in its parent, the base never moves
    deltas[0] = Vec3::ZERO;

    // every whole unit of smoothing is a full blur pass, the remainder blends
    // in one more pass, so values above 1 spread the bend further along the stem
    let smoothing = smoothing.clamp(0.0, MAX_SMOOTHING);
    let passes = smoothing.ceil() as usize;

    if deltas.len() >= 3 {
        for pass in 0..passes {
            let amount = (smoothing - pass as f32).min(1.0);
            let original = deltas.to_vec();
            for i in 1..deltas.len() {
                let before = original[i - 1];
                let after = if i + 1 < original.len() {
                    original[i + 1]
                } else {
                    original[i]
                };
                let average = before * 0.25 + original[i] * 0.5 + after * 0.25;
                deltas[i] = original[i].lerp(average, amount);
            }
        }
    }
}

/// Bends the path toward the pushed points without changing its segment
/// lengths. Only the directions of the segments change, the path keeps its
/// length and its base.
fn rebuild_path(path: nodarium_utils::geometry::PathDataMut, original: &[f32], deltas: &[Vec3]) {
    let mut cursor = Vec3::new(original[0], original[1], original[2]);

    for i in 0..path.length.saturating_sub(1) {
        let start = i * 4;
        let end = start + 4;

        let segment = Vec3::new(
            original[end] - original[start],
            original[end + 1] - original[start + 1],
            original[end + 2] - original[start + 2],
        );
        let length = segment.length();
        let desired = segment + deltas[i + 1] - deltas[i];
        let direction = desired
            .try_normalize()
            .unwrap_or_else(|| segment.try_normalize().unwrap_or(Vec3::Y));

        cursor += direction * length;
        path.points[end] = cursor.x;
        path.points[end + 1] = cursor.y;
        path.points[end + 2] = cursor.z;
    }
}

/// Iteratively pushes overlapping paths apart. Every iteration reads the
/// result of the one before, so a group of stems relaxes into an even spread.
fn repel(
    output: &mut [Vec<i32>],
    paths: &[usize],
    radius: f32,
    strength: f32,
    smoothing: f32,
    iterations: usize,
) {
    let count = paths.len();
    if count < 2 || strength <= 0.0 {
        return;
    }

    for _ in 0..iterations {
        let points: Vec<Vec<f32>> = paths
            .iter()
            .map(|index| wrap_path(&output[*index]).points.to_vec())
            .collect();
        let boxes: Vec<(Vec3, Vec3)> = points.iter().map(|p| bounds(p)).collect();
        let mut deltas: Vec<Vec<Vec3>> = points
            .iter()
            .map(|p| vec![Vec3::ZERO; p.len() / 4])
            .collect();
        let mut weights: Vec<Vec<f32>> = points.iter().map(|p| vec![0.0; p.len() / 4]).collect();

        for a in 0..count {
            for b in (a + 1)..count {
                let margin = Vec3::splat(radius);
                if (boxes[a].0 - margin).cmpgt(boxes[b].1).any()
                    || (boxes[b].0 - margin).cmpgt(boxes[a].1).any()
                {
                    continue;
                }

                let (deltas_a, deltas_b) = deltas.split_at_mut(b);
                let (weights_a, weights_b) = weights.split_at_mut(b);
                let mut side_a = Side {
                    points: &points[a],
                    deltas: &mut deltas_a[a],
                    weights: &mut weights_a[a],
                    order: a,
                };
                let mut side_b = Side {
                    points: &points[b],
                    deltas: &mut deltas_b[0],
                    weights: &mut weights_b[0],
                    order: b,
                };
                repel_pair(&mut side_a, &mut side_b, count, radius, strength);
            }
        }

        for (order, index) in paths.iter().enumerate() {
            // a point touched by many contacts takes their average, not their sum
            for (delta, weight) in deltas[order].iter_mut().zip(&weights[order]) {
                *delta /= weight.max(1.0);
            }
            smooth_deltas(&mut deltas[order], smoothing);

            let path = wrap_path_mut(&mut output[*index]);
            rebuild_path(path, &points[order], &deltas[order]);
        }
    }
}

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let mut ctx = Ctx::default();

    let plants = split_args(args[0]);
    let radius = ctx.float(args[1]).max(0.0);
    let strength = ctx.float(args[2]).clamp(0.0, 1.0);
    let smoothing = ctx.float(args[3]).clamp(0.0, MAX_SMOOTHING);
    let iterations = int(args[4]).clamp(1, 64) as usize;
    let depth = int(args[5]).max(1);

    let paths = paths_at_depth(&plants, depth);
    let mut output: Vec<Vec<i32>> = plants.iter().map(|p| p.to_vec()).collect();

    repel(&mut output, &paths, radius, strength, smoothing, iterations);

    concat_args(output.iter().map(|p| p.as_slice()).collect())
}

#[cfg(test)]
mod tests {
    use nodarium_utils::{
        encode_float,
        geometry::{create_multiple_paths, wrap_multiple_paths},
    };

    fn straight_stems(amount: usize) -> Vec<i32> {
        let mut data = create_multiple_paths(amount, 5, 0, 0);
        for stem in wrap_multiple_paths(&mut data).iter_mut() {
            for i in 0..stem.points.len() / 4 {
                stem.points[i * 4] = 0.0;
                stem.points[i * 4 + 1] = i as f32;
                stem.points[i * 4 + 2] = 0.0;
                stem.points[i * 4 + 3] = 0.1;
            }
        }
        data
    }

    fn run(data: &[i32]) -> Vec<i32> {
        super::__nodarium_execute_inner(&[
            data,
            &[encode_float(0.25)],
            &[encode_float(1.0)],
            &[encode_float(0.5)],
            &[8],
            &[1],
        ])
    }

    fn horizontal_distance(a: &[f32], b: &[f32], index: usize) -> f32 {
        let i = index * 4;
        let dx = a[i] - b[i];
        let dz = a[i + 2] - b[i + 2];
        (dx * dx + dz * dz).sqrt()
    }

    #[test]
    fn repel_separates_stems_that_share_a_path() {
        let data = straight_stems(2);
        let result = run(&data);
        let mut copy = result.clone();
        let paths = wrap_multiple_paths(&mut copy);

        let last = paths[0].length - 1;
        assert!(horizontal_distance(paths[0].points, paths[1].points, last) > 0.1);
    }

    #[test]
    fn stems_crossing_at_different_heights_are_separated() {
        // a vertical stem and a horizontal one that crosses it at a different
        // alpha than the point it is compared to
        let mut data = create_multiple_paths(2, 5, 0, 0);
        {
            let mut stems = wrap_multiple_paths(&mut data);
            for i in 0..5 {
                stems[0].points[i * 4..i * 4 + 4].copy_from_slice(&[0.0, i as f32, 0.0, 0.1]);
                stems[1].points[i * 4..i * 4 + 4].copy_from_slice(&[2.0 - i as f32, 1.0, 0.0, 0.1]);
            }
        }

        let result = run(&data);
        let mut copy = result.clone();
        let paths = wrap_multiple_paths(&mut copy);

        let mut closest = f32::MAX;
        for i in 0..4 {
            for j in 0..4 {
                let (s, t) = super::closest_segment_params(
                    super::point_at(paths[0].points, i),
                    super::point_at(paths[0].points, i + 1),
                    super::point_at(paths[1].points, j),
                    super::point_at(paths[1].points, j + 1),
                );
                let a = super::point_at(paths[0].points, i)
                    .lerp(super::point_at(paths[0].points, i + 1), s);
                let b = super::point_at(paths[1].points, j)
                    .lerp(super::point_at(paths[1].points, j + 1), t);
                closest = closest.min(a.distance(b));
            }
        }
        assert!(closest > 0.2, "stems are still {} apart", closest);
    }

    fn segment_lengths(data: &[i32]) -> Vec<Vec<f32>> {
        let mut copy = data.to_vec();
        let paths = wrap_multiple_paths(&mut copy);

        paths
            .iter()
            .map(|path| {
                (0..path.length.saturating_sub(1))
                    .map(|i| {
                        let start = i * 4;
                        let end = start + 4;
                        let dx = path.points[end] - path.points[start];
                        let dy = path.points[end + 1] - path.points[start + 1];
                        let dz = path.points[end + 2] - path.points[start + 2];
                        (dx * dx + dy * dy + dz * dz).sqrt()
                    })
                    .collect()
            })
            .collect()
    }

    #[test]
    fn the_path_length_does_not_change() {
        let data = straight_stems(3);
        let before = segment_lengths(&data);
        let result = run(&data);
        let after = segment_lengths(&result);

        for (path_before, path_after) in before.iter().zip(after.iter()) {
            assert_eq!(path_before.len(), path_after.len());
            for (a, b) in path_before.iter().zip(path_after.iter()) {
                assert!((a - b).abs() < 0.0001, "segment {} became {}", a, b);
            }
        }
    }

    #[test]
    fn the_base_stays_where_it_is() {
        let data = straight_stems(2);
        let result = run(&data);
        let mut copy = result.clone();
        let paths = wrap_multiple_paths(&mut copy);

        for path in paths {
            assert_eq!(path.points[0], 0.0);
            assert_eq!(path.points[2], 0.0);
        }
    }

    #[test]
    fn a_zero_radius_leaves_stems_without_thickness_alone() {
        let mut data = create_multiple_paths(2, 5, 0, 0);
        for stem in wrap_multiple_paths(&mut data).iter_mut() {
            for point in stem.points.iter_mut() {
                *point = 0.0;
            }
        }

        let result = super::__nodarium_execute_inner(&[
            data.as_slice(),
            &[encode_float(0.0)],
            &[encode_float(1.0)],
            &[encode_float(0.5)],
            &[8],
            &[1],
        ]);
        let mut copy = result.clone();
        let paths = wrap_multiple_paths(&mut copy);

        for path in paths {
            for point in path.points {
                assert_eq!(*point, 0.0);
            }
        }
    }

    #[test]
    fn only_paths_on_the_selected_depth_move() {
        let point_amount = 5;
        // header, points and the four brackets
        let path_size = point_amount * 4 + 7 + 4;
        let mut data = create_multiple_paths(4, point_amount, 0, 0);
        for path in 0..4 {
            // the last two paths sit one layer deeper
            data[2 + path * path_size + 3] = if path < 2 { 0 } else { 1 };
        }
        for stem in wrap_multiple_paths(&mut data).iter_mut() {
            for i in 0..stem.points.len() / 4 {
                stem.points[i * 4 + 1] = i as f32;
                stem.points[i * 4 + 3] = 0.1;
            }
        }

        // depth 1 works on the deepest layer, here the inner paths
        let result = super::__nodarium_execute_inner(&[
            data.as_slice(),
            &[encode_float(0.5)],
            &[encode_float(1.0)],
            &[encode_float(0.5)],
            &[8],
            &[1],
        ]);
        let mut copy = result.clone();
        let paths = wrap_multiple_paths(&mut copy);
        let last = paths[0].length - 1;

        assert_eq!(
            horizontal_distance(paths[0].points, paths[1].points, last),
            0.0
        );
        assert!(horizontal_distance(paths[2].points, paths[3].points, last) > 0.1);
    }
}
