use crate::log;

use super::NO_PARENT;

static GEOMETRY_HEADER_SIZE: usize = 4;
// 0: geometry type = 1
// 1: vertex amount
// 2: face amount
// 3: id of the path the geometry was made from, `NO_PATH` if it doesn't come from a path
// followed by the faces, positions and normals, and optionally an RGBA8 color per vertex

/// The path id of geometry that wasn't made from a path.
pub const NO_PATH: i32 = NO_PARENT;

pub struct GeometryData<'a> {
    pub positions: &'a mut [f32],
    pub normals: &'a mut [f32],
    pub faces: &'a mut [i32],
    /// An RGBA8 color per vertex, empty for a geometry without colors
    pub colors: &'a mut [u32],
}

impl GeometryData<'_> {
    pub fn set_position(&mut self, index: usize, x: f32, y: f32, z: f32) {
        assert!(index < self.positions.len() / 3, "Index out of bounds.");
        let i = index * 3;
        self.positions[i] = x;
        self.positions[i + 1] = y;
        self.positions[i + 2] = z;
    }
    pub fn set_normal(&mut self, index: usize, x: f32, y: f32, z: f32) {
        assert!(index < self.normals.len() / 3, "Index out of bounds.");
        let i = index * 3;
        self.normals[i] = x;
        self.normals[i + 1] = y;
        self.normals[i + 2] = z;
    }
    pub fn set_face(&mut self, index: usize, a: i32, b: i32, c: i32) {
        assert!(index < self.faces.len() / 3, "Index out of bounds.");
        let i = index * 3;
        self.faces[i] = a;
        self.faces[i + 1] = b;
        self.faces[i + 2] = c;
    }
}

/// Length of an encoded geometry, brackets included.
pub fn geometry_data_len(vertex_amount: usize, face_amount: usize) -> usize {
    GEOMETRY_HEADER_SIZE // definition (type, vertex_amount, face_amount, path id)
      + 4 // opening and closing brackets
      + vertex_amount * 3  // positions
      + vertex_amount * 3  // normals
      + face_amount * 3 // faces
}

/// Length of an encoded geometry with a color per vertex, brackets included.
pub fn colored_geometry_data_len(vertex_amount: usize, face_amount: usize) -> usize {
    geometry_data_len(vertex_amount, face_amount) + vertex_amount
}

pub fn create_geometry_data(vertex_amount: usize, face_amount: usize) -> Vec<i32> {
    let mut geo = Vec::new();
    append_geometry_data(&mut geo, vertex_amount, face_amount);
    geo
}

/// Appends an empty geometry to `out` and returns where it lies, so it can be filled in place
/// without building it in its own buffer first.
pub fn append_geometry_data(
    out: &mut Vec<i32>,
    vertex_amount: usize,
    face_amount: usize,
) -> std::ops::Range<usize> {
    append_geometry(out, vertex_amount, face_amount, false)
}

/// Like `append_geometry_data`, with room for a color per vertex, all of them `NO_COLOR`.
pub fn append_colored_geometry_data(
    out: &mut Vec<i32>,
    vertex_amount: usize,
    face_amount: usize,
) -> std::ops::Range<usize> {
    append_geometry(out, vertex_amount, face_amount, true)
}

fn append_geometry(
    out: &mut Vec<i32>,
    vertex_amount: usize,
    face_amount: usize,
    colored: bool,
) -> std::ops::Range<usize> {
    let amount = if colored {
        colored_geometry_data_len(vertex_amount, face_amount)
    } else {
        geometry_data_len(vertex_amount, face_amount)
    };
    let start = out.len();
    out.resize(start + amount, 0);

    log!(
        "create_geometry_data: vertices: {} faces: {}",
        vertex_amount,
        face_amount
    );

    let geo = &mut out[start..];
    geo[0] = 0; // opening bracket
    geo[1] = amount as i32 - 3;
    geo[2] = 1; // type: geometry
    geo[3] = vertex_amount as i32;
    geo[4] = face_amount as i32;
    geo[5] = NO_PATH;
    if colored {
        let colors = amount - 2 - vertex_amount;
        geo[colors..amount - 2].fill(crate::NO_COLOR as i32);
    }
    geo[amount - 2] = 1; // closing bracket
    geo[amount - 1] = 1; // closing bracket

    start..start + amount
}

/// Whether an encoded geometry (brackets included) has a color per vertex: it does when its
/// colors follow the normals.
pub fn has_geometry_colors(geometry: &[i32]) -> bool {
    let (v, f) = (geometry[3] as usize, geometry[4] as usize);
    geometry.len() == colored_geometry_data_len(v, f)
}

/// The id of the path an encoded geometry was made from, brackets included.
pub fn geometry_path_id(geometry: &[i32]) -> i32 {
    geometry[5]
}

/// Sets the id of the path an encoded geometry was made from, brackets included.
pub fn set_geometry_path_id(geometry: &mut [i32], path_id: i32) {
    geometry[5] = path_id;
}

/// Whether a list item is an encoded geometry, brackets included.
pub fn is_geometry(item: &[i32]) -> bool {
    item.len() >= 2 + GEOMETRY_HEADER_SIZE + 2 && item[2] == 1
}

