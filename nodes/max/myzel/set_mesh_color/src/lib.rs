use myzel_core::{paint_mesh, read_color, Mesh};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    // an input nothing is connected to is not a mesh
    let mut mesh = Mesh::try_decode(args[0]).unwrap_or_default();
    paint_mesh(&mut mesh, read_color(args[1]));
    mesh.encode()
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use myzel_core::{encode_color, opaque, Mesh, NO_COLOR};

    fn plate() -> Mesh {
        Mesh {
            faces: vec![0, 1, 2],
            positions: vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            normals: vec![0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0],
            colors: Vec::new(),
            groups: vec![0, 1, 1],
        }
    }

    #[test]
    fn only_the_colors_of_the_mesh_change() {
        let brown = [74, 50, 38];
        let mesh = Mesh::decode(&__nodarium_execute_inner(&[&plate().encode(), &encode_color(brown)]));
        assert_eq!(mesh.colors, vec![opaque(brown); 3]);
        assert_eq!(Mesh { colors: Vec::new(), ..mesh }, plate());
    }

    #[test]
    fn colors_that_were_there_are_replaced() {
        let before = Mesh { colors: vec![NO_COLOR, opaque([1, 2, 3]), NO_COLOR], ..plate() };
        let white = [240, 236, 228];
        let mesh = Mesh::decode(&__nodarium_execute_inner(&[&before.encode(), &encode_color(white)]));
        assert_eq!(mesh.colors, vec![opaque(white); 3]);
    }

    #[test]
    fn nothing_connected_gives_an_empty_mesh() {
        let mesh = __nodarium_execute_inner(&[&[0, 1, 1, 1], &encode_color([1, 2, 3])]);
        assert_eq!(Mesh::decode(&mesh), Mesh::default());
    }
}
