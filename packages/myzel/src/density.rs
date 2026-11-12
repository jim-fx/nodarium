use glam::Vec3;

use crate::mesh::Mesh;
use crate::spatial::{larger, smaller, unit, TubeTree};

const BLOCK: usize = 8;
const BLOCK_SAMPLES: usize = BLOCK * BLOCK * BLOCK;
const EMPTY: u32 = u32::MAX;

/// Thinner segments would fall between the samples and vanish. Only a blended segment that
/// would vanish and a strand get this radius, see [`SparseGrid::floor`]: tissue and blended
/// segments that join others keep their own, or they would swell as the cells grow.
const MIN_RADIUS_IN_CELLS: f32 = 0.75;

const CORNERS: [[usize; 3]; 8] =
    [[0, 0, 0], [1, 0, 0], [0, 1, 0], [1, 1, 0], [0, 0, 1], [1, 0, 1], [0, 1, 1], [1, 1, 1]];
const EDGES: [[usize; 2]; 12] =
    [[0, 1], [2, 3], [4, 5], [6, 7], [0, 2], [1, 3], [4, 6], [5, 7], [0, 4], [1, 5], [2, 6], [3, 7]];

/// The closing keeps this share of its radius away from the axes of the other
/// segments, so it doesn't fill the gaps between them
const GUARD: f32 = 0.5;
/// Blur of the closing relative to its radius, it smooths out the hyphae that stand out of the tissue
const SMOOTHING: f32 = 0.75;

/// The surface of the blurred closing goes through samples that are this much
/// inside, below a half so the blur doesn't shrink the tissue below its outer hyphae.
/// It lies as far out as the blur alone puts it, in world units, see [`ramp_shift`].
const SURFACE_SHARE: f32 = 0.3;

/// How much a closed segment counts for the owner of a sample past its kernel
const FILL_OWNER_WEIGHT: f32 = 1e-3;


#[derive(Clone, Copy)]
struct Closed {
    radius: f32,
    reach: f32,
    /// Radius of the tube the segment keeps where it leaves the tissue, if it is a strand
    strand: Option<f32>,
    /// Radius of the closing where this segment is the nearest
    close: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Segment {
    pub start: Vec3,
    pub end: Vec3,
    pub radius: f32,
}

/// How a segment is meshed, see [`surface`]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Meshing {
    /// Closed into one tissue with the other closed segments, see `close`
    pub closed: bool,
    /// Where two segments reach a vertex, the one with the larger kernel times priority owns it
    pub priority: f32,
    /// The segment continues the one before it in the same path. Their kernels don't add up
    /// where both reach, so a path is one tube of its own radius and only different paths blend.
    pub joined: bool,
    /// How far, in radii, its kernel reaches, segments closer than that join into one surface
    pub blend: f32,
    /// Radius of the closing a closed segment fills its gaps with, at 0 it is blended like the others
    pub close: f32,
}

impl Meshing {
    /// A segment of its own that blends `blend` radii far
    pub fn blended(blend: f32) -> Meshing {
        Meshing { closed: false, priority: 1.0, joined: false, blend, close: 0.0 }
    }

    /// A segment closed into tissue with a closing of radius `close`
    pub fn closed(blend: f32, close: f32) -> Meshing {
        Meshing { closed: true, close, ..Meshing::blended(blend) }
    }
}

/// Density of the surface around a lone segment whose kernel reaches `reach_factor` radii
fn threshold(reach_factor: f32) -> f32 {
    (1.0 - 1.0 / (reach_factor * reach_factor)).powi(3)
}

/// How far the kernel of a segment reaches and how much it weighs, see [`Frame::kernel`]
#[derive(Clone, Copy)]
struct Kernel {
    reach: f32,
    weight: f32,
}

/// Density samples stored in 8³ blocks, only blocks near a segment are allocated
struct SparseGrid {
    origin: Vec3,
    cell: f32,
    samples: [usize; 3],
    blocks: [usize; 3],
    block_slots: Vec<u32>,
    density: Vec<f32>,
    /// Per sample the segment with the largest kernel value there, and that value
    owner: Vec<u32>,
    owner_k: Vec<f32>,
    /// Signed distance to the nearest tube of the closed segments
    tube: Vec<f32>,
    /// Radius of the closing of that nearest closed segment
    tube_close: Vec<f32>,
    /// Signed distance to the nearest tube of the closed segments that are strands
    strand: Vec<f32>,
    /// Distance to the nearest axis of the other segments, the closing keeps away from them
    guard: Vec<f32>,
    /// Per sample the last blended segment that reached it and the largest kernel of the run of
    /// joined segments it ends, see [`Meshing::joined`]
    run: Vec<u32>,
    run_k: Vec<f32>,
}

impl SparseGrid {
    fn slot(&self, p: [usize; 3]) -> u32 {
        let b = [p[0] / BLOCK, p[1] / BLOCK, p[2] / BLOCK];
        self.block_slots[b[0] + self.blocks[0] * (b[1] + self.blocks[1] * b[2])]
    }

    fn local(p: [usize; 3]) -> usize {
        p[0] % BLOCK + BLOCK * (p[1] % BLOCK + BLOCK * (p[2] % BLOCK))
    }

    fn allocate(&mut self, lo: [usize; 3], hi: [usize; 3]) {
        for bz in lo[2] / BLOCK..=hi[2] / BLOCK {
            for by in lo[1] / BLOCK..=hi[1] / BLOCK {
                for bx in lo[0] / BLOCK..=hi[0] / BLOCK {
                    let i = bx + self.blocks[0] * (by + self.blocks[1] * bz);
                    if self.block_slots[i] == EMPTY {
                        self.block_slots[i] = (self.density.len() / BLOCK_SAMPLES) as u32;
                        self.density.resize(self.density.len() + BLOCK_SAMPLES, 0.0);
                        self.owner.resize(self.density.len(), EMPTY);
                        self.owner_k.resize(self.density.len(), 0.0);
                        self.tube.resize(self.density.len(), f32::INFINITY);
                        self.tube_close.resize(self.density.len(), 0.0);
                        self.strand.resize(self.density.len(), f32::INFINITY);
                        self.guard.resize(self.density.len(), f32::INFINITY);
                        self.run.resize(self.density.len(), EMPTY);
                        self.run_k.resize(self.density.len(), 0.0);
                    }
                }
            }
        }
    }

    /// Density at a sample, zero outside the grid and in blocks that were never touched
    fn get(&self, p: [usize; 3]) -> f32 {
        if p[0] >= self.samples[0] || p[1] >= self.samples[1] || p[2] >= self.samples[2] {
            return 0.0;
        }
        match self.slot(p) {
            EMPTY => 0.0,
            s => self.density[s as usize * BLOCK_SAMPLES + Self::local(p)],
        }
    }

    fn owner(&self, p: [usize; 3]) -> (f32, u32) {
        if p[0] >= self.samples[0] || p[1] >= self.samples[1] || p[2] >= self.samples[2] {
            return (0.0, EMPTY);
        }
        match self.slot(p) {
            EMPTY => (0.0, EMPTY),
            s => {
                let i = s as usize * BLOCK_SAMPLES + Self::local(p);
                (self.owner_k[i], self.owner[i])
            }
        }
    }

    fn gradient(&self, p: [usize; 3]) -> Vec3 {
        let at = |axis: usize, step: isize| {
            let mut q = p;
            q[axis] = q[axis].wrapping_add_signed(step);
            self.get(q)
        };
        Vec3::new(at(0, 1) - at(0, -1), at(1, 1) - at(1, -1), at(2, 1) - at(2, -1))
    }

    fn position(&self, p: Vec3) -> Vec3 {
        self.origin + p * self.cell
    }

    fn index(&self, p: [usize; 3]) -> Option<usize> {
        if p[0] >= self.samples[0] || p[1] >= self.samples[1] || p[2] >= self.samples[2] {
            return None;
        }
        match self.slot(p) {
            EMPTY => None,
            s => Some(s as usize * BLOCK_SAMPLES + Self::local(p)),
        }
    }

    fn guard(&mut self, segment: &Segment, reach: f32) {
        let lo_world = segment.start.min(segment.end) - Vec3::splat(reach);
        let hi_world = segment.start.max(segment.end) + Vec3::splat(reach);
        let to_sample = |v: Vec3, round: fn(f32) -> f32| {
            [0, 1, 2].map(|a| (round((v[a] - self.origin[a]) / self.cell).max(0.0) as usize).min(self.samples[a] - 1))
        };
        let (lo, hi) = (to_sample(lo_world, f32::ceil), to_sample(hi_world, f32::floor));
        let axis = segment.end - segment.start;
        let length_sq = axis.length_squared().max(f32::EPSILON);
        for z in lo[2]..=hi[2] {
            for y in lo[1]..=hi[1] {
                for x in lo[0]..=hi[0] {
                    let Some(i) = self.index([x, y, z]) else { continue };
                    let p = self.position(Vec3::new(x as f32, y as f32, z as f32));
                    let t = ((p - segment.start).dot(axis) / length_sq).clamp(0.0, 1.0);
                    self.guard[i] = self.guard[i].min((p - segment.start - axis * t).length());
                }
            }
        }
    }

    /// Trilinear density at a point
    fn density_at(&self, p: Vec3) -> f32 {
        let u = (p - self.origin) / self.cell;
        let base = [0, 1, 2].map(|a| u[a].floor().max(0.0) as usize);
        let f = (u - Vec3::from(base.map(|v| v as f32))).clamp(Vec3::ZERO, Vec3::ONE);
        CORNERS
            .iter()
            .map(|o| {
                let w = [0, 1, 2].map(|a| if o[a] == 1 { f[a] } else { 1.0 - f[a] });
                w[0] * w[1] * w[2] * self.get([base[0] + o[0], base[1] + o[1], base[2] + o[2]])
            })
            .sum()
    }

    /// Whether the surface at `threshold` encloses the axis of a segment, checked every half cell along it
    fn encloses(&self, segment: &Segment, threshold: f32) -> bool {
        let steps = ((segment.end - segment.start).length() / (0.5 * self.cell)).ceil().max(1.0) as usize;
        (0..=steps).all(|k| self.density_at(segment.start.lerp(segment.end, k as f32 / steps as f32)) > threshold)
    }

