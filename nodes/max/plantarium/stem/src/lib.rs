use glam::Vec3;
use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;
use nodarium_utils::{
    decode_points, int,
    geometry::{create_multiple_paths, wrap_multiple_paths},
    log, Ctx, PathInfo,
};

nodarium_definition_file!("src/definition.json");

/// The turn of a stem in its fan, from -fan/2 for the first to fan/2 for the last
fn fan_angle(fan: f32, rank: usize, count: usize) -> f32 {
    if count < 2 {
        return 0.0;
    }
    (rank as f32 / (count - 1) as f32 - 0.5) * fan.to_radians()
}

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let mut ctx = Ctx::default();

    let points = decode_points(args[1]);
    let amount = ctx.int(args[2]).max(0) as usize;
    let path_resolution = int(args[6]) as usize;

    log!("stem args: {:?}", args);

    // the graph node that makes the stems, it is the last input
    let creator = int(args[8]);

    let mut stem_data = create_multiple_paths(amount, path_resolution, 1, creator);

    let mut stems = wrap_multiple_paths(&mut stem_data);

    let stem_count = stems.iter_mut().count();
    // the stems of a point make one fan, they take the points in turn
    let fans = points.len().max(1);

    for (index, stem) in stems.iter_mut().enumerate() {
        // alpha is where this stem sits among all stems, from 0 at the first to 1 at the last
        ctx.set_path_info(PathInfo {
            index,
            rank: index,
            count: stem_count,
            depth: 1,
            ..Default::default()
        });
        // the random values of a stem start from its id, so they stay when the amount changes
        ctx.seed_path(stem.link.id);
        // read with points too, so a stem draws the same values on a point as without
        let origin = ctx.vec3(args[0]);
        let length = ctx.float(args[4]);
        // path_info length reads the length of this stem, so its thickness can follow it
        ctx.path.length = length;
        let amount_points = stem.points.len() / 4;
        let alphas = (0..amount_points).map(|i| i as f32 / (path_resolution - 1) as f32);
        // alpha goes along the stem, so the thickness can follow a curve
        let thickness = ctx.float_along(args[5], alphas.clone());
        let taper = ctx.float(args[7]);
        let fan = ctx.float(args[3]);

        let fan_size = (stem_count - index % fans).div_ceil(fans);
        let turn = fan_angle(fan, index / fans, fan_size);
        let (base, direction, side) = match points.get(index % fans) {
            Some(point) => (point.position, point.direction, point.tangent),
            None => (Vec3::from_array(origin), Vec3::Y, Vec3::X),
        };
        // fan 0 keeps the stems as they were bit for bit, -0 + 0 would flip signs of zeros
        let direction = if turn == 0.0 { direction } else { direction * turn.cos() + side * turn.sin() };

        for (i, a) in alphas.enumerate() {
            let p = base + direction * a * length;
            stem.points[i * 4..i * 4 + 3].copy_from_slice(&p.to_array());
            stem.points[i * 4 + 3] = thickness[i] * (1.0 - a * taper);
        }
    }

    log!("stem_data: {:?}", stem_data);

    stem_data
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use glam::Vec3;
    use nodarium_utils::{
        concat_args, encode_float, encode_points,
        geometry::{path_id, wrap_multiple_paths, NO_PARENT},
        Point,
    };

    fn origin() -> Vec<i32> {
        concat_args(vec![&[encode_float(0.5)], &[encode_float(0.0)], &[encode_float(-0.25)]])
    }

    fn run_fan(points: &[i32], amount: i32, length: f32, fan: f32) -> Vec<i32> {
        let creator = 2;
        __nodarium_execute_inner(&[&origin(), points, &[amount], &[encode_float(fan)], &[encode_float(length)], &[encode_float(0.1)], &[5], &[encode_float(1.0)], &[creator]])
    }

    fn run(points: &[i32], amount: i32, length: f32) -> Vec<i32> {
        run_fan(points, amount, length, 0.0)
    }

    fn point(index: i32, position: Vec3, direction: Vec3) -> Point {
        Point { position, direction, tangent: Vec3::X, scale: 1.0, parent_id: NO_PARENT, alpha: 0.0, depth: 0, index }
    }

    #[test]
    fn the_thickness_can_follow_the_length_of_the_stem() {
        let origin = concat_args(vec![&[encode_float(0.0)], &[encode_float(0.0)], &[encode_float(0.0)]]);
        let length = concat_args(vec![&[1], &[encode_float(1.0)], &[encode_float(3.0)], &[7]]);
        // a tenth of the length of the stem
        let path_length = concat_args(vec![&[5], &[5]]);
        let thickness = concat_args(vec![&[0], &[2], &path_length, &[encode_float(0.1)]]);

        let mut data = __nodarium_execute_inner(&[&origin, &[], &[4], &[encode_float(0.0)], &length, &thickness, &[8], &[encode_float(0.0)], &[2]]);
        let stems = wrap_multiple_paths(&mut data);
        assert_eq!(stems.len(), 4);
        let mut lengths = Vec::new();
        for stem in &stems {
            let top = stem.points[stem.points.len() - 3];
            for point in stem.points.chunks_exact(4) {
                assert!((point[3] - 0.1 * top).abs() < 1e-5, "{} for a stem {} long", point[3], top);
            }
            lengths.push(top);
        }
        assert!(lengths.windows(2).any(|w| w[0] != w[1]), "{:?}", lengths);
    }

    #[test]
    fn without_points_the_stems_grow_up_from_the_origin() {
        // what the host sends for an input nothing is connected to
        for nothing in [&[][..], &[0, 1, 1, 1]] {
            let mut data = run(nothing, 2, 0.4);
            for stem in wrap_multiple_paths(&mut data) {
                let ends = [&stem.points[..3], &stem.points[stem.points.len() - 4..stem.points.len() - 1]];
                assert_eq!(ends, [&[0.5, 0.0, -0.25][..], &[0.5, 0.4, -0.25]]);
            }
        }
    }

    #[test]
    fn on_points_every_stem_starts_on_a_point_and_grows_along_its_direction() {
        let tilted = Vec3::new(0.6, 0.8, 0.0);
        let points = encode_points(&[point(0, Vec3::new(1.0, 2.0, 3.0), Vec3::Y), point(1, Vec3::new(-1.0, 0.5, 0.0), tilted)]);
        let mut data = run(&points, 3, 0.5);
        let stems = wrap_multiple_paths(&mut data);
        assert_eq!(stems.len(), 3);
        // the stems take the points in turn, the third starts on the first point again
        for (stem, (base, direction)) in stems.iter().zip([(Vec3::new(1.0, 2.0, 3.0), Vec3::Y), (Vec3::new(-1.0, 0.5, 0.0), tilted), (Vec3::new(1.0, 2.0, 3.0), Vec3::Y)]) {
            let n = stem.points.len() / 4;
            assert!(Vec3::from_slice(&stem.points[..3]).distance(base) < 1e-6);
            let tip = Vec3::from_slice(&stem.points[(n - 1) * 4..(n - 1) * 4 + 3]);
            assert!(tip.distance(base + direction * 0.5) < 1e-6, "{:?}", tip);
            assert_eq!(stem.link.parent_id, NO_PARENT);
        }
        // the ids are those of the stems without points, so they keep their random values
        let mut plain = run(&[], 3, 0.5);
        for (a, b) in stems.iter().zip(wrap_multiple_paths(&mut plain).iter()) {
            assert_eq!(a.link, b.link);
            assert_eq!(a.link.id, path_id(2, NO_PARENT, a.link.index));
        }
    }

    fn directions(data: &mut [i32]) -> Vec<Vec3> {
        wrap_multiple_paths(data)
            .iter()
            .map(|stem| {
                let n = stem.points.len() / 4;
                let base = Vec3::from_slice(&stem.points[..3]);
                (Vec3::from_slice(&stem.points[(n - 1) * 4..(n - 1) * 4 + 3]) - base).normalize()
            })
            .collect()
    }

    #[test]
    fn a_fan_spreads_the_stems_evenly_in_the_plane_of_x_and_y() {
        let mut data = run_fan(&[], 5, 0.4, 180.0);
        let stems = wrap_multiple_paths(&mut data);
        for stem in &stems {
            assert!(Vec3::from_slice(&stem.points[..3]).distance(Vec3::new(0.5, 0.0, -0.25)) < 1e-6);
            let n = stem.points.len() / 4;
            let tip = Vec3::from_slice(&stem.points[(n - 1) * 4..(n - 1) * 4 + 3]);
            assert!((tip.distance(Vec3::new(0.5, 0.0, -0.25)) - 0.4).abs() < 1e-5);
        }
        let directions = directions(&mut data);
        let expected = [-Vec3::X, Vec3::new(-1.0, 1.0, 0.0).normalize(), Vec3::Y, Vec3::new(1.0, 1.0, 0.0).normalize(), Vec3::X];
        for (direction, expected) in directions.iter().zip(expected) {
            assert!(direction.distance(expected) < 1e-5, "{:?} instead of {:?}", direction, expected);
        }
    }

    #[test]
    fn a_fan_keeps_the_ids_and_random_values_of_the_stems() {
        let length = concat_args(vec![&[1], &[encode_float(1.0)], &[encode_float(3.0)], &[7]]);
        let stems = |fan: f32| {
            let mut data = __nodarium_execute_inner(&[&origin(), &[], &[3], &[encode_float(fan)], &length, &[encode_float(0.1)], &[5], &[encode_float(1.0)], &[2]]);
            wrap_multiple_paths(&mut data)
                .iter()
                .map(|stem| {
                    let n = stem.points.len() / 4;
                    let tip = Vec3::from_slice(&stem.points[(n - 1) * 4..(n - 1) * 4 + 3]);
                    (stem.link, tip.distance(Vec3::from_slice(&stem.points[..3])))
                })
                .collect::<Vec<_>>()
        };
        let (plain, fanned) = (stems(0.0), stems(90.0));
        for ((a, length_a), (b, length_b)) in plain.iter().zip(fanned.iter()) {
            assert_eq!(a, b);
            assert!((length_a - length_b).abs() < 1e-5, "{} and {}", length_a, length_b);
        }
        assert!(plain.windows(2).any(|w| w[0].1 != w[1].1), "{:?}", plain);
        // a single stem has nothing to fan out from
        let mut single = run_fan(&[], 1, 0.5, 120.0);
        assert!(directions(&mut single)[0].distance(Vec3::Y) < 1e-6);
    }

    #[test]
    fn on_points_the_stems_of_every_point_make_their_own_fan_toward_its_tangent() {
        let points = encode_points(&[point(0, Vec3::ZERO, Vec3::Y), point(1, Vec3::new(2.0, 0.0, 0.0), Vec3::Z)]);
        // in turn: stems 0, 2 and 4 on the first point, 1 and 3 on the second
        let mut data = run_fan(&points, 5, 1.0, 90.0);
        let directions = directions(&mut data);
        let diagonal = |a: Vec3, b: Vec3| (a + b).normalize();
        let expected = [diagonal(Vec3::Y, -Vec3::X), diagonal(Vec3::Z, -Vec3::X), Vec3::Y, diagonal(Vec3::Z, Vec3::X), diagonal(Vec3::Y, Vec3::X)];
        for (direction, expected) in directions.iter().zip(expected) {
            assert!(direction.distance(expected) < 1e-5, "{:?} instead of {:?}", direction, expected);
        }
    }
}
