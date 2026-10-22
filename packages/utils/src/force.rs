//! Forces that nodes like `apply_force` push paths with.
//!
//! An encoded force is `[0, 9, 3, kind, x, y, z, strength, radius, falloff, 1, 1]`, a list of
//! forces is a framed list of those. The type word 3 sets it apart from paths (0), geometry (1)
//! and instances (2).

use crate::tree::split_args;
use crate::{decode_float, encode_float};
use glam::Vec3;

/// The type word of an encoded force.
pub const FORCE_TYPE: i32 = 3;

// type, kind, position (3), strength, radius, falloff
const FORCE_WORDS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForceKind {
    /// Pulls towards a position, a negative strength pushes away from it.
    Point = 0,
}

impl ForceKind {
    pub fn from_index(index: i32) -> Option<Self> {
        match index {
            0 => Some(Self::Point),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Force {
    pub kind: ForceKind,
    pub position: Vec3,
    pub strength: f32,
    /// How far the force reaches. A radius of 0 or less reaches nothing.
    pub radius: f32,
    /// 0 keeps the full strength up to the radius, 1 fades smoothly to nothing at the radius.
    pub falloff: f32,
}

fn smoothstep(value: f32) -> f32 {
    let t = value.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl Force {
    /// The force as one framed item, ready for `concat_arg_vecs`.
    pub fn encode(&self) -> Vec<i32> {
        let mut data = vec![0, FORCE_WORDS as i32 + 1, FORCE_TYPE, self.kind as i32];
        data.extend(
            [
                self.position.x,
                self.position.y,
                self.position.z,
                self.strength,
                self.radius,
                self.falloff,
            ]
            .map(encode_float),
        );
        data.extend([1, 1]);
        data
    }

    /// Reads one framed item, `None` if it is not a force.
    pub fn decode(item: &[i32]) -> Option<Self> {
        if item.len() != FORCE_WORDS + 4 || item[2] != FORCE_TYPE {
            return None;
        }
        let value = |i: usize| decode_float(item[4 + i]);
        Some(Self {
            kind: ForceKind::from_index(item[3])?,
            position: Vec3::new(value(0), value(1), value(2)),
            strength: value(3),
            radius: value(4),
            falloff: value(5),
        })
    }

    /// The push the force gives at `at`, zero outside of its radius.
    pub fn sample(&self, at: Vec3) -> Vec3 {
        match self.kind {
            ForceKind::Point => {
                if self.radius <= 0.0 {
                    return Vec3::ZERO;
                }
                let to_force = self.position - at;
                let distance = to_force.length();
                if distance >= self.radius {
                    return Vec3::ZERO;
                }
                let Some(direction) = to_force.try_normalize() else {
                    return Vec3::ZERO;
                };
                let falloff = self.falloff.clamp(0.0, 1.0);
                let intensity = 1.0 - falloff * smoothstep(distance / self.radius);
                direction * self.strength * intensity
            }
        }
    }
}

/// Every force in a list, other items are ignored. An unconnected input gives no forces.
pub fn decode_forces(list: &[i32]) -> Vec<Force> {
    if list.len() < 4 {
        return Vec::new();
    }
    split_args(list)
        .into_iter()
        .filter_map(Force::decode)
        .collect()
}

/// The sum of the push of all forces at `at`.
pub fn sample_forces(forces: &[Force], at: Vec3) -> Vec3 {
    forces.iter().map(|force| force.sample(at)).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::concat_arg_vecs;

    fn point(position: Vec3, strength: f32, radius: f32, falloff: f32) -> Force {
        Force {
            kind: ForceKind::Point,
            position,
            strength,
            radius,
            falloff,
        }
    }

    #[test]
    fn a_force_survives_a_round_trip_in_a_list() {
        let a = point(Vec3::new(1.0, 2.0, 3.0), 0.5, 4.0, 0.25);
        let b = point(Vec3::new(-1.0, 0.0, 0.5), -2.0, 1.0, 1.0);
        let list = concat_arg_vecs(vec![a.encode(), b.encode()]);

        assert_eq!(decode_forces(&list), vec![a, b]);
    }

    #[test]
    fn a_single_force_decodes_from_its_list() {
        let a = point(Vec3::ONE, 1.0, 2.0, 0.0);
        let list = concat_arg_vecs(vec![a.encode()]);

        assert_eq!(decode_forces(&list), vec![a]);
    }

    #[test]
    fn an_unconnected_input_has_no_forces() {
        assert!(decode_forces(&[]).is_empty());
    }

    #[test]
    fn paths_are_not_forces() {
        let path = vec![0, 6, 0, 1, 0, -1, 0, 1, 1];
        assert_eq!(Force::decode(&path), None);
    }

    #[test]
    fn a_point_force_pulls_towards_its_position() {
        let force = point(Vec3::new(2.0, 0.0, 0.0), 1.0, 5.0, 0.0);
        let push = force.sample(Vec3::ZERO);
        assert!((push - Vec3::X).length() < 1e-6, "{:?}", push);
    }

    #[test]
    fn a_negative_strength_pushes_away() {
        let force = point(Vec3::new(2.0, 0.0, 0.0), -1.0, 5.0, 0.0);
        assert!(force.sample(Vec3::ZERO).x < 0.0);
    }

    #[test]
    fn nothing_happens_outside_of_the_radius() {
        let force = point(Vec3::new(2.0, 0.0, 0.0), 1.0, 1.5, 0.0);
        assert_eq!(force.sample(Vec3::ZERO), Vec3::ZERO);
        assert_eq!(point(Vec3::X, 1.0, 0.0, 0.0).sample(Vec3::ZERO), Vec3::ZERO);
    }

    #[test]
    fn falloff_fades_the_force_towards_the_radius() {
        let force = point(Vec3::new(4.0, 0.0, 0.0), 1.0, 4.0, 1.0);
        let near = force.sample(Vec3::new(3.0, 0.0, 0.0)).length();
        let far = force.sample(Vec3::new(0.5, 0.0, 0.0)).length();
        assert!(near > far, "near {} far {}", near, far);
        assert!(far > 0.0);

        let hard = point(Vec3::new(4.0, 0.0, 0.0), 1.0, 4.0, 0.0);
        assert_eq!(hard.sample(Vec3::new(0.5, 0.0, 0.0)).length(), 1.0);
    }

    #[test]
    fn forces_add_up() {
        let a = point(Vec3::X, 1.0, 5.0, 0.0);
        let b = point(Vec3::X, 1.0, 5.0, 0.0);
        let sum = sample_forces(&[a, b], Vec3::ZERO);
        assert!((sum - Vec3::X * 2.0).length() < 1e-6);
    }

    #[test]
    fn a_point_on_the_force_gets_no_push() {
        let force = point(Vec3::ONE, 1.0, 5.0, 0.0);
        assert_eq!(force.sample(Vec3::ONE), Vec3::ZERO);
    }
}