    /// Raises the density to the kernel of a segment wherever the kernel is larger. A segment thinner than
    /// a cell that no other segment joins gets a kernel wide enough for the samples, without swelling the
    /// surface where it is inside other segments.
    fn floor(&mut self, index: u32, meshing: &Meshing, segment: &Segment, kernel: Kernel) {
        let reach = kernel.reach;
        let lo_world = segment.start.min(segment.end) - Vec3::splat(reach);
        let hi_world = segment.start.max(segment.end) + Vec3::splat(reach);
        let to_sample = |v: Vec3, round: fn(f32) -> f32| {
            [0, 1, 2].map(|a| (round((v[a] - self.origin[a]) / self.cell).max(0.0) as usize).min(self.samples[a] - 1))
        };
        let (lo, hi) = (to_sample(lo_world, f32::ceil), to_sample(hi_world, f32::floor));
        self.allocate(lo.map(|v| v.saturating_sub(1)), hi);
        let axis = segment.end - segment.start;
        let length_sq = axis.length_squared().max(f32::EPSILON);
        for z in lo[2]..=hi[2] {
            for y in lo[1]..=hi[1] {
                for x in lo[0]..=hi[0] {
                    let p = self.position(Vec3::new(x as f32, y as f32, z as f32));
                    let t = ((p - segment.start).dot(axis) / length_sq).clamp(0.0, 1.0);
                    let falloff = (1.0 - (p - segment.start - axis * t).length_squared() / (reach * reach)).max(0.0);
                    let k = kernel.weight * falloff * falloff * falloff;
                    if k <= 0.0 {
                        continue;
                    }
                    let i = self.slot([x, y, z]) as usize * BLOCK_SAMPLES + Self::local([x, y, z]);
                    self.density[i] = self.density[i].max(k);
                    if k * meshing.priority > self.owner_k[i] {
                        self.owner_k[i] = k * meshing.priority;
                        self.owner[i] = index;
                    }
                }
            }
        }
    }

    /// Adds the kernel of one segment to every sample it reaches. Where the segment before it in
    /// the same path reached the sample too, only the larger of the two counts, see
    /// [`Meshing::joined`]. Summing them would count every joint twice and swell the path
    /// beyond its radius, the "bulging" of summed distance surfaces (Zanni et al. 2013,
    /// "SCALe-invariant Integral Surfaces", §2.1).
    ///
    /// A closed segment instead adds its distance to `tube` out to `closed.reach`
    /// and only uses the kernel to claim samples. Past the kernel it claims them
    /// with a tiny weight, so that the closing has an owner where it fills a gap,
    /// but no blended segment loses a sample to it.
    fn splat(&mut self, index: u32, meshing: &Meshing, segment: &Segment, kernel: Kernel, closed: Option<Closed>) {
        let priority = meshing.priority;
        let weight = kernel.weight;
        let reach = closed.map_or(kernel.reach, |c| c.reach.max(kernel.reach));
        let lo_world = segment.start.min(segment.end) - Vec3::splat(reach);
        let hi_world = segment.start.max(segment.end) + Vec3::splat(reach);
        let to_sample = |v: Vec3, round: fn(f32) -> f32| {
            [0, 1, 2].map(|a| (round((v[a] - self.origin[a]) / self.cell).max(0.0) as usize).min(self.samples[a] - 1))
        };
        let lo = to_sample(lo_world, f32::ceil);
        let hi = to_sample(hi_world, f32::floor);
        // one more block row below, so every cell next to an inside sample has a vertex slot
        self.allocate(lo.map(|v| v.saturating_sub(1)), hi);

        let axis = segment.end - segment.start;
        let length_sq = axis.length_squared().max(f32::EPSILON);
        let reach_sq = reach * reach;
        for z in lo[2]..=hi[2] {
            for y in lo[1]..=hi[1] {
                for x in lo[0]..=hi[0] {
                    let p = self.position(Vec3::new(x as f32, y as f32, z as f32));
                    let t = ((p - segment.start).dot(axis) / length_sq).clamp(0.0, 1.0);
                    let d_sq = (p - segment.start - axis * t).length_squared();
                    if d_sq >= reach_sq {
                        continue;
                    }
                    let i = self.slot([x, y, z]) as usize * BLOCK_SAMPLES + Self::local([x, y, z]);
                    let k = match closed {
                        Some(c) => self.close_sample(i, d_sq, c, kernel, reach),
                        None => {
                            let k = weight * falloff(d_sq, reach * reach);
                            if meshing.joined && self.run[i] == index.wrapping_sub(1) {
                                // a run of joined segments that reach the sample one after another
                                // is one tube, a path that comes back to it starts a new run
                                let largest = self.run_k[i].max(k);
                                self.density[i] += largest - self.run_k[i];
                                self.run_k[i] = largest;
                            } else {
                                self.density[i] += k;
                                self.run_k[i] = k;
                            }
                            self.run[i] = index;
                            k
                        }
                    };
                    self.claim(i, index, k * priority);
                }
            }
        }
    }

    fn claim(&mut self, i: usize, index: u32, k: f32) {
        if k > self.owner_k[i] {
            self.owner_k[i] = k;
            self.owner[i] = index;
        }
    }

    /// What a closed segment does at a sample `d_sq` from its axis, inside its `reach`, see [`SparseGrid::splat`]:
    /// the tube and the strand, and the kernel it claims the sample with
    fn close_sample(&mut self, i: usize, d_sq: f32, c: Closed, kernel: Kernel, reach: f32) -> f32 {
        let tube = d_sq.sqrt() - c.radius;
        if tube < self.tube[i] {
            self.tube[i] = tube;
            self.tube_close[i] = c.close;
        }
        if let Some(radius) = c.strand {
            self.strand[i] = self.strand[i].min(d_sq.sqrt() - radius);
        }
        if d_sq < kernel.reach * kernel.reach {
            kernel.weight * falloff(d_sq, kernel.reach * kernel.reach)
        } else {
            kernel.weight * FILL_OWNER_WEIGHT * falloff(d_sq, reach * reach)
        }
    }

    /// [`SparseGrid::splat`] of every closed segment in turn on a grid that has none yet, without strands. In
    /// dense tissue hundreds of segments reach a sample but the few nearest decide its tube and its owner, so
    /// every 2³ samples look up the segments that reach them in a [`TubeTree`] and every sample goes through
    /// them from the nearest on, until the rest are too far to be nearer or to claim it by more.
    fn splat_closed(&mut self, splats: &[ClosedSplat]) {
        debug_assert!(splats.iter().all(|s| s.closed.strand.is_none()));
        let to_sample = |v: Vec3, round: fn(f32) -> f32| {
            [0, 1, 2].map(|a| (round((v[a] - self.origin[a]) / self.cell).max(0.0) as usize).min(self.samples[a] - 1))
        };
        let boxes: Vec<([usize; 3], [usize; 3])> = splats
            .iter()
            .map(|s| {
                let reach = Vec3::splat(s.reach);
                let (start, end) = (s.segment.start, s.segment.end);
                (to_sample(start.min(end) - reach, f32::ceil), to_sample(start.max(end) + reach, f32::floor))
            })
            .collect();
        for &(lo, hi) in &boxes {
            self.allocate(lo.map(|v| v.saturating_sub(1)), hi);
        }
        let mut tree = TubeTree::new(
            splats.iter().enumerate().map(|(k, s)| (s.segment.start, s.segment.end, s.closed.radius, k as u32)).collect(),
        );
        // in the order of the tree, the segments a search finds lie close together
        let order = tree.number_in_order();
        let splats: Vec<ClosedSplat> = order.iter().map(|&k| splats[k as usize]).collect();
        let boxes: Vec<([usize; 3], [usize; 3])> = order.iter().map(|&k| boxes[k as usize]).collect();
        let max = |f: fn(&ClosedSplat) -> f32| splats.iter().map(f).fold(0.0f32, f32::max);
        let (reach_max, radius_max) = (max(|s| s.reach), max(|s| s.closed.radius));
        let (kernel_max, weight_max) = (max(|s| s.kernel.reach), max(|s| s.weight()));
        // a little more than half the diagonal of 2³ samples, every one of them is that close to their centre
        let half = 0.87 * self.cell;
        // the bounds are only so exact, segments within the margins are looked at
        let (tube_margin, owner_margin) = (1e-3 * self.cell, 1e-5 * weight_max);
        // how far from the centre of the samples a segment may be and still be nearer than `tube` or claim one
        // of them by more than `owner`
        let search = |tube: f32, owner: f32| {
            let need = owner - owner_margin;
            let owner_reach = if need <= 0.0 {
                reach_max
            } else {
                let within =
                    |reach: f32, weight: f32| if need < weight { reach * (1.0 - (need / weight).cbrt()).sqrt() } else { 0.0 };
                larger(within(kernel_max, weight_max), within(reach_max, weight_max * FILL_OWNER_WEIGHT)) * 1.001
            };
            smaller(larger(tube + radius_max + tube_margin, owner_reach), reach_max) + half
        };
        let ring = 0.25 * self.cell;

        let mut found: Vec<(f32, u32)> = Vec::new();
        let mut ordered: Vec<(f32, u32)> = Vec::new();
        let mut starts: Vec<usize> = Vec::new();
        let [bx, by, _] = self.blocks;
        let tiles = BLOCK / 2;
        for b in 0..self.block_slots.len() {
            let slot = self.block_slots[b];
            if slot == EMPTY {
                continue;
            }
            let base = [b % bx * BLOCK, b / bx % by * BLOCK, b / (bx * by) * BLOCK];
            for tile in 0..tiles * tiles * tiles {
                let t = [base[0] + 2 * (tile % tiles), base[1] + 2 * (tile / tiles % tiles), base[2] + 2 * (tile / (tiles * tiles))];
                if (0..3).any(|a| t[a] >= self.samples[a]) {
                    continue;
                }
                let centre = self.position(Vec3::new(t[0] as f32, t[1] as f32, t[2] as f32) + Vec3::splat(0.5));
                // a segment that reaches all the samples bounds their nearest tube and how much their owner claims them
                let (mut tube, mut owner) = (f32::INFINITY, 0.0f32);
                let mut reach = search(tube, owner);
                found.clear();
                tree.for_each_within(centre, reach, |k, d| {
                    let s = &splats[k as usize];
                    if d + half + tube_margin < s.reach {
                        let (nearer, more) = (d + half - s.closed.radius, s.least_claim(larger(d - half, 0.0), d + half));
                        if nearer < tube || more > owner {
                            (tube, owner) = (smaller(tube, nearer), larger(owner, more));
                            reach = search(tube, owner);
                        }
                    }
                    if d - half < s.reach {
                        found.push((d, k));
                    }
                    reach
                });
                // nearest first in rings a quarter cell wide, a sample stops at the first ring too far to change it
                let rings = (reach / ring) as usize + 1;
                starts.clear();
                starts.resize(rings, 0);
                for &(d, _) in found.iter().filter(|f| f.0 < reach) {
                    starts[(d / ring) as usize] += 1;
                }
                let mut total = 0;
                for start in starts.iter_mut() {
                    (*start, total) = (total, total + *start);
                }
                ordered.resize(total, (0.0, 0));
                for &(d, k) in found.iter().filter(|f| f.0 < reach) {
                    let r = (d / ring) as usize;
                    ordered[starts[r]] = (d, k);
                    starts[r] += 1;
                }
                // `starts[r]` is now where ring `r` ends
                for local in 0..8 {
                    let x = [t[0] + local % 2, t[1] + local / 2 % 2, t[2] + local / 4];
                    if (0..3).any(|a| x[a] >= self.samples[a]) {
                        continue;
                    }
                    let p = self.position(Vec3::new(x[0] as f32, x[1] as f32, x[2] as f32));
                    // the first segment wins a tie, as splatting them in turn has it
                    let (mut tube, mut tube_by, mut owner, mut owner_by) = (f32::INFINITY, None, 0.0f32, None);
                    let mut reach = f32::INFINITY;
                    for r in 0..rings {
                        if r as f32 * ring >= reach {
                            break;
                        }
                        let from = if r == 0 { 0 } else { starts[r - 1] };
                        for &(d, k) in &ordered[from..starts[r]] {
                            if d >= reach {
                                continue;
                            }
                            let k = k as usize;
                            let (lo, hi) = boxes[k];
                            if (0..3).any(|a| x[a] < lo[a] || x[a] > hi[a]) {
                                continue;
                            }
                            let s = &splats[k];
                            let d_sq = s.distance_sq(p);
                            if d_sq >= s.reach_sq {
                                continue;
                            }
                            let nearer = d_sq.sqrt() - s.closed.radius;
                            let claim = s.claim(d_sq);
                            let mut changed = false;
                            if nearer < tube || (nearer == tube && tube_by.is_some_and(|(index, _)| s.index < index)) {
                                (tube, tube_by, changed) = (nearer, Some((s.index, s.closed.close)), true);
                            }
                            if claim > owner || (claim == owner && owner_by.is_some_and(|index| s.index < index)) {
                                (owner, owner_by, changed) = (claim, Some(s.index), true);
                            }
                            if changed {
                                reach = search(tube, owner);
                            }
                        }
                    }
                    let i = slot as usize * BLOCK_SAMPLES + Self::local(x);
                    if let Some((_, close)) = tube_by {
                        self.tube[i] = tube;
                        self.tube_close[i] = close;
                    }
                    if let Some(index) = owner_by {
                        self.owner_k[i] = owner;
                        self.owner[i] = index;
                    }
                }
            }
        }
    }
}

