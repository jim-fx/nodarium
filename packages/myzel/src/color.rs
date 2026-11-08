use crate::mesh::{pack_rgba, Mesh, MESH_TAG};
use nodarium_utils::{concat_arg_vecs, split_args};

/// A vertex without a color of its own: the viewer shows its material there. White, so programs
/// that multiply the vertex color with their material see the material.
pub const NO_COLOR: u32 = 0x00ff_ffff;

/// A color input, the sRGB bytes `[0, 4, r, g, b, 1, 1]`. That is how the host sends a color set
/// on the node and how a color node sends one, a connected node may wrap it in a list of its own.
pub fn read_color(arg: &[i32]) -> [u8; 3] {
    let values = match arg {
        [0, 4, r, g, b, 1, 1] | [0, 1, 0, 4, r, g, b, 1, 1, 1, 1] => [*r, *g, *b],
        _ => panic!("expected a color [0, 4, r, g, b, 1, 1], got {} words: {:?}", arg.len(), arg),
    };
    values.map(|v| v.clamp(0, 255) as u8)
}

/// A color as a node sends it, see [`read_color`]
pub fn encode_color([r, g, b]: [u8; 3]) -> Vec<i32> {
    vec![0, 4, r as i32, g as i32, b as i32, 1, 1]
}

fn to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn to_srgb(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// An RGBA8 word as linear RGB and alpha, which mix like light does
pub fn linear_rgba(word: u32) -> [f32; 4] {
    let byte = |i: u32| ((word >> (i * 8)) & 0xff) as f32 / 255.0;
    [to_linear(byte(0)), to_linear(byte(1)), to_linear(byte(2)), byte(3)]
}

/// The RGBA8 word of a linear color, see [`linear_rgba`]
pub fn pack_linear([r, g, b, a]: [f32; 4]) -> u32 {
    pack_rgba(to_srgb(r), to_srgb(g), to_srgb(b), a)
}

/// The opaque RGBA8 word of sRGB bytes
pub fn opaque([r, g, b]: [u8; 3]) -> u32 {
    r as u32 | (g as u32) << 8 | (b as u32) << 16 | 0xff00_0000
}

/// `a` at 0 to `b` at 1, mixed in linear light like paint under the same light
pub fn mix_colors(a: [u8; 3], b: [u8; 3], factor: f32) -> [u8; 3] {
    let t = factor.clamp(0.0, 1.0);
    let (a, b) = (linear_rgba(opaque(a)), linear_rgba(opaque(b)));
    let mixed = pack_linear([0, 1, 2, 3].map(|i| a[i] + (b[i] - a[i]) * t));
    [0, 8, 16].map(|s| (mixed >> s) as u8)
}

/// Gives every vertex of the mesh `color`
pub fn paint_mesh(mesh: &mut Mesh, color: [u8; 3]) {
    mesh.colors = vec![opaque(color); mesh.vertex_count()];
}

fn is_wrapped_mesh(item: &[i32]) -> bool {
    let n = item.len();
    n >= 5 && item[0] == 0 && item[1] as usize == n - 3 && item[2] == MESH_TAG
}

/// A list inside a list starts with a group, its count then points at index 2
fn is_list(item: &[i32]) -> bool {
    item.len() >= 4 && item[0] == 0 && item[1] == 1
}

/// The meshes in a value, one mesh or a list of them from a join, all painted in `color`.
/// Everything else in a list stays as it is.
pub fn paint_meshes(arg: &[i32], color: [u8; 3]) -> Vec<i32> {
    if arg.first() == Some(&MESH_TAG) {
        // anything else that starts with the tag stays as it is
        let Some(mut mesh) = Mesh::try_decode(arg) else { return arg.to_vec() };
        paint_mesh(&mut mesh, color);
        return mesh.encode();
    }
    if is_wrapped_mesh(arg) {
        return nodarium_utils::wrap_arg(&paint_meshes(&arg[2..arg.len() - 2], color));
    }
    if !is_list(arg) || arg.len() == 4 {
        return arg.to_vec();
    }
    concat_arg_vecs(
        split_args(arg)
            .into_iter()
            .map(|item| if is_wrapped_mesh(item) || is_list(item) { paint_meshes(item, color) } else { item.to_vec() })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodarium_utils::wrap_arg;

    fn triangle() -> Mesh {
        Mesh {
            faces: vec![0, 1, 2],
            positions: vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            normals: vec![0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0],
            colors: Vec::new(),
            groups: vec![0, 0, 0],
            tissue_groups: false,
        }
    }

    #[test]
    fn reads_a_color_set_on_the_node_or_connected() {
        assert_eq!(read_color(&[0, 4, 250, 128, 0, 1, 1]), [250, 128, 0]);
        assert_eq!(read_color(&[0, 1, 0, 4, 1, 2, 3, 1, 1, 1, 1]), [1, 2, 3]);
        assert_eq!(read_color(&encode_color([9, 8, 7])), [9, 8, 7]);
    }

    #[test]
    fn mixes_in_linear_light() {
        assert_eq!(mix_colors([255, 0, 0], [0, 0, 255], 0.0), [255, 0, 0]);
        assert_eq!(mix_colors([255, 0, 0], [0, 0, 255], 1.0), [0, 0, 255]);
        assert_eq!(mix_colors([255, 0, 0], [0, 0, 255], 7.0), [0, 0, 255]);
        // half of white light is lighter than the middle byte
        let [g, _, _] = mix_colors([0, 0, 0], [255, 255, 255], 0.5);
        assert!((186..=189).contains(&g), "{}", g);
        for c in [0u8, 1, 50, 128, 200, 255] {
            assert_eq!(pack_linear(linear_rgba(opaque([c, c, c]))), opaque([c, c, c]));
        }
    }

    #[test]
    fn paints_a_mesh_and_the_meshes_of_a_list() {
        let red = [200, 10, 10];
        let one = Mesh::decode(&paint_meshes(&triangle().encode(), red));
        assert_eq!(one.colors, vec![opaque(red); 3]);
        assert_eq!(Mesh { colors: Vec::new(), ..one }, triangle());

        let path = [0, 3, 7, 7, 1, 1];
        let list = concat_arg_vecs(vec![wrap_arg(&triangle().encode()), path.to_vec(), wrap_arg(&triangle().encode())]);
        let painted = paint_meshes(&list, red);
        let items = split_args(&painted);
        assert_eq!(items.len(), 3);
        assert_eq!(Mesh::decode(&items[0][2..items[0].len() - 2]).colors, vec![opaque(red); 3]);
        assert_eq!(items[1], path);
        // nothing connected
        assert_eq!(paint_meshes(&[0, 1, 1, 1], red), vec![0, 1, 1, 1]);
    }
}
