use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{
    abi::{self, Output},
    concat_arg_vecs,
    geometry::{
        is_geometry, split_items, vertex_places, with_geometry_colors, with_path_colors, wrap_path,
        DeepestPaths, MeshWords,
    },
    decode_float, opaque, reads_place, Ctx,
};

nodarium_definition_file!("src/definition.json");

fn color_geometry(ctx: &mut Ctx, geometry: &[i32], color: &[i32]) -> Vec<i32> {
    let positions: Vec<f32> = match MeshWords::read(geometry) {
        Some(words) => words.positions().iter().map(|&w| decode_float(w)).collect(),
        None => return geometry.to_vec(),
    };
    let colors: Vec<u32> = ctx.colors_at(color, vertex_places(&positions)).into_iter().map(opaque).collect();
    with_geometry_colors(geometry, &colors)
}

/// A geometry in one color, its groups and everything else as they are. In place when it is an
/// input the host copied in, like the large surface of a mushroom.
fn paint(input: &[i32], geometry: MeshWords, color: u32) -> Output {
    let more = geometry.painted_len() - input.len();
    let in_place = if more == 0 { abi::frame_input(input, &[], &[]) } else { abi::extend_input(input, more) };
    match in_place {
        Some(mut words) => {
            // SAFETY: the input isn't read again
            MeshWords::paint_in_place(unsafe { words.words_mut() }, color);
            words.into()
        }
        None => abi::write_result(geometry.painted_len(), |words| {
            geometry.paint_into(color, words);
            words.len()
        })
        .into(),
    }
}