/// `(1 - d² / reach²)³`, the kernel of a segment at `d_sq` from it
fn falloff(d_sq: f32, reach_sq: f32) -> f32 {
    let k = larger(1.0 - d_sq / reach_sq, 0.0);
    k * k * k
}

/// A closed segment and how it is splatted, see [`SparseGrid::splat_closed`]
#[derive(Clone, Copy)]
struct ClosedSplat {
    index: u32,
    segment: Segment,
    priority: f32,
    kernel: Kernel,
    closed: Closed,
    reach: f32,
    reach_sq: f32,
    kernel_sq: f32,
    axis: Vec3,
    length_sq: f32,
}

impl ClosedSplat {
    fn new(index: u32, meshing: &Meshing, segment: Segment, kernel: Kernel, closed: Closed) -> ClosedSplat {
        let reach = closed.reach.max(kernel.reach);
        let axis = segment.end - segment.start;
        ClosedSplat {
            index,
            segment,
            priority: meshing.priority,
            kernel,
            closed,
            reach,
            reach_sq: reach * reach,
            kernel_sq: kernel.reach * kernel.reach,
            axis,
            length_sq: axis.length_squared().max(f32::EPSILON),
        }
    }

    fn weight(&self) -> f32 {
        (self.kernel.weight * self.priority).max(0.0)
    }

    /// Squared distance from `p` to the axis, as [`SparseGrid::splat`] has it
    fn distance_sq(&self, p: Vec3) -> f32 {
        let t = unit((p - self.segment.start).dot(self.axis) / self.length_sq);
        (p - self.segment.start - self.axis * t).length_squared()
    }

    /// The least it claims a sample with that lies `closest` to `farthest` from its axis, inside its reach
    fn least_claim(&self, closest: f32, farthest: f32) -> f32 {
        let kernel = self.kernel.reach;
        if farthest < kernel {
            self.weight() * falloff(farthest * farthest, self.kernel_sq)
        } else if closest >= kernel {
            self.weight() * FILL_OWNER_WEIGHT * falloff(farthest * farthest, self.reach_sq)
        } else {
            0.0
        }
    }

    /// What it claims a sample `d_sq` from its axis with, as [`SparseGrid::splat`] has it
    fn claim(&self, d_sq: f32) -> f32 {
        let k = if d_sq < self.kernel_sq {
            self.kernel.weight * falloff(d_sq, self.kernel_sq)
        } else {
            self.kernel.weight * FILL_OWNER_WEIGHT * falloff(d_sq, self.reach_sq)
        };
        k * self.priority
    }
}

/// Squared distance transform along one line (Felzenszwalb, Huttenlocher), every
/// sample gets the squared distance to and the feature of the nearest seed
fn transform_line(d: &mut [f32], feature: &mut [u32], v: &mut Vec<usize>, z: &mut Vec<f32>, out: &mut Vec<(f32, u32)>) {
    v.clear();
    z.clear();
    for q in 0..d.len() {
        if !d[q].is_finite() {
            continue;
        }
        let height = |i: usize| d[i] + (i * i) as f32;
        loop {
            let Some(&last) = v.last() else {
                v.push(q);
                z.push(f32::NEG_INFINITY);
                break;
            };
            let at = (height(q) - height(last)) / (2.0 * (q - last) as f32);
            if at <= *z.last().unwrap() {
                v.pop();
                z.pop();
            } else {
                v.push(q);
                z.push(at);
                break;
            }
        }
    }
    if v.is_empty() {
        return;
    }
    out.clear();
    let mut k = 0;
    for q in 0..d.len() {
        while k + 1 < v.len() && z[k + 1] < q as f32 {
            k += 1;
        }
        let offset = q as f32 - v[k] as f32;
        out.push((offset * offset + d[v[k]], feature[v[k]]));
    }
    for (q, &(dq, fq)) in out.iter().enumerate() {
        d[q] = dq;
        feature[q] = fq;
    }
}

/// Squared distance in samples from every sample to the nearest one with a
/// finite `d`, plus `d` there, and the `feature` of that sample
fn distance_transform(d: &mut [f32], feature: &mut [u32], samples: [usize; 3]) {
    let [nx, ny, nz] = samples;
    let n = d.len();
    let (mut v, mut z, mut out) = (Vec::new(), Vec::new(), Vec::new());
    let (mut line_d, mut line_f) = (Vec::new(), Vec::new());
    for (len, stride) in [(nx, 1), (ny, nx), (nz, nx * ny)] {
        let starts = (0..n / (len * stride)).flat_map(|outer| (0..stride).map(move |inner| outer * len * stride + inner));
        for start in starts {
            line_d.clear();
            line_f.clear();
            line_d.extend((0..len).map(|k| d[start + k * stride]));
            line_f.extend((0..len).map(|k| feature[start + k * stride]));
            transform_line(&mut line_d, &mut line_f, &mut v, &mut z, &mut out);
            for k in 0..len {
                d[start + k * stride] = line_d[k];
                feature[start + k * stride] = line_f[k];
            }
        }
    }
}

/// Morphological closing of the tubes of the closed segments, blurred: per
/// sample its share of tissue, the surface of the tissue is where it reaches
/// [`SURFACE_SHARE`].
///
/// Outside the closing are the points within `rho` of a point whose distance
/// to the tubes is more than `rho`, so an empty ball of radius `rho` fits
/// around them. Gaps the ball can't pass through fill, while convex parts keep
/// their size. `rho` is the closing radius of the segment whose tube is the
/// nearest to the centre of the ball, so the gaps between the segments of each
/// closing radius fill as if it were the only one. Where no tube is near, the
/// balls take the largest radius. The closing stays [`GUARD`] times `rho` away
/// from the axes of the other segments, and its blur is [`SMOOTHING`] times
/// `rho`, see [`blur_by_radius`].
///
/// Every sample farther than its `rho` from the tubes is the centre of an empty
/// ball as large as that distance, the balls of `rho` around the centres near
/// it lie in it. Every other sample takes the ball that covers it most, by the
/// power diagram of the balls: the distance transform starts at `−R²` at every
/// centre (Felzenszwalb, Huttenlocher). Where the closing is just the tubes,
/// the distance to that ball is the distance to the tubes at any cell size, the
/// nearest centre by grid steps would lie up to half a cell off and swell it.
fn closing(grid: &SparseGrid, frame: &Frame) -> Vec<f32> {
    let [nx, ny, nz] = grid.samples;
    let n = nx * ny * nz;
    // the tube distance at a sample and the closing radius there, in world units
    let tube_at = |grid: &SparseGrid, i: usize| {
        grid.index([i % nx, i / nx % ny, i / (nx * ny)])
            .map(|s| (grid.tube[s], grid.tube_close[s]))
            .filter(|(t, _)| t.is_finite())
    };
    // every tube reaches the largest radius and a little further, past that a ball of the largest radius is empty
    let rho_far = frame.close_max;
    // the radius in cells of the empty ball around a centre, where no tube reaches the sample it is at least as
    // far as every tube reaches
    let ball = |grid: &SparseGrid, i: usize| {
        let (tube, rho) = tube_at(grid, i).unwrap_or((rho_far + 2.0 * grid.cell, rho_far));
        (tube > rho).then_some(tube / grid.cell)
    };

    // the centre of the empty ball that covers each sample most, by squared distance in cells less the squared
    // radius of the ball
    let mut d = vec![f32::INFINITY; n];
    let mut feature = vec![EMPTY; n];
    for i in 0..n {
        if let Some(r) = ball(grid, i) {
            d[i] = -r * r;
            feature[i] = i as u32;
        }
    }
    distance_transform(&mut d, &mut feature, grid.samples);

    // share of every sample inside the closing, ramping from 0 to 1 over two cells across its surface
    let mut shifts: Vec<(f32, f32)> = Vec::new();
    let position = |i: usize| Vec3::new((i % nx) as f32, (i / nx % ny) as f32, (i / (nx * ny)) as f32);
    let mut solid = d;
    for i in 0..n {
        let Some(slot) = grid.index([i % nx, i / nx % ny, i / (nx * ny)]) else {
            solid[i] = 0.0;
            continue;
        };
        let tube = grid.tube[slot] / grid.cell;
        if !tube.is_finite() {
            solid[i] = 0.0;
            continue;
        }
        let rho = grid.tube_close[slot];
        let shift = match shifts.iter().find(|(r, _)| *r == rho) {
            Some(&(_, shift)) => shift,
            None => {
                let shift = ramp_shift(SMOOTHING * rho / grid.cell);
                shifts.push((rho, shift));
                shift
            }
        };
        // signed distance to the closed surface in cells, positive outside
        let outside = if tube > rho / grid.cell || feature[i] == EMPTY {
            tube
        } else {
            let f = feature[i] as usize;
            // no point of the closed surface is farther than the tubes
            (ball(grid, f).unwrap_or(0.0) - position(i).distance(position(f))).min(tube)
        };
        let allowed = grid.guard[slot] / grid.cell - GUARD * rho / grid.cell;
        let closing = (0.5 - 0.5 * (outside + shift)).min(0.5 + 0.5 * (allowed - shift));
        solid[i] = closing.max(0.5 - 0.5 * (tube + shift)).clamp(0.0, 1.0);
    }
    blur_by_radius(&mut solid, grid, frame);
    solid
}

