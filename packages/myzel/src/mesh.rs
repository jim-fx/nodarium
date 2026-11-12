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
    /// Empty or one color per vertex, [`crate::NO_COLOR`] where the vertex has none of its own
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

    /// Room for exactly `vertices` and `faces` more, without colors. A mesh that grows by
    /// doubling holds up to twice what it needs, and the old and the new buffer while it grows.
    pub fn reserve_exact(&mut self, vertices: usize, faces: usize) {
        self.positions.reserve_exact(vertices * 3);
        self.normals.reserve_exact(vertices * 3);
        self.groups.reserve_exact(vertices);
        self.faces.reserve_exact(faces * 3);
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

    /// Adds another mesh to this one, its groups numbered from `first_group` on. When only one of
    /// them has colors, the vertices of the other get [`crate::NO_COLOR`].
    pub fn append(&mut self, other: &Mesh, first_group: u32) {
        let offset = self.vertex_count() as u32;
        if !self.colors.is_empty() || !other.colors.is_empty() {
            self.colors.resize(self.vertex_count(), crate::NO_COLOR);
            if other.colors.is_empty() {
                self.colors.extend(std::iter::repeat_n(crate::NO_COLOR, other.vertex_count()));
            } else {
                self.colors.extend(&other.colors);
            }
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

    /// The mesh when `words` are one, `None` for anything else, see [`MeshWords::read`]
    pub fn try_decode(words: &[i32]) -> Option<Mesh> {
        MeshWords::read(words).map(|mesh| Self::decode_unchecked(mesh.words))
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
        Mesh { faces, positions, normals, colors, groups }
    }
}

/// A mesh read in place from its words. Nodes that only pass meshes on build their output from
/// these, a decoded [`Mesh`] would be one more copy of every mesh in the node's memory.
#[derive(Clone, Copy, Debug)]
pub struct MeshWords<'a> {
    words: &'a [i32],
    v: usize,
    f: usize,
}

impl<'a> MeshWords<'a> {
    /// The mesh when `words` are one, `None` for anything else: counts that don't match the
    /// length, unknown flags or a face with a vertex that isn't there
    pub fn read(words: &'a [i32]) -> Option<Self> {
        if words.len() < HEADER_LEN || words[0] != MESH_TAG {
            return None;
        }
        let (v, f, flags) = (usize::try_from(words[1]).ok()?, usize::try_from(words[2]).ok()?, words[3]);
        if flags & !mesh_flags::COLORS != 0 {
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
        Some(MeshWords { words, v, f })
    }

    pub fn vertex_count(&self) -> usize {
        self.v
    }

    fn section(&self, start: usize, len: usize) -> &'a [i32] {
        &self.words[HEADER_LEN + start..HEADER_LEN + start + len]
    }

    fn faces(&self) -> &'a [i32] {
        self.section(0, self.f * 3)
    }

    fn positions(&self) -> &'a [i32] {
        self.section(self.f * 3, self.v * 3)
    }

    fn normals(&self) -> &'a [i32] {
        self.section(self.f * 3 + self.v * 3, self.v * 3)
    }

    fn colors(&self) -> Option<&'a [i32]> {
        (self.words[3] & mesh_flags::COLORS != 0).then(|| self.section(self.f * 3 + self.v * 6, self.v))
    }

    fn groups(&self) -> &'a [i32] {
        &self.words[self.words.len() - self.v..]
    }

    /// The encoded mesh with every vertex in `color`
    pub fn painted(&self, color: u32) -> Vec<i32> {
        let flags = if self.v > 0 { mesh_flags::COLORS } else { 0 };
        let mut words = Vec::with_capacity(encoded_len(self.v, self.f, flags));
        words.extend([MESH_TAG, self.v as i32, self.f as i32, flags]);
        words.extend_from_slice(self.faces());
        words.extend_from_slice(self.positions());
        words.extend_from_slice(self.normals());
        words.resize(words.len() + self.v, color as i32);
        words.extend_from_slice(self.groups());
        words
    }

    /// The encoded meshes one after the other, the groups of each numbered after those of the
    /// meshes before it. Meshes without vertices are left out. When only some of them have
    /// colors, the vertices of the others get [`crate::NO_COLOR`].
    pub fn join(meshes: &[MeshWords]) -> Vec<i32> {
        let meshes: Vec<&MeshWords> = meshes.iter().filter(|m| m.v > 0).collect();
        let v = meshes.iter().map(|m| m.v).sum();
        let f = meshes.iter().map(|m| m.f).sum();
        let colored = meshes.iter().any(|m| m.colors().is_some());
        let flags = if colored { mesh_flags::COLORS } else { 0 };
        let mut words = Vec::with_capacity(encoded_len(v, f, flags));
        words.extend([MESH_TAG, v as i32, f as i32, flags]);
        let mut offset = 0u32;
        for mesh in &meshes {
            words.extend(mesh.faces().iter().map(|&i| (i as u32 + offset) as i32));
            offset += mesh.v as u32;
        }
        for mesh in &meshes {
            words.extend_from_slice(mesh.positions());
        }
        for mesh in &meshes {
            words.extend_from_slice(mesh.normals());
        }
        if colored {
            for mesh in &meshes {
                match mesh.colors() {
                    Some(colors) => words.extend_from_slice(colors),
                    None => words.resize(words.len() + mesh.v, crate::NO_COLOR as i32),
                }
            }
        }
        let mut next = 0u32;
        for mesh in &meshes {
            words.extend(mesh.groups().iter().map(|&g| (g as u32 + next) as i32));
            next += mesh.groups().iter().map(|&g| g as u32).max().map_or(0, |g| g + 1);
        }
        words
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
    fn appends_a_mesh_with_its_groups_after_the_others() {
        let mut mesh = Mesh { colors: Vec::new(), ..triangle() };
        mesh.append(&triangle(), 3);
        assert_eq!(mesh.vertex_count(), 6);
        assert_eq!(mesh.faces, vec![0, 1, 2, 3, 4, 5]);
        assert_eq!(mesh.groups, vec![0, 2, 2, 3, 5, 5]);
        // the vertices without colors keep the material
        let red = pack_rgba(1.0, 0.0, 0.0, 1.0);
        assert_eq!(mesh.colors, [[crate::NO_COLOR; 3], [red; 3]].concat());
        let mut plain = Mesh { colors: Vec::new(), ..triangle() };
        plain.append(&Mesh { colors: Vec::new(), ..triangle() }, 0);
        assert!(plain.colors.is_empty());
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

    fn other(colored: bool) -> Mesh {
        Mesh {
            faces: vec![0, 1, 2, 2, 1, 3],
            positions: vec![0.5, 0.0, 0.0, 1.5, 0.0, 0.0, 0.5, 1.0, 0.0, 1.5, 1.0, -0.0],
            normals: vec![0.0, 0.0, -1.0, 0.0, f32::NAN, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0],
            colors: if colored { vec![1, 2, 3, crate::NO_COLOR] } else { Vec::new() },
            groups: vec![1, 0, 4, 1],
        }
    }

    #[test]
    fn joins_the_words_like_appending_the_meshes() {
        let plain = Mesh { colors: Vec::new(), ..triangle() };
        let cases = [
            vec![],
            vec![Mesh::default()],
            vec![triangle()],
            vec![plain.clone(), other(false)],
            vec![other(true), Mesh::default(), plain.clone(), triangle()],
            vec![plain, other(false), other(true)],
        ];
        for meshes in cases {
            let mut appended = Mesh::default();
            let mut next = 0;
            for mesh in meshes.iter().filter(|m| m.vertex_count() > 0) {
                appended.append(mesh, next);
                next += mesh.groups.iter().max().map_or(0, |g| g + 1);
            }
            let encoded: Vec<Vec<i32>> = meshes.iter().map(Mesh::encode).collect();
            let words: Vec<MeshWords> = encoded.iter().map(|w| MeshWords::read(w).unwrap()).collect();
            assert_eq!(MeshWords::join(&words), appended.encode());
        }
    }

    #[test]
    fn paints_the_words_like_the_mesh() {
        let color = pack_rgba(0.2, 0.4, 0.6, 1.0);
        for mesh in [Mesh::default(), triangle(), other(false), other(true)] {
            let words = mesh.encode();
            let painted = Mesh { colors: vec![color; mesh.vertex_count()], ..mesh };
            assert_eq!(MeshWords::read(&words).unwrap().painted(color), painted.encode());
        }
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
