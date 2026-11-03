use glam::{Mat4, Vec3};

/// Whether a transform scales everything to nothing, like an instance of size 0. What it
/// places has no area and no normals.
pub fn is_collapsed(transform: &Mat4) -> bool {
    transform.determinant().abs() < 1e-12
}

pub fn transform_geometry(mut geometry: Vec<i32>, matrix: Mat4) -> Vec<i32> {
    let (_header, rest) = geometry.split_at_mut(5);

    let vertices_amount = _header[3] as usize;
    let face_amount = _header[4] as usize;

    let (_indices, rest) = rest.split_at_mut(face_amount * 3);
    let (_positions, _normals) = rest.split_at_mut(vertices_amount * 3);
    let positions: &mut [f32];
    unsafe {
        positions =
            std::slice::from_raw_parts_mut(_positions.as_mut_ptr() as *mut f32, _positions.len());
    }

    for i in 0..vertices_amount {
        let pos = Mat4::transform_point3(&matrix, Vec3::from_slice(&positions[i * 3..i * 3 + 3]));
        positions[i * 3] = pos.x;
        positions[i * 3 + 1] = pos.y;
        positions[i * 3 + 2] = pos.z;
    }

    geometry
}
