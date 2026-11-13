use glam::{Quat, Vec3, Vec4, Vec4Swizzles};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{
    concat_arg_vecs, encode_points,
    geometry::{
        colors_at_lengths, create_path, follow_parents, length_shares, orthogonal_to,
        rotate_vector_by_angle, set_path_link, with_path_colors,
        wrap_path, wrap_path_mut, DeepestPaths,
    },
    split_args, Ctx, Point,
};

nodarium_definition_file!("src/definition.json");

/// One kink, read from the inputs before the path is bent.
struct Kink {
    /// The number of the kink on its path, the first is 0.
    index: i32,
    /// Where on the path the kink sits, along its length.
    alpha: f32,
    /// How far everything above the kink turns, in radians.
    angle: f32,
    /// How far the kink is turned around the path, in radians.
    turn: f32,
}

/// The length from the base to every point.
fn distances(points: &[Vec4]) -> Vec<f32> {
    let mut distances = Vec::with_capacity(points.len());
    let mut sum = 0.0;
    for (i, point) in points.iter().enumerate() {
        if i > 0 {
            sum += (point.xyz() - points[i - 1].xyz()).length();
        }
        distances.push(sum);
    }
    distances
}

/// The index of a point at `alpha` along the length of the path. A point is put in if there is
/// none there yet, the path keeps its shape and its length.
fn point_at(points: &mut Vec<Vec4>, alpha: f32) -> usize {
    let distances = distances(points);
    let total = *distances.last().unwrap();
    let target = alpha * total;
    let epsilon = total * 1e-4;

    for i in 0..points.len() - 1 {
        if target > distances[i + 1] + epsilon {
            continue;
        }
        if target - distances[i] <= epsilon {
            return i;
        }
        if distances[i + 1] - target <= epsilon {
            return i + 1;
        }
        let t = (target - distances[i]) / (distances[i + 1] - distances[i]);
        points.insert(i + 1, points[i].lerp(points[i + 1], t));
        return i + 1;
    }
    points.len() - 1
}

/// The direction the path comes in to the point at `index`.
fn tangent_at(points: &[Vec4], index: usize) -> Vec3 {
    let (a, b) = if index > 0 {
        (points[index - 1], points[index])
    } else {
        (points[0], points[1])
    };
    (b.xyz() - a.xyz()).try_normalize().unwrap_or(Vec3::Y)
}