#[nodarium_execute(arena)]
pub fn execute(args: &[&[i32]]) -> Output {
    let mut ctx = Ctx::default();
    // a single geometry stays one, it is no list
    if let Some(geometry) = MeshWords::read(args[0]) {
        if !reads_place(args[1]) {
            return paint(args[0], geometry, opaque(ctx.color(args[1])));
        }
        return color_geometry(&mut ctx, args[0], args[1]).into();
    }

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
    concat_arg_vecs(colored).into()
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use nodarium_utils::{
        abi::{self, Output},
        concat_args, encode_color, encode_float,
        geometry::{
            create_multiple_paths, extrude_path, is_path, wrap_geometry_data, wrap_multiple_paths,
            wrap_path, Mesh, COLORED_PATH, NO_PATH,
        },
        mix_colors, opaque, split_args, wrap_arg, MIX_COLOR, NO_COLOR,
    };
    use std::sync::Mutex;

    /// The buffers of the ABI are global, tests that use them take turns
    static STATE: Mutex<()> = Mutex::new(());

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
        let mut data = vec![1, 4, 2, NO_PATH, 0, 0, 1, 2, 1, 3, 2];
        let positions = [0.0, 0.0, 0.0, -0.5, 0.0, -1.0, 0.5, 0.0, -1.0, 0.0, 0.0, -2.0f32];
        data.extend(positions.iter().map(|&p| encode_float(p)));
        data.extend([0.0, 1.0, 0.0f32].repeat(4).iter().map(|&n| encode_float(n)));
        wrap_arg(&data)
    }

    fn gradient() -> Vec<i32> {
        concat_args(vec![&[MIX_COLOR], &encode_color(BROWN), &encode_color(GREEN), &concat_args(vec![&[2], &[0]])])
    }

    fn run(input: &[i32], color: &[i32]) -> Vec<i32> {
        let _turn = STATE.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { __nodarium_execute_inner(&[input, color, &[1]]).words() }.to_vec()
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

    /// A plate of a Myzel node, three groups made by two nodes
    fn plate() -> Mesh {
        Mesh {
            faces: vec![0, 1, 2],
            positions: vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            normals: vec![0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0],
            colors: Vec::new(),
            groups: vec![0, 1, 1],
            creators: vec![21, 22],
            paths: Vec::new(),
        }
    }

    #[test]
    fn a_geometry_with_groups_keeps_them() {
        let mesh = Mesh::decode(&run(&plate().encode(), &encode_color(BROWN)));
        assert_eq!(mesh.colors, vec![opaque(BROWN); 3]);
        assert_eq!(Mesh { colors: Vec::new(), ..mesh }, plate());
        // colors that were there are replaced
        let before = Mesh { colors: vec![NO_COLOR, opaque(GREEN), NO_COLOR], ..plate() };
        assert_eq!(Mesh::decode(&run(&before.encode(), &encode_color(BROWN))).colors, vec![opaque(BROWN); 3]);
        // and a gradient keeps them too
        let mesh = Mesh::decode(&run(&plate().encode(), &gradient()));
        assert_eq!((mesh.groups, mesh.creators), (plate().groups, plate().creators));
    }

    #[test]
    fn a_color_that_reads_no_place_is_the_one_of_every_vertex() {
        let half = concat_args(vec![&[MIX_COLOR], &encode_color(BROWN), &encode_color(GREEN), &[encode_float(0.5)]]);
        let mut out = run(&leaf(), &half);
        assert_eq!(wrap_geometry_data(&mut out).colors, &[opaque(mix_colors(BROWN, GREEN, 0.5)); 4]);
        let mut out = run(&leaf(), &encode_color(GREEN));
        assert_eq!(out.len(), leaf().len() + 4);
        assert_eq!(wrap_geometry_data(&mut out).colors, &[opaque(GREEN); 4]);
    }

    /// A geometry larger than a page, so the host's buffer of it is in the arena
    fn large(colors: bool) -> Mesh {
        let v = 3000;
        Mesh {
            faces: (0..v as u32 - 2).flat_map(|i| [i, i + 1, i + 2]).collect(),
            positions: (0..v * 3).map(|i| i as f32 * 0.25).collect(),
            normals: [0.0, 0.0, 1.0].repeat(v),
            colors: if colors { (0..v as u32).collect() } else { Vec::new() },
            groups: (0..v as u32).map(|i| i % 3).collect(),
            creators: vec![1, 2, 3],
            paths: Vec::new(),
        }
    }

    /// What the host does: allocates a buffer and writes `words` into it
    fn host_input(words: &[i32]) -> &'static [i32] {
        let ptr = abi::alloc(words.len() * 4);
        unsafe {
            std::ptr::copy_nonoverlapping(words.as_ptr(), ptr, words.len());
            std::slice::from_raw_parts(ptr, words.len())
        }
    }

    #[test]
    fn a_geometry_the_host_copied_in_is_painted_where_it_is() {
        let _turn = STATE.lock().unwrap_or_else(|e| e.into_inner());
        abi::use_arena();
        let white = [240, 236, 228];
        for colors in [false, true] {
            abi::reset();
            let mesh = large(colors).encode();
            let input = host_input(&mesh);
            let color = host_input(&encode_color(white));
            let painted = __nodarium_execute_inner(&[input, color, &[1]]);
            assert!(matches!(painted, Output::InPlace(_)));
            let words = unsafe { painted.words() };
            assert_eq!(words.as_ptr(), input.as_ptr());
            assert_eq!(words, Mesh { colors: vec![opaque(white); 3000], ..large(false) }.encode());
            abi::store_result(painted);
        }
        abi::reset();
    }

    #[test]
    fn a_result_passed_back_in_is_painted_into_a_new_geometry() {
        let _turn = STATE.lock().unwrap_or_else(|e| e.into_inner());
        abi::use_arena();
        abi::reset();
        let input = host_input(&large(false).encode());
        let Output::InPlace(first) = __nodarium_execute_inner(&[input, &encode_color([1, 2, 3]), &[1]]) else {
            panic!("the input wasn't painted in place");
        };
        abi::store_result(Output::from(first));
        let before = unsafe { first.words() }.to_vec();
        // other nodes may still read the first result
        let second = __nodarium_execute_inner(&[unsafe { first.words() }, &encode_color([4, 5, 6]), &[1]]);
        assert_ne!(unsafe { second.words() }.as_ptr(), unsafe { first.words() }.as_ptr());
        assert_eq!(unsafe { first.words() }, before);
        assert_eq!(Mesh::decode(unsafe { second.words() }).colors, vec![opaque([4, 5, 6]); 3000]);
        abi::reset();
    }
}
