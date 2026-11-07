use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;
use nodarium_utils::{
    int,
    geometry::{create_multiple_paths, wrap_multiple_paths},
    log, Ctx, PathInfo,
};

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let mut ctx = Ctx::default();


    let amount = ctx.int(args[1]).max(0) as usize;
    let path_resolution = int(args[4]) as usize;

    log!("stem args: {:?}", args);

    // the graph node that makes the stems, it is the last input
    let creator = int(args[6]);

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
        let origin = ctx.vec3(args[0]);
        let length = ctx.float(args[2]);
        // path_info length reads the length of this stem, so its thickness can follow it
        ctx.path.length = length;
        let amount_points = stem.points.len() / 4;
        let alphas = (0..amount_points).map(|i| i as f32 / (path_resolution - 1) as f32);
        // alpha goes along the stem, so the thickness can follow a curve
        let thickness = ctx.float_along(args[3], alphas.clone());
        let taper = ctx.float(args[5]);

        for (i, a) in alphas.enumerate() {
            stem.points[i * 4] = origin[0];
            stem.points[i * 4 + 1] = origin[1] + a * length;
            stem.points[i * 4 + 2] = origin[2];
            stem.points[i * 4 + 3] = thickness[i] * (1.0 - a * taper);
        }
    }

    log!("stem_data: {:?}", stem_data);

    stem_data
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use nodarium_utils::{concat_args, encode_float, geometry::wrap_multiple_paths};

    #[test]
    fn the_thickness_can_follow_the_length_of_the_stem() {
        let origin = concat_args(vec![&[encode_float(0.0)], &[encode_float(0.0)], &[encode_float(0.0)]]);
        let length = concat_args(vec![&[1], &[encode_float(1.0)], &[encode_float(3.0)], &[7]]);
        // a tenth of the length of the stem
        let path_length = concat_args(vec![&[5], &[5]]);
        let thickness = concat_args(vec![&[0], &[2], &path_length, &[encode_float(0.1)]]);

        let mut data = __nodarium_execute_inner(&[&origin, &[4], &length, &thickness, &[8], &[encode_float(0.0)], &[2]]);
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
        assert!(lengths.windows(2).any(|w| w[0] != w[1]), "{lengths:?}");
    }
}
