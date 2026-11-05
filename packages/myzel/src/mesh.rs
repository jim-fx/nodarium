use nodarium_utils::{decode_float, encode_float};

pub const MESH_TAG: i32 = 14;
const HEADER_LEN: usize = 3;

/// Triangle mesh with one color per vertex, see the crate docs for the format
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesh {
    pub faces: Vec<u32>,
    pub positions: Vec<f32>,
    pub normals: Vec<f32>,
    pub colors: Vec<u32>,
}

/// Packs a color with components in [0, 1] into one RGBA8 word, red in the lowest byte
pub fn pack_rgba(r: f32, g: f32, b: f32, a: f32) -> u32 {
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
    byte(r) | byte(g) << 8 | byte(b) << 16 | byte(a) << 24
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
        let mut words = Vec::with_capacity(HEADER_LEN + f * 3 + v * 7);
        words.extend([MESH_TAG, v as i32, f as i32]);
        words.extend(self.faces.iter().map(|&i| i as i32));
        words.extend(self.positions.iter().map(|&p| encode_float(p)));
        words.extend(self.normals.iter().map(|&n| encode_float(n)));
        words.extend(self.colors.iter().map(|&c| c as i32));
        words
    }

    pub fn decode(words: &[i32]) -> Mesh {
        assert!(
            words.len() >= HEADER_LEN && words[0] == MESH_TAG,
            "expected a mesh [14, v, f, ...], got {} words starting with {:?}",
            words.len(),
            words.first()
        );
        let (v, f) = (words[1] as usize, words[2] as usize);
        assert!(
            words.len() == HEADER_LEN + f * 3 + v * 7,
            "mesh with {} vertices and {} faces needs {} words, got {}",
            v,
            f,
            HEADER_LEN + f * 3 + v * 7,
            words.len()
        );
        let mut rest = &words[HEADER_LEN..];
        let mut take = |n: usize| {
            let (head, tail) = rest.split_at(n);
            rest = tail;
            head
        };
        Mesh {
            faces: take(f * 3).iter().map(|&i| i as u32).collect(),
            positions: take(v * 3).iter().map(|&w| decode_float(w)).collect(),
            normals: take(v * 3).iter().map(|&w| decode_float(w)).collect(),
            colors: take(v).iter().map(|&c| c as u32).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_and_decodes() {
        let mesh = Mesh {
            faces: vec![0, 1, 2],
            positions: vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            normals: vec![0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0],
            colors: vec![pack_rgba(1.0, 0.0, 0.0, 1.0); 3],
        };
        let words = mesh.encode();
        assert_eq!(&words[..3], &[MESH_TAG, 3, 1]);
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
        Mesh::decode(&[MESH_TAG, 3, 1, 0, 1]);
    }
}
