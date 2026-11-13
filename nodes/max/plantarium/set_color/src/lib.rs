use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{
    concat_arg_vecs,
    geometry::{
        is_geometry, split_items, with_geometry_colors, with_path_colors, wrap_path, DeepestPaths,
    },
    decode_float, opaque, Ctx,
};

nodarium_definition_file!("src/definition.json");

/// Where every vertex of a geometry sits: along from its base (z = 0, where an instance places
/// it) to the vertex farthest along z, and across from its middle (x = 0) to its edge at the
/// same height. A leaf grows along -z and its width lies along x.
fn vertex_places(positions: &[f32]) -> Vec<(f32, f32)> {
    let point = |i: usize| (positions[i * 3].abs(), positions[i * 3 + 2].abs());
    let n = positions.len() / 3;
    let reach = (0..n).map(|i| point(i).1).fold(0.0, f32::max);
    let along: Vec<f32> = (0..n)
        .map(|i| if reach > 0.0 { point(i).1 / reach } else { 0.0 })
        .collect();

    // the width at a height is the widest vertex around it
    let bins = ((n as f32).sqrt().round() as usize).clamp(4, 64);
    let bin = |a: f32| ((a * bins as f32) as usize).min(bins - 1);
    let mut width = vec![0.0f32; bins];
    for i in 0..n {
        let b = bin(along[i]);
        width[b] = width[b].max(point(i).0);
    }
    (0..n)
        .map(|i| {
            let b = bin(along[i]);
            let w = width[b];
            let across = if w > 0.0 { (point(i).0 / w).min(1.0) } else { 0.0 };
            (along[i], across)
        })
        .collect()
}

