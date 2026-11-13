use myzel_core::MeshWords;
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::abi::{self, Output};

nodarium_definition_file!("src/definition.json");

#[nodarium_execute(arena)]
pub fn execute(args: &[&[i32]]) -> Output {
    // an input nothing is connected to is not a mesh
    let meshes: Vec<MeshWords> = args.iter().filter_map(|arg| MeshWords::read(arg)).collect();
    abi::write_result(MeshWords::join_len(&meshes), |words| {
        MeshWords::join_into(&meshes, words);
        words.len()
    })
    .into()
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use myzel_core::{opaque, Mesh, NO_COLOR};

    fn plate(x: f32, groups: [u32; 3], colored: bool) -> Mesh {
        Mesh {
            faces: vec![0, 1, 2],
            positions: vec![x, 0.0, 0.0, x + 0.1, 0.0, 0.0, x, 0.1, 0.0],
            normals: vec![0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0],
            colors: if colored { vec![opaque([80, 52, 40]); 3] } else { Vec::new() },
            groups: groups.to_vec(),
        }
    }

    fn run_words(inputs: [Vec<i32>; 4]) -> Mesh {
        let refs: Vec<&[i32]> = inputs.iter().map(|w| w.as_slice()).collect();
        Mesh::decode(unsafe { __nodarium_execute_inner(&refs).words() })
    }

    fn run(inputs: &[&Mesh]) -> Mesh {
        run_words([0, 1, 2, 3].map(|i| inputs.get(i).map_or(Vec::new(), |m| m.encode())))
    }

    #[test]
    fn one_mesh_gives_that_mesh() {
        let mesh = plate(0.0, [0, 1, 1], true);
        assert_eq!(run(&[&mesh]), mesh);
        assert_eq!(run(&[]), Mesh::default());
    }

    #[test]
    fn the_groups_of_the_second_follow_the_first() {
        let (a, b) = (plate(0.0, [0, 1, 1], false), plate(0.5, [0, 0, 2], false));
        let mesh = run(&[&a, &b]);
        assert_eq!(mesh.groups, vec![0, 1, 1, 2, 2, 4]);
        assert_eq!(mesh.faces, vec![0, 1, 2, 3, 4, 5]);
        assert_eq!(&mesh.positions[..9], &a.positions[..]);
        assert_eq!(&mesh.positions[9..], &b.positions[..]);
    }

    #[test]
    fn a_third_follows_the_second_and_a_gap_is_skipped() {
        let (a, b) = (plate(0.0, [0, 0, 0], false), plate(0.5, [0, 1, 1], false));
        let mesh = run_words([a.encode(), Vec::new(), b.encode(), a.encode()]);
        assert_eq!(mesh.groups, vec![0, 0, 0, 1, 2, 2, 3, 3, 3]);
    }

    #[test]
    fn colors_survive() {
        let (painted, plain) = (plate(0.0, [0, 0, 0], true), plate(0.5, [0, 0, 0], false));
        let brown = opaque([80, 52, 40]);
        assert_eq!(run(&[&painted, &plain]).colors, [[brown; 3], [NO_COLOR; 3]].concat());
        assert_eq!(run(&[&plain, &painted]).colors, [[NO_COLOR; 3], [brown; 3]].concat());
        assert!(run(&[&plain, &plain]).colors.is_empty());
    }

    #[test]
    fn empty_inputs_are_skipped() {
        let mesh = plate(0.0, [0, 1, 1], true);
        assert_eq!(run(&[&Mesh::default(), &mesh, &Mesh::default()]), mesh);
        // the list the host sends for nothing connected
        assert_eq!(run_words([vec![0, 1, 1, 1], mesh.encode(), Vec::new(), Vec::new()]), mesh);
    }
}
