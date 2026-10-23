use super::{GeometryData, NO_PATH};

pub struct InstanceData<'a> {
    pub instances: &'a mut [f32], // View into `data`
    /// For every instance the id of the path it sits on, `NO_PATH` for none.
    pub path_ids: &'a mut [i32], // View into `data`
    pub instance_amount: usize,
    pub positions: &'a mut [f32], // View into `data`
    pub normals: &'a mut [f32],   // View into `data`
    pub faces: &'a mut [i32],     // View into `data`
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
// 3: stem_depth
// 4: instance amount
// followed by faces, positions, normals, a 4x4 matrix per instance and the id of the path each
// instance sits on

pub fn create_instance_data(
    vertex_amount: usize,
    face_amount: usize,
    instance_amount: usize,
) -> Vec<i32> {
    let amount = INSTANCE_HEADER_SIZE // definition (type, vertex_amount, face_amount, instance_amount)
      + 4 // opening and closing brackets
      + instance_amount * 16  // instances stored as 4x4 matrices
      + instance_amount // the id of the path each instance sits on
      + vertex_amount * 3  // positions
      + vertex_amount * 3  // normals
      + face_amount * 3; // faces

    let mut geo = vec![0; amount];

    geo[0] = 0; // opening bracket
    geo[1] = amount as i32 - 3; // opening bracket
    geo[2] = 2; // type: instance
    geo[3] = vertex_amount as i32;
    geo[4] = face_amount as i32;
    geo[5] = instance_amount as i32;
    geo[6] = 1; // stem_depth
    // no instance sits on a path until it is set
    geo[amount - 2 - instance_amount..amount - 2].fill(NO_PATH);

    geo[amount - 2] = 1; // closing bracket
    geo[amount - 1] = 1; // closing bracket

    geo
}

pub fn wrap_instance_data(instances: &mut [i32]) -> InstanceData<'_> {
    assert!(
        instances.len() > INSTANCE_HEADER_SIZE,
        "Instance vector does not contain enough data for a header."
    );

    let (header, rest) = instances.split_at_mut(2 + INSTANCE_HEADER_SIZE);

    let vertices_amount = header[3] as usize;
    let face_amount = header[4] as usize;
    let instance_amount = header[5] as usize;

    let (faces, rest) = rest.split_at_mut(face_amount * 3);
    let (positions_slice, rest) = rest.split_at_mut(vertices_amount * 3);
    let (normals_slice, rest) = rest.split_at_mut(vertices_amount * 3);
    let (instances_slice, rest) = rest.split_at_mut(instance_amount * 16);
    let (path_ids, _) = rest.split_at_mut(instance_amount);

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

    InstanceData {
        instances,
        path_ids,
        instance_amount,
        positions,
        normals,
        faces,
    }
}