/// Splits an input that takes a list into its items. A single geometry comes without a list
/// around it and is one item, an unconnected input is the empty list `[0, 1, 1, 1]`.
pub fn split_items(input: &[i32]) -> Vec<&[i32]> {
    if is_geometry(input) {
        vec![input]
    } else {
        crate::split_args(input)
    }
}

pub fn wrap_geometry_data(geometry: &mut [i32]) -> GeometryData<'_> {
    assert!(
        geometry.len() > GEOMETRY_HEADER_SIZE,
        "Geometry vector does not contain enough data for a header."
    );

    let colored = has_geometry_colors(geometry);
    let (header, rest) = geometry.split_at_mut(2 + GEOMETRY_HEADER_SIZE);

    let vertices_amount = header[3] as usize;
    let face_amount = header[4] as usize;
    let total_floats = vertices_amount * 3 * 2;

    let (faces, rest) = rest.split_at_mut(face_amount * 3);
    let (positions_slice, rest) = rest.split_at_mut(vertices_amount * 3);
    let (normals_slice, rest) = rest.split_at_mut(vertices_amount * 3);
    let (colors_slice, _) = rest.split_at_mut(if colored { vertices_amount } else { 0 });

    assert!(
        positions_slice.len() + normals_slice.len() == total_floats,
        "Slices do not match the expected sizes."
    );

    let positions: &mut [f32] = unsafe {
        std::slice::from_raw_parts_mut(
            positions_slice.as_mut_ptr() as *mut f32,
            positions_slice.len(),
        )
    };
    let normals: &mut [f32] = unsafe {
        std::slice::from_raw_parts_mut(normals_slice.as_mut_ptr() as *mut f32, normals_slice.len())
    };
    let colors: &mut [u32] = unsafe {
        std::slice::from_raw_parts_mut(colors_slice.as_mut_ptr() as *mut u32, colors_slice.len())
    };

    GeometryData {
        positions,
        normals,
        faces,
        colors,
    }
}

/// A copy of an encoded geometry (brackets included) with the given color per vertex, colors it
/// had are replaced.
pub fn with_geometry_colors(geometry: &[i32], colors: &[u32]) -> Vec<i32> {
    let (v, f) = (geometry[3] as usize, geometry[4] as usize);
    assert_eq!(colors.len(), v, "one color per vertex");
    let plain = 2 + GEOMETRY_HEADER_SIZE + f * 3 + v * 6;
    let mut out = Vec::with_capacity(colored_geometry_data_len(v, f));
    out.extend_from_slice(&geometry[..plain]);
    out[1] = (colored_geometry_data_len(v, f) - 3) as i32;
    out.extend(colors.iter().map(|&c| c as i32));
    out.extend_from_slice(&[1, 1]);
    out
}

/// Where every vertex of a geometry sits: along from its base (the origin, where an instance
/// places it) to the vertex farthest from it, and across from its middle (x = 0) to its edge at
/// the same place along. A leaf grows along -z and its width lies along x, the distance along
/// leaves x out, so it grows toward the tip however far the leaf curls up or down.
pub fn vertex_places(positions: &[f32]) -> Vec<(f32, f32)> {
    let point = |i: usize| {
        let (x, y, z) = (positions[i * 3], positions[i * 3 + 1], positions[i * 3 + 2]);
        (x.abs(), y.hypot(z))
    };
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{concat_args, wrap_arg};

    fn geometry(vertices: i32) -> Vec<i32> {
        let mut data = vec![1, vertices, 1, NO_PATH, 0, 1, 2];
        data.extend(vec![0; vertices as usize * 6]);
        wrap_arg(&data)
    }

    #[test]
    fn a_single_geometry_is_one_item() {
        let geometry = geometry(3);
        assert_eq!(split_items(&geometry), vec![geometry.as_slice()]);
    }

    #[test]
    fn a_list_is_split_into_its_items() {
        let a = geometry(3);
        let b = geometry(4);
        let list = concat_args(vec![&a, &b]);
        assert_eq!(split_items(&list), vec![a.as_slice(), b.as_slice()]);
    }

    #[test]
    fn created_geometry_can_be_split_back_out_of_a_list() {
        let a = create_geometry_data(3, 1);
        let b = create_geometry_data(4, 2);
        assert_eq!(a[1], wrap_arg(&a[2..a.len() - 2])[1]);

        let list = concat_args(vec![&a, &b, &a]);
        assert_eq!(
            crate::split_args(&list),
            vec![a.as_slice(), b.as_slice(), a.as_slice()]
        );
    }

    #[test]
    fn an_unconnected_input_has_no_items() {
        assert!(split_items(&[0, 1, 1, 1]).is_empty());
    }

    #[test]
    fn a_leaf_that_curls_far_up_still_ends_at_its_tip() {
        // bends from -z up past +y, by 144 degrees
        let positions: Vec<f32> = (0..5)
            .flat_map(|i| {
                let a = i as f32 / 4.0 * 2.5;
                [0.0, 1.0 - a.cos(), -a.sin()]
            })
            .collect();
        let along: Vec<f32> = vertex_places(&positions).iter().map(|p| p.0).collect();
        assert!(along.windows(2).all(|w| w[1] > w[0]), "{:?}", along);
        assert_eq!(along[4], 1.0);
    }
}
