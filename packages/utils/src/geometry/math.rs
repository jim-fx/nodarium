use glam::{Quat, Vec3};

pub fn rotate_vector_by_angle(vector: Vec3, axis: Vec3, angle_radians: f32) -> Vec3 {
    let normalized_axis = axis.normalize();

    let rotation_quat = Quat::from_axis_angle(normalized_axis, angle_radians);

    rotation_quat.mul_vec3(vector)
}
