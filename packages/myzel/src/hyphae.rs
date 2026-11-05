use glam::Vec3;
use nodarium_utils::{decode_float, encode_float};

use crate::density::Segment;

pub const HYPHAE_TAG: i32 = 11;
const HEADER_LEN: usize = 2;
const SEGMENT_WORDS: usize = 8;

/// What a segment has become, the mesher colors by it and `expand` moves by it
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum Tissue {
    Mycelium = 0,
    Stipe = 1,
    Cap = 2,
    Gill = 3,
    Veil = 4,
    CapCuticle = 5,
}

impl Tissue {
    fn from_word(word: i32) -> Tissue {
        match word {
            0 => Tissue::Mycelium,
            1 => Tissue::Stipe,
            2 => Tissue::Cap,
            3 => Tissue::Gill,
            4 => Tissue::Veil,
            5 => Tissue::CapCuticle,
            _ => panic!("unknown tissue {}", word),
        }
    }
}

/// Bits of [`HyphaSegment::flags`]
pub mod flags {
    /// The lowest two bits hold the kind of hypha
    pub const KIND: u32 = 0b11;
    pub const STANDARD: u32 = 0;
    /// Starts new strands, ignores galvanotropism
    pub const LEADING: u32 = 1;
    /// Branched later from an old branch point
    pub const SECONDARY: u32 = 2;
    /// The segment ends in a growing tip
    pub const ACTIVE_TIP: u32 = 1 << 2;
    /// Torn off its parent, for example the veil after it ripped
    pub const BROKEN: u32 = 1 << 3;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HyphaSegment {
    pub end: Vec3,
    pub radius: f32,
    /// Index of the segment this one grows from, `None` for a root
    pub parent: Option<u32>,
    /// Simulation step the segment was grown in
    pub birth: f32,
    pub tissue: Tissue,
    pub flags: u32,
}

/// Hyphae as a forest of segments `[11, n, ...segments]`, 8 words per segment:
/// `x, y, z, radius, parent, birth, tissue, flags`. Parents come before their
/// children, so one pass from start to end can rebuild every position after
/// moving parents. A root has parent `-1` and is a single point.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Hyphae {
    pub segments: Vec<HyphaSegment>,
}

impl Hyphae {
    pub fn len(&self) -> usize {
        self.segments.len()
    }

    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    /// Adds a segment and returns its index
    pub fn push(&mut self, segment: HyphaSegment) -> u32 {
        let index = self.segments.len() as u32;
        if let Some(parent) = segment.parent {
            assert!(parent < index, "segment {} grows from {}, parents have to come first", index, parent);
        }
        self.segments.push(segment);
        index
    }

    /// Where a segment starts: its parent's end, or its own end for a root
    pub fn start(&self, index: usize) -> Vec3 {
        let segment = &self.segments[index];
        segment.parent.map_or(segment.end, |p| self.segments[p as usize].end)
    }

