use super::{GeometryData, NO_PATH};

pub struct InstanceData<'a> {
    pub instances: &'a mut [f32],
    /// For every instance the id of the path it sits on, `NO_PATH` for none.
    pub path_ids: &'a mut [i32],
    pub instance_amount: usize,
    pub positions: &'a mut [f32],
    pub normals: &'a mut [f32],
    pub faces: &'a mut [i32],
    /// An RGBA8 color per vertex of the geometry, empty for a geometry without colors
    pub colors: &'a mut [u32],
}

impl InstanceData<'_> {
    pub fn set_geometry(&mut self, geometry: GeometryData) {
        assert_eq!(self.positions.len(), geometry.positions.len());

        for i in 0..self.positions.len() {
            self.positions[i] = geometry.positions[i];
        }
        for i in 0..self.normals.len() {
            self.normals[i] = geometry.normals[i];
        }
        for i in 0..self.faces.len() {
            self.faces[i] = geometry.faces[i];
        }
        if self.colors.len() == geometry.colors.len() {
            self.colors.copy_from_slice(geometry.colors);
        }
    }

    pub fn set_path_id(&mut self, index: usize, path_id: i32) {
        assert!(
            index < self.path_ids.len(),
            "Instance out of bounds. index: {} len: {}",
            index,
            self.path_ids.len()
        );
        self.path_ids[index] = path_id;
    }

    pub fn set_transformation_matrix(&mut self, index: usize, matrix: &[f32; 16]) {
        assert_eq!(matrix.len(), 16, "Matrix length must be 16.");
        assert!(
            index * 16 + 16 <= self.instances.len(),
            "Matrix does not fit. index: {} len: {}",
            index,
            self.instances.len()
        );

        (0..16).for_each(|i| {
            self.instances[index * 16 + i] = matrix[i];
        });
    }
}

static INSTANCE_HEADER_SIZE: usize = 5;
// 0: type instance = 2;
// 1: vertex amount
// 2: face amount
// 3: instance amount
// 4: stem_depth
// followed by faces, positions, normals, a 4x4 matrix per instance, the id of the path each
// instance sits on and, for a geometry with colors, an RGBA8 color per vertex

fn instance_data_len(vertex_amount: usize, face_amount: usize, instance_amount: usize) -> usize {
    INSTANCE_HEADER_SIZE // type, vertex_amount, face_amount, instance_amount, stem_depth
      + 4 // opening and closing brackets
      + instance_amount * 16  // instances stored as 4x4 matrices
      + instance_amount // the id of the path each instance sits on
      + vertex_amount * 3  // positions
      + vertex_amount * 3  // normals
      + face_amount * 3 // faces
}

pub fn create_instance_data(
    vertex_amount: usize,
    face_amount: usize,
    instance_amount: usize,
) -> Vec<i32> {
    create_instances(vertex_amount, face_amount, instance_amount, false)
}

/// Like `create_instance_data`, with room for a color per vertex of the geometry.
pub fn create_colored_instance_data(
    vertex_amount: usize,
    face_amount: usize,
    instance_amount: usize,
) -> Vec<i32> {
    create_instances(vertex_amount, face_amount, instance_amount, true)
}

fn create_instances(
    vertex_amount: usize,
    face_amount: usize,
    instance_amount: usize,
    colored: bool,
) -> Vec<i32> {
    let colors = if colored { vertex_amount } else { 0 };
    let amount = instance_data_len(vertex_amount, face_amount, instance_amount) + colors;

    let mut geo = vec![0; amount];

    geo[0] = 0; // opening bracket
    geo[1] = amount as i32 - 3;
    geo[2] = 2; // type: instance
    geo[3] = vertex_amount as i32;
    geo[4] = face_amount as i32;
    geo[5] = instance_amount as i32;
    geo[6] = 1; // stem_depth
    // no instance sits on a path until it is set
    let path_ids = amount - 2 - colors - instance_amount;
    geo[path_ids..path_ids + instance_amount].fill(NO_PATH);
    geo[amount - 2 - colors..amount - 2].fill(crate::NO_COLOR as i32);

    geo[amount - 2] = 1; // closing bracket
    geo[amount - 1] = 1; // closing bracket

    geo
}

/// Which of `count` geometries an instance places, the `pick` of `instance` and
/// `instance_from_points`. By index, `value` 0 is the first geometry and 1 the last, they share
/// the range evenly. At random, `hash` picks and `value` is the bias: above 0 the later
/// geometries are picked more often, below 0 the earlier ones.
pub fn pick_geometry(by_index: bool, value: f32, hash: i32, count: usize) -> usize {
    let index = |v: f32| ((v.clamp(0.0, 1.0) * count as f32) as usize).min(count - 1);
    if by_index {
        return index(value);
    }
    if value == 0.0 {
        return hash as usize % count;
    }
    let x = hash as f32 / 2_147_483_648.0;
    index(x.powf(4.0_f32.powf(-value.clamp(-1.0, 1.0))))
}

