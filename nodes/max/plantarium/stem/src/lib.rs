use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;
use nodarium_utils::{
    decode_points, int,
    geometry::{create_multiple_paths, wrap_multiple_paths},
    log, Ctx, PathInfo,
};

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let mut ctx = Ctx::default();


    let points = decode_points(args[1]);
    let amount = ctx.int(args[2]).max(0) as usize;
    let path_resolution = int(args[5]) as usize;

    log!("stem args: {:?}", args);

    // the graph node that makes the stems, it is the last input
    let creator = int(args[7]);

    let mut stem_data = create_multiple_paths(amount, path_resolution, 1, creator);

    let mut stems = wrap_multiple_paths(&mut stem_data);

    let stem_count = stems.iter_mut().count();

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
        let length = ctx.float(args[3]);
        // path_info length reads the length of this stem, so its thickness can follow it
        ctx.path.length = length;
        let amount_points = stem.points.len() / 4;
        let alphas = (0..amount_points).map(|i| i as f32 / (path_resolution - 1) as f32);
        // alpha goes along the stem, so the thickness can follow a curve
        let thickness = ctx.float_along(args[4], alphas.clone());
        let taper = ctx.float(args[6]);

        match points.get(index % points.len().max(1)) {
            Some(point) => {
                for (i, a) in alphas.enumerate() {
                    let p = point.position + point.direction * a * length;
                    stem.points[i * 4..i * 4 + 3].copy_from_slice(&p.to_array());
                    stem.points[i * 4 + 3] = thickness[i] * (1.0 - a * taper);
                }
            }
            None => {
                for (i, a) in alphas.enumerate() {
                    stem.points[i * 4] = origin[0];
                    stem.points[i * 4 + 1] = origin[1] + a * length;
                    stem.points[i * 4 + 2] = origin[2];
                    stem.points[i * 4 + 3] = thickness[i] * (1.0 - a * taper);
                }
            }
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

    fn run(points: &[i32], amount: i32, length: f32) -> Vec<i32> {
        let creator = 2;
        __nodarium_execute_inner(&[&origin(), points, &[amount], &[encode_float(length)], &[encode_float(0.1)], &[5], &[encode_float(1.0)], &[creator]])
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

        let mut data = __nodarium_execute_inner(&[&origin, &[], &[4], &length, &thickness, &[8], &[encode_float(0.0)], &[2]]);
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
}
