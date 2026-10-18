use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;
use nodarium_utils::{
    int,
    geometry::{create_multiple_paths, wrap_multiple_paths},
    log, Ctx, };

nodarium_definition_file!("src/input.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let mut ctx = Ctx::default();


    let amount = int(args[1]) as usize;
    let path_resolution = int(args[4]) as usize;

    log!("stem args: {:?}", args);

    let mut stem_data = create_multiple_paths(amount, path_resolution, 1);

    let mut stems = wrap_multiple_paths(&mut stem_data);

    for stem in stems.iter_mut() {
        let origin = ctx.vec3(args[0]);
        let length = ctx.float(args[2]);
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