    /// Every segment as `(start, end, radius, tissue)`, roots included as points
    pub fn iter(&self) -> impl Iterator<Item = (Vec3, Vec3, f32, Tissue)> + '_ {
        (0..self.segments.len()).map(|i| {
            let s = &self.segments[i];
            (self.start(i), s.end, s.radius, s.tissue)
        })
    }

    /// The geometry for the mesher, broken segments are cut loose from their parent
    pub fn to_segments(&self) -> Vec<Segment> {
        self.segments
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let start = if s.flags & flags::BROKEN != 0 { s.end } else { self.start(i) };
                Segment { start, end: s.end, radius: s.radius }
            })
            .collect()
    }

    pub fn encode(&self) -> Vec<i32> {
        let mut words = Vec::with_capacity(HEADER_LEN + self.segments.len() * SEGMENT_WORDS);
        words.extend([HYPHAE_TAG, self.segments.len() as i32]);
        for s in &self.segments {
            words.extend([
                encode_float(s.end.x),
                encode_float(s.end.y),
                encode_float(s.end.z),
                encode_float(s.radius),
                s.parent.map_or(-1, |p| p as i32),
                encode_float(s.birth),
                s.tissue as i32,
                s.flags as i32,
            ]);
        }
        words
    }

    pub fn decode(words: &[i32]) -> Hyphae {
        assert!(
            words.len() >= HEADER_LEN && words[0] == HYPHAE_TAG,
            "expected hyphae [11, n, ...], got {} words starting with {:?}",
            words.len(),
            words.first()
        );
        let n = words[1] as usize;
        assert!(
            words.len() == HEADER_LEN + n * SEGMENT_WORDS,
            "{} hyphal segments need {} words, got {}",
            n,
            HEADER_LEN + n * SEGMENT_WORDS,
            words.len()
        );
        let mut hyphae = Hyphae { segments: Vec::with_capacity(n) };
        for w in words[HEADER_LEN..].chunks_exact(SEGMENT_WORDS) {
            hyphae.push(HyphaSegment {
                end: Vec3::new(decode_float(w[0]), decode_float(w[1]), decode_float(w[2])),
                radius: decode_float(w[3]),
                parent: (w[4] >= 0).then_some(w[4] as u32),
                birth: decode_float(w[5]),
                tissue: Tissue::from_word(w[6]),
                flags: w[7] as u32,
            });
        }
        hyphae
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn segment(end: Vec3, parent: Option<u32>) -> HyphaSegment {
        HyphaSegment { end, radius: 0.001, parent, birth: 0.0, tissue: Tissue::Mycelium, flags: 0 }
    }

    fn sample() -> Hyphae {
        let mut h = Hyphae::default();
        let root = h.push(segment(Vec3::ZERO, None));
        let a = h.push(HyphaSegment {
            end: Vec3::new(0.0, 0.01, 0.0),
            radius: 0.002,
            parent: Some(root),
            birth: 1.0,
            tissue: Tissue::Stipe,
            flags: flags::LEADING,
        });
        h.push(HyphaSegment {
            end: Vec3::new(0.005, 0.02, -0.001),
            radius: 0.0015,
            parent: Some(a),
            birth: 2.5,
            tissue: Tissue::CapCuticle,
            flags: flags::SECONDARY | flags::ACTIVE_TIP,
        });
        h
    }

    #[test]
    fn encodes_and_decodes() {
        let h = sample();
        let words = h.encode();
        assert_eq!(&words[..2], &[HYPHAE_TAG, 3]);
        assert_eq!(words.len(), 2 + 3 * 8);
        assert_eq!(words[2 + 4], -1);
        assert_eq!(Hyphae::decode(&words), h);
    }

    #[test]
    fn segments_start_at_their_parent() {
        let h = sample();
        let segments: Vec<_> = h.iter().collect();
        assert_eq!(segments[0].0, segments[0].1);
        assert_eq!(segments[1].0, Vec3::ZERO);
        assert_eq!(segments[2].0, Vec3::new(0.0, 0.01, 0.0));
        assert_eq!(segments[2].3, Tissue::CapCuticle);
    }

    #[test]
    fn broken_segments_are_cut_loose() {
        let mut h = sample();
        h.segments[2].flags |= flags::BROKEN;
        let s = h.to_segments();
        assert_eq!(s[1].start, Vec3::ZERO);
        assert_eq!(s[2].start, s[2].end);
    }

    #[test]
    #[should_panic(expected = "parents have to come first")]
    fn rejects_children_before_parents() {
        let mut words = sample().encode();
        // make the root point at its own child
        words[2 + 4] = 1;
        Hyphae::decode(&words);
    }

    #[test]
    #[should_panic(expected = "need")]
    fn rejects_truncated_hyphae() {
        let words = sample().encode();
        Hyphae::decode(&words[..words.len() - 1]);
    }
}