/// Instances of `geometry` (brackets included), one per transform, each on the path of its id.
pub fn encode_instances(geometry: &[i32], transforms: &[[f32; 16]], path_ids: &[i32]) -> Vec<i32> {
    let mut geo_data = geometry.to_vec();
    let geo = super::wrap_geometry_data(&mut geo_data);
    let create = if geo.colors.is_empty() { create_instance_data } else { create_colored_instance_data };
    let mut data = create(geo.positions.len() / 3, geo.faces.len() / 3, transforms.len());
    let mut instances = wrap_instance_data(&mut data);
    instances.set_geometry(geo);
    for (i, (transform, path_id)) in transforms.iter().zip(path_ids).enumerate() {
        instances.set_transformation_matrix(i, transform);
        instances.set_path_id(i, *path_id);
    }
    data
}

/// Whether encoded instances (brackets included) bring a color per vertex of their geometry
pub fn has_instance_colors(instances: &[i32]) -> bool {
    let (v, f, n) = (instances[3] as usize, instances[4] as usize, instances[5] as usize);
    v > 0 && instances.len() == instance_data_len(v, f, n) + v
}

pub fn wrap_instance_data(instances: &mut [i32]) -> InstanceData<'_> {
    assert!(
        instances.len() > INSTANCE_HEADER_SIZE,
        "Instance vector does not contain enough data for a header."
    );

    let colored = has_instance_colors(instances);
    let (header, rest) = instances.split_at_mut(2 + INSTANCE_HEADER_SIZE);

    let vertices_amount = header[3] as usize;
    let face_amount = header[4] as usize;
    let instance_amount = header[5] as usize;

    let (faces, rest) = rest.split_at_mut(face_amount * 3);
    let (positions_slice, rest) = rest.split_at_mut(vertices_amount * 3);
    let (normals_slice, rest) = rest.split_at_mut(vertices_amount * 3);
    let (instances_slice, rest) = rest.split_at_mut(instance_amount * 16);
    let (path_ids, rest) = rest.split_at_mut(instance_amount);
    let (colors_slice, _) = rest.split_at_mut(if colored { vertices_amount } else { 0 });

    let positions: &mut [f32] = unsafe {
        std::slice::from_raw_parts_mut(
            positions_slice.as_mut_ptr() as *mut f32,
            positions_slice.len(),
        )
    };

    let normals: &mut [f32] = unsafe {
        std::slice::from_raw_parts_mut(normals_slice.as_mut_ptr() as *mut f32, normals_slice.len())
    };

    let instances: &mut [f32] = unsafe {
        std::slice::from_raw_parts_mut(
            instances_slice.as_mut_ptr() as *mut f32,
            instances_slice.len(),
        )
    };

    let colors: &mut [u32] = unsafe {
        std::slice::from_raw_parts_mut(colors_slice.as_mut_ptr() as *mut u32, colors_slice.len())
    };

    InstanceData {
        instances,
        path_ids,
        instance_amount,
        positions,
        normals,
        faces,
        colors,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{
        append_colored_geometry_data, create_geometry_data, has_geometry_colors,
        wrap_geometry_data,
    };

    #[test]
    fn instances_without_colors_stay_as_they_were() {
        let data = create_instance_data(3, 1, 2);
        assert_eq!(data.len(), 2 + 5 + 3 + 18 + 32 + 2 + 2);
        assert!(!has_instance_colors(&data));
    }

    #[test]
    fn colors_of_the_geometry_follow_the_path_ids() {
        let mut geometry = Vec::new();
        append_colored_geometry_data(&mut geometry, 3, 1);
        assert!(has_geometry_colors(&geometry));
        assert!(!has_geometry_colors(&create_geometry_data(3, 1)));
        wrap_geometry_data(&mut geometry).colors.copy_from_slice(&[1, 2, 3]);

        let mut data = create_colored_instance_data(3, 1, 2);
        assert!(has_instance_colors(&data));
        let mut instances = wrap_instance_data(&mut data);
        assert_eq!(instances.colors, &[crate::NO_COLOR; 3]);
        assert_eq!(instances.path_ids, &[NO_PATH; 2]);
        instances.set_geometry(wrap_geometry_data(&mut geometry));
        instances.set_path_id(1, 9);
        assert_eq!(instances.colors, &[1, 2, 3]);
        assert_eq!(&data[data.len() - 7..], &[NO_PATH, 9, 1, 2, 3, 1, 1]);
    }
}
