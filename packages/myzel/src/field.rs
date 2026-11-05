use glam::{Vec2, Vec3};
use nodarium_utils::{decode_float, encode_float};

use crate::eval::Program;

pub const FIELD_TAG: i32 = 10;
const HEADER_LEN: usize = 8;

/// Opcodes of the field bytecode, followed by the listed f32 operands
pub mod opcode {
    /// `r`, centered at the origin
    pub const SPHERE: i32 = 1;
    /// `ax ay az bx by bz r`
    pub const CAPSULE: i32 = 2;
    /// `n x0 y0 .. xn yn`, a closed polygon in (radius, height) revolved around the y axis
    pub const REVOLVE: i32 = 3;
    pub const UNION: i32 = 10;
    /// `k`
    pub const SMOOTH_UNION: i32 = 11;
    /// `k`, subtracts the top field from the one below it
    pub const SMOOTH_SUBTRACT: i32 = 12;
    /// `x y z`
    pub const TRANSLATE: i32 = 20;
    /// `k`, rotates around z by `k * y`, positive `k` bends towards +x
    pub const BEND: i32 = 21;
    /// `amplitude frequency seed`, displaces the point by value noise
    pub const WARP: i32 = 22;
    /// closes the innermost transform
    pub const END: i32 = 29;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bbox {
    pub min: Vec3,
    pub max: Vec3,
}

impl Bbox {
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    pub fn union(self, other: Bbox) -> Bbox {
        Bbox::new(self.min.min(other.min), self.max.max(other.max))
    }

    pub fn pad(self, amount: f32) -> Bbox {
        Bbox::new(self.min - Vec3::splat(amount), self.max + Vec3::splat(amount))
    }

    pub fn translate(self, offset: Vec3) -> Bbox {
        Bbox::new(self.min + offset, self.max + offset)
    }

    pub fn size(&self) -> Vec3 {
        self.max - self.min
    }

    /// Largest distance of the box from the z axis
    fn max_xy_norm(&self) -> f32 {
        self.min.abs().max(self.max.abs()).truncate().length()
    }
}

/// A shape as field bytecode, see the crate docs for the format
#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    pub bbox: Bbox,
    pub ops: Vec<i32>,
}

fn push_op(ops: &mut Vec<i32>, opcode: i32, operands: &[f32]) {
    ops.push(opcode);
    ops.extend(operands.iter().map(|&v| encode_float(v)));
}

impl Field {
    pub fn sphere(radius: f32) -> Field {
        let mut ops = Vec::with_capacity(2);
        push_op(&mut ops, opcode::SPHERE, &[radius]);
        Field { bbox: Bbox::new(Vec3::splat(-radius), Vec3::splat(radius)), ops }
    }

    pub fn capsule(a: Vec3, b: Vec3, radius: f32) -> Field {
        let mut ops = Vec::with_capacity(8);
        push_op(&mut ops, opcode::CAPSULE, &[a.x, a.y, a.z, b.x, b.y, b.z, radius]);
        let bbox = Bbox::new(a.min(b), a.max(b)).pad(radius);
        Field { bbox, ops }
    }

    /// Revolves a closed polygon of (radius, height) points around the y axis
    pub fn revolve(profile: &[Vec2]) -> Field {
        assert!(profile.len() >= 3, "revolve needs a profile of at least 3 points, got {}", profile.len());
        let mut ops = Vec::with_capacity(2 + profile.len() * 2);
        push_op(&mut ops, opcode::REVOLVE, &[profile.len() as f32]);
        let mut radius = 0.0f32;
        let (mut low, mut high) = (f32::INFINITY, f32::NEG_INFINITY);
        for point in profile {
            ops.push(encode_float(point.x));
            ops.push(encode_float(point.y));
            radius = radius.max(point.x.abs());
            low = low.min(point.y);
            high = high.max(point.y);
        }
        let bbox = Bbox::new(Vec3::new(-radius, low, -radius), Vec3::new(radius, high, radius));
        Field { bbox, ops }
    }

    fn combine(self, other: Field, opcode: i32, operands: &[f32], bbox: Bbox) -> Field {
        let mut ops = self.ops;
        ops.reserve(other.ops.len() + 1 + operands.len());
        ops.extend(other.ops);
        push_op(&mut ops, opcode, operands);
        Field { bbox, ops }
    }