/// Blurs the closing by [`SMOOTHING`] times the closing radius of every sample, so a tissue that
/// fills wider gaps is as smooth as it would be alone. Every radius blurs the whole closing,
/// and every sample takes the blurs weighted by how many samples of each radius are around
/// it, blurred by the largest blur: where tissues of different radii meet, the surface goes
/// over from the one blur to the other within about that blur.
fn blur_by_radius(solid: &mut Vec<f32>, grid: &SparseGrid, frame: &Frame) {
    let [nx, ny, _] = grid.samples;
    let radius_at = |i: usize| {
        grid.index([i % nx, i / nx % ny, i / (nx * ny)]).filter(|&s| grid.tube[s].is_finite()).map(|s| grid.tube_close[s])
    };
    let mut radii: Vec<f32> = Vec::new();
    for rho in (0..solid.len()).filter_map(radius_at) {
        if !radii.contains(&rho) {
            radii.push(rho);
        }
    }
    radii.sort_by(f32::total_cmp);
    if radii.len() < 2 {
        let rho = radii.first().copied().unwrap_or(frame.close_min);
        blur(solid, grid.samples, SMOOTHING * rho / grid.cell);
        return;
    }
    let sharp = std::mem::take(solid);
    let mut sum = vec![0.0f32; sharp.len()];
    let mut total = vec![0.0f32; sharp.len()];
    for &rho in &radii {
        let mut weight: Vec<f32> = (0..sharp.len()).map(|i| if radius_at(i) == Some(rho) { 1.0 } else { 0.0 }).collect();
        blur(&mut weight, grid.samples, SMOOTHING * frame.close_max / grid.cell);
        let mut blurred = sharp.clone();
        blur(&mut blurred, grid.samples, SMOOTHING * rho / grid.cell);
        for i in 0..sharp.len() {
            sum[i] += weight[i] * blurred[i];
            total[i] += weight[i];
        }
    }
    *solid = sum.iter().zip(&total).map(|(&s, &t)| if t > 0.0 { s / t } else { 0.0 }).collect();
}

/// Adds the closed tissue to the density so that its surface lies at `threshold`.
///
/// The blur wears away thin parts and absorbs the hyphae that stand out of the
/// tissue. Only the strand segments keep their own tube outside it, which ones
/// are strands is decided before, see [`TissueField`]. Other stubs, such as a
/// tip that ends just outside the tissue or a hypha lying on it, would show as
/// dots and dashes on a smooth surface.
fn add_tissue(grid: &mut SparseGrid, solid: &[f32], threshold: f32) {
    let [nx, ny, _] = grid.samples;
    for (i, &v) in solid.iter().enumerate() {
        if let Some(slot) = grid.index([i % nx, i / nx % ny, i / (nx * ny)]) {
            let mut share = v / SURFACE_SHARE;
            if v < SURFACE_SHARE {
                share = share.max(2.0 * (0.5 - 0.5 * grid.strand[slot] / grid.cell).clamp(0.0, 1.0));
            }
            grid.density[slot] += threshold * share;
        }
    }
}

/// The union of balls around samples, every one grown by `scale`, as the lower
/// envelope of parabolas (Felzenszwalb, Huttenlocher): a ball of radius `R`
/// around `x` is where `|q - x|² - (scale R)² <= 0`. `radius` is in cells, a
/// sample without a ball has radius 0.
///
/// Per sample it gives the smallest `|q - x|² - (scale R(x))²` of all balls, at
/// most 0 inside their union, and the centre `x` of that ball. Growing only the
/// medial balls gives the same union as growing all of them, since a ball inside
/// another stays inside it when both grow, so no medial axis is needed.
fn grown_balls(radius: &[f32], samples: [usize; 3], scale: f32) -> (Vec<f32>, Vec<u32>) {
    let mut power: Vec<f32> =
        radius.iter().map(|&r| if r > 0.0 { -(scale * r) * (scale * r) } else { f32::INFINITY }).collect();
    let mut centre: Vec<u32> = (0..radius.len()).map(|i| if radius[i] > 0.0 { i as u32 } else { EMPTY }).collect();
    distance_transform(&mut power, &mut centre, samples);
    (power, centre)
}

/// The tissue the closed segments form, and whether a strand leaving it is a
/// small feature next to a large one.
///
/// This is the scale axis rule (Miklos, Giesen, Pauly 2010, "Discrete scale axis
/// representations for 3D geometry"): every ball inside the tissue grows by
/// `scale`. A strand the grown tissue covers is absorbed, one it doesn't cover
/// is a free strand. A strand standing `h` out of tissue whose inner ball there
/// has radius `R` is absorbed while `h < (scale - 1) R`, so a thick stipe absorbs
/// longer strays than a thin cap margin, and a thin bundle far from any tissue
/// stays. Nothing depends on the size of the scene.
pub struct TissueField {
    frame: Frame,
    /// The closed segments on the grid of the frame, the start of a mesh of the same segments
    grid: SparseGrid,
    /// Share of tissue per sample, the surface lies at [`SURFACE_SHARE`]
    solid: Vec<f32>,
    /// Per sample how deep it lies in the tissue, the radius of its ball
    depth: Vec<f32>,
    /// Per sample the centre of the grown ball that covers it best, `EMPTY` where no ball is
    centre: Vec<u32>,
    scale: f32,
    any: bool,
}

impl TissueField {
    /// The tissue of the closed segments, closed and blurred on the grid of
    /// `frame` as [`surface_in`] does it. Its balls grow by `scale`, at infinity
    /// it covers everything. `None` when nothing is closed.
    pub fn new(frame: &Frame, segments: &[Segment], meshing: &[Meshing], scale: f32) -> Option<TissueField> {
        if !frame.closing {
            return None;
        }
        let mut grid = frame.grid();
        let closed: Vec<ClosedSplat> = segments
            .iter()
            .zip(meshing)
            .enumerate()
            .filter(|(_, (_, m))| frame.closes(m))
            .map(|(i, (s, m))| ClosedSplat::new(i as u32, m, *s, frame.owner_kernel(m, s), frame.closed(m, s, false)))
            .collect();
        grid.splat_closed(&closed);
        frame.guard(&mut grid, segments, meshing);
        let solid = closing(&grid, frame);

        // the surface lies between a tissue sample and the next one outside, so half a cell above the outer samples
        let n = solid.len();
        let mut depth: Vec<f32> = solid.iter().map(|&v| if v < SURFACE_SHARE { 0.0 } else { f32::INFINITY }).collect();
        distance_transform(&mut depth, &mut vec![EMPTY; n], grid.samples);
        for d in &mut depth {
            *d = if *d > 0.0 && d.is_finite() { d.sqrt() - 0.5 } else { 0.0 };
        }
        let any = depth.iter().any(|&d| d > 0.0);
        let centre = if any && scale.is_finite() { grown_balls(&depth, grid.samples, scale.max(1.0)).1 } else { Vec::new() };
        for d in &mut depth {
            *d *= grid.cell;
        }
        Some(TissueField { frame: frame.clone(), grid, solid, depth, centre, scale, any })
    }

    /// [`surface_in`] of the segments the field was made of, without closing them again.
    /// `meshing` must be the same as for the field.
    pub fn surface(self, segments: &[Segment], meshing: &[Meshing], strand: impl Fn(usize) -> bool) -> SegmentSurface {
        let TissueField { frame, mut grid, solid, .. } = self;
        for (i, (s, m)) in segments.iter().zip(meshing).enumerate() {
            if !frame.closes(m) {
                grid.splat(i as u32, m, s, frame.kernel(m, s.radius), None);
            } else if strand(i) {
                // the tube and the owners are there already, splatting them again only adds the strand
                grid.splat(i as u32, m, s, frame.owner_kernel(m, s), Some(frame.closed(m, s, true)));
            }
        }
        add_tissue(&mut grid, &solid, frame.threshold);
        frame.floor(&mut grid, segments, meshing);
        extract(&grid, &frame)
    }

    /// Whether there is any tissue at all
    pub fn has_tissue(&self) -> bool {
        self.any
    }

    fn index(&self, p: [usize; 3]) -> usize {
        p[0] + self.frame.samples[0] * (p[1] + self.frame.samples[1] * p[2])
    }

    /// The sample below `p` and where `p` lies in the cell above it
    fn cell_of(&self, p: Vec3) -> ([usize; 3], Vec3) {
        let u = (p - self.frame.origin) / self.frame.cell;
        let base = [0, 1, 2].map(|a| (u[a].floor().max(0.0) as usize).min(self.frame.samples[a] - 2));
        (base, (u - Vec3::from(base.map(|v| v as f32))).clamp(Vec3::ZERO, Vec3::ONE))
    }

    fn share(&self, p: Vec3) -> f32 {
        let (base, f) = self.cell_of(p);
        CORNERS
            .iter()
            .map(|o| {
                let w = [0, 1, 2].map(|a| if o[a] == 1 { f[a] } else { 1.0 - f[a] });
                w[0] * w[1] * w[2] * self.solid[self.index([base[0] + o[0], base[1] + o[1], base[2] + o[2]])]
            })
            .sum()
    }

    /// Whether `p` lies in the tissue
    pub fn inside(&self, p: Vec3) -> bool {
        self.share(p) >= SURFACE_SHARE
    }

