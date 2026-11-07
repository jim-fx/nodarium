//! A 2D pattern of values, like the stripes `reaction_diffusion` makes under a cap.
//!
//! An encoded pattern is
//! `[15, kind, rings, inner, outer, spacing, ...counts, ...values]`, all but the first three words
//! are f32 except the counts. `kind` 0 is the only kind so far, a disc: values on `rings` rings
//! around an axis, evenly spaced from the radius `inner` to `outer` (in metres). Ring `i` holds
//! `counts[i]` values, the values of all rings follow each other. Value `j` of a ring of `n` sits
//! at the angle `TAU * (j + 0.5) / n`, measured around the axis from the side a node picks with
//! [`crate::geometry::orthogonal_to`], like the rays of `cap`. Values go from 0 to 1, a feature
//! (a stripe, a spot) is where they are over 0.5. `spacing` is how far apart the features are, so
//! a node that uses the pattern knows its scale.
//!
//! The type word 15 sets it apart from paths (0), geometry (1), instances (2), forces (3),
//! points (4) and the Myzel types (10 to 14).

use crate::{decode_float, encode_float};
use std::f32::consts::TAU;

/// The type word of an encoded pattern.
pub const PATTERN_TAG: i32 = 15;
/// The kind word of a pattern on a disc.
const DISC: i32 = 0;
const HEADER_WORDS: usize = 6;

/// Values on rings around an axis, see the module docs.
#[derive(Debug, Clone, PartialEq)]
pub struct Pattern {
    /// Radius of the first ring
    pub inner: f32,
    /// Radius of the last ring
    pub outer: f32,
    /// How far apart neighbouring features are
    pub spacing: f32,
    /// The values of every ring, from the inner to the outer one
    pub rings: Vec<Vec<f32>>,
}

/// The angle of value `j` of a ring of `n` values.
pub fn ring_angle(j: usize, n: usize) -> f32 {
    TAU * (j as f32 + 0.5) / n as f32
}

/// The words inside the brackets of a framed item `[0, n + 1, ...words, 1, 1]`.
fn unframe(arg: &[i32]) -> Option<&[i32]> {
    let n = arg.len();
    let framed = n >= 4 && arg[0] == 0 && arg[1] as usize == n - 3 && arg[n - 2] == 1 && arg[n - 1] == 1;
    framed.then(|| &arg[2..n - 2])
}

impl Pattern {
    pub fn radius(&self, ring: usize) -> f32 {
        match self.rings.len() {
            0 | 1 => self.inner,
            n => self.inner + (self.outer - self.inner) * ring as f32 / (n - 1) as f32,
        }
    }

    /// The value of a ring at an angle, linear between the two values around it.
    pub fn ring_value(&self, ring: usize, angle: f32) -> f32 {
        let values = &self.rings[ring];
        let n = values.len();
        if n == 0 {
            return 0.0;
        }
        let x = angle.rem_euclid(TAU) / TAU * n as f32 - 0.5;
        let x0 = x.floor();
        let t = x - x0;
        let j0 = (x0 as i64).rem_euclid(n as i64) as usize;
        values[j0] * (1.0 - t) + values[(j0 + 1) % n] * t
    }

    /// The value at an angle and a radius, linear between the rings around it. `None` outside
    /// the disc.
    pub fn sample(&self, angle: f32, radius: f32) -> Option<f32> {
        let rings = self.rings.len();
        if rings == 0 || radius < self.inner || radius > self.outer {
            return None;
        }
        if rings == 1 {
            return Some(self.ring_value(0, angle));
        }
        let x = (radius - self.inner) / (self.outer - self.inner) * (rings - 1) as f32;
        let i = (x.floor() as usize).min(rings - 2);
        let t = x - i as f32;
        Some(self.ring_value(i, angle) * (1.0 - t) + self.ring_value(i + 1, angle) * t)
    }

    pub fn encode(&self) -> Vec<i32> {
        let values: usize = self.rings.iter().map(|r| r.len()).sum();
        let mut words = Vec::with_capacity(HEADER_WORDS + self.rings.len() + values);
        words.extend([PATTERN_TAG, DISC, self.rings.len() as i32]);
        words.extend([self.inner, self.outer, self.spacing].map(encode_float));
        words.extend(self.rings.iter().map(|r| r.len() as i32));
        for ring in &self.rings {
            words.extend(ring.iter().map(|&v| encode_float(v)));
        }
        words
    }