    pub fn union(self, other: Field) -> Field {
        let bbox = self.bbox.union(other.bbox);
        self.combine(other, opcode::UNION, &[], bbox)
    }

    pub fn smooth_union(self, other: Field, k: f32) -> Field {
        // the polynomial blend grows the surface by at most k / 4
        let bbox = self.bbox.union(other.bbox).pad(k.max(0.0) * 0.25);
        self.combine(other, opcode::SMOOTH_UNION, &[k], bbox)
    }

    /// Cuts `other` out of this field
    pub fn smooth_subtract(self, other: Field, k: f32) -> Field {
        let bbox = self.bbox.pad(k.max(0.0) * 0.25);
        self.combine(other, opcode::SMOOTH_SUBTRACT, &[k], bbox)
    }

    fn wrap(self, opcode: i32, operands: &[f32], bbox: Bbox) -> Field {
        let mut ops = Vec::with_capacity(self.ops.len() + 2 + operands.len());
        push_op(&mut ops, opcode, operands);
        ops.extend(self.ops);
        ops.push(opcode::END);
        Field { bbox, ops }
    }

    pub fn translate(self, offset: Vec3) -> Field {
        let bbox = self.bbox.translate(offset);
        self.wrap(opcode::TRANSLATE, &[offset.x, offset.y, offset.z], bbox)
    }

    pub fn bend(self, k: f32) -> Field {
        // the bend rotates around the z axis, so z stays and x, y stay within
        // the distance of the farthest corner from that axis
        let r = self.bbox.max_xy_norm();
        let bbox = Bbox::new(Vec3::new(-r, -r, self.bbox.min.z), Vec3::new(r, r, self.bbox.max.z));
        self.wrap(opcode::BEND, &[k], bbox)
    }

    pub fn warp(self, amplitude: f32, frequency: f32, seed: u32) -> Field {
        // every axis moves by at most the amplitude
        let bbox = self.bbox.pad(amplitude.abs() * 3f32.sqrt());
        self.wrap(opcode::WARP, &[amplitude, frequency, seed as f32], bbox)
    }

    pub fn encode(&self) -> Vec<i32> {
        let mut words = Vec::with_capacity(HEADER_LEN + self.ops.len());
        words.push(FIELD_TAG);
        words.push(self.ops.len() as i32);
        for v in [self.bbox.min, self.bbox.max] {
            words.extend([encode_float(v.x), encode_float(v.y), encode_float(v.z)]);
        }
        words.extend_from_slice(&self.ops);
        words
    }

    pub fn decode(words: &[i32]) -> Field {
        assert!(
            words.len() >= HEADER_LEN && words[0] == FIELD_TAG,
            "expected a field [10, n, bbox, ...ops], got {} words starting with {:?}",
            words.len(),
            words.first()
        );
        let n = words[1] as usize;
        assert!(
            words.len() == HEADER_LEN + n,
            "field says it has {} op words but carries {}",
            n,
            words.len() - HEADER_LEN
        );
        let f = |i: usize| decode_float(words[i]);
        let bbox = Bbox::new(Vec3::new(f(2), f(3), f(4)), Vec3::new(f(5), f(6), f(7)));
        Field { bbox, ops: words[HEADER_LEN..].to_vec() }
    }

    pub fn program(&self) -> Program {
        Program::parse(&self.ops)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_and_decodes() {
        let field = Field::sphere(1.0).translate(Vec3::Y).smooth_union(Field::capsule(Vec3::ZERO, Vec3::Y, 0.2), 0.3);
        let words = field.encode();
        assert_eq!(words[0], FIELD_TAG);
        assert_eq!(words[1] as usize, words.len() - HEADER_LEN);
        assert_eq!(Field::decode(&words), field);
    }

    #[test]
    fn combined_bbox_covers_both() {
        let field = Field::sphere(1.0).union(Field::sphere(0.5).translate(Vec3::new(3.0, 0.0, 0.0)));
        assert_eq!(field.bbox, Bbox::new(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(3.5, 1.0, 1.0)));
    }

    #[test]
    fn warp_pads_bbox() {
        let field = Field::sphere(1.0).warp(0.1, 2.0, 7);
        assert!(field.bbox.max.x > 1.0 + 0.1);
    }

    #[test]
    #[should_panic(expected = "expected a field")]
    fn rejects_other_data() {
        Field::decode(&[0, 3, 1, 2, 1, 1]);
    }
}
