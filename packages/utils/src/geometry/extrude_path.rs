use super::{create_geometry_data, wrap_geometry_data, PathData};
use glam::{Mat4, Vec3};

fn create_circle(res: usize) -> Vec<f32> {
    let angle = (2.0 * std::f32::consts::PI) / res as f32;
    let mut circle = Vec::new();
    for i in 0..res {
        circle.push((angle * i as f32).cos());
        circle.push((angle * i as f32).sin());
    }
    circle
}

pub fn extrude_path(input_path: PathData, res_x: usize) -> Vec<i32> {
    let point_amount = input_path.points.len() / 4;

    // a single point has no direction and nothing to extrude
    if point_amount < 2 || res_x == 0 {
        return create_geometry_data(0, 0);
    }

    // the tube itself, a duplicated ring and an apex vertex that close the end
    let tube_vertices = point_amount * res_x;
    let cap_ring = tube_vertices;
    let cap_center = tube_vertices + res_x;
    let vertices_amount = tube_vertices + res_x + 1;

    let tube_faces = (point_amount - 1) * res_x * 2;
    let face_amount = tube_faces + res_x;

    let circle = create_circle(res_x);

    let mut geometry_data = create_geometry_data(vertices_amount, face_amount);

    let geometry = wrap_geometry_data(&mut geometry_data);

    let normals = geometry.normals;
    let positions = geometry.positions;
    let indices = geometry.faces;

    let path: &[f32] = input_path.points;

    // a segment without length has no direction, it keeps the one before it
    let mut last_dir = Vec3::NEG_Y;

    for i in 0..input_path.length {
        let index_offset = i * res_x * 6;
        let position_offset = i * res_x;

        let pos = Vec3::new(path[i * 4], path[i * 4 + 1], path[i * 4 + 2]);
        let thickness = path[i * 4 + 3].max(0.000001);

        // Get direction of the current segment
        let segment_dir = (if i == 0 {
            Vec3::new(
                pos[0] - path[i * 4 + 4],
                pos[1] - path[i * 4 + 5],
                pos[2] - path[i * 4 + 6],
            )
        } else if i == point_amount - 1 {
            Vec3::new(
                path[i * 4 - 4] - pos[0],
                path[i * 4 - 3] - pos[1],
                path[i * 4 - 2] - pos[2],
            )
        } else {
            Vec3::new(
                path[i * 4 - 4] - path[i * 4 + 4],
                path[i * 4 - 3] - path[i * 4 + 5],
                path[i * 4 - 2] - path[i * 4 + 6],
            )
        })
        .try_normalize()
        .unwrap_or(last_dir);
        last_dir = segment_dir;

        // In our case the up vector is always the Y axis
        let up_vector = Vec3::NEG_Y;

        let binormal = up_vector.cross(segment_dir);

        let rotation_angle = up_vector.dot(segment_dir).clamp(-1.0, 1.0).acos();

        let rotation_matrix = Mat4::from_axis_angle(binormal, rotation_angle);

        for j in 0..res_x {
            if i < point_amount - 1 {
                let i_index_offset = index_offset + j * 6;
                let i_position_offset = position_offset + j;

                //log!("i: {}, j: {}, i_index_offset: {}, i_position_offset: {} res_x: {}", i, j, i_index_offset, i_position_offset,res_x);

                if j == res_x - 1 {
                    indices[i_index_offset] = (i_position_offset + 1) as i32;
                    indices[i_index_offset + 1] = (i_position_offset + 1 - res_x) as i32;
                    indices[i_index_offset + 2] = (i_position_offset) as i32;

                    indices[i_index_offset + 3] = (i_position_offset) as i32;
                    indices[i_index_offset + 4] = (i_position_offset + res_x) as i32;
                    indices[i_index_offset + 5] = (i_position_offset + 1) as i32;
                } else {
                    indices[i_index_offset] = (i_position_offset + res_x + 1) as i32;
                    indices[i_index_offset + 1] = (i_position_offset + 1) as i32;
                    indices[i_index_offset + 2] = (i_position_offset) as i32;

                    indices[i_index_offset + 3] = (i_position_offset) as i32;
                    indices[i_index_offset + 4] = (i_position_offset + res_x) as i32;
                    indices[i_index_offset + 5] = (i_position_offset + res_x + 1) as i32;
                }
            }

            // construct the points
            let idx = i * res_x * 3 + j * 3;

            let circle_x = circle[j * 2] * thickness;
            let circle_z = circle[j * 2 + 1] * thickness;

            let point = rotation_matrix.transform_point3(Vec3::new(circle_x, 0.0, circle_z)) + pos;
            let normal = rotation_matrix
                .transform_vector3(Vec3::new(circle_x, 0.0, circle_z))
                .normalize();

            normals[idx] = normal[0];
            normals[idx + 1] = normal[1];
            normals[idx + 2] = normal[2];

            positions[idx] = point[0];
            positions[idx + 1] = point[1];
            positions[idx + 2] = point[2];
        }
    }

    // `last_dir` points back towards the root, the cap faces the other way
    let cap_dir = (-last_dir).try_normalize().unwrap_or(Vec3::Y);

    // the cap gets its own vertices, sharing the last ring would smooth its
    // flat normal into the radial normals of the tube
    let last_ring = (point_amount - 1) * res_x;
    for j in 0..res_x {
        let src = (last_ring + j) * 3;
        let idx = (cap_ring + j) * 3;

        positions[idx] = positions[src];
        positions[idx + 1] = positions[src + 1];
        positions[idx + 2] = positions[src + 2];

        normals[idx] = cap_dir[0];
        normals[idx + 1] = cap_dir[1];
        normals[idx + 2] = cap_dir[2];
    }

    let last_point = (point_amount - 1) * 4;
    let apex = Vec3::new(path[last_point], path[last_point + 1], path[last_point + 2]);
    let idx = cap_center * 3;
    positions[idx] = apex[0];
    positions[idx + 1] = apex[1];
    positions[idx + 2] = apex[2];
    normals[idx] = cap_dir[0];
    normals[idx + 1] = cap_dir[1];
    normals[idx + 2] = cap_dir[2];

    // a triangle fan from the apex closes the end, wound so it faces outwards
    let cap_face_offset = tube_faces * 3;
    for j in 0..res_x {
        let i_index_offset = cap_face_offset + j * 3;
        let next = if j == res_x - 1 {
            cap_ring
        } else {
            cap_ring + j + 1
        };

        indices[i_index_offset] = cap_center as i32;
        indices[i_index_offset + 1] = next as i32;
        indices[i_index_offset + 2] = (cap_ring + j) as i32;
    }

    geometry_data
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{create_path, wrap_path, wrap_path_mut};

    fn extrude(points: &[[f32; 4]]) -> Vec<f32> {
        let mut data = create_path(points.len(), 0);
        let path = wrap_path_mut(&mut data);
        for (i, p) in points.iter().enumerate() {
            path.points[i * 4..i * 4 + 4].copy_from_slice(p);
        }
        let mut geometry = extrude_path(wrap_path(&data), 8);
        wrap_geometry_data(&mut geometry).positions.to_vec()
    }

    #[test]
    fn test_extrude_collapsed_path() {
        let positions = extrude(&[
            [1.0, 2.0, 3.0, 0.1],
            [1.0, 2.0, 3.0, 0.1],
            [1.0, 2.0, 3.0, 0.1],
        ]);
        assert!(positions.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn test_extrude_path_with_a_collapsed_segment() {
        let positions = extrude(&[
            [0.0, 0.0, 0.0, 0.1],
            [0.0, 1.0, 0.0, 0.1],
            [0.0, 1.0, 0.0, 0.1],
        ]);
        assert!(positions.iter().all(|v| v.is_finite()));
    }

    fn extrude_raw(points: &[[f32; 4]], res: usize) -> Vec<i32> {
        let mut data = create_path(points.len(), 0);
        let path = wrap_path_mut(&mut data);
        for (i, p) in points.iter().enumerate() {
            path.points[i * 4..i * 4 + 4].copy_from_slice(p);
        }
        extrude_path(wrap_path(&data), res)
    }

    #[test]
    fn test_extrude_closes_the_end() {
        let res = 8;
        let points = [[0.0, 0.0, 0.0, 0.1], [0.0, 1.0, 0.0, 0.1]];
        let mut geometry = extrude_raw(&points, res);

        let data = wrap_geometry_data(&mut geometry);
        let vertex_amount = data.positions.len() / 3;
        let face_amount = data.faces.len() / 3;

        assert_eq!(vertex_amount, points.len() * res + res + 1);
        assert_eq!(face_amount, (points.len() - 1) * res * 2 + res);
        assert!(data.faces.iter().all(|i| (*i as usize) < vertex_amount));

        // the apex sits on the last path point, the cap ring duplicates the last ring
        let cap_center = vertex_amount - 1;
        assert_eq!(
            &data.positions[cap_center * 3..cap_center * 3 + 3],
            &points[1][..3]
        );

        let cap_ring = points.len() * res;
        let last_ring = (points.len() - 1) * res;
        for j in 0..res {
            assert_eq!(
                &data.positions[(cap_ring + j) * 3..(cap_ring + j) * 3 + 3],
                &data.positions[(last_ring + j) * 3..(last_ring + j) * 3 + 3]
            );
            assert!((data.normals[(cap_ring + j) * 3 + 1] - 1.0).abs() < 1e-6);
        }

        // the cap triangles point along the direction of travel (+Y here)
        for face in 0..res {
            let i = ((points.len() - 1) * res * 2 + face) * 3;
            let [a, b, c] = [
                Vec3::from_slice(&data.positions[data.faces[i] as usize * 3..][..3]),
                Vec3::from_slice(&data.positions[data.faces[i + 1] as usize * 3..][..3]),
                Vec3::from_slice(&data.positions[data.faces[i + 2] as usize * 3..][..3]),
            ];
            let normal = (b - a).cross(c - a);
            assert!(normal.y > 0.0, "cap face {face} faces {normal:?}");
        }
    }

    #[test]
    fn test_extrude_a_single_point() {
        let mut geometry = extrude_raw(&[[0.0, 0.0, 0.0, 0.1]], 8);
        let data = wrap_geometry_data(&mut geometry);
        assert!(data.positions.is_empty());
        assert!(data.faces.is_empty());
    }
}