    /// Reads a pattern as a node sends it, or wrapped in the brackets of a list. `None` for
    /// anything else.
    pub fn decode(arg: &[i32]) -> Option<Pattern> {
        if arg.first() != Some(&PATTERN_TAG) {
            let n = arg.len();
            // a list around the pattern: [0, 1, [0, m + 1, ...pattern, 1, 1], 1, 1]
            if n >= 8 && arg[..2] == [0, 1] && arg[n - 2..] == [1, 1] {
                if let Some(pattern) = unframe(&arg[2..n - 2]).and_then(Pattern::decode) {
                    return Some(pattern);
                }
            }
            return unframe(arg).and_then(Pattern::decode);
        }
        if arg.len() < HEADER_WORDS || arg[1] != DISC || arg[2] < 0 {
            return None;
        }
        let rings = arg[2] as usize;
        let counts = arg.get(HEADER_WORDS..HEADER_WORDS + rings)?;
        if counts.iter().any(|&c| c < 0) {
            return None;
        }
        let total: usize = counts.iter().map(|&c| c as usize).sum();
        let values = arg.get(HEADER_WORDS + rings..)?;
        if values.len() != total {
            return None;
        }
        let mut offset = 0;
        let rings = counts
            .iter()
            .map(|&c| {
                let ring = values[offset..offset + c as usize].iter().map(|&w| decode_float(w)).collect();
                offset += c as usize;
                ring
            })
            .collect();
        Some(Pattern { inner: decode_float(arg[3]), outer: decode_float(arg[4]), spacing: decode_float(arg[5]), rings })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{concat_args, wrap_arg};

    fn pattern() -> Pattern {
        Pattern {
            inner: 0.01,
            outer: 0.03,
            spacing: 0.002,
            rings: vec![vec![0.0, 1.0, 0.0, 1.0], vec![1.0; 6], vec![0.25, 0.5, 0.75, 1.0, 0.0]],
        }
    }

    #[test]
    fn encodes_and_decodes() {
        let p = pattern();
        let words = p.encode();
        assert_eq!(words[0], PATTERN_TAG);
        assert_eq!(Pattern::decode(&words), Some(p.clone()));
        // as one item of a list, the way a connected node can send it
        assert_eq!(Pattern::decode(&wrap_arg(&words)), Some(p.clone()));
        assert_eq!(Pattern::decode(&concat_args(vec![&wrap_arg(&words)])), Some(p));
    }

    #[test]
    fn rejects_other_data() {
        assert_eq!(Pattern::decode(&[]), None);
        assert_eq!(Pattern::decode(&[10, 0, 0]), None);
        let mut words = pattern().encode();
        words.pop();
        assert_eq!(Pattern::decode(&words), None);
    }

    #[test]
    fn samples_between_rings_and_values() {
        let p = pattern();
        assert_eq!(p.radius(0), 0.01);
        assert_eq!(p.radius(2), 0.03);
        assert!((p.radius(1) - 0.02).abs() < 1e-7);
        // value 1 of 4 sits at a quarter turn plus an eighth
        assert!((p.ring_value(0, ring_angle(1, 4)) - 1.0).abs() < 1e-6);
        // halfway between value 0 and 1
        assert!((p.ring_value(0, TAU * 0.25) - 0.5).abs() < 1e-6);
        // around the end of the ring back to the start
        assert!((p.ring_value(0, TAU - 1e-4) - 0.5).abs() < 1e-3);
        // halfway between the first two rings
        let v = p.sample(ring_angle(1, 4), 0.015).unwrap();
        assert!((v - 1.0).abs() < 1e-6);
        let v = p.sample(ring_angle(0, 4), 0.015).unwrap();
        assert!((v - 0.5).abs() < 1e-6);
        assert_eq!(p.sample(0.0, 0.005), None);
        assert_eq!(p.sample(0.0, 0.031), None);
    }
}