/// Bends `points` at every kink and returns where the kinks ended up.
///
/// At a kink everything above it turns by the angle, away from the direction of the kink, so a
/// branch on that direction grows out of the outer side of the bend. The side the kinks turn
/// from is carried along with every bend, so kinks 180 degrees apart stay in one plane, however
/// far the path has bent before them.
fn bend(points: &mut Vec<Vec4>, kinks: &mut [Kink], path: (i32, i32)) -> Vec<Point> {
    let (parent_id, depth) = path;
    kinks.sort_by(|a, b| a.alpha.total_cmp(&b.alpha));
    // the points have to be in before bending, so every kink finds its spot on the straight path
    let indexes: Vec<usize> = kinks.iter().map(|k| point_at(points, k.alpha)).collect();

    let mut side: Option<Vec3> = None;
    let mut result = Vec::with_capacity(kinks.len());

    for (kink, &index) in kinks.iter().zip(&indexes) {
        let pivot = points[index].xyz();
        let tangent = tangent_at(points, index);
        // the path may curve between two kinks, the side stays orthogonal to it
        let reference = side
            .and_then(|s| (s - tangent * s.dot(tangent)).try_normalize())
            .unwrap_or_else(|| orthogonal_to(tangent));
        let direction = rotate_vector_by_angle(reference, tangent, kink.turn);

        // turning around this axis moves the tangent away from the direction
        let axis = direction.cross(tangent).normalize();
        let rotation = Quat::from_axis_angle(axis, kink.angle);
        for point in points[index + 1..].iter_mut() {
            let moved = pivot + rotation * (point.xyz() - pivot);
            *point = moved.extend(point.w);
        }

        // the point halfway between the tangent below and above the kink
        let half = Quat::from_axis_angle(axis, kink.angle / 2.0);
        result.push(Point {
            position: pivot,
            direction: half * direction,
            tangent: half * tangent,
            scale: points[index].w,
            parent_id,
            alpha: kink.alpha,
            depth,
            index: kink.index,
        });

        side = Some(rotation * reference);
    }

    result.sort_by_key(|p| p.index);
    result
}

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> (Vec<i32>, Vec<i32>) {
    let mut ctx = Ctx::default();
    let original: Vec<Vec<i32>> = split_args(args[0]).iter().map(|p| p.to_vec()).collect();
    let depth = ctx.int(args[6]);

    let selected = DeepestPaths::new(&original, depth);
    let path_count = selected.count(&original);

    let mut current = original.clone();
    let mut kink_points = Vec::new();
    let mut path_index = 0;

    for item in current.iter_mut() {
        if !selected.contains(item) {
            continue;
        }
        let path = wrap_path(item);
        if path.length < 2 || path.get_length() <= 0.0 {
            continue;
        }

        // alpha is where this path sits among all paths, from 0 at the first to 1 at the last
        ctx.set_path(item, path_index, path_count);
        path_index += 1;

        let amount = ctx.int(args[1]).max(0);
        let lowest = ctx.float(args[2]);
        let highest = ctx.float(args[3]);

        // each kink reads its angle and the rotation step itself, so a random node gives every
        // kink its own bend and turn
        let mut turn = 0.0;
        let mut kinks: Vec<Kink> = (0..amount)
            .map(|i| {
                let a = i as f32 / (amount - 1).max(1) as f32;
                let alpha = (lowest + a * (highest - lowest)).clamp(0.0, 1.0);
                ctx.alpha = alpha;
                let kink = Kink {
                    index: i,
                    alpha,
                    angle: ctx.float(args[4]).to_radians(),
                    turn,
                };
                turn += ctx.float(args[5]).to_radians();
                kink
            })
            .collect();

        let mut points: Vec<Vec4> = path.points.chunks_exact(4).map(Vec4::from_slice).collect();
        // the colors stay where they were along the path, it keeps its length
        let colored = (!path.colors.is_empty()).then(|| (path.points.to_vec(), path.colors.to_vec()));
        let link = path.link;
        let path_depth = path.depth;
        kink_points.extend(bend(&mut points, &mut kinks, (link.id, path_depth)));

        // the path may have got points where the kinks are, it keeps its header
        let mut bent = create_path(points.len(), path_depth);
        set_path_link(&mut bent, link);
        let bent_path = wrap_path_mut(&mut bent);
        for (i, point) in points.iter().enumerate() {
            bent_path.points[i * 4..i * 4 + 4].copy_from_slice(&point.to_array());
        }
        if let Some((old_points, old_colors)) = colored {
            let shares = length_shares(wrap_path(&bent).points);
            bent = with_path_colors(&bent, &colors_at_lengths(&old_points, &old_colors, shares));
        }
        *item = bent;
    }

    // what grows on a bent path moves with the spot it is attached to
    follow_parents(&original, &mut current);

    (concat_arg_vecs(current), encode_points(&kink_points))
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use glam::Vec3;
    use nodarium_utils::{
        concat_arg_vecs, decode_points, encode_float,
        geometry::{
            create_multiple_paths, create_path, path_link, set_path_link, with_path_colors,
            wrap_multiple_paths, wrap_path, wrap_path_mut, PathLink,
        },
        split_args, Point,
    };

    /// A straight stem from (0, 0, 0) up to (0, 4, 0) with 5 points.
    fn stem() -> Vec<i32> {
        let mut stems = create_multiple_paths(1, 5, 1, 3);
        for stem in wrap_multiple_paths(&mut stems).iter_mut() {
            for i in 0..5 {
                stem.points[i * 4..i * 4 + 4].copy_from_slice(&[0.0, i as f32, 0.0, 0.1]);
            }
        }
        stems
    }

    fn run(
        plant: &[i32],
        amount: i32,
        range: (f32, f32),
        angle: f32,
        rotation: f32,
    ) -> (Vec<i32>, Vec<i32>) {
        __nodarium_execute_inner(&[
            plant,
            &[amount],
            &[encode_float(range.0)],
            &[encode_float(range.1)],
            &[encode_float(angle)],
            &[encode_float(rotation)],
            &[1],
        ])
    }

    fn points_of(path: &[i32]) -> Vec<Vec3> {
        wrap_path(path)
            .points
            .chunks_exact(4)
            .map(|p| Vec3::new(p[0], p[1], p[2]))
            .collect()
    }

    fn tip(path: &[i32]) -> Vec3 {
        *points_of(path).last().unwrap()
    }

    #[test]
    fn everything_above_a_kink_turns_and_the_path_keeps_its_length() {
        let (plant, points) = run(&stem(), 1, (0.5, 0.5), 90.0, 0.0);
        let path = split_args(&plant)[0].to_vec();

        // the kink sits on a point, so no point is added
        assert_eq!(wrap_path(&path).length, 5);
        assert!((wrap_path(&path).get_length() - 4.0).abs() < 1e-4);
        // the lower half stays, the upper half lies flat
        assert_eq!(points_of(&path)[2], Vec3::new(0.0, 2.0, 0.0));
        let top = tip(&path);
        assert!((top.y - 2.0).abs() < 1e-4, "{:?}", top);
        assert!(((top - Vec3::new(0.0, 2.0, 0.0)).length() - 2.0).abs() < 1e-4);

        let points = decode_points(&points);
        assert_eq!(points.len(), 1);
        assert!((points[0].position - Vec3::new(0.0, 2.0, 0.0)).length() < 1e-5);
    }

    #[test]
    fn the_kink_points_to_the_outer_side_of_the_bend() {
        let (plant, points) = run(&stem(), 1, (0.5, 0.5), 30.0, 0.0);
        let path = split_args(&plant)[0].to_vec();
        let point = decode_points(&points)[0];

        let bent_toward = tip(&path) - point.position;
        assert!(point.direction.dot(bent_toward) < 0.0);
        assert!(point.direction.is_normalized());
        assert!(point.direction.dot(point.tangent).abs() < 1e-5);
        // the tangent is halfway between up and the new direction of the path
        let up_angle = point.tangent.angle_between(Vec3::Y).to_degrees();
        assert!((up_angle - 15.0).abs() < 1e-3, "{}", up_angle);
    }

    #[test]
    fn a_kink_between_two_points_adds_one() {
        let (plant, points) = run(&stem(), 1, (0.3, 0.3), 20.0, 0.0);
        let path = split_args(&plant)[0].to_vec();

        assert_eq!(wrap_path(&path).length, 6);
        assert!((wrap_path(&path).get_length() - 4.0).abs() < 1e-4);
        let point = decode_points(&points)[0];
        assert!((point.position - Vec3::new(0.0, 1.2, 0.0)).length() < 1e-5);
        assert!((point.alpha - 0.3).abs() < 1e-6);
    }

    #[test]
    fn kinks_half_a_turn_apart_zig_zag_in_one_plane() {
        let (plant, points) = run(&stem(), 4, (0.2, 0.8), 20.0, 180.0);
        let path = split_args(&plant)[0].to_vec();

        // all points stay in the plane of the first bend
        let normal = Vec3::X;
        for point in points_of(&path) {
            assert!(point.dot(normal).abs() < 1e-4, "{:?}", point);
        }
        // an even number of opposite kinks points the tip straight up again
        let p = points_of(&path);
        let last = (p[p.len() - 1] - p[p.len() - 2]).normalize();
        assert!((last - Vec3::Y).length() < 1e-4, "{:?}", last);

        let points = decode_points(&points);
        assert_eq!(points.len(), 4);
        for pair in points.windows(2) {
            assert!(pair[0].direction.dot(pair[1].direction) < 0.0);
        }
    }

    #[test]
    fn kink_points_know_their_path_and_number() {
        let plant = stem();
        let stem_link = path_link(split_args(&plant)[0]);
        let (_, points) = run(&plant, 3, (0.2, 0.8), 10.0, 137.5);
        let points: Vec<Point> = decode_points(&points);

        assert_eq!(
            points.iter().map(|p| p.index).collect::<Vec<_>>(),
            [0, 1, 2]
        );
        for point in &points {
            assert_eq!(point.parent_id, stem_link.id);
            assert_eq!(point.depth, 1);
            assert!((point.scale - 0.1).abs() < 1e-6);
        }
    }

    #[test]
    fn the_path_keeps_its_header() {
        let plant = stem();
        let before = path_link(split_args(&plant)[0]);
        let (bent, _) = run(&plant, 2, (0.3, 0.7), 10.0, 180.0);

        assert_eq!(path_link(split_args(&bent)[0]), before);
        assert_eq!(wrap_path(split_args(&bent)[0]).depth, 1);
    }

    #[test]
    fn what_grows_on_a_bent_path_moves_with_it() {
        let plant = stem();
        let stem_id = path_link(split_args(&plant)[0]).id;
        // a branch at the tip of the stem
        let mut branch = create_path(2, 2);
        set_path_link(&mut branch, PathLink::child(9, stem_id, 1.0, 0));
        wrap_path_mut(&mut branch)
            .points
            .copy_from_slice(&[0.0, 4.0, 0.0, 0.05, 1.0, 4.0, 0.0, 0.0]);
        let mut items: Vec<Vec<i32>> = split_args(&plant).iter().map(|p| p.to_vec()).collect();
        items.push(branch);

        // depth 1 only bends the deepest layer, the branch, so this uses depth 2
        let result = __nodarium_execute_inner(&[
            &concat_arg_vecs(items),
            &[1],
            &[encode_float(0.5)],
            &[encode_float(0.5)],
            &[encode_float(90.0)],
            &[encode_float(0.0)],
            &[2],
        ]);
        let items = split_args(&result.0);
        let stem_tip = tip(items[0]);
        let branch_base = points_of(items[1])[0];

        assert!(
            (branch_base - stem_tip).length() < 1e-4,
            "{:?} {:?}",
            branch_base,
            stem_tip
        );
    }

    #[test]
    fn no_kinks_leave_the_plant_as_it_is() {
        let plant = stem();
        let (bent, points) = run(&plant, 0, (0.2, 0.8), 20.0, 180.0);

        assert_eq!(bent, plant);
        assert!(decode_points(&points).is_empty());
    }

    #[test]
    fn colors_stay_where_they_were_along_the_path() {
        let plain = stem();
        let colors = [0xff00_0000, 0xff00_0040, 0xff00_0080, 0xff00_00c0, 0xff00_00ff];
        let colored = concat_arg_vecs(vec![with_path_colors(split_args(&plain)[0], &colors)]);
        // one kink halfway between two points adds a point there
        let (bent, _) = run(&colored, 1, (0.375, 0.375), 30.0, 180.0);
        let (bent_plain, _) = run(&plain, 1, (0.375, 0.375), 30.0, 180.0);
        let (path, path_plain) = (wrap_path(split_args(&bent)[0]), wrap_path(split_args(&bent_plain)[0]));
        assert_eq!(path.points, path_plain.points);
        assert_eq!(path.colors.len(), path.length);
        assert_eq!(path.length, 6);
        assert_eq!(path.colors[0], colors[0]);
        assert_eq!(path.colors[5], colors[4]);
        assert!(path.colors[2] > colors[1] && path.colors[2] < colors[2], "{:x?}", path.colors);
    }
}
