use glam::{Vec2, Vec3};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{
    concat_arg_vecs,
    geometry::{follow_parents, interpolate_along_path, is_path, path_link, translate_path, wrap_path},
    split_args, Ctx,
};

nodarium_definition_file!("src/definition.json");

/// The angle between the directions two stems on the same spot leave in
const GOLDEN_ANGLE: f32 = 2.399_963;

/// A stem in the plane seen from above
struct Spot {
    index: usize,
    start: Vec2,
    at: Vec2,
    radius: f32,
}

/// Pushes every pair of overlapping circles apart, the larger circle moves less
fn relax(spots: &mut [Spot], iterations: usize) {
    for _ in 0..iterations {
        let mut moved = false;
        for i in 0..spots.len() {
            for j in i + 1..spots.len() {
                let (a, b) = (&spots[i], &spots[j]);
                let reach = a.radius + b.radius;
                let d = b.at - a.at;
                let distance = d.length();
                if reach <= 0.0 || distance >= reach {
                    continue;
                }
                let direction = if distance > reach * 1e-4 {
                    d / distance
                } else {
                    Vec2::from_angle(GOLDEN_ANGLE * (i + j) as f32)
                };
                let (ra, rb) = (a.radius * a.radius, b.radius * b.radius);
                let push = direction * (reach - distance);
                spots[i].at -= push * rb / (ra + rb);
                spots[j].at += push * ra / (ra + rb);
                moved = true;
            }
        }
        if !moved {
            break;
        }
    }
}

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let mut ctx = Ctx::default();
    let original: Vec<Vec<i32>> = split_args(args[0]).iter().map(|p| p.to_vec()).collect();
    let at = ctx.float(args[2]).clamp(0.0, 1.0);
    let iterations = ctx.int(args[3]).clamp(1, 256) as usize;

    let stems: Vec<usize> = (0..original.len())
        .filter(|&i| is_path(&original[i]) && !path_link(&original[i]).has_parent() && wrap_path(&original[i]).length >= 2)
        .collect();
    let mut spots: Vec<Spot> = stems
        .iter()
        .enumerate()
        .map(|(rank, &index)| {
            let path = &original[index];
            ctx.set_path(path, rank, stems.len());
            let radius = ctx.float(args[1]).max(0.0);
            let p = interpolate_along_path(wrap_path(path).points, at).0;
            let start = Vec2::new(p.x, p.z);
            Spot { index, start, at: start, radius }
        })
        .collect();
    relax(&mut spots, iterations);

    let mut current = original.clone();
    for spot in &spots {
        let shift = spot.at - spot.start;
        translate_path(&mut current[spot.index], Vec3::new(shift.x, 0.0, shift.y));
    }
    follow_parents(&original, &mut current);
    concat_arg_vecs(current)
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use glam::{Vec2, Vec3};
    use nodarium_utils::{
        concat_arg_vecs, concat_args, encode_float,
        geometry::{create_path, set_path_link, wrap_path, wrap_path_mut, PathLink},
        split_args,
    };

    /// A straight path from `from` to `to` with 5 points
    fn path(link: PathLink, from: Vec3, to: Vec3) -> Vec<i32> {
        let mut data = create_path(5, if link.has_parent() { 2 } else { 1 });
        set_path_link(&mut data, link);
        let p = wrap_path_mut(&mut data);
        for i in 0..5 {
            let q = from.lerp(to, i as f32 / 4.0);
            p.points[i * 4..i * 4 + 4].copy_from_slice(&[q.x, q.y, q.z, 0.01]);
        }
        data
    }

    fn stem(index: i32, x: f32, height: f32) -> Vec<i32> {
        path(PathLink::root(1, index), Vec3::new(x, 0.0, 0.0), Vec3::new(x, height, 0.0))
    }

    fn run(paths: Vec<Vec<i32>>, radius: &[i32], at: f32) -> Vec<Vec<Vec3>> {
        let out = __nodarium_execute_inner(&[&concat_arg_vecs(paths), radius, &[encode_float(at)], &[24]]);
        split_args(&out)
            .iter()
            .map(|p| wrap_path(p).points.chunks_exact(4).map(|q| Vec3::new(q[0], q[1], q[2])).collect())
            .collect()
    }

    fn flat(p: Vec3) -> Vec2 {
        Vec2::new(p.x, p.z)
    }

    #[test]
    fn stems_far_enough_apart_stay() {
        let paths = vec![stem(0, 0.0, 1.0), stem(1, 0.5, 1.0)];
        let out = run(paths, &[encode_float(0.2)], 1.0);
        assert_eq!(out[0][4], Vec3::new(0.0, 1.0, 0.0));
        assert_eq!(out[1][4], Vec3::new(0.5, 1.0, 0.0));
    }

    #[test]
    fn overlapping_stems_move_until_they_touch_and_keep_their_shape() {
        let paths = vec![stem(0, 0.0, 1.0), stem(1, 0.1, 1.0), stem(2, 0.1, 1.0)];
        let out = run(paths, &[encode_float(0.2)], 0.0);
        for i in 0..3 {
            for j in i + 1..3 {
                let d = flat(out[i][0]).distance(flat(out[j][0]));
                assert!(d > 0.4 - 1e-3, "{i} and {j} are {d} apart");
            }
            // moved sideways as a whole
            assert!((out[i][4] - out[i][0] - Vec3::Y).length() < 1e-5);
            assert_eq!(out[i][0].y, 0.0);
        }
    }

    #[test]
    fn the_larger_stem_moves_less() {
        // the radius follows the length of each stem
        let length = concat_args(vec![&[5], &[5]]);
        let radius = concat_args(vec![&[0], &[2], &length, &[encode_float(0.2)]]);
        let out = run(vec![stem(0, 0.0, 2.0), stem(1, 0.1, 0.5)], &radius, 1.0);
        let (big, small) = (out[0][4].x.abs(), (out[1][4].x - 0.1).abs());
        assert!(big < small, "{big} and {small}");
        assert!((out[1][4].x - out[0][4].x - 0.5).abs() < 1e-3);
    }

    #[test]
    fn the_place_is_measured_where_at_says() {
        // two stems that lean together: apart at the base, crossing at the tip
        let a = path(PathLink::root(1, 0), Vec3::new(-0.3, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0));
        let b = path(PathLink::root(1, 1), Vec3::new(0.3, 0.0, 0.0), Vec3::new(0.05, 1.0, 0.0));
        let base = run(vec![a.clone(), b.clone()], &[encode_float(0.2)], 0.0);
        assert_eq!(base[0][0], Vec3::new(-0.3, 0.0, 0.0));
        let tip = run(vec![a, b], &[encode_float(0.2)], 1.0);
        assert!((tip[1][4].x - tip[0][4].x - 0.4).abs() < 1e-3);
    }

    #[test]
    fn what_grows_on_a_stem_moves_along() {
        let s0 = stem(0, 0.0, 1.0);
        let s1 = stem(1, 0.0, 1.0);
        let parent = PathLink::root(1, 1).id;
        let leaf = path(PathLink::child(2, parent, 0.5, 0), Vec3::new(0.0, 0.5, 0.0), Vec3::new(0.2, 0.6, 0.0));
        let out = run(vec![s0, s1, leaf], &[encode_float(0.1)], 1.0);
        let shift = out[1][0] - Vec3::ZERO;
        assert!(shift.length() > 0.05);
        assert!((out[2][0] - (Vec3::new(0.0, 0.5, 0.0) + shift)).length() < 1e-5);
    }
}
