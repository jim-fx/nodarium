use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use myzel_core::{polygonize, read_field, read_int};

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let field = read_field(args[0]);
    let resolution = read_int(args[1]).clamp(16, 256) as usize;
    let mesh = polygonize(&field, resolution).encode();

    // the viewer reads a list of framed items: [0, 1, [0, n + 1, ...mesh, 1, 1], 1, 1]
    let mut output = Vec::with_capacity(mesh.len() + 8);
    output.extend([0, 1, 0, mesh.len() as i32 + 1]);
    output.extend(mesh);
    output.extend([1, 1, 1, 1]);
    output
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    #[allow(unused_imports)]
    use glam::Vec3;
    #[allow(unused_imports)]
    use myzel_core::{eval, Field};
    #[allow(unused_imports)]
    use nodarium_utils::{decode_float, encode_float};

    #[allow(dead_code)]
    fn vec3(x: f32, y: f32, z: f32) -> Vec<i32> {
        vec![0, 4, encode_float(x), encode_float(y), encode_float(z), 1, 1]
    }

    use myzel_core::{Mesh, MESH_TAG};
    use nodarium_utils::split_args;

    #[test]
    fn wraps_the_mesh_in_a_list() {
        let field = Field::sphere(1.0).encode();
        let output = __nodarium_execute_inner(&[&field, &[16]]);
        let items = split_args(&output);
        assert_eq!(items.len(), 1);
        let item = items[0];
        assert_eq!(item[2], MESH_TAG);
        let mesh = Mesh::decode(&item[2..item.len() - 2]);
        assert!(mesh.face_count() > 0);
    }
}
