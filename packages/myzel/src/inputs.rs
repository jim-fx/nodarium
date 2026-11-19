use crate::Mesh;

/// The geometry of an input, the geometries of a list joined into one, see
/// [`crate::geometry_input`]. `None` when it brings none, like an input nothing is connected to.
pub fn read_mesh(arg: &[i32]) -> Option<Mesh> {
    crate::geometry_input(arg).map(|words| Mesh::decode(&words))
}
