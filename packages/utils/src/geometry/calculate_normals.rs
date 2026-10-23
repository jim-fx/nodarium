use crate::{decode_float, encode_float};
use glam::Vec3;

pub fn calculate_normals(geometry: &mut [i32]) -> &mut [i32] {
    let vertex_count = geometry[1] as usize;
    let face_count = geometry[2] as usize;
    // type, vertex amount, face amount and the id of the path
    let index_start = 4;
    let indices_end = index_start + face_count * 3;
    let positions_start = indices_end;
    let positions_end = positions_start + vertex_count * 3;
    let normals_start = positions_end;

    let mut last_normal = Vec3::ZERO;

    for i in 0..face_count {
        let base_idx1 = geometry[index_start + i * 3] as usize * 3;
        let base_idx2 = geometry[index_start + i * 3 + 1] as usize * 3;
        let base_idx3 = geometry[index_start + i * 3 + 2] as usize * 3;

        let v1 = Vec3::new(
            decode_float(geometry[positions_start + base_idx1]),
            decode_float(geometry[positions_start + base_idx1 + 1]),
            decode_float(geometry[positions_start + base_idx1 + 2]),
        );
        let v2 = Vec3::new(
            decode_float(geometry[positions_start + base_idx2]) - v1.x,
            decode_float(geometry[positions_start + base_idx2 + 1]) - v1.y,
            decode_float(geometry[positions_start + base_idx2 + 2]) - v1.z,
        );
        let v3 = Vec3::new(
            decode_float(geometry[positions_start + base_idx3]) - v1.x,
            decode_float(geometry[positions_start + base_idx3 + 1]) - v1.y,
            decode_float(geometry[positions_start + base_idx3 + 2]) - v1.z,
        );

        let mut normal = v2.cross(v3).normalize();
        if normal.length_squared() == 0.0 {
            normal = last_normal;
        } else {
            last_normal = normal;
        }

        for j in 0..3 {
            let idx = geometry[index_start + i * 3 + j] as usize * 3;
            geometry[normals_start + idx] = encode_float(normal.x);
            geometry[normals_start + idx + 1] = encode_float(normal.y);
            geometry[normals_start + idx + 2] = encode_float(normal.z);
        }
    }
    geometry
}