    /// Where the line from `inside` to `outside` crosses the surface, as a share of the way
    pub fn crossing(&self, inside: Vec3, outside: Vec3) -> f32 {
        let (mut lo, mut hi) = (0.0, 1.0);
        for _ in 0..8 {
            let mid = 0.5 * (lo + hi);
            if self.inside(inside.lerp(outside, mid)) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        0.5 * (lo + hi)
    }

    /// Whether the grown tissue covers the ball of `radius` around `p`, grown as
    /// well: a strand there is a small feature next to the tissue
    pub fn covers(&self, p: Vec3, radius: f32) -> bool {
        if !self.scale.is_finite() {
            return self.any;
        }
        if self.centre.is_empty() {
            return false;
        }
        let [nx, ny, _] = self.frame.samples;
        let (base, _) = self.cell_of(p);
        CORNERS.iter().any(|o| {
            let c = self.centre[self.index([base[0] + o[0], base[1] + o[1], base[2] + o[2]])];
            if c == EMPTY {
                return false;
            }
            let c = c as usize;
            let at = self.frame.origin
                + Vec3::new((c % nx) as f32, (c / nx % ny) as f32, (c / (nx * ny)) as f32) * self.frame.cell;
            p.distance(at) + self.scale * radius <= self.scale * self.depth[c]
        })
    }
}

/// The weights of a gaussian of `sigma` samples from `-radius` to `radius`, none when it is too narrow to blur
fn blur_weights(sigma: f32) -> Vec<f32> {
    if sigma < 0.25 {
        return Vec::new();
    }
    let radius = (3.0 * sigma).ceil() as isize;
    let weights: Vec<f32> = (-radius..=radius).map(|i| (-(i * i) as f32 / (2.0 * sigma * sigma)).exp()).collect();
    let total: f32 = weights.iter().sum();
    weights.iter().map(|w| w / total).collect()
}

/// How far, in cells, the ramp of the share in [`closing`] moves inwards so that after the blur of `sigma` cells
/// the share reaches [`SURFACE_SHARE`] as far outside the closed surface as a blur of `sigma` alone puts it.
///
/// The ramp is a cell wide on either side of the surface, as if the surface lay anywhere in there, and the blur
/// spreads it further: the share at `x` is the chance that a uniform `U(-1, 1)` plus the blur's normal `N(0, σ²)`
/// lies beyond `x`. Without the shift the ramp alone put the surface 0.4 cells out where the blur is skipped, so it
/// moved out as the cells grew.
fn ramp_shift(sigma: f32) -> f32 {
    let weights = blur_weights(sigma);
    let radius = (weights.len() / 2) as f32;
    let blurred: f32 = weights.iter().enumerate().map(|(k, w)| w * (k as f32 - radius).powi(2)).sum();
    level(1.0, blurred.sqrt() as f64) - level(0.0, sigma as f64)
}

/// Where the share of a surface spread by `U(-ramp, ramp) + N(0, sigma²)` drops to [`SURFACE_SHARE`]
fn level(ramp: f64, sigma: f64) -> f32 {
    let normal = |z: f64| 0.5 * (1.0 + erf(z / std::f64::consts::SQRT_2));
    let density = |z: f64| (-0.5 * z * z).exp() / (2.0 * std::f64::consts::PI).sqrt();
    // the integral of the normal distribution
    let integral = |z: f64| z * normal(z) + density(z);
    let below = |x: f64| {
        if sigma < 1e-6 {
            if ramp < 1e-6 { (x >= 0.0) as u8 as f64 } else { ((x + ramp) / (2.0 * ramp)).clamp(0.0, 1.0) }
        } else if ramp < 1e-6 {
            normal(x / sigma)
        } else {
            sigma / (2.0 * ramp) * (integral((x + ramp) / sigma) - integral((x - ramp) / sigma))
        }
    };
    let target = 1.0 - SURFACE_SHARE as f64;
    let (mut lo, mut hi) = (-ramp - 8.0 * sigma - 1.0, ramp + 8.0 * sigma + 1.0);
    for _ in 0..60 {
        let mid = 0.5 * (lo + hi);
        if below(mid) < target {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    (0.5 * (lo + hi)) as f32
}

/// The error function (Abramowitz, Stegun 7.1.26), to about 1.5e-7
fn erf(x: f64) -> f64 {
    let t = 1.0 / (1.0 + 0.327_591_1 * x.abs());
    let poly = t * (0.254_829_592 + t * (-0.284_496_736 + t * (1.421_413_741 + t * (-1.453_152_027 + t * 1.061_405_429))));
    (1.0 - poly * (-x * x).exp()).copysign(x)
}

/// Separable gaussian blur of a dense grid, `sigma` in samples
fn blur(values: &mut [f32], samples: [usize; 3], sigma: f32) {
    let weights = blur_weights(sigma);
    if weights.is_empty() {
        return;
    }
    let radius = weights.len() / 2;
    let [nx, ny, nz] = samples;
    // the taps of a sample `k` along a line of `len` that lie on the line
    let taps = |k: usize, len: usize| radius.saturating_sub(k)..weights.len().min(len + radius - k);
    let mut line = vec![0.0; nx];
    for row in values.chunks_exact_mut(nx) {
        line.copy_from_slice(row);
        for (k, out) in row.iter_mut().enumerate() {
            let mut sum = 0.0;
            for o in taps(k, nx) {
                sum += weights[o] * line[k + o - radius];
            }
            *out = sum;
        }
    }
    // along y and z whole rows at once, the samples of a row lie next to each other
    let mut source = values.to_vec();
    for (len, stride) in [(ny, nx), (nz, nx * ny)] {
        source.copy_from_slice(values);
        for (outer, out) in values.chunks_exact_mut(len * stride).enumerate() {
            let source = &source[outer * len * stride..(outer + 1) * len * stride];
            for (k, row) in out.chunks_exact_mut(stride).enumerate() {
                row.fill(0.0);
                for o in taps(k, len) {
                    let from = &source[(k + o - radius) * stride..(k + o - radius + 1) * stride];
                    for (v, &s) in row.iter_mut().zip(from) {
                        *v += weights[o] * s;
                    }
                }
            }
        }
    }
}

/// Meshes the blended surface around segments: every segment adds a compact
/// kernel `(1 - d²/R²)³` to a sparse density grid and Surface Nets extracts the
/// level set. `resolution` is the number of cells along the longest side.
///
/// The kernel of a segment reaches `blend` times its radius, so segments closer
/// than that join into one surface. The threshold is chosen so that a lone
/// segment's surface still sits at its radius, whatever `blend` is.
pub fn mesh_segments(segments: &[Segment], resolution: usize, blend: f32) -> Mesh {
    surface(segments, resolution, blend, 0.0, |_| false, |_| false, |_| 1.0).mesh
}

/// A mesh from [`surface`] with what each vertex was made of
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SegmentSurface {
    pub mesh: Mesh,
    /// Per vertex the index of the segment that contributes most to it
    pub owners: Vec<u32>,
    /// Edge length of the grid cells, the smallest detail the mesh can show
    pub cell: f32,
}

/// The grid segments are meshed on, see [`Frame::new`]
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    /// The largest reach of a kernel, in radii
    reach_factor: f32,
    /// Density of the surface, the one around a lone segment of the largest reach
    threshold: f32,
    /// The largest and the smallest radius of the closing of a closed segment
    close_max: f32,
    close_min: f32,
    /// Whether any segment is closed into a tissue
    closing: bool,
    cell: f32,
    min_radius: f32,
    origin: Vec3,
    samples: [usize; 3],
}

impl Frame {
    /// The grid around `segments`, with `resolution` cells along its longest side, and how
    /// they are meshed on it, see [`surface`]
    pub fn new(segments: &[Segment], meshing: &[Meshing], resolution: usize) -> Option<Frame> {
        if segments.is_empty() {
            return None;
        }
        let reach_factor = meshing.iter().map(|m| m.blend.max(1.1)).fold(1.1, f32::max);
        let threshold = threshold(reach_factor);
        let closes = |m: &&Meshing| m.closed && m.close > 0.0;
        let closing = meshing.iter().any(|m| closes(&m));
        let (close_min, close_max) = if closing {
            meshing.iter().filter(closes).fold((f32::INFINITY, 0.0f32), |(lo, hi), m| (lo.min(m.close), hi.max(m.close)))
        } else {
            (0.0, 0.0)
        };
        let (mut min, mut max) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
        let mut widest = 0.0f32;
        for (s, m) in segments.iter().zip(meshing) {
            min = min.min(s.start.min(s.end));
            max = max.max(s.start.max(s.end));
            widest = widest.max(if closes(&m) { s.radius + m.close } else { m.blend.max(1.1) * s.radius });
        }
        let longest = ((max - min) + Vec3::splat(2.0 * widest)).max_element();
        if !longest.is_finite() || longest <= 0.0 {
            return None;
        }
        let cell = longest / resolution.max(2) as f32;
        let min_radius = MIN_RADIUS_IN_CELLS * cell;

        // two cells of empty padding keep the surface closed
        let pad = widest.max(reach_factor * min_radius) + 4.0 * cell;
        let origin = min - Vec3::splat(pad);
        let size = max - min + Vec3::splat(2.0 * pad);
        let samples = [0, 1, 2].map(|a| (size[a] / cell).ceil() as usize + 1);
        Some(Frame { reach_factor, threshold, close_max, close_min, closing, cell, min_radius, origin, samples })
    }

    fn grid(&self) -> SparseGrid {
        let blocks = self.samples.map(|s| s.div_ceil(BLOCK));
        SparseGrid {
            origin: self.origin,
            cell: self.cell,
            samples: self.samples,
            blocks,
            block_slots: vec![EMPTY; blocks[0] * blocks[1] * blocks[2]],
            density: Vec::new(),
            owner: Vec::new(),
            owner_k: Vec::new(),
            tube: Vec::new(),
            tube_close: Vec::new(),
            strand: Vec::new(),
            guard: Vec::new(),
            run: Vec::new(),
            run_k: Vec::new(),
        }
    }

    /// Whether a segment is closed into the tissue, a closed segment without a closing radius is blended
    fn closes(&self, meshing: &Meshing) -> bool {
        self.closing && meshing.closed && meshing.close > 0.0
    }

    /// The kernel of a segment of `radius`. A kernel that reaches less far than the largest one
    /// is weighted so that a lone segment's surface still sits at its radius, at the density of
    /// the largest one: `T(b) / T(b_i) · (1 − d² / (b_i·r)²)³` with `T(b) = (1 − 1/b²)³` (de Groot,
    /// Wyvill, van de Wetering 2009, "Locally restricted blending of Blobtrees", the iso-value
    /// of every primitive is mapped to itself). The weight is 1 when all reach as far.
    fn kernel(&self, meshing: &Meshing, radius: f32) -> Kernel {
        let reach_factor = meshing.blend.max(1.1);
        let weight = if reach_factor == self.reach_factor { 1.0 } else { self.threshold / threshold(reach_factor) };
        Kernel { reach: reach_factor * radius, weight }
    }

    /// A closed segment reaches a little past the largest closing, so every sample near the closed surface knows its
    /// distance and which closing radius it has. Its tube in the tissue has its own radius, the closing gives the
    /// tissue a body larger than a cell, but a strand is a line of its own and has at least the smallest radius.
    fn closed(&self, meshing: &Meshing, segment: &Segment, strand: bool) -> Closed {
        let radius = segment.radius;
        Closed {
            radius,
            reach: radius + self.close_max + 2.0 * self.cell,
            strand: strand.then(|| radius.max(self.min_radius)),
            close: meshing.close,
        }
    }

    /// The kernel a closed segment claims samples with, it never adds to the density, so it may be wider than the
    /// segment: thin ones still claim the samples around them
    fn owner_kernel(&self, meshing: &Meshing, segment: &Segment) -> Kernel {
        self.kernel(meshing, segment.radius.max(self.min_radius))
    }

    /// Every blended segment thinner than the smallest radius whose axis the surface doesn't enclose gets its
    /// kernel at the smallest radius, see [`SparseGrid::floor`]. Which ones is decided before any of them is
    /// raised, so a raised one doesn't spare its neighbours.
    fn floor(&self, grid: &mut SparseGrid, segments: &[Segment], meshing: &[Meshing]) {
        let thin: Vec<usize> = (0..segments.len())
            .filter(|&i| !self.closes(&meshing[i]) && segments[i].radius < self.min_radius)
            .filter(|&i| !grid.encloses(&segments[i], self.threshold))
            .collect();
        for i in thin {
            grid.floor(i as u32, &meshing[i], &segments[i], self.kernel(&meshing[i], self.min_radius));
        }
    }

    /// Keeps the closing out of the gaps between the segments that aren't closed
    fn guard(&self, grid: &mut SparseGrid, segments: &[Segment], meshing: &[Meshing]) {
        for (s, m) in segments.iter().zip(meshing) {
            if !self.closes(m) {
                grid.guard(s, GUARD * self.close_max + 2.0 * self.cell);
            }
        }
    }
}

/// [`mesh_segments`] that also tells which segment every vertex belongs to.
/// A segment owns a vertex where its kernel times `priority(index)` is the largest.
///
/// The segments for which `closed(index)` holds are not blended but form a
/// tissue: the morphological closing of their tubes with a ball of radius
/// `close`, blurred a little. Gaps between them that the ball can't pass
/// through fill, while convex parts and wider gaps stay about as they are, so
/// a loose bundle of segments gets one smooth surface without swelling where
/// it is dense. The closing keeps out of the gaps between the other segments.
///
/// The closed segments for which `strand(index)` holds keep their own tube
/// where they leave the tissue, of the others only the tissue shows.
///
/// Every segment here has the same `blend` and `close` and is a path of its own, [`surface_in`]
/// takes them per segment.
pub fn surface(
    segments: &[Segment],
    resolution: usize,
    blend: f32,
    close: f32,
    closed: impl Fn(usize) -> bool,
    strand: impl Fn(usize) -> bool,
    priority: impl Fn(usize) -> f32,
) -> SegmentSurface {
    let meshing: Vec<Meshing> = (0..segments.len())
        .map(|i| Meshing { closed: closed(i), priority: priority(i), joined: false, blend, close })
        .collect();
    match Frame::new(segments, &meshing, resolution) {
        Some(frame) => surface_in(&frame, segments, &meshing, strand),
        None => SegmentSurface::default(),
    }
}

/// [`surface`] on the grid of `frame`, which the segments must lie in, every segment meshed as
/// `meshing` says. Meshes of segments taken from the same set, on the frame of the whole set,
/// are sampled at the same places.
pub fn surface_in(
    frame: &Frame,
    segments: &[Segment],
    meshing: &[Meshing],
    strand: impl Fn(usize) -> bool,
) -> SegmentSurface {
    if segments.is_empty() {
        return SegmentSurface::default();
    }
    let mut grid = frame.grid();
    for (i, (s, m)) in segments.iter().zip(meshing).enumerate() {
        if frame.closes(m) {
            grid.splat(i as u32, m, s, frame.owner_kernel(m, s), Some(frame.closed(m, s, strand(i))));
        } else {
            grid.splat(i as u32, m, s, frame.kernel(m, s.radius), None);
        }
    }
    if frame.closing {
        frame.guard(&mut grid, segments, meshing);
        let solid = closing(&grid, frame);
        add_tissue(&mut grid, &solid, frame.threshold);
    }
    frame.floor(&mut grid, segments, meshing);
    extract(&grid, frame)
}

/// Surface Nets on the density of the grid
fn extract(grid: &SparseGrid, frame: &Frame) -> SegmentSurface {
    let (threshold, cell, samples, blocks) = (frame.threshold, frame.cell, grid.samples, grid.blocks);

    let mut mesh = Mesh::default();
    let mut owners = Vec::new();
    let mut vertex_of_cell = vec![EMPTY; grid.density.len()];
    let block_origins: Vec<[usize; 3]> = {
        let mut origins = vec![[0; 3]; grid.density.len() / BLOCK_SAMPLES];
        for bz in 0..blocks[2] {
            for by in 0..blocks[1] {
                for bx in 0..blocks[0] {
                    let slot = grid.block_slots[bx + blocks[0] * (by + blocks[1] * bz)];
                    if slot != EMPTY {
                        origins[slot as usize] = [bx * BLOCK, by * BLOCK, bz * BLOCK];
                    }
                }
            }
        }
        origins
    };
    let cell_of = |grid: &SparseGrid, c: [usize; 3]| match grid.slot(c) {
        EMPTY => None,
        s => Some(s as usize * BLOCK_SAMPLES + SparseGrid::local(c)),
    };

    // one vertex per cell the surface passes through, at the mean of its edge crossings
    for (slot, base) in block_origins.iter().enumerate() {
        for local in 0..BLOCK_SAMPLES {
            let c = [base[0] + local % BLOCK, base[1] + local / BLOCK % BLOCK, base[2] + local / (BLOCK * BLOCK)];
            if c[0] + 1 >= samples[0] || c[1] + 1 >= samples[1] || c[2] + 1 >= samples[2] {
                continue;
            }
            let d = CORNERS.map(|o| grid.get([c[0] + o[0], c[1] + o[1], c[2] + o[2]]));
            let inside = d.iter().filter(|&&v| v > threshold).count();
            if inside == 0 || inside == 8 {
                continue;
            }
            let mut sum = Vec3::ZERO;
            let mut crossings = 0.0;
            for [a, b] in EDGES {
                if (d[a] > threshold) == (d[b] > threshold) {
                    continue;
                }
                let t = (threshold - d[a]) / (d[b] - d[a]);
                let (ca, cb) = (Vec3::from(CORNERS[a].map(|v| v as f32)), Vec3::from(CORNERS[b].map(|v| v as f32)));
                sum += ca + (cb - ca) * t;
                crossings += 1.0;
            }
            let f = sum / crossings;
            let p = grid.position(Vec3::new(c[0] as f32, c[1] as f32, c[2] as f32) + f);

            // density rises inwards, the normal is the trilinear blend of the corner gradients, negated
            let mut gradient = Vec3::ZERO;
            let (mut best, mut owner) = (-1.0, 0);
            for o in CORNERS {
                let w = [0, 1, 2].map(|a| if o[a] == 1 { f[a] } else { 1.0 - f[a] });
                let w = w[0] * w[1] * w[2];
                let corner = [c[0] + o[0], c[1] + o[1], c[2] + o[2]];
                gradient += grid.gradient(corner) * w;
                let (k, i) = grid.owner(corner);
                if i != EMPTY && k * w > best {
                    (best, owner) = (k * w, i);
                }
            }
            let n = (-gradient).try_normalize().unwrap_or(Vec3::Y);
            owners.push(owner);

            vertex_of_cell[slot * BLOCK_SAMPLES + local] = mesh.vertex_count() as u32;
            mesh.positions.extend([p.x, p.y, p.z]);
            mesh.normals.extend([n.x, n.y, n.z]);
        }
    }

    // a quad for every grid edge the surface crosses, joining the four cells around it
    for base in &block_origins {
        for local in 0..BLOCK_SAMPLES {
            let p = [base[0] + local % BLOCK, base[1] + local / BLOCK % BLOCK, base[2] + local / (BLOCK * BLOCK)];
            if p[0] >= samples[0] || p[1] >= samples[1] || p[2] >= samples[2] {
                continue;
            }
            let inside = grid.get(p) > threshold;
            for axis in 0..3 {
                let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
                if p[u] == 0 || p[v] == 0 {
                    continue;
                }
                let mut next = p;
                next[axis] += 1;
                if inside == (grid.get(next) > threshold) {
                    continue;
                }
                let corner = |du: usize, dv: usize| {
                    let mut c = p;
                    c[u] = p[u] - 1 + du;
                    c[v] = p[v] - 1 + dv;
                    cell_of(grid, c).map(|i| vertex_of_cell[i]).filter(|&i| i != EMPTY)
                };
                let (Some(a), Some(b), Some(c), Some(d)) = (corner(0, 0), corner(1, 0), corner(1, 1), corner(0, 1))
                else {
                    debug_assert!(false, "crossing at {:?} without four cells", p);
                    continue;
                };
                if inside {
                    mesh.faces.extend([a, b, c, a, c, d]);
                } else {
                    mesh.faces.extend([a, c, b, a, d, c]);
                }
            }
        }
    }

    mesh.groups = vec![0; mesh.vertex_count()];
    SegmentSurface { mesh, owners, cell }
}

#[cfg(test)]
pub(crate) fn assert_closed(mesh: &Mesh) {
    use std::collections::HashMap;
    assert!(mesh.face_count() > 0);
    let mut edges: HashMap<(u32, u32), i32> = HashMap::new();
    for f in mesh.faces.chunks_exact(3) {
        for (a, b) in [(f[0], f[1]), (f[1], f[2]), (f[2], f[0])] {
            *edges.entry((a, b)).or_default() += 1;
        }
    }
    // Surface Nets can join two sheets at one edge where blobs touch diagonally,
    // closed means every edge is used as often in both directions
    for (&(a, b), &count) in &edges {
        assert_eq!(edges.get(&(b, a)), Some(&count), "edge {}-{} is open", a, b);
    }
}

/// The number of separate pieces of a mesh
#[cfg(test)]
pub(crate) fn components(mesh: &Mesh) -> usize {
    let mut parent: Vec<usize> = (0..mesh.vertex_count()).collect();
    fn find(parent: &mut [usize], i: usize) -> usize {
        let mut i = i;
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    for f in mesh.faces.chunks_exact(3) {
        for (a, b) in [(f[0], f[1]), (f[1], f[2])] {
            let (ra, rb) = (find(&mut parent, a as usize), find(&mut parent, b as usize));
            parent[ra] = rb;
        }
    }
    (0..parent.len()).filter(|&i| find(&mut parent, i) == i).count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    fn segment(start: Vec3, end: Vec3, radius: f32) -> Segment {
        Segment { start, end, radius }
    }

    fn vertex(mesh: &Mesh, i: u32) -> Vec3 {
        Vec3::from_slice(&mesh.positions[i as usize * 3..i as usize * 3 + 3])
    }

    fn volume(mesh: &Mesh) -> f32 {
        mesh.faces
            .chunks_exact(3)
            .map(|f| vertex(mesh, f[0]).dot(vertex(mesh, f[1]).cross(vertex(mesh, f[2]))) / 6.0)
            .sum()
    }

    #[test]
    fn closed_segments_splat_together_as_one_after_another() {
        let mut seed = 9u32;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed as f32 / u32::MAX as f32
        };
        // a dense bundle of thin segments of mixed closings, priorities and blends, a few thick ones whose
        // kernel reaches past their closing, and some far from the others
        let mut segments = Vec::new();
        let mut meshing = Vec::new();
        for i in 0..1500 {
            let far = i % 50 == 0;
            let at = Vec3::new(next(), next(), next()) * if far { 0.3 } else { 0.08 };
            let along = Vec3::new(next() - 0.5, next() - 0.5, next() - 0.5).normalize() * (0.004 + 0.02 * next());
            let radius = if i % 97 == 0 { 0.02 } else { 0.001 + 0.004 * next() };
            segments.push(segment(at, at + along, radius));
            let close = [0.004, 0.008, 0.0][i % 3];
            meshing.push(Meshing { priority: [1.0, 0.5, 2.0, 0.0][i % 4], ..Meshing::closed([3.0, 2.0][i % 2], close) });
        }
        let frame = Frame::new(&segments, &meshing, 48).unwrap();
        let mut in_turn = frame.grid();
        let mut splats = Vec::new();
        for (i, (s, m)) in segments.iter().zip(&meshing).enumerate().filter(|(_, (_, m))| frame.closes(m)) {
            in_turn.splat(i as u32, m, s, frame.owner_kernel(m, s), Some(frame.closed(m, s, false)));
            splats.push(ClosedSplat::new(i as u32, m, *s, frame.owner_kernel(m, s), frame.closed(m, s, false)));
        }
        let mut together = frame.grid();
        together.splat_closed(&splats);
        assert_eq!(together.block_slots, in_turn.block_slots);
        assert_eq!(together.tube, in_turn.tube);
        assert_eq!(together.tube_close, in_turn.tube_close);
        assert_eq!(together.owner, in_turn.owner);
        assert_eq!(together.owner_k, in_turn.owner_k);
        assert!(in_turn.owner.iter().filter(|&&o| o != EMPTY).count() > 5000);
    }

    #[test]
    fn the_blur_is_a_gaussian_along_every_axis() {
        let samples = [9, 11, 13];
        let n = samples[0] * samples[1] * samples[2];
        let at = |x: usize, y: usize, z: usize| x + samples[0] * (y + samples[1] * z);
        let mut values = vec![0.0; n];
        values[at(4, 5, 6)] = 1.0;
        values[at(0, 10, 0)] = 2.0;
        let sigma = 1.3;
        blur(&mut values, samples, sigma);
        let weights = blur_weights(sigma);
        let radius = weights.len() as isize / 2;
        let weight = |d: isize| if d.abs() <= radius { weights[(d + radius) as usize] } else { 0.0 };
        for z in 0..samples[2] {
            for y in 0..samples[1] {
                for x in 0..samples[0] {
                    let (x, y, z) = (x as isize, y as isize, z as isize);
                    let expected = weight(x - 4) * weight(y - 5) * weight(z - 6) + 2.0 * weight(x) * weight(y - 10) * weight(z);
                    let got = values[at(x as usize, y as usize, z as usize)];
                    assert!((got - expected).abs() < 1e-6, "{} vs {} at {} {} {}", got, expected, x, y, z);
                }
            }
        }
    }

    #[test]
    fn a_lone_segment_keeps_its_radius() {
        let (r, length) = (0.1, 1.0);
        let mesh = mesh_segments(&[segment(Vec3::ZERO, Vec3::Y * length, r)], 64, 2.0);
        assert_closed(&mesh);
        let expected = PI * r * r * length + 4.0 / 3.0 * PI * r * r * r;
        let v = volume(&mesh);
        assert!((v - expected).abs() < expected * 0.1, "volume {} vs {}", v, expected);
        for i in 0..mesh.vertex_count() as u32 {
            let p = vertex(&mesh, i);
            let d = (p - Vec3::Y * p.y.clamp(0.0, length)).length();
            assert!((d - r).abs() < 1.2 / 64.0 * 1.4, "vertex {} away from the axis", d);
        }
    }

    #[test]
    fn neighbouring_segments_blend_into_one_surface() {
        let r = 0.05;
        let bundle: Vec<Segment> = (0..12)
            .map(|i| {
                let a = i as f32 / 12.0 * 2.0 * PI;
                let foot = Vec3::new(a.cos(), 0.0, a.sin()) * 0.12;
                segment(foot, foot + Vec3::Y, r)
            })
            .collect();
        let mesh = mesh_segments(&bundle, 64, 2.0);
        assert_closed(&mesh);
        assert_eq!(components(&mesh), 1);
    }

    #[test]
    fn distant_segments_stay_apart() {
        let mesh = mesh_segments(
            &[segment(Vec3::ZERO, Vec3::Y, 0.05), segment(Vec3::X * 0.5, Vec3::new(0.5, 1.0, 0.0), 0.05)],
            64,
            2.0,
        );
        assert_closed(&mesh);
        assert_eq!(components(&mesh), 2);
    }

    #[test]
    fn a_larger_blend_bridges_wider_gaps() {
        let pair = [segment(Vec3::ZERO, Vec3::Y, 0.05), segment(Vec3::X * 0.18, Vec3::new(0.18, 1.0, 0.0), 0.05)];
        assert_eq!(components(&mesh_segments(&pair, 64, 2.0)), 2);
        let blended = mesh_segments(&pair, 64, 4.0);
        assert_closed(&blended);
        assert_eq!(components(&blended), 1);
    }

    #[test]
    fn normals_point_outwards() {
        let mesh = mesh_segments(&[segment(Vec3::ZERO, Vec3::Y, 0.1)], 48, 2.0);
        for i in 0..mesh.vertex_count() {
            let p = vertex(&mesh, i as u32);
            let n = Vec3::from_slice(&mesh.normals[i * 3..i * 3 + 3]);
            let outwards = p - Vec3::Y * p.y.clamp(0.0, 1.0);
            assert!(n.dot(outwards.normalize()) > 0.9);
        }
        assert!(mesh.colors.is_empty());
        assert_eq!(mesh.groups.len(), mesh.vertex_count());
    }

    #[test]
    fn thin_segments_do_not_vanish() {
        let mesh = mesh_segments(&[segment(Vec3::ZERO, Vec3::Y, 0.0001)], 32, 2.0);
        assert_closed(&mesh);
    }

    #[test]
    fn same_input_gives_the_same_output() {
        let walk = || {
            let mut p = Vec3::ZERO;
            (0..200)
                .map(|i| {
                    let next = p + Vec3::new((i as f32 * 1.3).sin(), 0.8, (i as f32 * 0.7).cos()) * 0.02;
                    let s = segment(p, next, 0.01);
                    p = next;
                    s
                })
                .collect::<Vec<_>>()
        };
        let mesh = mesh_segments(&walk(), 64, 2.0);
        assert_closed(&mesh);
        assert_eq!(mesh.encode(), mesh_segments(&walk(), 64, 2.0).encode());
    }

    #[test]
    fn every_vertex_belongs_to_the_nearest_segment() {
        let pair = [segment(Vec3::ZERO, Vec3::Y, 0.05), segment(Vec3::X * 0.5, Vec3::new(0.5, 1.0, 0.0), 0.05)];
        let s = surface(&pair, 48, 2.0, 0.0, |_| false, |_| false, |_| 1.0);
        assert_eq!(s.owners.len(), s.mesh.vertex_count());
        for (i, &owner) in s.owners.iter().enumerate() {
            assert_eq!(owner, (vertex(&s.mesh, i as u32).x > 0.25) as u32);
        }
    }

    #[test]
    fn priority_lets_a_segment_win_where_both_reach() {
        let pair = [segment(Vec3::ZERO, Vec3::Y, 0.05), segment(Vec3::X * 0.12, Vec3::new(0.12, 1.0, 0.0), 0.05)];
        let even = surface(&pair, 48, 3.0, 0.0, |_| false, |_| false, |_| 1.0);
        let first = surface(&pair, 48, 3.0, 0.0, |_| false, |_| false, |i| if i == 0 { 4.0 } else { 1.0 });
        let count = |s: &SegmentSurface| s.owners.iter().filter(|&&o| o == 0).count();
        assert_eq!(even.mesh, first.mesh);
        assert!(count(&first) > count(&even));
    }

    fn closed_surface(segments: &[Segment], close: f32, closed: impl Fn(usize) -> bool) -> SegmentSurface {
        surface(segments, 64, 3.0, close, closed, |_| false, |_| 1.0)
    }

    /// Every segment closed into one tissue, those for which `strand` holds keep their tube out of it
    fn strand_surface(segments: &[Segment], close: f32, strand: impl Fn(usize) -> bool) -> SegmentSurface {
        surface(segments, 64, 3.0, close, |_| true, strand, |_| 1.0)
    }

    #[test]
    fn closing_fills_the_gaps_between_hyphae_of_one_tissue() {
        // a strand of 3 x 3 hyphae wider apart than the kernels reach
        let spacing = 0.15;
        let strand: Vec<Segment> = (0..9)
            .map(|i| {
                let foot = Vec3::new((i % 3) as f32 - 1.0, 0.0, (i / 3) as f32 - 1.0) * spacing;
                segment(foot, foot + Vec3::Y, 0.03)
            })
            .collect();
        assert_eq!(components(&closed_surface(&strand, 0.0, |_| true).mesh), 9);
        let closed = closed_surface(&strand, 0.2, |_| true);
        assert_closed(&closed.mesh);
        assert_eq!(components(&closed.mesh), 1);
        // no surface left between the hyphae inside
        let s = &closed.mesh;
        assert!((0..s.vertex_count())
            .map(|v| vertex(s, v as u32))
            .filter(|p| (0.2..0.8).contains(&p.y))
            .all(|p| p.x.abs().max(p.z.abs()) > spacing * 0.5));
    }

    #[test]
    fn closing_leaves_wider_gaps_open() {
        // thinner than the blur, they only show as strands
        let pair = [segment(Vec3::ZERO, Vec3::Y, 0.03), segment(Vec3::X * 0.5, Vec3::new(0.5, 1.0, 0.0), 0.03)];
        let closed = strand_surface(&pair, 0.2, |_| true);
        assert_closed(&closed.mesh);
        assert_eq!(components(&closed.mesh), 2);
    }

    #[test]
    fn closing_does_not_swell_a_dense_bundle() {
        let (r, foot) = (0.03, 0.15);
        let bundle: Vec<Segment> = (0..16)
            .map(|i| {
                let a = i as f32 / 16.0 * 2.0 * PI;
                let at = Vec3::new(a.cos(), 0.0, a.sin()) * foot;
                segment(at, at + Vec3::Y, r)
            })
            .collect();
        let close = 0.15;
        let s = closed_surface(&bundle, close, |_| true);
        assert_closed(&s.mesh);
        assert_eq!(components(&s.mesh), 1);
        let widest = (0..s.mesh.vertex_count())
            .map(|v| vertex(&s.mesh, v as u32))
            .filter(|p| (0.2..0.8).contains(&p.y))
            .map(|p| Vec3::new(p.x, 0.0, p.z).length())
            .fold(0.0, f32::max);
        // the blur may round the surface out a little, a blend that joins the bundle swells it by far more
        assert!(widest < foot + r + 0.25 * SMOOTHING * close + s.cell, "bundle {} wide", widest);
        assert!(widest > foot + r - s.cell, "bundle {} wide", widest);
    }

    #[test]
    fn closing_keeps_away_from_the_gaps_of_other_segments() {
        // a cap of closed hyphae with two thin gills below it, the gap between them is narrower than the closing
        let r = 0.02;
        let mut segments: Vec<Segment> =
            (0..9).map(|i| segment(Vec3::new(-0.4, 1.0, (i as f32 - 4.0) * 0.05), Vec3::new(0.4, 1.0, (i as f32 - 4.0) * 0.05), r)).collect();
        for x in [-0.08, 0.08] {
            for i in 0..9 {
                let z = (i as f32 - 4.0) * 0.03;
                segments.push(segment(Vec3::new(x, 0.98, z), Vec3::new(x, 0.6, z), r));
            }
        }
        let is_cap = |i: usize| i < 9;
        let inner_faces = |s: &SegmentSurface| {
            (0..s.mesh.vertex_count())
                .map(|v| vertex(&s.mesh, v as u32))
                .filter(|p| (0.7..0.85).contains(&p.y) && p.z.abs() < 0.08 && p.x.abs() < 0.08 - r + 0.5 * s.cell)
                .count()
        };
        let gills = closed_surface(&segments, 0.15, is_cap);
        assert_closed(&gills.mesh);
        assert!(inner_faces(&gills) > 0, "the gap between the gills closed");
        // as one tissue the same gap fills
        assert_eq!(inner_faces(&closed_surface(&segments, 0.15, |_| true)), 0);
    }

    /// A dense bundle of 16 hyphae from y 0 to 1 that closes into one body
    fn body() -> Vec<Segment> {
        (0..16)
            .map(|i| {
                let a = i as f32 / 16.0 * 2.0 * PI;
                let at = Vec3::new(a.cos(), 0.0, a.sin()) * 0.15;
                segment(at, at + Vec3::Y, 0.03)
            })
            .collect()
    }

    fn highest(s: &SegmentSurface) -> f32 {
        (0..s.mesh.vertex_count()).map(|v| vertex(&s.mesh, v as u32).y).fold(f32::NEG_INFINITY, f32::max)
    }

    #[test]
    fn a_tip_just_outside_the_tissue_leaves_no_spike() {
        let (r, close) = (0.03, 0.18);
        let plain = closed_surface(&body(), close, |_| true);
        let mut tipped = body();
        let rim = tipped[0].end;
        tipped.push(segment(rim, rim + Vec3::Y * 2.0 * r, r));
        let tipped = closed_surface(&tipped, close, |_| true);
        assert_closed(&tipped.mesh);
        assert_eq!(components(&tipped.mesh), 1);
        // as its own tube the tip would reach 3 r above the top of the body
        assert!(highest(&tipped) < highest(&plain) + r, "{} vs {}", highest(&tipped), highest(&plain));
    }

    #[test]
    fn a_strand_keeps_its_tube_out_of_the_tissue() {
        let (r, close) = (0.03, 0.18);
        let mut segments = body();
        let rim = segments[0].end;
        segments.push(segment(rim, rim + Vec3::Y * 4.0 * close, r));
        let strand = strand_surface(&segments, close, |i| i == 16);
        assert_closed(&strand.mesh);
        assert_eq!(components(&strand.mesh), 1);
        assert!(highest(&strand) > 1.0 + 4.0 * close + 0.5 * r, "strand ends at {}", highest(&strand));
        // not a strand, the tissue absorbs it
        let absorbed = strand_surface(&segments, close, |_| false);
        assert!(highest(&absorbed) < 1.0 + close, "absorbed strand ends at {}", highest(&absorbed));
    }

    #[test]
    fn a_thin_strand_bundle_does_not_vanish() {
        // narrower than the blur, it stays as its hyphae
        let r = 0.02;
        let bundle: Vec<Segment> = (0..4)
            .map(|i| {
                let a = i as f32 / 4.0 * 2.0 * PI;
                let at = Vec3::new(a.cos(), 0.0, a.sin()) * 0.03;
                segment(at, at + Vec3::Y, r)
            })
            .collect();
        let s = strand_surface(&bundle, 0.3, |_| true);
        assert_closed(&s.mesh);
        assert!(highest(&s) > 1.0, "bundle ends at {}", highest(&s));
        let lowest = (0..s.mesh.vertex_count()).map(|v| vertex(&s.mesh, v as u32).y).fold(f32::INFINITY, f32::min);
        assert!(lowest < 0.0, "bundle starts at {}", lowest);
    }

    /// A small deterministic random number in 0..1
    fn random(i: u32) -> f32 {
        let mut x = i.wrapping_mul(0x9E37_79B9) ^ 0x85EB_CA6B;
        x ^= x >> 15;
        x = x.wrapping_mul(0x2C1B_3C6D);
        x ^= x >> 12;
        (x & 0xFF_FFFF) as f32 / 0x100_0000 as f32
    }

    #[test]
    fn grown_balls_are_their_union() {
        let samples = [23, 17, 19];
        let n = samples[0] * samples[1] * samples[2];
        let at = |i: usize| Vec3::new((i % 23) as f32, (i / 23 % 17) as f32, (i / (23 * 17)) as f32);
        for (round, scale) in [1.0, 1.7, 4.0].into_iter().enumerate() {
            let mut radius = vec![0.0; n];
            for k in 0..12 {
                let i = (random(round as u32 * 100 + k) * n as f32) as usize % n;
                radius[i] = 0.3 + 2.7 * random(round as u32 * 100 + 50 + k);
            }
            let (power, centre) = grown_balls(&radius, samples, scale);
            for q in 0..n {
                let brute = (0..n)
                    .filter(|&x| radius[x] > 0.0)
                    .map(|x| at(q).distance_squared(at(x)) - (scale * radius[x]).powi(2))
                    .fold(f32::INFINITY, f32::min);
                assert!((power[q] - brute).abs() < 1e-3, "sample {} at scale {}: {} vs {}", q, scale, power[q], brute);
                let x = centre[q] as usize;
                assert!((at(q).distance_squared(at(x)) - (scale * radius[x]).powi(2) - brute).abs() < 1e-3);
            }
        }
    }

    #[test]
    fn grown_tissue_covers_a_strand_by_the_thickness_next_to_it() {
        // a slab 0.4 thick, its inner balls have radius 0.2
        let slab: Vec<Segment> = (0..21)
            .flat_map(|i| {
                (0..9).map(move |j| {
                    let (x, y) = (i as f32 * 0.05 - 0.5, j as f32 * 0.05 - 0.2);
                    segment(Vec3::new(x, y, -0.5), Vec3::new(x, y, 0.5), 0.03)
                })
            })
            .collect();
        let close = 0.18;
        let meshing = vec![Meshing::closed(3.0, close); slab.len()];
        let frame = Frame::new(&slab, &meshing, 96).unwrap();
        let field = |scale: f32| TissueField::new(&frame, &slab, &meshing, scale).unwrap();
        let f = field(1.0);
        assert!(f.has_tissue());
        // the blur rounds the slab out a little past its outer hyphae
        let top = f.crossing(Vec3::ZERO, Vec3::Y);
        assert!((0.23..0.23 + SMOOTHING * close).contains(&top), "surface at {}", top);
        assert!(f.inside(Vec3::Y * (top - 0.02)) && !f.inside(Vec3::Y * (top + 0.02)));
        // the slab is symmetric, its middle ball reaches the surface
        let at = |h: f32| Vec3::new(0.0, top + h, 0.0);
        // at scale 1 nothing outside is covered, at 3 up to (3 - 1) R above the surface, less 3 times the strand radius
        assert!(!f.covers(at(0.03), 0.0));
        let f = field(3.0);
        assert!(f.covers(at(0.2), 0.0) && f.covers(at(2.0 * top - 0.08), 0.0));
        assert!(!f.covers(at(2.0 * top + 0.08), 0.0));
        assert!(f.covers(at(2.0 * top - 0.3), 0.06) && !f.covers(at(2.0 * top - 0.1), 0.06));
        // at infinity everything is covered
        assert!(field(f32::INFINITY).covers(at(5.0), 1.0));
    }

    #[test]
    fn a_field_meshes_like_its_segments() {
        // a closed body with a strand on top, gills of other segments below it, one hypha twice as important
        let mut segments = body();
        let rim = segments[0].end;
        segments.push(segment(rim, rim + Vec3::Y * 0.6, 0.03));
        for x in [-0.08, 0.08] {
            segments.push(segment(Vec3::new(x, -0.02, 0.0), Vec3::new(x, -0.3, 0.0), 0.02));
        }
        let closed = |i: usize| i < 17;
        let strand = |i: usize| i == 16;
        let priority = |i: usize| if i == 3 { 2.0 } else { 1.0 };
        let meshing: Vec<Meshing> = (0..segments.len())
            .map(|i| Meshing { closed: closed(i), priority: priority(i), ..Meshing::closed(3.0, 0.18) })
            .collect();
        let frame = Frame::new(&segments, &meshing, 64).unwrap();
        let field = TissueField::new(&frame, &segments, &meshing, 5.0).unwrap();
        let reused = field.surface(&segments, &meshing, strand);
        let fresh = surface_in(&frame, &segments, &meshing, strand);
        assert!(reused.mesh.face_count() > 0);
        assert_eq!(reused, fresh);
    }

    #[test]
    fn touching_bundles_leave_no_groove() {
        // two bundles of hyphae side by side, further apart from each other than within each
        let r = 0.02;
        let mut segments = Vec::new();
        for x in [-0.25, -0.19, -0.13, -0.07, 0.07, 0.13, 0.19, 0.25] {
            for y in [-0.12, -0.06, 0.0] {
                segments.push(segment(Vec3::new(x, y, -0.5), Vec3::new(x, y, 0.5), r));
            }
        }
        let top = |close: f32| {
            let s = surface(&segments, 96, 3.0, close, |_| true, |_| false, |_| 1.0);
            assert_closed(&s.mesh);
            let mut top = [f32::NEG_INFINITY; 2];
            for v in 0..s.mesh.vertex_count() {
                let p = vertex(&s.mesh, v as u32);
                if p.z.abs() < 0.3 {
                    let i = (p.x.abs() > 0.1) as usize;
                    if p.x.abs() < 0.04 || i == 1 {
                        top[i] = top[i].max(p.y);
                    }
                }
            }
            // depth of the groove between the bundles below their tops
            top[1] - top[0]
        };
        assert!(top(0.0) > r, "without closing the bundles stay apart");
        let depth = top(6.0 * r);
        assert!(depth < 0.25 * r, "groove {} deep", depth);
    }

    #[test]
    fn closing_gives_the_same_output_for_the_same_input() {
        let segments: Vec<Segment> = (0..40)
            .map(|i| {
                let a = i as f32 * 2.4;
                let foot = Vec3::new(a.cos(), 0.0, a.sin()) * (0.05 + 0.005 * i as f32);
                segment(foot, foot + Vec3::new(0.1 * a.sin(), 1.0, 0.0), 0.02)
            })
            .collect();
        let a = closed_surface(&segments, 0.1, |i| i % 3 != 0);
        assert_closed(&a.mesh);
        let b = closed_surface(&segments, 0.1, |i| i % 3 != 0);
        assert_eq!(a.mesh.encode(), b.mesh.encode());
        assert_eq!(a.owners, b.owners);
    }

    #[test]
    fn no_segments_give_an_empty_mesh() {
        assert_eq!(mesh_segments(&[], 64, 2.0), Mesh::default());
    }
}
