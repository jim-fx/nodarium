use myzel_core::{opaque, read_color, MeshWords, MESH_TAG};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::abi::{self, Output};

nodarium_definition_file!("src/definition.json");

#[nodarium_execute(arena)]
pub fn execute(args: &[&[i32]]) -> Output {
    let color = opaque(read_color(args[1]));
    // an input nothing is connected to is not a mesh
    let Some(mesh) = MeshWords::read(args[0]) else {
        return vec![MESH_TAG, 0, 0, 0].into();
    };
    let more = mesh.painted_len() - args[0].len();
    let in_place = if more == 0 { abi::frame_input(args[0], &[], &[]) } else { abi::extend_input(args[0], more) };
    match in_place {
        Some(mut words) => {
            // SAFETY: the input isn't read again
            MeshWords::paint_in_place(unsafe { words.words_mut() }, color);
            words.into()
        }
        None => abi::write_result(mesh.painted_len(), |words| {
            mesh.paint_into(color, words);
            words.len()
        })
        .into(),
    }
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use myzel_core::{encode_color, opaque, Mesh, NO_COLOR};
    use nodarium_utils::abi::{self, Output};
    use std::sync::Mutex;

    /// The buffers of the ABI are global, tests that use them take turns
    static STATE: Mutex<()> = Mutex::new(());

    fn plate() -> Mesh {
        Mesh {
            faces: vec![0, 1, 2],
            positions: vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            normals: vec![0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0],
            colors: Vec::new(),
            groups: vec![0, 1, 1],
        }
    }

    /// A mesh larger than a page, so the host's buffer of it is in the arena
    fn large(colors: bool) -> Mesh {
        let v = 3000;
        Mesh {
            faces: (0..v as u32 - 2).flat_map(|i| [i, i + 1, i + 2]).collect(),
            positions: (0..v * 3).map(|i| i as f32 * 0.25).collect(),
            normals: [0.0, 0.0, 1.0].repeat(v),
            colors: if colors { (0..v as u32).collect() } else { Vec::new() },
            groups: (0..v as u32).map(|i| i % 3).collect(),
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
    fn a_mesh_the_host_copied_in_is_painted_where_it_is() {
        let _turn = STATE.lock().unwrap_or_else(|e| e.into_inner());
        abi::use_arena();
        let white = [240, 236, 228];
        for colors in [false, true] {
            abi::reset();
            let mesh = large(colors).encode();
            let input = host_input(&mesh);
            let color = host_input(&encode_color(white));
            let painted = __nodarium_execute_inner(&[input, color]);
            assert!(matches!(painted, Output::InPlace(_)));
            let words = unsafe { painted.words() };
            assert_eq!(words.as_ptr(), input.as_ptr());
            assert_eq!(words, Mesh { colors: vec![opaque(white); 3000], ..large(false) }.encode());
            abi::store_result(painted);
        }
        abi::reset();
    }

    #[test]
    fn a_result_passed_back_in_is_painted_into_a_new_mesh() {
        let _turn = STATE.lock().unwrap_or_else(|e| e.into_inner());
        abi::use_arena();
        abi::reset();
        let input = host_input(&large(false).encode());
        let Output::InPlace(first) = __nodarium_execute_inner(&[input, &encode_color([1, 2, 3])]) else {
            panic!("the input wasn't painted in place");
        };
        abi::store_result(Output::from(first));
        let before = unsafe { first.words() }.to_vec();
        // other nodes may still read the first result
        let second = __nodarium_execute_inner(&[unsafe { first.words() }, &encode_color([4, 5, 6])]);
        assert_ne!(unsafe { second.words() }.as_ptr(), unsafe { first.words() }.as_ptr());
        assert_eq!(unsafe { first.words() }, before);
        assert_eq!(Mesh::decode(unsafe { second.words() }).colors, vec![opaque([4, 5, 6]); 3000]);
        abi::reset();
    }

    #[test]
    fn only_the_colors_of_the_mesh_change() {
        let _turn = STATE.lock().unwrap_or_else(|e| e.into_inner());
        let brown = [74, 50, 38];
        let mesh = Mesh::decode(unsafe { __nodarium_execute_inner(&[&plate().encode(), &encode_color(brown)]).words() });
        assert_eq!(mesh.colors, vec![opaque(brown); 3]);
        assert_eq!(Mesh { colors: Vec::new(), ..mesh }, plate());
    }

    #[test]
    fn colors_that_were_there_are_replaced() {
        let _turn = STATE.lock().unwrap_or_else(|e| e.into_inner());
        let before = Mesh { colors: vec![NO_COLOR, opaque([1, 2, 3]), NO_COLOR], ..plate() };
        let white = [240, 236, 228];
        let mesh = Mesh::decode(unsafe { __nodarium_execute_inner(&[&before.encode(), &encode_color(white)]).words() });
        assert_eq!(mesh.colors, vec![opaque(white); 3]);
    }

    #[test]
    fn nothing_connected_gives_an_empty_mesh() {
        let _turn = STATE.lock().unwrap_or_else(|e| e.into_inner());
        let mesh = __nodarium_execute_inner(&[&[0, 1, 1, 1], &encode_color([1, 2, 3])]);
        assert_eq!(Mesh::decode(unsafe { mesh.words() }), Mesh::default());
    }
}
