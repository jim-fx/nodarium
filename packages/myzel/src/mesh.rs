use nodarium_utils::{decode_float, encode_float};

pub const MESH_TAG: i32 = 14;
const HEADER_LEN: usize = 4;

/// Bits of the flags word in the mesh header
pub mod mesh_flags {
    /// One RGBA8 color per vertex follows the normals
    pub const COLORS: i32 = 1;
}

/// Triangle mesh with a material group and optionally a color per vertex, see the crate docs for the format
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesh {
    pub faces: Vec<u32>,
    pub positions: Vec<f32>,
    pub normals: Vec<f32>,
    /// Empty or one color per vertex
    pub colors: Vec<u32>,
    /// One group per vertex, other programs can give each group its own material
    pub groups: Vec<u32>,
}

/// Packs a color with components in [0, 1] into one RGBA8 word, red in the lowest byte
pub fn pack_rgba(r: f32, g: f32, b: f32, a: f32) -> u32 {
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
    byte(r) | byte(g) << 8 | byte(b) << 16 | byte(a) << 24
}

fn encoded_len(v: usize, f: usize, flags: i32) -> usize {
    let colors = if flags & mesh_flags::COLORS != 0 { v } else { 0 };
    HEADER_LEN + f * 3 + v * 7 + colors
}

impl Mesh {
    pub fn vertex_count(&self) -> usize {
        self.positions.len() / 3
    }

    pub fn face_count(&self) -> usize {
        self.faces.len() / 3
    }

    pub fn encode(&self) -> Vec<i32> {
        let (v, f) = (self.vertex_count(), self.face_count());
        assert!(self.colors.is_empty() || self.colors.len() == v, "{} colors for {} vertices", self.colors.len(), v);
        assert_eq!(self.groups.len(), v, "every vertex needs a group");
        let flags = if self.colors.is_empty() { 0 } else { mesh_flags::COLORS };
        let mut words = Vec::with_capacity(encoded_len(v, f, flags));
        words.extend([MESH_TAG, v as i32, f as i32, flags]);
        words.extend(self.faces.iter().map(|&i| i as i32));
        words.extend(self.positions.iter().map(|&p| encode_float(p)));
        words.extend(self.normals.iter().map(|&n| encode_float(n)));
        words.extend(self.colors.iter().map(|&c| c as i32));
        words.extend(self.groups.iter().map(|&g| g as i32));
        words
    }

    pub fn decode(words: &[i32]) -> Mesh {
        assert!(
            words.len() >= HEADER_LEN && words[0] == MESH_TAG,
            "expected a mesh [14, v, f, flags, ...], got {} words starting with {:?}",
            words.len(),
            words.first()
        );
        let (v, f, flags) = (words[1] as usize, words[2] as usize, words[3]);
        let len = encoded_len(v, f, flags);
        assert!(
            words.len() == len,
            "mesh with {} vertices, {} faces and flags {} needs {} words, got {}",
            v,
            f,
            flags,
            len,
            words.len()
        );
        let mut rest = &words[HEADER_LEN..];
        let mut take = |n: usize| {
            let (head, tail) = rest.split_at(n);
            rest = tail;
            head
        };
        let faces = take(f * 3).iter().map(|&i| i as u32).collect();
        let positions = take(v * 3).iter().map(|&w| decode_float(w)).collect();
        let normals = take(v * 3).iter().map(|&w| decode_float(w)).collect();
        let colors = if flags & mesh_flags::COLORS != 0 { take(v).iter().map(|&c| c as u32).collect() } else { Vec::new() };
        let groups = take(v).iter().map(|&g| g as u32).collect();
        Mesh { faces, positions, normals, colors, groups }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn triangle() -> Mesh {
        Mesh {
            faces: vec![0, 1, 2],
            positions: vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            normals: vec![0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0],
            colors: vec![pack_rgba(1.0, 0.0, 0.0, 1.0); 3],
            groups: vec![0, 2, 2],
        }
    }

    #[test]
    fn encodes_and_decodes() {
        let mesh = triangle();
        let words = mesh.encode();
        assert_eq!(&words[..4], &[MESH_TAG, 3, 1, mesh_flags::COLORS]);
        assert_eq!(&words[words.len() - 3..], &[0, 2, 2]);
        assert_eq!(Mesh::decode(&words), mesh);
    }

    #[test]
    fn colors_are_optional() {
        let mesh = Mesh { colors: Vec::new(), ..triangle() };
        let words = mesh.encode();
        assert_eq!(&words[..4], &[MESH_TAG, 3, 1, 0]);
        assert_eq!(words.len(), 4 + 3 + 3 * 7);
        assert_eq!(Mesh::decode(&words), mesh);
    }

    #[test]
    fn packs_red_into_the_lowest_byte() {
        assert_eq!(pack_rgba(1.0, 0.0, 0.0, 0.0), 0xff);
        assert_eq!(pack_rgba(0.0, 0.0, 0.0, 1.0), 0xff00_0000);
    }

    #[test]
    #[should_panic(expected = "needs")]
    fn rejects_truncated_meshes() {
        Mesh::decode(&[MESH_TAG, 3, 1, 0, 0, 1]);
    }

    #[test]
    #[should_panic(expected = "group")]
    fn every_vertex_needs_a_group() {
        Mesh { groups: Vec::new(), ..triangle() }.encode();
    }
}
