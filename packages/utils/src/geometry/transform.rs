use glam::{Mat3, Mat4, Vec3};

/// Whether a transform scales everything to nothing, like an instance of size 0, or flattens
/// it. What it places has no area and no normals.
pub fn is_collapsed(transform: &Mat4) -> bool {
    let axes = Mat3::from_mat4(*transform);
    // the determinant grows with the cube of the size, on its own it would take an instance of
    // 0.1 mm for one of size 0
    let lengths = axes.x_axis.length() * axes.y_axis.length() * axes.z_axis.length();
    axes.determinant().abs() <= lengths * 1e-6
}

pub fn transform_geometry(mut geometry: Vec<i32>, matrix: Mat4) -> Vec<i32> {
    let (_header, rest) = geometry.split_at_mut(super::GEOMETRY_HEAD);

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

#[cfg(test)]
mod tests {
    use super::is_collapsed;
    use glam::{Mat4, Quat, Vec3};

    fn scaled(size: f32) -> Mat4 {
        Mat4::from_scale_rotation_translation(
            Vec3::splat(size),
            Quat::from_rotation_y(0.7),
            Vec3::new(3.0, 1.0, -2.0),
        )
    }

    #[test]
    fn a_tiny_instance_is_not_collapsed() {
        for size in [1.0, 1e-2, 1e-4, 1e-6, 1e-8] {
            assert!(!is_collapsed(&scaled(size)), "size {size}");
        }
    }

    #[test]
    fn a_huge_instance_is_not_collapsed() {
        assert!(!is_collapsed(&scaled(1e6)));
    }

    #[test]
    fn an_instance_of_size_0_is_collapsed() {
        assert!(is_collapsed(&scaled(0.0)));
    }

    #[test]
    fn a_flattened_instance_is_collapsed() {
        let flat = Mat4::from_scale(Vec3::new(1e-3, 0.0, 1e-3));
        assert!(is_collapsed(&flat));
        let sheared = Mat4::from_cols(
            Vec3::X.extend(0.0),
            Vec3::Y.extend(0.0),
            (Vec3::X + Vec3::Y).extend(0.0),
            Vec3::ZERO.extend(1.0),
        );
        assert!(is_collapsed(&sheared));
    }
}