fn color_geometry(ctx: &mut Ctx, geometry: &[i32], color: &[i32]) -> Vec<i32> {
    let (v, f) = (geometry[3] as usize, geometry[4] as usize);
    let start = 6 + f * 3;
    let positions: Vec<f32> = geometry[start..start + v * 3].iter().map(|&w| decode_float(w)).collect();
    let colors: Vec<u32> = ctx.colors_at(color, vertex_places(&positions)).into_iter().map(opaque).collect();
    with_geometry_colors(geometry, &colors)
}

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let mut ctx = Ctx::default();
    let items = split_items(args[0]);
    let depth = ctx.int(args[2]);
    let selected = DeepestPaths::new(&items, depth);
    let count = selected.count(&items);
    let mut rank = 0;

    let colored: Vec<Vec<i32>> = items
        .iter()
        .map(|item| {
            if selected.contains(item) {
                ctx.set_path(item, rank, count);
                rank += 1;
                let points = wrap_path(item).length;
                let alphas = (0..points).map(|i| (i as f32 / (points - 1).max(1) as f32, 0.0));
                let colors: Vec<u32> = ctx.colors_at(args[1], alphas).into_iter().map(opaque).collect();
                with_path_colors(item, &colors)
            } else if is_geometry(item) {
                color_geometry(&mut ctx, item, args[1])
            } else {
                item.to_vec()
            }
        })
        .collect();

    // a single geometry stays one, it is no list
    if is_geometry(args[0]) {
        return colored.into_iter().next().unwrap();
    }
    concat_arg_vecs(colored)
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use nodarium_utils::{
        concat_args, encode_color, encode_float,
        geometry::{
            create_multiple_paths, extrude_path, is_path, wrap_geometry_data, wrap_multiple_paths,
            wrap_path, COLORED_PATH, NO_PATH,
        },
        mix_colors, opaque, split_args, wrap_arg, MIX_COLOR,
    };

    const GREEN: [u8; 3] = [60, 140, 50];
    const BROWN: [u8; 3] = [110, 80, 50];

    fn stems(amount: usize, points: usize) -> Vec<i32> {
        let mut stems = create_multiple_paths(amount, points, 1, 6);
        for (k, stem) in wrap_multiple_paths(&mut stems).iter_mut().enumerate() {
            for i in 0..points {
                stem.points[i * 4..i * 4 + 4].copy_from_slice(&[k as f32, i as f32, 0.0, 0.1]);
            }
        }
        stems
    }

    /// A leaf of two triangles: its base at the origin, its tip at z -2, 1 wide at z -1
    fn leaf() -> Vec<i32> {
        let mut data = vec![1, 4, 2, NO_PATH, 0, 1, 2, 1, 3, 2];
        let positions = [0.0, 0.0, 0.0, -0.5, 0.0, -1.0, 0.5, 0.0, -1.0, 0.0, 0.0, -2.0f32];
        data.extend(positions.iter().map(|&p| encode_float(p)));
        data.extend([0.0, 1.0, 0.0f32].repeat(4).iter().map(|&n| encode_float(n)));
        wrap_arg(&data)
    }

    fn gradient() -> Vec<i32> {
        concat_args(vec![&[MIX_COLOR], &encode_color(BROWN), &encode_color(GREEN), &concat_args(vec![&[2], &[0]])])
    }

    fn run(input: &[i32], color: &[i32]) -> Vec<i32> {
        __nodarium_execute_inner(&[input, color, &[1]])
    }

    #[test]
    fn a_path_gets_a_color_at_every_point() {
        let stems = stems(2, 3);
        let out = run(&stems, &encode_color(GREEN));
        let items = split_args(&out);
        assert_eq!(items.len(), 2);
        for (item, before) in items.iter().zip(split_args(&stems)) {
            assert!(is_path(item));
            assert_eq!(item[2], COLORED_PATH);
            let (path, plain) = (wrap_path(item), wrap_path(before));
            assert_eq!(path.points, plain.points);
            assert_eq!(path.link, plain.link);
            assert_eq!(path.colors, &[opaque(GREEN); 3]);
        }
    }

    #[test]
    fn a_gradient_goes_from_the_base_to_the_tip() {
        let out = run(&stems(1, 3), &gradient());
        let path = wrap_path(split_args(&out)[0]);
        assert_eq!(path.colors, &[opaque(BROWN), opaque(mix_colors(BROWN, GREEN, 0.5)), opaque(GREEN)]);

        // the extruded rings take the colors of their points
        let mut geometry = extrude_path(path, 4);
        let data = wrap_geometry_data(&mut geometry);
        assert_eq!(data.colors.len(), 3 * 4 + 4 + 1);
        assert_eq!(&data.colors[..4], &[opaque(BROWN); 4]);
        assert_eq!(&data.colors[4..8], &[opaque(mix_colors(BROWN, GREEN, 0.5)); 4]);
        assert!(data.colors[8..].iter().all(|&c| c == opaque(GREEN)));
    }

    #[test]
    fn only_the_deepest_paths_and_geometry_are_colored() {
        let stems = stems(1, 3);
        let mut deeper = split_args(&stems)[0].to_vec();
        deeper[3] = 2;
        let input = concat_args(vec![split_args(&stems)[0], &deeper, &leaf()]);
        let out = run(&input, &encode_color(GREEN));
        let items = split_args(&out);
        assert_eq!(items[0], split_args(&stems)[0]);
        assert_eq!(items[1][2], COLORED_PATH);
        let mut leaf = items[2].to_vec();
        assert_eq!(wrap_geometry_data(&mut leaf).colors, &[opaque(GREEN); 4]);
    }

    #[test]
    fn a_geometry_is_colored_from_its_base_to_its_tip_and_across() {
        let mut out = run(&leaf(), &gradient());
        // a single geometry comes back as one
        assert_eq!(out.len(), leaf().len() + 4);
        let colors = wrap_geometry_data(&mut out).colors.to_vec();
        assert_eq!(colors, vec![opaque(BROWN), opaque(mix_colors(BROWN, GREEN, 0.5)), opaque(mix_colors(BROWN, GREEN, 0.5)), opaque(GREEN)]);

        let across = concat_args(vec![&[MIX_COLOR], &encode_color(BROWN), &encode_color(GREEN), &concat_args(vec![&[2], &[1]])]);
        let mut out = run(&leaf(), &across);
        let colors = wrap_geometry_data(&mut out).colors.to_vec();
        assert_eq!(colors, vec![opaque(BROWN), opaque(GREEN), opaque(GREEN), opaque(BROWN)]);
    }

    #[test]
    fn nothing_connected_gives_nothing() {
        assert_eq!(run(&[0, 1, 1, 1], &encode_color(GREEN)), vec![0, 1, 1, 1]);
    }
}
