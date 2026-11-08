use nodarium_utils::{decode_float, encode_float};

pub const MESH_TAG: i32 = 14;
const HEADER_LEN: usize = 4;

/// Bits of the flags word in the mesh header
pub mod mesh_flags {
    /// One RGBA8 color per vertex follows the normals
    pub const COLORS: i32 = 1;
    /// The groups below [`crate::TISSUE_COUNT`] are tissues of grown hyphae
    pub const TISSUE_GROUPS: i32 = 2;
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
    /// See [`mesh_flags::TISSUE_GROUPS`]
    pub tissue_groups: bool,
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
        let flags = if self.colors.is_empty() { 0 } else { mesh_flags::COLORS }
            | if self.tissue_groups { mesh_flags::TISSUE_GROUPS } else { 0 };
        let mut words = Vec::with_capacity(encoded_len(v, f, flags));
        words.extend([MESH_TAG, v as i32, f as i32, flags]);
        words.extend(self.faces.iter().map(|&i| i as i32));
        words.extend(self.positions.iter().map(|&p| encode_float(p)));
        words.extend(self.normals.iter().map(|&n| encode_float(n)));
        words.extend(self.colors.iter().map(|&c| c as i32));
        words.extend(self.groups.iter().map(|&g| g as i32));
        words
    }

    /// Adds another mesh to this one, its groups numbered from `first_group` on. Colors only
    /// stay when both meshes have them.
    pub fn append(&mut self, other: &Mesh, first_group: u32) {
        let offset = self.vertex_count() as u32;
        let keep_colors = (!self.colors.is_empty() || self.vertex_count() == 0) && !other.colors.is_empty();
        if keep_colors {
            self.colors.extend(&other.colors);
        } else {
            self.colors.clear();
        }
        self.faces.extend(other.faces.iter().map(|&i| i + offset));
        self.positions.extend(&other.positions);
        self.normals.extend(&other.normals);
        self.groups.extend(other.groups.iter().map(|&g| g + first_group));
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
        Self::decode_unchecked(words)
    }

    /// The mesh when `words` are one, `None` for anything else: counts that don't match the
    /// length, unknown flags or a face with a vertex that isn't there
    pub fn try_decode(words: &[i32]) -> Option<Mesh> {
        if words.len() < HEADER_LEN || words[0] != MESH_TAG {
            return None;
        }
        let (v, f, flags) = (usize::try_from(words[1]).ok()?, usize::try_from(words[2]).ok()?, words[3]);
        if flags & !(mesh_flags::COLORS | mesh_flags::TISSUE_GROUPS) != 0 {
            return None;
        }
        // in u64, so counts up to i32::MAX can't overflow on wasm32
        let per_vertex = 7 + u64::from(flags & mesh_flags::COLORS != 0);
        if words.len() as u64 != HEADER_LEN as u64 + f as u64 * 3 + v as u64 * per_vertex {
            return None;
        }
        if words[HEADER_LEN..HEADER_LEN + f * 3].iter().any(|&i| i < 0 || i as usize >= v) {
            return None;
        }
        Some(Self::decode_unchecked(words))
    }

    fn decode_unchecked(words: &[i32]) -> Mesh {
        let (v, f, flags) = (words[1] as usize, words[2] as usize, words[3]);
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
        Mesh { faces, positions, normals, colors, groups, tissue_groups: flags & mesh_flags::TISSUE_GROUPS != 0 }
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
            tissue_groups: false,
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
    fn appends_a_mesh_with_its_groups_after_the_others() {
        let mut mesh = Mesh { colors: Vec::new(), ..triangle() };
        mesh.append(&triangle(), 3);
        assert_eq!(mesh.vertex_count(), 6);
        assert_eq!(mesh.faces, vec![0, 1, 2, 3, 4, 5]);
        assert_eq!(mesh.groups, vec![0, 2, 2, 3, 5, 5]);
        // one of them has no colors, so the result has none
        assert!(mesh.colors.is_empty());
        let mut empty = Mesh::default();
        empty.append(&triangle(), 0);
        assert_eq!(empty, triangle());
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
    fn tells_tissue_groups_apart() {
        let mesh = Mesh { tissue_groups: true, ..triangle() };
        let words = mesh.encode();
        assert_eq!(words[3], mesh_flags::COLORS | mesh_flags::TISSUE_GROUPS);
        assert_eq!(Mesh::decode(&words), mesh);
    }

    #[test]
    fn packs_red_into_the_lowest_byte() {
        assert_eq!(pack_rgba(1.0, 0.0, 0.0, 0.0), 0xff);
        assert_eq!(pack_rgba(0.0, 0.0, 0.0, 1.0), 0xff00_0000);
    }

    #[test]
    fn try_decode_takes_only_whole_meshes() {
        let words = triangle().encode();
        assert_eq!(Mesh::try_decode(&words), Some(triangle()));
        assert_eq!(Mesh::try_decode(&Mesh::default().encode()), Some(Mesh::default()));
        assert_eq!(Mesh::try_decode(&[]), None);
        assert_eq!(Mesh::try_decode(&[MESH_TAG]), None);
        assert_eq!(Mesh::try_decode(&words[..words.len() - 1]), None);
        assert_eq!(Mesh::try_decode(&[words.clone(), vec![0]].concat()), None);
        // a 14 in front of other words, like the index of a path
        assert_eq!(Mesh::try_decode(&[MESH_TAG, 2, 0, 0, 5, 9]), None);
        assert_eq!(Mesh::try_decode(&[MESH_TAG, -1, 0, 0]), None);
        assert_eq!(Mesh::try_decode(&[MESH_TAG, i32::MAX, i32::MAX, 0]), None);
        let mut unknown_flag = words.clone();
        unknown_flag[3] |= 8;
        assert_eq!(Mesh::try_decode(&unknown_flag), None);
        let mut missing_vertex = words.clone();
        missing_vertex[HEADER_LEN + 2] = 3;
        assert_eq!(Mesh::try_decode(&missing_vertex), None);
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
