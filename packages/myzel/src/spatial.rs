use std::collections::HashMap;

use glam::{IVec3, Vec3};

/// Buckets segments into a uniform grid so the segments near a point can be
/// found without looking at all of them. Segments can be added while querying,
/// which the growth simulation needs.
pub struct SpatialHash {
    cell: f32,
    buckets: HashMap<IVec3, Vec<u32>>,
    segments: Vec<(Vec3, Vec3)>,
    // the query a segment was last reported in, so one spanning several cells is reported once
    seen: Vec<u32>,
    query: u32,
}

/// Squared distance from `p` to the segment `a`-`b`
pub fn distance_sq_to_segment(p: Vec3, a: Vec3, b: Vec3) -> f32 {
    let axis = b - a;
    let length_sq = axis.length_squared();
    let t = if length_sq > 0.0 { ((p - a).dot(axis) / length_sq).clamp(0.0, 1.0) } else { 0.0 };
    (p - a - axis * t).length_squared()
}

impl SpatialHash {
    /// `cell` should be about the radius of the typical query
    pub fn new(cell: f32) -> SpatialHash {
        assert!(cell > 0.0, "the cell size of a spatial hash has to be positive, got {}", cell);
        SpatialHash { cell, buckets: HashMap::new(), segments: Vec::new(), seen: Vec::new(), query: 0 }
    }

    pub fn from_segments(cell: f32, segments: impl IntoIterator<Item = (Vec3, Vec3)>) -> SpatialHash {
        let mut hash = SpatialHash::new(cell);
        for (a, b) in segments {
            hash.insert(a, b);
        }
        hash
    }

    pub fn len(&self) -> usize {
        self.segments.len()
    }

    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    fn key(&self, p: Vec3) -> IVec3 {
        (p / self.cell).floor().as_ivec3()
    }

    /// Adds a segment and returns its index, indices count up from 0 in insertion order
    pub fn insert(&mut self, a: Vec3, b: Vec3) -> u32 {
        let index = self.segments.len() as u32;
        let (lo, hi) = (self.key(a.min(b)), self.key(a.max(b)));
        for z in lo.z..=hi.z {
            for y in lo.y..=hi.y {
                for x in lo.x..=hi.x {
                    self.buckets.entry(IVec3::new(x, y, z)).or_default().push(index);
                }
            }
        }
        self.segments.push((a, b));
        self.seen.push(u32::MAX);
        index
    }

    /// Calls `visit(index, distance)` once for every segment closer than `radius` to `p`
    pub fn for_each_near(&mut self, p: Vec3, radius: f32, mut visit: impl FnMut(u32, f32)) {
        self.query = self.query.wrapping_add(1);
        let radius_sq = radius * radius;
        let (lo, hi) = (self.key(p - Vec3::splat(radius)), self.key(p + Vec3::splat(radius)));
        for z in lo.z..=hi.z {
            for y in lo.y..=hi.y {
                for x in lo.x..=hi.x {
                    let Some(bucket) = self.buckets.get(&IVec3::new(x, y, z)) else { continue };
                    for &i in bucket {
                        if self.seen[i as usize] == self.query {
                            continue;
                        }
                        self.seen[i as usize] = self.query;
                        let (a, b) = self.segments[i as usize];
                        let d_sq = distance_sq_to_segment(p, a, b);
                        if d_sq < radius_sq {
                            visit(i, d_sq.sqrt());
                        }
                    }
                }
            }
        }
    }

    /// The segments closer than `radius` to `p`, sorted by index
    pub fn near(&mut self, p: Vec3, radius: f32) -> Vec<(u32, f32)> {
        let mut found = Vec::new();
        self.for_each_near(p, radius, |i, d| found.push((i, d)));
        found.sort_by_key(|&(i, _)| i);
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Rng(u32);
    impl Rng {
        fn next(&mut self) -> f32 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 17;
            self.0 ^= self.0 << 5;
            self.0 as f32 / u32::MAX as f32 * 2.0 - 1.0
        }
        fn vec(&mut self) -> Vec3 {
            Vec3::new(self.next(), self.next(), self.next())
        }
    }

    #[test]
    fn finds_the_same_segments_as_brute_force() {
        let mut rng = Rng(42);
        let segments: Vec<(Vec3, Vec3)> = (0..2000)
            .map(|_| {
                let a = rng.vec() * 0.05;
                // some segments span many cells
                let length = if rng.next() > 0.8 { 0.03 } else { 0.003 };
                (a, a + rng.vec().normalize() * length)
            })
            .collect();
        let mut hash = SpatialHash::from_segments(0.004, segments.iter().copied());
        assert_eq!(hash.len(), segments.len());

        for _ in 0..200 {
            let p = rng.vec() * 0.05;
            let radius = 0.002 + (rng.next() + 1.0) * 0.004;
            let expected: Vec<u32> = segments
                .iter()
                .enumerate()
                .filter(|(_, &(a, b))| distance_sq_to_segment(p, a, b) < radius * radius)
                .map(|(i, _)| i as u32)
                .collect();
            let found = hash.near(p, radius);
            assert_eq!(found.iter().map(|&(i, _)| i).collect::<Vec<_>>(), expected);
            for (i, d) in found {
                let (a, b) = segments[i as usize];
                assert!((d - distance_sq_to_segment(p, a, b).sqrt()).abs() < 1e-6);
            }
        }
    }

    #[test]
    fn segments_added_between_queries_are_found() {
        let mut hash = SpatialHash::new(0.01);
        assert!(hash.near(Vec3::ZERO, 0.02).is_empty());
        hash.insert(Vec3::new(-0.05, 0.0, 0.0), Vec3::new(0.05, 0.0, 0.0));
        hash.insert(Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 1.1, 0.0));
        assert_eq!(hash.near(Vec3::new(0.0, 0.005, 0.0), 0.01), vec![(0, 0.005)]);
    }

    #[test]
    fn points_are_segments_of_length_zero() {
        let mut hash = SpatialHash::new(0.01);
        hash.insert(Vec3::ONE, Vec3::ONE);
        assert_eq!(hash.near(Vec3::new(1.0, 1.0, 1.003), 0.005).len(), 1);
        assert!(hash.near(Vec3::new(1.0, 1.0, 1.006), 0.005).is_empty());
    }
}
