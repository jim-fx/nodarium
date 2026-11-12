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

/// `t.clamp(0.0, 1.0)` without the library calls of `f32::min` and `max` on wasm
pub(crate) fn unit(t: f32) -> f32 {
    if t < 0.0 {
        0.0
    } else if t > 1.0 {
        1.0
    } else {
        t
    }
}

/// Squared distance from `p` to the segment `a`-`b`
pub fn distance_sq_to_segment(p: Vec3, a: Vec3, b: Vec3) -> f32 {
    let axis = b - a;
    let length_sq = axis.length_squared();
    let t = if length_sq > 0.0 { unit((p - a).dot(axis) / length_sq) } else { 0.0 };
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

const LEAF_TUBES: usize = 8;

/// `f32::min` and `max` are library calls on wasm, these compile to a compare
pub(crate) fn smaller(a: f32, b: f32) -> f32 {
    if a < b { a } else { b }
}

pub(crate) fn larger(a: f32, b: f32) -> f32 {
    if a > b { a } else { b }
}

fn lowest(a: Vec3, b: Vec3) -> Vec3 {
    Vec3::new(smaller(a.x, b.x), smaller(a.y, b.y), smaller(a.z, b.z))
}

fn highest(a: Vec3, b: Vec3) -> Vec3 {
    Vec3::new(larger(a.x, b.x), larger(a.y, b.y), larger(a.z, b.z))
}

/// Squared distance from `p` to the box `lo`-`hi`
fn box_distance_sq(p: Vec3, lo: Vec3, hi: Vec3) -> f32 {
    let axis = |p: f32, lo: f32, hi: f32| if p < lo { lo - p } else if p > hi { p - hi } else { 0.0 };
    let d = Vec3::new(axis(p.x, lo.x, hi.x), axis(p.y, lo.y, hi.y), axis(p.z, lo.z, hi.z));
    d.length_squared()
}

#[derive(Clone, Copy)]
struct TubeNode {
    lo: Vec3,
    hi: Vec3,
    /// The thickest tube below the node
    radius: f32,
    /// A leaf holds the tubes `first..first + count`, an inner node has its children at `first` and `first + 1`
    first: u32,
    count: u32,
}

/// A bounding volume hierarchy of tubes, segments with a radius, that finds the tube whose surface is
/// nearest to a point. Unlike [`SpatialHash`] it needs no search radius: far subtrees are skipped by
/// the distance to their bounds, so a query costs about the logarithm of the tubes, however dense they are.
pub struct TubeTree {
    nodes: Vec<TubeNode>,
    /// The tubes in leaf order with their index
    tubes: Vec<(Vec3, Vec3, f32, u32)>,
}

impl TubeTree {
    /// A tree of the tubes `(start, end, radius, index)`, `index` is what [`TubeTree::nearest`] returns
    pub fn new(tubes: Vec<(Vec3, Vec3, f32, u32)>) -> TubeTree {
        // the centres (doubled) with the tube they belong to are what the tree is split by
        let mut order: Vec<(Vec3, u32)> = tubes.iter().enumerate().map(|(k, t)| (t.0 + t.1, k as u32)).collect();
        let mut nodes = Vec::with_capacity(2 * tubes.len().div_ceil(LEAF_TUBES));
        if !tubes.is_empty() {
            nodes.push(TubeNode { lo: Vec3::ZERO, hi: Vec3::ZERO, radius: 0.0, first: 0, count: 0 });
            Self::build(&mut nodes, &tubes, &mut order, 0, 0);
        }
        TubeTree { nodes, tubes: order.iter().map(|&(_, k)| tubes[k as usize]).collect() }
    }

    /// Splits the tubes of `order` at the median of their centres along the longest side
    fn build(nodes: &mut Vec<TubeNode>, tubes: &[(Vec3, Vec3, f32, u32)], order: &mut [(Vec3, u32)], node: usize, first: usize) {
        let (mut lo, mut hi) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
        if order.len() <= LEAF_TUBES {
            let mut radius = 0.0f32;
            for &(_, k) in order.iter() {
                let (a, b, r, _) = tubes[k as usize];
                (lo, hi) = (lowest(lowest(lo, a), b), highest(highest(hi, a), b));
                radius = larger(radius, r);
            }
            nodes[node] = TubeNode { lo, hi, radius, first: first as u32, count: order.len() as u32 };
            return;
        }
        for &(centre, _) in order.iter() {
            (lo, hi) = (lowest(lo, centre), highest(hi, centre));
        }
        let half = order.len() / 2;
        match (hi - lo).max_position() {
            0 => order.select_nth_unstable_by(half, |x, y| x.0.x.total_cmp(&y.0.x)),
            1 => order.select_nth_unstable_by(half, |x, y| x.0.y.total_cmp(&y.0.y)),
            _ => order.select_nth_unstable_by(half, |x, y| x.0.z.total_cmp(&y.0.z)),
        };
        let children = nodes.len();
        nodes.push(nodes[node]);
        nodes.push(nodes[node]);
        let (left, right) = order.split_at_mut(half);
        Self::build(nodes, tubes, left, children, first);
        Self::build(nodes, tubes, right, children + 1, first + half);
        let (l, r) = (nodes[children], nodes[children + 1]);
        nodes[node] = TubeNode {
            lo: lowest(l.lo, r.lo),
            hi: highest(l.hi, r.hi),
            radius: larger(l.radius, r.radius),
            first: children as u32,
            count: 0,
        };
    }

    /// The tube whose surface is nearest to `p` and closer than `limit`, as the signed distance to its
    /// surface and its index. Of equally near tubes the one with the smallest index.
    pub fn nearest(&self, p: Vec3, limit: f32) -> Option<(f32, u32)> {
        let mut best: Option<(f32, u32)> = None;
        let mut bound = limit;
        let below = |node: u32| {
            let node = &self.nodes[node as usize];
            box_distance_sq(p, node.lo, node.hi).sqrt() - node.radius
        };
        // the nodes to look at with how near their tubes can be, the tree is balanced and every level adds
        // at most one node that waits
        let mut stack = [(0u32, 0.0f32); 64];
        let mut waiting = 0;
        if !self.nodes.is_empty() {
            stack[0] = (0, below(0));
            waiting = 1;
        }
        while waiting > 0 {
            waiting -= 1;
            let (n, near) = stack[waiting];
            if near > bound {
                continue;
            }
            let node = &self.nodes[n as usize];
            if node.count > 0 {
                let first = node.first as usize;
                for &(a, b, r, index) in &self.tubes[first..first + node.count as usize] {
                    let d = distance_sq_to_segment(p, a, b).sqrt() - r;
                    let better = match best {
                        Some((best_d, best_index)) => d < best_d || (d == best_d && index < best_index),
                        None => d < limit,
                    };
                    if better {
                        best = Some((d, index));
                        bound = d;
                    }
                }
                continue;
            }
            let (left, right) = (node.first, node.first + 1);
            let (dl, dr) = (below(left), below(right));
            // the nearer child is searched first, its tubes lower the bound for the other
            let [near, far] = if dl <= dr { [(left, dl), (right, dr)] } else { [(right, dr), (left, dl)] };
            if far.1 <= bound {
                stack[waiting] = far;
                waiting += 1;
            }
            if near.1 <= bound {
                stack[waiting] = near;
                waiting += 1;
            }
        }
        best
    }

    /// Numbers the tubes by their place in the tree, so data kept in that order lies close together for the
    /// tubes a query finds. Returns the old index of every tube.
    pub fn number_in_order(&mut self) -> Vec<u32> {
        self.tubes.iter_mut().enumerate().map(|(k, t)| std::mem::replace(&mut t.3, k as u32)).collect()
    }

    /// Calls `visit(index, distance)` for every tube whose axis is closer to `p` than `reach`, with the
    /// distance to its axis. `visit` returns the reach from then on, so what it found can narrow the search.
    pub fn for_each_within(&self, p: Vec3, mut reach: f32, mut visit: impl FnMut(u32, f32) -> f32) {
        let below_sq = |node: u32| {
            let node = &self.nodes[node as usize];
            box_distance_sq(p, node.lo, node.hi)
        };
        // the nodes to look at with their squared distance, which stays a bound when the reach narrows
        let mut stack = [(0u32, 0.0f32); 64];
        let mut waiting = 0;
        if !self.nodes.is_empty() && reach > 0.0 {
            stack[0] = (0, below_sq(0));
            waiting = 1;
        }
        let mut reach_sq = reach * reach;
        while waiting > 0 {
            waiting -= 1;
            let (n, d_sq) = stack[waiting];
            if d_sq >= reach_sq {
                continue;
            }
            let node = &self.nodes[n as usize];
            if node.count > 0 {
                let first = node.first as usize;
                for &(a, b, _, index) in &self.tubes[first..first + node.count as usize] {
                    let d_sq = distance_sq_to_segment(p, a, b);
                    if d_sq < reach_sq {
                        reach = visit(index, d_sq.sqrt());
                        reach_sq = reach * reach;
                    }
                }
                continue;
            }
            let (left, right) = (node.first, node.first + 1);
            let (dl, dr) = (below_sq(left), below_sq(right));
            // the nearer child is searched first, what it finds may narrow the reach for the other
            let [near, far] = if dl <= dr { [(left, dl), (right, dr)] } else { [(right, dr), (left, dl)] };
            if far.1 < reach_sq {
                stack[waiting] = far;
                waiting += 1;
            }
            if near.1 < reach_sq {
                stack[waiting] = near;
                waiting += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tube_tree_finds_the_nearest_tube_as_brute_force() {
        let mut rng = Rng(7);
        let tubes: Vec<(Vec3, Vec3, f32, u32)> = (0..3000)
            .map(|i| {
                let a = rng.vec() * 0.05;
                let length = if rng.next() > 0.8 { 0.03 } else { 0.003 };
                (a, a + rng.vec().normalize() * length, 0.001 + 0.002 * rng.next().abs(), i)
            })
            .collect();
        let tree = TubeTree::new(tubes.clone());
        let surface = |p: Vec3, &(a, b, r, _): &(Vec3, Vec3, f32, u32)| distance_sq_to_segment(p, a, b).sqrt() - r;
        for _ in 0..500 {
            let p = rng.vec() * 0.06;
            let limit = (rng.next() + 1.0) * 0.004;
            let expected = tubes
                .iter()
                .map(|t| (surface(p, t), t.3))
                .filter(|&(d, _)| d < limit)
                .min_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)));
            assert_eq!(tree.nearest(p, limit), expected);
        }
        assert_eq!(TubeTree::new(Vec::new()).nearest(Vec3::ZERO, f32::INFINITY), None);
    }

    #[test]
    fn the_tube_tree_visits_the_tubes_within_reach() {
        let mut rng = Rng(11);
        let tubes: Vec<(Vec3, Vec3, f32, u32)> = (0..2000)
            .map(|i| {
                let a = rng.vec() * 0.05;
                (a, a + rng.vec().normalize() * 0.004, 0.001, i)
            })
            .collect();
        let mut tree = TubeTree::new(tubes.clone());
        for _ in 0..200 {
            let p = rng.vec() * 0.06;
            let reach = (rng.next() + 1.0) * 0.005;
            let mut expected: Vec<u32> =
                tubes.iter().filter(|t| distance_sq_to_segment(p, t.0, t.1) < reach * reach).map(|t| t.3).collect();
            let mut found = Vec::new();
            tree.for_each_within(p, reach, |i, d| {
                assert!((d - distance_sq_to_segment(p, tubes[i as usize].0, tubes[i as usize].1).sqrt()).abs() < 1e-7);
                found.push(i);
                reach
            });
            found.sort();
            expected.sort();
            assert_eq!(found, expected);
        }
        // numbered in their order the tubes keep their places
        let old = tree.number_in_order();
        let p = Vec3::splat(0.01);
        let mut renumbered = Vec::new();
        tree.for_each_within(p, 0.01, |i, _| {
            renumbered.push(old[i as usize]);
            0.01
        });
        renumbered.sort();
        let near: Vec<u32> = tubes.iter().filter(|t| distance_sq_to_segment(p, t.0, t.1) < 1e-4).map(|t| t.3).collect();
        assert_eq!(renumbered, near);
    }

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
