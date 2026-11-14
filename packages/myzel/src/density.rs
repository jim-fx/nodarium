use glam::Vec3;

use crate::mesh::Mesh;
use crate::spatial::{distance_sq_to_segment, larger, smaller, TubeTree};

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

/// How far from its tube a strand is written when its segment is on the grid already, in cells. The
/// share of a strand in [`add_tissue`] is 0 from a cell on, the rest is a margin for rounding.
const STRAND_REACH_IN_CELLS: f32 = 1.01;

/// How much a closed segment counts for the owner of a sample past its kernel
const FILL_OWNER_WEIGHT: f32 = 1e-3;

/// Points around a tube that [`TissueField::holds`] looks at
const TUBE_RING: usize = 8;
/// Where the sample nearest a point is deeper than this plus a radius, every sample that the ring of that
/// radius around the point reads is inside: they lie within the radius, half a cell diagonal to the
/// point and a cell diagonal across the cell of a point of the ring, and a depth leaves out half a cell
const HELD_DEPTH_IN_CELLS: f32 = 2.1;


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
    /// Whether `density`, `strand`, `run` and `run_k` are there. The closing of a [`TissueField`] reads none of
    /// them, they stay empty until its surface is meshed, see [`SparseGrid::start_meshing`].
    meshing: bool,
}

impl SparseGrid {
    fn slot(&self, p: [usize; 3]) -> u32 {
        let b = [p[0] / BLOCK, p[1] / BLOCK, p[2] / BLOCK];
        self.block_slots[b[0] + self.blocks[0] * (b[1] + self.blocks[1] * b[2])]
    }

    fn local(p: [usize; 3]) -> usize {
        p[0] % BLOCK + BLOCK * (p[1] % BLOCK + BLOCK * (p[2] % BLOCK))
    }

    /// The samples `reach` around a segment, without those outside the grid
    fn sample_box(&self, segment: &Segment, reach: f32) -> ([usize; 3], [usize; 3]) {
        let lo_world = segment.start.min(segment.end) - Vec3::splat(reach);
        let hi_world = segment.start.max(segment.end) + Vec3::splat(reach);
        let to_sample = |v: Vec3, round: fn(f32) -> f32| {
            [0, 1, 2].map(|a| (round((v[a] - self.origin[a]) / self.cell).max(0.0) as usize).min(self.samples[a] - 1))
        };
        (to_sample(lo_world, f32::ceil), to_sample(hi_world, f32::floor))
    }

    /// Every block without a slot that has a sample within `reach` of a segment, or a cell with such a sample as a
    /// corner: the block row below counts too, so every cell next to an inside sample has a vertex slot
    fn reach_blocks(&self, segment: &Segment, reach: f32) -> impl Iterator<Item = usize> + '_ {
        let (lo, hi) = self.sample_box(segment, reach);
        let lo = lo.map(|v| v.saturating_sub(1) / BLOCK);
        let hi = hi.map(|v| v / BLOCK);
        // splatting rounds its distances, a block a little out of reach costs less than a sample without a slot
        let reach = reach + 1e-3 * self.cell;
        let (start, end) = (segment.start, segment.end);
        let last = Vec3::new(self.samples[0] as f32, self.samples[1] as f32, self.samples[2] as f32) - Vec3::ONE;
        let half_diagonal = 0.5 * BLOCK as f32 * 3f32.sqrt() * self.cell;
        (lo[2]..=hi[2])
            .flat_map(move |bz| (lo[1]..=hi[1]).flat_map(move |by| (lo[0]..=hi[0]).map(move |bx| [bx, by, bz])))
            .filter(move |&[bx, by, bz]| {
                if self.block_slots[bx + self.blocks[0] * (by + self.blocks[1] * bz)] != EMPTY {
                    return false;
                }
                let first = Vec3::new(bx as f32, by as f32, bz as f32) * BLOCK as f32;
                let (lo, hi) = (self.position(first), self.position((first + Vec3::splat(BLOCK as f32)).min(last)));
                // the centre decides most blocks, only those near the reach need the whole box
                let d = distance_sq_to_segment(0.5 * (lo + hi), start, end).sqrt();
                d < reach || (d < reach + half_diagonal && segment_box_distance_sq(start, end, lo, hi) < reach * reach)
            })
            .map(|[bx, by, bz]| bx + self.blocks[0] * (by + self.blocks[1] * bz))
    }

    fn block_count(&self) -> usize {
        self.tube.len() / BLOCK_SAMPLES
    }

    /// Gives the blocks a segment reaches that have none a slot, numbered in turn, see [`SparseGrid::reach_blocks`]
    fn allocate(&mut self, segment: &Segment, reach: f32) {
        self.allocate_all([(*segment, reach)]);
    }

    /// [`SparseGrid::allocate`] of every segment and its reach in turn, with the arrays grown once to what they hold
    fn allocate_all(&mut self, reaches: impl IntoIterator<Item = (Segment, f32)>) {
        let mut count = self.block_count();
        let (mut segments, mut new) = (0, Vec::new());
        for (segment, reach) in reaches {
            new.clear();
            new.extend(self.reach_blocks(&segment, reach));
            for &i in &new {
                self.block_slots[i] = count as u32;
                count += 1;
            }
            segments += 1;
        }
        self.grow(count * BLOCK_SAMPLES, segments > 1);
    }

    /// Grows the arrays to `len` samples. A grid that grows by many boxes at once knows what it needs, one
    /// that grows box by box leaves room as a `Vec` does.
    fn grow(&mut self, len: usize, exactly: bool) {
        fn to<T: Clone>(v: &mut Vec<T>, len: usize, value: T, exactly: bool) {
            if exactly {
                v.reserve_exact(len - v.len());
            }
            v.resize(len, value);
        }
        if len == self.tube.len() {
            return;
        }
        to(&mut self.owner, len, EMPTY, exactly);
        to(&mut self.owner_k, len, 0.0, exactly);
        to(&mut self.tube, len, f32::INFINITY, exactly);
        to(&mut self.tube_close, len, 0.0, exactly);
        to(&mut self.guard, len, f32::INFINITY, exactly);
        if self.meshing {
            to(&mut self.density, len, 0.0, exactly);
            to(&mut self.strand, len, f32::INFINITY, exactly);
            to(&mut self.run, len, EMPTY, exactly);
            to(&mut self.run_k, len, 0.0, exactly);
        }
    }

    /// Adds the arrays a mesh needs to a grid that was only closed
    fn start_meshing(&mut self) {
        if self.meshing {
            return;
        }
        self.meshing = true;
        let len = self.tube.len();
        self.density = vec![0.0; len];
        self.strand = vec![f32::INFINITY; len];
        self.run = vec![EMPTY; len];
        self.run_k = vec![0.0; len];
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

    /// Every sample of a block in the grid, by its index in the dense grid and its slot
    fn samples_in_blocks(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        let ([bx, by, _], [nx, ny, nz]) = (self.blocks, self.samples);
        self.block_slots.iter().enumerate().filter(|(_, &slot)| slot != EMPTY).flat_map(move |(b, &slot)| {
            let first = [b % bx, b / bx % by, b / (bx * by)].map(|v| v * BLOCK);
            (0..BLOCK_SAMPLES).filter_map(move |l| {
                let p = [first[0] + l % BLOCK, first[1] + l / BLOCK % BLOCK, first[2] + l / (BLOCK * BLOCK)];
                (p[0] < nx && p[1] < ny && p[2] < nz).then_some((p[0] + nx * (p[1] + ny * p[2]), slot as usize * BLOCK_SAMPLES + l))
            })
        })
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
        let (lo, hi) = self.sample_box(segment, reach);
        self.allocate(segment, reach);
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
        let reach = splat_reach(kernel, closed);
        let (lo, hi) = self.sample_box(segment, reach);
        self.allocate(segment, reach);

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

    /// The strand of a closed segment whose tube and owners are on the grid already, see
    /// [`SparseGrid::close_sample`]. [`add_tissue`] reads a strand up to a cell away from its tube, it
    /// is written a little further and no further: the rest of the reach of the segment is
    /// splatted already.
    fn splat_strand(&mut self, segment: &Segment, radius: f32) {
        let reach = radius + STRAND_REACH_IN_CELLS * self.cell;
        let lo_world = segment.start.min(segment.end) - Vec3::splat(reach);
        let hi_world = segment.start.max(segment.end) + Vec3::splat(reach);
        let to_sample = |v: Vec3, round: fn(f32) -> f32| {
            [0, 1, 2].map(|a| (round((v[a] - self.origin[a]) / self.cell).max(0.0) as usize).min(self.samples[a] - 1))
        };
        let (lo, hi) = (to_sample(lo_world, f32::ceil), to_sample(hi_world, f32::floor));
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
                    if let Some(i) = self.index([x, y, z]) {
                        self.strand[i] = self.strand[i].min(d_sq.sqrt() - radius);
                    }
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
    ///
    /// `splat(k)` is the `k`-th of `count` closed segments, it is asked for each more than once, so the
    /// splats are only kept in one list, in the order of the tree.
    fn splat_closed(&mut self, count: usize, splat: impl Fn(usize) -> ClosedSplat) {
        self.allocate_all((0..count).map(|k| {
            let s = splat(k);
            (s.segment, s.reach)
        }));
        let mut tree = TubeTree::new(
            (0..count)
                .map(|k| {
                    let s = splat(k);
                    (s.segment.start, s.segment.end, s.closed.radius, k as u32)
                })
                .collect(),
        );
        // in the order of the tree, the segments a search finds lie close together
        let splats: Vec<ClosedSplat> = tree.number_in_order().into_iter().map(|k| splat(k as usize)).collect();
        debug_assert!(splats.iter().all(|s| s.closed.strand.is_none()));
        let max = |f: fn(&ClosedSplat) -> f32| splats.iter().map(f).fold(0.0f32, f32::max);
        let (reach_max, radius_max) = (max(|s| s.reach), max(|s| s.closed.radius));
        let (kernel_max, weight_max) = (max(|s| s.kernel.reach), max(|s| s.weight()));
        let search = ClosedSearch {
            tree: &tree,
            splats: &splats,
            reach_max,
            radius_max,
            kernel_max,
            weight_max,
            // a little more than half the diagonal of 2³ samples, every one of them is that close to their centre
            half: 0.87 * self.cell,
            // the bounds are only so exact, segments within the margins are looked at
            tube_margin: 1e-3 * self.cell,
            owner_margin: 1e-5 * weight_max,
            ring: 0.25 * self.cell,
        };
        let mut scratch = BlockScratch::default();
        for b in 0..self.block_slots.len() {
            search.block(self, b, &mut scratch);
        }
    }
}

/// What [`ClosedSearch::block`] reuses from block to block
#[derive(Default)]
struct BlockScratch {
    found: Vec<(f32, u32)>,
    ordered: Vec<(f32, u32)>,
    starts: Vec<usize>,
}

/// The closed segments of [`SparseGrid::splat_closed`] and the bounds of its search
struct ClosedSearch<'a> {
    tree: &'a TubeTree,
    splats: &'a [ClosedSplat],
    reach_max: f32,
    radius_max: f32,
    kernel_max: f32,
    weight_max: f32,
    half: f32,
    tube_margin: f32,
    owner_margin: f32,
    ring: f32,
}

impl ClosedSearch<'_> {
    /// How far from the centre of the samples a segment may be and still be nearer than `tube` or claim one
    /// of them by more than `owner`
    fn reach_for(&self, tube: f32, owner: f32) -> f32 {
        let need = owner - self.owner_margin;
        let owner_reach = if need <= 0.0 {
            self.reach_max
        } else {
            let within =
                |reach: f32, weight: f32| if need < weight { reach * (1.0 - (need / weight).cbrt()).sqrt() } else { 0.0 };
            larger(within(self.kernel_max, self.weight_max), within(self.reach_max, self.weight_max * FILL_OWNER_WEIGHT))
                * 1.001
        };
        smaller(larger(tube + self.radius_max + self.tube_margin, owner_reach), self.reach_max) + self.half
    }

    /// One block. The loop bodies split off like this one are kept out of line because V8 runs a wasm call that is
    /// already running in baseline code until it returns, so a long loop only gets the optimized code of its body
    /// if the body is called each time.
    #[inline(never)]
    fn block(&self, grid: &mut SparseGrid, b: usize, scratch: &mut BlockScratch) {
        let BlockScratch { found, ordered, starts } = scratch;
        let [bx, by, _] = grid.blocks;
        let tiles = BLOCK / 2;

        let slot = grid.block_slots[b];
        if slot == EMPTY {
            return;
        }
        let base = [b % bx * BLOCK, b / bx % by * BLOCK, b / (bx * by) * BLOCK];
        for tile in 0..tiles * tiles * tiles {
            let t = [base[0] + 2 * (tile % tiles), base[1] + 2 * (tile / tiles % tiles), base[2] + 2 * (tile / (tiles * tiles))];
            if (0..3).any(|a| t[a] >= grid.samples[a]) {
                continue;
            }
            let centre = grid.position(Vec3::new(t[0] as f32, t[1] as f32, t[2] as f32) + Vec3::splat(0.5));
            // a segment that reaches all the samples bounds their nearest tube and how much their owner claims them
            let (mut tube, mut owner) = (f32::INFINITY, 0.0f32);
            let mut reach = self.reach_for(tube, owner);
            found.clear();
            self.tree.for_each_within(centre, reach, |k, d| {
                let s = &self.splats[k as usize];
                if d + self.half + self.tube_margin < s.reach {
                    let (nearer, more) = (d + self.half - s.closed.radius, s.least_claim(larger(d - self.half, 0.0), d + self.half));
                    if nearer < tube || more > owner {
                        (tube, owner) = (smaller(tube, nearer), larger(owner, more));
                        reach = self.reach_for(tube, owner);
                    }
                }
                if d - self.half < s.reach {
                    found.push((d, k));
                }
                reach
            });
            // nearest first in rings a quarter cell wide, a sample stops at the first self.ring too far to change it
            let rings = (reach / self.ring) as usize + 1;
            starts.clear();
            starts.resize(rings, 0);
            for &(d, _) in found.iter().filter(|f| f.0 < reach) {
                starts[(d / self.ring) as usize] += 1;
            }
            let mut total = 0;
            for start in starts.iter_mut() {
                (*start, total) = (total, total + *start);
            }
            ordered.resize(total, (0.0, 0));
            for &(d, k) in found.iter().filter(|f| f.0 < reach) {
                let r = (d / self.ring) as usize;
                ordered[starts[r]] = (d, k);
                starts[r] += 1;
            }
            // `starts[r]` is now where self.ring `r` ends
            let mut tile = Tile::default();
            for local in 0..TILE {
                let x = [t[0] + local % 2, t[1] + local / 2 % 2, t[2] + local / 4];
                let p = grid.position(Vec3::new(x[0] as f32, x[1] as f32, x[2] as f32));
                (tile.x[local], tile.y[local], tile.z[local]) = (p.x, p.y, p.z);
            }
            // the first segment wins a tie, as splatting them in turn has it
            let mut tube = [f32::INFINITY; TILE];
            let mut tube_by: [Option<(u32, f32)>; TILE] = [None; TILE];
            let mut owner = [0.0f32; TILE];
            let mut owner_by: [Option<u32>; TILE] = [None; TILE];
            let mut reach = [f32::INFINITY; TILE];
            let mut active: [bool; TILE] = std::array::from_fn(|local| {
                let x = [t[0] + local % 2, t[1] + local / 2 % 2, t[2] + local / 4];
                (0..3).all(|a| x[a] < grid.samples[a])
            });
            // every sample goes through the candidates of a ring in turn and leaves at the first ring too far to
            // change it, as it would on its own; the candidates are measured for all samples at once
            for r in 0..rings {
                for local in 0..TILE {
                    active[local] &= (r as f32 * self.ring) < reach[local];
                }
                if !active.contains(&true) {
                    break;
                }
                let from = if r == 0 { 0 } else { starts[r - 1] };
                for &(d, k) in &ordered[from..starts[r]] {
                    let s = &self.splats[k as usize];
                    let mut measured = TileMeasures::default();
                    s.measure(&tile, &mut measured);
                    for local in 0..TILE {
                        if !active[local] || d >= reach[local] {
                            continue;
                        }
                        let d_sq = measured.d_sq[local];
                        if d_sq >= s.reach_sq {
                            continue;
                        }
                        // a sample within reach lies in the box splatting goes through, only rounding can put
                        // one just inside the reach outside it
                        let inner = s.reach - 1e-3 * grid.cell;
                        if d_sq >= inner * inner {
                            let x = [t[0] + local % 2, t[1] + local / 2 % 2, t[2] + local / 4];
                            let (lo, hi) = grid.sample_box(&s.segment, s.reach);
                            if (0..3).any(|a| x[a] < lo[a] || x[a] > hi[a]) {
                                continue;
                            }
                        }
                        let (nearer, claim) = (measured.nearer[local], measured.claim[local]);
                        let mut changed = false;
                        if nearer < tube[local]
                            || (nearer == tube[local] && tube_by[local].is_some_and(|(index, _)| s.index < index))
                        {
                            (tube[local], tube_by[local], changed) = (nearer, Some((s.index, s.closed.close)), true);
                        }
                        if claim > owner[local]
                            || (claim == owner[local] && owner_by[local].is_some_and(|index| s.index < index))
                        {
                            (owner[local], owner_by[local], changed) = (claim, Some(s.index), true);
                        }
                        if changed {
                            reach[local] = self.reach_for(tube[local], owner[local]);
                        }
                    }
                }
            }
            for local in 0..TILE {
                let x = [t[0] + local % 2, t[1] + local / 2 % 2, t[2] + local / 4];
                if (0..3).any(|a| x[a] >= grid.samples[a]) {
                    continue;
                }
                let i = slot as usize * BLOCK_SAMPLES + SparseGrid::local(x);
                if let Some((_, close)) = tube_by[local] {
                    grid.tube[i] = tube[local];
                    grid.tube_close[i] = close;
                }
                if let Some(index) = owner_by[local] {
                    grid.owner_k[i] = owner[local];
                    grid.owner[i] = index;
                }
            }
        }
    }
}

/// The 2³ samples [`ClosedSearch::block`] looks up the segments for at once
const TILE: usize = 8;

/// The positions of the samples of a tile, a coordinate at a time
#[derive(Default)]
struct Tile {
    x: [f32; TILE],
    y: [f32; TILE],
    z: [f32; TILE],
}

/// What a closed segment does at every sample of a [`Tile`], see [`ClosedSplat::measure`]
#[derive(Default)]
struct TileMeasures {
    d_sq: [f32; TILE],
    nearer: [f32; TILE],
    claim: [f32; TILE],
}

/// How far [`SparseGrid::splat`] reaches around a segment
fn splat_reach(kernel: Kernel, closed: Option<Closed>) -> f32 {
    closed.map_or(kernel.reach, |c| c.reach.max(kernel.reach))
}

/// Squared distance between the segment `a`-`b` and the box `lo`-`hi`. Along the segment it is a convex
/// sum of one quadratic per axis outside the box, so between the points where the segment crosses a side
/// of the box it is one quadratic, whose least value is exact.
fn segment_box_distance_sq(a: Vec3, b: Vec3, lo: Vec3, hi: Vec3) -> f32 {
    let axis = b - a;
    let mut ts = [0.0f32; 8];
    let mut count = 2;
    ts[1] = 1.0;
    for i in 0..3 {
        if axis[i] != 0.0 {
            for side in [lo[i], hi[i]] {
                let t = (side - a[i]) / axis[i];
                if t > 0.0 && t < 1.0 {
                    ts[count] = t;
                    count += 1;
                }
            }
        }
    }
    let ts = &mut ts[..count];
    for i in 1..ts.len() {
        let mut j = i;
        while j > 0 && ts[j - 1] > ts[j] {
            ts.swap(j - 1, j);
            j -= 1;
        }
    }
    let at = |t: f32| {
        let p = a + axis * t;
        (0..3).map(|i| if p[i] < lo[i] { lo[i] - p[i] } else if p[i] > hi[i] { p[i] - hi[i] } else { 0.0 }).map(|d| d * d).sum::<f32>()
    };
    let mut best = smaller(at(0.0), at(1.0));
    for w in ts.windows(2) {
        let mid = 0.5 * (w[0] + w[1]);
        let p = a + axis * mid;
        let (mut slope, mut square) = (0.0, 0.0);
        for i in 0..3 {
            let side = if p[i] < lo[i] { lo[i] } else if p[i] > hi[i] { hi[i] } else { continue };
            slope += axis[i] * (a[i] - side);
            square += axis[i] * axis[i];
        }
        best = smaller(best, at(if square > 0.0 { larger(w[0], smaller(-slope / square, w[1])) } else { mid }));
    }
    best
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
    #[cfg(test)]
    fn distance_sq(&self, p: Vec3) -> f32 {
        let t = crate::spatial::unit((p - self.segment.start).dot(self.axis) / self.length_sq);
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

    /// [`ClosedSplat::distance_sq`], the distance to the tube and [`ClosedSplat::claim`] at every sample of the
    /// tile. On wasm four samples at a time, every lane does what the scalar code does in the same order.
    #[cfg_attr(target_arch = "wasm32", target_feature(enable = "simd128"))]
    #[inline(never)]
    fn measure(&self, tile: &Tile, out: &mut TileMeasures) {
        let (a, axis) = (self.segment.start, self.axis);
        let (kernel_weight, fill_weight) = (self.kernel.weight, self.kernel.weight * FILL_OWNER_WEIGHT);
        for k in 0..TILE {
            let (qx, qy, qz) = (tile.x[k] - a.x, tile.y[k] - a.y, tile.z[k] - a.z);
            let t = (qx * axis.x + qy * axis.y + qz * axis.z) / self.length_sq;
            // `unit` in the order that becomes two masks
            let t = if t > 1.0 { 1.0 } else { t };
            let t = if t < 0.0 { 0.0 } else { t };
            let (rx, ry, rz) = (qx - axis.x * t, qy - axis.y * t, qz - axis.z * t);
            let d_sq = rx * rx + ry * ry + rz * rz;
            out.d_sq[k] = d_sq;
            out.nearer[k] = d_sq.sqrt() - self.closed.radius;
            let (inside, past) = (falloff(d_sq, self.kernel_sq), falloff(d_sq, self.reach_sq));
            let claim = if d_sq < self.kernel_sq { kernel_weight * inside } else { fill_weight * past };
            out.claim[k] = claim * self.priority;
        }
    }

    /// What it claims a sample `d_sq` from its axis with, as [`SparseGrid::splat`] has it
    #[cfg(test)]
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
/// sample from `written.0` to `written.1` gets the squared distance to and the feature of the nearest seed
fn transform_line(
    d: &mut [f32],
    mut feature: Option<&mut [u32]>,
    written: (usize, usize),
    v: &mut Vec<usize>,
    z: &mut Vec<f32>,
    out: &mut Vec<(f32, u32)>,
) {
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
    for q in written.0..written.1 {
        while k + 1 < v.len() && z[k + 1] < q as f32 {
            k += 1;
        }
        let offset = q as f32 - v[k] as f32;
        out.push((offset * offset + d[v[k]], feature.as_ref().map_or(0, |f| f[v[k]])));
    }
    for (q, &(dq, fq)) in (written.0..).zip(out.iter()) {
        d[q] = dq;
        if let Some(f) = feature.as_mut() {
            f[q] = fq;
        }
    }
}

/// Whether [`transform_line`] leaves a line as it is: without a seed, or with the same value at every
/// sample, where every sample is its own nearest seed. Most lines of a transform over the bounding box are
/// one of them. Neighbouring parabolas of a constant line cross half way between their seeds, a sample away
/// from the next crossing, and the rounding of `d + i²` moves a crossing by far less than that while the
/// line and the value are no longer than [`UNCHANGED_LONGEST`] and [`UNCHANGED_LARGEST`].
fn unchanged_line(line: &[f32]) -> bool {
    let first = line[0];
    if first == f32::INFINITY {
        return line.iter().all(|&v| v == f32::INFINITY);
    }
    line.len() <= UNCHANGED_LONGEST
        && first.abs() <= UNCHANGED_LARGEST
        && line.iter().all(|v| v.to_bits() == first.to_bits())
}

const UNCHANGED_LONGEST: usize = 1024;
const UNCHANGED_LARGEST: f32 = 65536.0;

/// Per line of a dense grid along each axis, the samples from the first to the last one in a block of a
/// [`SparseGrid`]. Most of the bounding box lies outside the blocks, where nothing reads a transform.
struct LineSpans {
    samples: [usize; 3],
    blocks: [usize; 3],
    /// Per axis and line of blocks along it, by the block coordinates across it in order, the first block and
    /// one past the last
    spans: [Vec<(u32, u32)>; 3],
}

impl LineSpans {
    const ACROSS: [[usize; 2]; 3] = [[1, 2], [0, 2], [0, 1]];

    fn of(grid: &SparseGrid) -> LineSpans {
        Self::of_blocks(grid.samples, |b| grid.block_slots[b] != EMPTY)
    }

    /// The spans of the blocks of a grid of `samples` that `occupied` marks, by their index
    fn of_blocks(samples: [usize; 3], occupied: impl Fn(usize) -> bool) -> LineSpans {
        let b = samples.map(|n| n.div_ceil(BLOCK));
        let mut spans = Self::ACROSS.map(|[u, v]| vec![(u32::MAX, 0u32); b[u] * b[v]]);
        for i in (0..b[0] * b[1] * b[2]).filter(|&i| occupied(i)) {
            let at = [i % b[0], i / b[0] % b[1], i / (b[0] * b[1])];
            for (axis, [u, v]) in Self::ACROSS.into_iter().enumerate() {
                let span = &mut spans[axis][at[u] + b[u] * at[v]];
                span.0 = span.0.min(at[axis] as u32);
                span.1 = span.1.max(at[axis] as u32 + 1);
            }
        }
        LineSpans { samples, blocks: b, spans }
    }

    /// Every line from end to end
    #[cfg(test)]
    fn whole(samples: [usize; 3]) -> LineSpans {
        Self::of_blocks(samples, |_| true)
    }

    /// The samples of the line along `axis` at `u` and `v` across it
    fn along(&self, axis: usize, u: usize, v: usize) -> (usize, usize) {
        let [bu, _] = Self::ACROSS[axis];
        let (first, end) = self.spans[axis][u / BLOCK + self.blocks[bu] * (v / BLOCK)];
        if first >= end {
            return (0, 0);
        }
        (first as usize * BLOCK, (end as usize * BLOCK).min(self.samples[axis]))
    }
}

/// Squared distance in samples from every sample to the nearest one with a
/// finite `d`, plus `d` there, and the `feature` of that sample if given.
///
/// Only the `spans` of the lines are written, the samples beyond them keep their values. Unless `exact`, the seeds
/// beyond the spans are left out too, and a seed counts only where it reaches a sample through the spans of the
/// passes. When `exact`, the passes along x and y transform whole lines and the one along z reads them, so the
/// spans get the transform of the whole grid. The lines along y and z are gathered a row at a time into a buffer
/// and written back the same way, and lines that [`unchanged_line`] finds are left out.
fn distance_transform(d: &mut [f32], mut feature: Option<&mut [u32]>, spans: &LineSpans, exact: bool) {
    let [nx, ny, nz] = spans.samples;
    let (mut v, mut z, mut out) = (Vec::new(), Vec::new(), Vec::new());
    for (i, line) in d.chunks_exact_mut(nx).enumerate() {
        let (start, end) = if exact { (0, nx) } else { spans.along(0, i % ny, i / ny) };
        let line = &mut line[start..end];
        if !line.is_empty() && !unchanged_line(line) {
            let f = feature.as_mut().map(|f| &mut f[i * nx + start..i * nx + end]);
            transform_line(line, f, (0, end - start), &mut v, &mut z, &mut out);
        }
    }
    let longest = ny.max(nz);
    let mut buffers = PlaneBuffers {
        d: vec![0.0f32; nx * longest],
        f: vec![0u32; if feature.is_some() { nx * longest } else { 0 }],
        changed: vec![false; nx],
        spans: vec![(0, 0); nx],
        whole: false,
        v,
        z,
        out,
    };
    // the lines of one plane, along y in a plane of one z or along z in a plane of one y: sample `k` of the
    // line at `x` is at `first + k * stride + x`
    for (axis, len, stride, planes, plane_step) in [(1, ny, nx, nz, nx * ny), (2, nz, nx * ny, ny, nx)] {
        buffers.whole = exact;
        for plane in 0..planes {
            for (x, span) in buffers.spans.iter_mut().enumerate() {
                *span = if exact && axis == 1 { (0, len) } else { spans.along(axis, x, plane) };
            }
            transform_plane(d, feature.as_deref_mut(), &mut buffers, len, stride, plane * plane_step);
        }
    }
}

/// What [`transform_plane`] reuses from plane to plane
struct PlaneBuffers {
    d: Vec<f32>,
    f: Vec<u32>,
    changed: Vec<bool>,
    /// The span of the line at each x
    spans: Vec<(usize, usize)>,
    /// Whether the lines are read as a whole, and only their spans are written
    whole: bool,
    v: Vec<usize>,
    z: Vec<f32>,
    out: Vec<(f32, u32)>,
}

/// The lines of one plane of [`distance_transform`], that starts at `first`
#[inline(never)]
fn transform_plane(
    d: &mut [f32],
    mut feature: Option<&mut [u32]>,
    buffers: &mut PlaneBuffers,
    len: usize,
    stride: usize,
    first: usize,
) {
    let PlaneBuffers { d: buf_d, f: buf_f, changed, spans, whole, v, z, out } = buffers;
    let (mut rows, mut columns) = ((len, 0), (spans.len(), 0));
    for (x, &(start, end)) in spans.iter().enumerate().filter(|(_, s)| s.0 < s.1) {
        rows = (rows.0.min(start), rows.1.max(end));
        columns = (columns.0.min(x), x + 1);
    }
    if columns.0 >= columns.1 {
        return;
    }
    let columns = columns.0..columns.1;
    let read = if *whole { (0, len) } else { rows };
    for k in read.0..read.1 {
        let row = first + k * stride;
        for x in columns.clone() {
            buf_d[x * len + k] = d[row + x];
        }
        if let Some(f) = feature.as_ref() {
            for x in columns.clone() {
                buf_f[x * len + k] = f[row + x];
            }
        }
    }
    for x in columns.clone() {
        let (start, end) = spans[x];
        let (from, written) = if *whole { (0, (start, end)) } else { (start, (0, end - start)) };
        let to = if *whole { len } else { end };
        let line = &mut buf_d[x * len + from..x * len + to];
        changed[x] = start < end && !unchanged_line(line);
        if changed[x] {
            let f = feature.is_some().then(|| &mut buf_f[x * len + from..x * len + to]);
            transform_line(line, f, written, v, z, out);
        }
    }
    if !changed[columns.clone()].iter().any(|&c| c) {
        return;
    }
    for k in rows.0..rows.1 {
        let row = first + k * stride;
        let written = |&x: &usize| changed[x] && spans[x].0 <= k && k < spans[x].1;
        for x in columns.clone().filter(written) {
            d[row + x] = buf_d[x * len + k];
        }
        if let Some(f) = feature.as_mut() {
            for x in columns.clone().filter(written) {
                f[row + x] = buf_f[x * len + k];
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
/// `rho`, see [`blur_by_radius`]. The blur wears a tissue thinner than about
/// itself down to a core or away, the paths in it are then strands, see
/// [`TissueField::holds`].
///
/// Every sample of a block farther than its `rho` from the tubes is the centre
/// of an empty ball as large as that distance, the balls of `rho` around the
/// centres near it lie in it. The samples outside the blocks are no centres: a
/// ball there is at most as large as the largest radius plus two cells and at
/// least that far from every tube, so the balls in the blocks around it cover
/// what it covers of the blocks. Every other sample takes the ball that covers
/// it most, by the power diagram of the balls: the distance transform starts at
/// `−R²` at every centre (Felzenszwalb, Huttenlocher). Where the closing is just
/// the tubes, the distance to that ball is the distance to the tubes at any cell
/// size, the nearest centre by grid steps would lie up to half a cell off and
/// swell it.
fn closing(grid: &SparseGrid, frame: &Frame) -> Vec<f32> {
    let [nx, ny, nz] = grid.samples;
    let n = nx * ny * nz;
    // every tube reaches the largest radius and a little further, past that a ball of the largest radius is empty
    let rho_far = frame.close_max;

    // the centre of the empty ball that covers each sample most, by squared distance in cells less the squared
    // radius of the ball
    let mut d = vec![f32::INFINITY; n];
    let mut feature = vec![EMPTY; n];
    for (row, (d, feature)) in d.chunks_mut(nx).zip(feature.chunks_mut(nx)).enumerate() {
        seed_balls(grid, rho_far, row * nx, d, feature);
    }
    let spans = LineSpans::of(grid);
    distance_transform(&mut d, Some(&mut feature), &spans, false);

    // share of every sample inside the closing, ramping from 0 to 1 over two cells across its surface
    let mut shifts: Vec<(f32, f32)> = Vec::new();
    let mut solid = d;
    for (row, solid) in solid.chunks_mut(nx).enumerate() {
        closing_row(grid, rho_far, row * nx, solid, &feature, &mut shifts);
    }
    drop(feature);
    blur_by_radius(&mut solid, grid, frame);
    solid
}

/// The radius in cells of the empty ball around sample `i` of a block, where no tube reaches the sample it is at
/// least as far as every tube reaches
fn ball(grid: &SparseGrid, rho_far: f32, i: usize) -> Option<f32> {
    let [nx, ny, _] = grid.samples;
    let slot = grid.index([i % nx, i / nx % ny, i / (nx * ny)])?;
    let (tube, rho) = match grid.tube[slot] {
        tube if tube.is_finite() => (tube, grid.tube_close[slot]),
        _ => (rho_far + 2.0 * grid.cell, rho_far),
    };
    (tube > rho).then_some(tube / grid.cell)
}

/// The balls of the samples `first..` for [`closing`]
#[inline(never)]
fn seed_balls(grid: &SparseGrid, rho_far: f32, first: usize, d: &mut [f32], feature: &mut [u32]) {
    for (k, (d, feature)) in d.iter_mut().zip(feature).enumerate() {
        if let Some(r) = ball(grid, rho_far, first + k) {
            *d = -r * r;
            *feature = (first + k) as u32;
        }
    }
}

/// The share of the closing at the samples `first..`, `solid` holds their distance transform on the way in
#[inline(never)]
fn closing_row(
    grid: &SparseGrid,
    rho_far: f32,
    first: usize,
    solid: &mut [f32],
    feature: &[u32],
    shifts: &mut Vec<(f32, f32)>,
) {
    let [nx, ny, _] = grid.samples;
    let position = |i: usize| Vec3::new((i % nx) as f32, (i / nx % ny) as f32, (i / (nx * ny)) as f32);
    for (k, solid) in solid.iter_mut().enumerate() {
        let i = first + k;
        let Some(slot) = grid.index([i % nx, i / nx % ny, i / (nx * ny)]) else {
            *solid = 0.0;
            continue;
        };
        let tube = grid.tube[slot] / grid.cell;
        if !tube.is_finite() {
            *solid = 0.0;
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
            (ball(grid, rho_far, f).unwrap_or(0.0) - position(i).distance(position(f))).min(tube)
        };
        let allowed = grid.guard[slot] / grid.cell - GUARD * rho / grid.cell;
        let closing = (0.5 - 0.5 * (outside + shift)).min(0.5 + 0.5 * (allowed - shift));
        *solid = closing.max(0.5 - 0.5 * (tube + shift)).clamp(0.0, 1.0);
    }
}

/// Blurs the closing by [`SMOOTHING`] times the closing radius of every sample, so a tissue that
/// fills wider gaps is as smooth as it would be alone. Every radius blurs the whole closing,
/// and every sample takes the blurs weighted by how many samples of each radius are around
/// it, blurred by the largest blur: where tissues of different radii meet, the surface goes
/// over from the one blur to the other within about that blur.
fn blur_by_radius(solid: &mut Vec<f32>, grid: &SparseGrid, frame: &Frame) {
    let [nx, _, _] = grid.samples;
    let mut radii: Vec<f32> = Vec::new();
    for s in (0..grid.tube.len()).filter(|&s| grid.tube[s].is_finite()) {
        if !radii.contains(&grid.tube_close[s]) {
            radii.push(grid.tube_close[s]);
        }
    }
    radii.sort_by(f32::total_cmp);
    if radii.len() < 2 {
        let rho = radii.first().copied().unwrap_or(frame.close_min);
        blur(solid, grid.samples, SMOOTHING * rho / grid.cell);
        return;
    }
    let sharp = std::mem::take(solid);
    let n = sharp.len();
    // per sample which of the radii closes it, `u16::MAX` where no tube is
    let mut which = vec![u16::MAX; n];
    for (i, s) in grid.samples_in_blocks().filter(|&(_, s)| grid.tube[s].is_finite()) {
        which[i] = radii.iter().position(|&rho| rho == grid.tube_close[s]).unwrap_or(0) as u16;
    }
    let mut sum = vec![0.0f32; n];
    let mut total = vec![0.0f32; n];
    // the same buffers for every radius, and the copy each blur reads from
    let (mut weight, mut blurred, mut source) = (vec![0.0f32; n], vec![0.0f32; n], Vec::new());
    for (k, &rho) in radii.iter().enumerate() {
        for (w, &which) in weight.iter_mut().zip(&which) {
            *w = (which as usize == k) as u8 as f32;
        }
        blur_rows(&mut weight, grid.samples, SMOOTHING * frame.close_max / grid.cell, None, &mut source);
        // the closing blurred by this radius only counts where its weight does, the rest of the grid isn't blurred
        let wanted: Vec<bool> = weight.chunks_exact(nx).map(|row| row.iter().any(|&w| w != 0.0)).collect();
        blurred.copy_from_slice(&sharp);
        blur_rows(&mut blurred, grid.samples, SMOOTHING * rho / grid.cell, Some(&wanted), &mut source);
        for (((sum, total), weight), blurred) in
            sum.chunks_mut(nx).zip(total.chunks_mut(nx)).zip(weight.chunks(nx)).zip(blurred.chunks(nx))
        {
            add_weighted(sum, total, weight, blurred);
        }
    }
    drop((sharp, weight, blurred, source));
    for (s, &t) in sum.iter_mut().zip(&total) {
        *s = if t > 0.0 { *s / t } else { 0.0 };
    }
    *solid = sum;
}

/// Adds the blurred closing to `sum` and its weight to `total`, where the weight isn't 0
#[inline(never)]
fn add_weighted(sum: &mut [f32], total: &mut [f32], weight: &[f32], blurred: &[f32]) {
    for i in (0..sum.len()).filter(|&i| weight[i] != 0.0) {
        sum[i] += weight[i] * blurred[i];
        total[i] += weight[i];
    }
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
/// another stays inside it when both grow, so no medial axis is needed. Only the
/// samples in the `spans` get theirs.
fn grown_balls(radius: &[f32], spans: &LineSpans, scale: f32) -> (Vec<f32>, Vec<u32>) {
    let mut power: Vec<f32> =
        radius.iter().map(|&r| if r > 0.0 { -(scale * r) * (scale * r) } else { f32::INFINITY }).collect();
    let mut centre: Vec<u32> = (0..radius.len()).map(|i| if radius[i] > 0.0 { i as u32 } else { EMPTY }).collect();
    distance_transform(&mut power, Some(&mut centre), spans, true);
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
    /// The largest depth, 0 without any tissue
    deepest: f32,
}

impl TissueField {
    /// The tissue of the closed segments, closed and blurred on the grid of
    /// `frame` as [`surface_in`] does it. Its balls grow by `scale`, at infinity
    /// it covers everything. `None` when nothing is closed.
    pub fn new(frame: &Frame, segments: &[Segment], meshing: &[Meshing], scale: f32) -> Option<TissueField> {
        if !frame.closing {
            return None;
        }
        let mut grid = frame.tissue_grid();
        let closed: Vec<u32> = (0..segments.len() as u32).filter(|&i| frame.closes(&meshing[i as usize])).collect();
        grid.splat_closed(closed.len(), |k| frame.closed_splat(closed[k], &segments[closed[k] as usize], &meshing[closed[k] as usize]));
        drop(closed);
        frame.guard(&mut grid, segments, meshing);
        let solid = closing(&grid, frame);

        // the surface lies between a tissue sample and the next one outside, so half a cell above the outer samples
        let mut depth: Vec<f32> = solid.iter().map(|&v| if v < SURFACE_SHARE { 0.0 } else { f32::INFINITY }).collect();
        let spans = LineSpans::of(&grid);
        distance_transform(&mut depth, None, &spans, false);
        for d in &mut depth {
            *d = if *d > 0.0 && d.is_finite() { d.sqrt() - 0.5 } else { 0.0 };
        }
        let deepest = depth.iter().fold(0.0f32, |a, &d| a.max(d)) * grid.cell;
        let centre =
            if deepest > 0.0 && scale.is_finite() { grown_balls(&depth, &spans, scale.max(1.0)).1 } else { Vec::new() };
        for d in &mut depth {
            *d *= grid.cell;
        }
        Some(TissueField { frame: frame.clone(), grid, solid, depth, centre, scale, deepest })
    }

    /// [`surface_in`] of the segments the field was made of, without closing them again.
    /// `meshing` must be the same as for the field.
    pub fn surface(self, segments: &[Segment], meshing: &[Meshing], strand: impl Fn(usize) -> bool) -> SegmentSurface {
        let TissueField { frame, mut grid, solid, depth, centre, .. } = self;
        drop((depth, centre));
        grid.start_meshing();
        grid.allocate_all(segments.iter().zip(meshing).filter(|(_, m)| !frame.closes(m)).map(|(s, m)| (*s, frame.splat_reach(m, s, false))));
        for (i, (s, m)) in segments.iter().zip(meshing).enumerate() {
            if !frame.closes(m) {
                grid.splat(i as u32, m, s, frame.kernel(m, s.radius), None);
            } else if strand(i) {
                // the tube and the owners are there already, only the strand is added
                if let Some(radius) = frame.closed(m, s, true).strand {
                    grid.splat_strand(s, radius);
                }
            }
        }
        add_tissue(&mut grid, &solid, frame.threshold);
        drop(solid);
        frame.floor(&mut grid, segments, meshing);
        extract(&grid, &frame)
    }

    /// Whether there is any tissue at all
    pub fn has_tissue(&self) -> bool {
        self.deepest > 0.0
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

    /// Whether the tissue holds the tube of `radius` around `p` on a path along `direction`: its axis and
    /// a ring around it lie inside. The blur wears a tissue thinner than about itself down to a core
    /// around the axis, a path there sticks out of it.
    pub fn holds(&self, p: Vec3, direction: Vec3, radius: f32) -> bool {
        let u = (p - self.frame.origin) / self.frame.cell;
        let nearest = [0, 1, 2].map(|a| (u[a].round().max(0.0) as usize).min(self.frame.samples[a] - 1));
        if self.depth[self.index(nearest)] > radius + HELD_DEPTH_IN_CELLS * self.frame.cell {
            return true;
        }
        if !self.inside(p) {
            return false;
        }
        match direction.try_normalize() {
            Some(d) => {
                let (u, v) = d.any_orthonormal_pair();
                (0..TUBE_RING).all(|k| {
                    let (sin, cos) = (k as f32 * std::f32::consts::TAU / TUBE_RING as f32).sin_cos();
                    self.inside(p + (u * cos + v * sin) * radius)
                })
            }
            None => [Vec3::X, Vec3::Y, Vec3::Z].iter().all(|&a| self.inside(p + a * radius) && self.inside(p - a * radius)),
        }
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
    /// well: a strand there is a small feature next to the tissue. Only a ball of
    /// the tissue deeper than `radius` grows faster than the strand's, so at an
    /// infinite scale one of them anywhere covers it, and a tissue thinner than
    /// the strand never does.
    pub fn covers(&self, p: Vec3, radius: f32) -> bool {
        if !self.scale.is_finite() {
            return self.deepest > radius;
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
    blur_rows(values, samples, sigma, None, &mut Vec::new());
}

/// [`blur`] that only has to be right in the rows along x that `wanted` marks, one flag per row.
/// The other rows keep finite values when `values` are finite, but no meaningful ones. `source` is
/// where the passes copy what they read, it can be reused for the next blur.
///
/// Most rows lie outside the tissue and are all zero. They add nothing to any sum, so they are left
/// out as sources, and a row that is all zero stays so along x. Of the passes before the last only
/// the rows that a wanted row reads are blurred. The sums of the rows that are blurred keep their
/// terms and their order. On wasm the loops over whole rows run four samples at a time.
#[cfg_attr(target_arch = "wasm32", target_feature(enable = "simd128"))]
fn blur_rows(values: &mut [f32], samples: [usize; 3], sigma: f32, wanted: Option<&[bool]>, source: &mut Vec<f32>) {
    let weights = blur_weights(sigma);
    if weights.is_empty() {
        return;
    }
    let radius = weights.len() / 2;
    let [nx, ny, nz] = samples;
    let rows = ny * nz;
    // the taps of a sample `k` along a line of `len` that lie on the line
    let taps = |k: usize, len: usize| tap_range(&weights, radius, k, len);
    // the rows the pass along z writes, along y and along x: each pass reads its taps' rows of the one before
    let along_z = wanted.map_or_else(|| vec![true; rows], <[bool]>::to_vec);
    let widen = |wanted: &[bool], len: usize, rows_per_step: usize| -> Vec<bool> {
        (0..rows)
            .map(|r| {
                let k = r / rows_per_step % len;
                let first = r - k * rows_per_step;
                taps(k, len).any(|o| wanted[first + (k + o - radius) * rows_per_step])
            })
            .collect()
    };
    let along_y = if wanted.is_some() { widen(&along_z, nz, ny) } else { along_z.clone() };
    let along_x = if wanted.is_some() { widen(&along_y, ny, 1) } else { along_z.clone() };

    let mut line = vec![0.0; nx];
    for (row, _) in values.chunks_exact_mut(nx).zip(&along_x).filter(|(_, w)| **w) {
        blur_row_along_x(row, &mut line, &weights);
    }
    // along y and z whole rows at once, the samples of a row lie next to each other
    source.clear();
    source.extend_from_slice(values);
    let mut filled = vec![false; rows];
    for (len, rows_per_step, wanted) in [(ny, 1, &along_y), (nz, ny, &along_z)] {
        source.copy_from_slice(values);
        for (filled, row) in filled.iter_mut().zip(source.chunks_exact(nx)) {
            *filled = row.iter().any(|&v| v != 0.0);
        }
        for (r, row) in values.chunks_exact_mut(nx).enumerate().filter(|&(r, _)| wanted[r]) {
            blur_row_across(row, r, source, &filled, &weights, len, rows_per_step);
        }
    }
}

/// The taps of a sample `k` along a line of `len` that lie on the line
fn tap_range(weights: &[f32], radius: usize, k: usize, len: usize) -> std::ops::Range<usize> {
    radius.saturating_sub(k)..weights.len().min(len + radius - k)
}

/// One row of [`blur_rows`] along x, `line` is where it copies the row to read from
#[cfg_attr(target_arch = "wasm32", target_feature(enable = "simd128"))]
#[inline(never)]
fn blur_row_along_x(row: &mut [f32], line: &mut [f32], weights: &[f32]) {
    if row.iter().all(|&v| v == 0.0) {
        return;
    }
    let (nx, radius) = (row.len(), weights.len() / 2);
    line.copy_from_slice(row);
    // at the ends of the row the taps that fall off it are left out
    let inner = radius.min(nx)..nx.saturating_sub(radius).max(radius.min(nx));
    for k in (0..inner.start).chain(inner.end..nx) {
        let mut sum = 0.0;
        for o in tap_range(weights, radius, k, nx) {
            sum += weights[o] * line[k + o - radius];
        }
        row[k] = sum;
    }
    convolve(&mut row[inner.clone()], &line[inner.start - radius..], weights);
}

/// Row `r` of [`blur_rows`] along y or z
#[cfg_attr(target_arch = "wasm32", target_feature(enable = "simd128"))]
#[inline(never)]
fn blur_row_across(
    row: &mut [f32],
    r: usize,
    source: &[f32],
    filled: &[bool],
    weights: &[f32],
    len: usize,
    rows_per_step: usize,
) {
    let (nx, radius) = (row.len(), weights.len() / 2);
    let k = r / rows_per_step % len;
    let first = r - k * rows_per_step;
    row.fill(0.0);
    for o in tap_range(weights, radius, k, len) {
        let from = first + (k + o - radius) * rows_per_step;
        if !filled[from] {
            continue;
        }
        add_scaled(row, &source[from * nx..(from + 1) * nx], weights[o]);
    }
}

/// `to[k] = Σ weights[o] · from[k + o]` over all taps `o` in order, four samples at a time on wasm
#[cfg_attr(target_arch = "wasm32", target_feature(enable = "simd128"))]
fn convolve(to: &mut [f32], from: &[f32], weights: &[f32]) {
    #[cfg(not(target_arch = "wasm32"))]
    let k = 0;
    #[cfg(target_arch = "wasm32")]
    let k = {
        let mut k = 0;
        use core::arch::wasm32::*;
        while k + 4 <= to.len() {
            let mut sum = f32x4_splat(0.0);
            for (o, &w) in weights.iter().enumerate() {
                let s = &from[k + o..k + o + 4];
                // SAFETY: `s` and the four samples of `to` are 16 bytes each, v128 needs no alignment
                sum = f32x4_add(sum, f32x4_mul(f32x4_splat(w), unsafe { v128_load(s.as_ptr().cast()) }));
            }
            unsafe { v128_store(to[k..k + 4].as_mut_ptr().cast(), sum) };
            k += 4;
        }
        k
    };
    for k in k..to.len() {
        let mut sum = 0.0;
        for (o, &w) in weights.iter().enumerate() {
            sum += w * from[k + o];
        }
        to[k] = sum;
    }
}

/// `to[k] += w · from[k]`, four samples at a time on wasm
#[cfg_attr(target_arch = "wasm32", target_feature(enable = "simd128"))]
fn add_scaled(to: &mut [f32], from: &[f32], w: f32) {
    #[cfg(not(target_arch = "wasm32"))]
    let k = 0;
    #[cfg(target_arch = "wasm32")]
    let k = {
        let mut k = 0;
        use core::arch::wasm32::*;
        let n = to.len().min(from.len());
        let w4 = f32x4_splat(w);
        while k + 4 <= n {
            // SAFETY: four samples of each, 16 bytes, v128 needs no alignment
            unsafe {
                let sum = f32x4_add(v128_load(to[k..k + 4].as_ptr().cast()), f32x4_mul(w4, v128_load(from[k..k + 4].as_ptr().cast())));
                v128_store(to[k..k + 4].as_mut_ptr().cast(), sum);
            }
            k += 4;
        }
        k
    };
    for (v, &s) in to[k..].iter_mut().zip(&from[k..]) {
        *v += w * s;
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
            meshing: true,
        }
    }

    /// The grid of a [`TissueField`], without what only its mesh needs, see [`SparseGrid::start_meshing`]
    fn tissue_grid(&self) -> SparseGrid {
        SparseGrid { meshing: false, ..self.grid() }
    }

    /// How far a segment is splatted, see [`SparseGrid::splat`]
    fn splat_reach(&self, meshing: &Meshing, segment: &Segment, strand: bool) -> f32 {
        if self.closes(meshing) {
            splat_reach(self.owner_kernel(meshing, segment), Some(self.closed(meshing, segment, strand)))
        } else {
            self.kernel(meshing, segment.radius).reach
        }
    }

    /// The closed segment `index` as [`SparseGrid::splat_closed`] splats it
    fn closed_splat(&self, index: u32, segment: &Segment, meshing: &Meshing) -> ClosedSplat {
        ClosedSplat::new(index, meshing, *segment, self.owner_kernel(meshing, segment), self.closed(meshing, segment, false))
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
        grid.allocate_all(thin.iter().map(|&i| (segments[i], self.kernel(&meshing[i], self.min_radius).reach)));
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
    grid.allocate_all(segments.iter().zip(meshing).enumerate().map(|(i, (s, m))| (*s, frame.splat_reach(m, s, strand(i)))));
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
            splats.push(frame.closed_splat(i as u32, s, m));
        }
        let mut together = frame.grid();
        together.splat_closed(splats.len(), |k| splats[k]);
        assert_eq!(together.block_slots, in_turn.block_slots);
        assert_eq!(together.tube, in_turn.tube);
        assert_eq!(together.tube_close, in_turn.tube_close);
        assert_eq!(together.owner, in_turn.owner);
        assert_eq!(together.owner_k, in_turn.owner_k);
        assert!(in_turn.owner.iter().filter(|&&o| o != EMPTY).count() > 5000);
    }

    #[test]
    fn a_tile_measures_a_closed_segment_as_every_sample_on_its_own() {
        let mut seed = 3u32;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed as f32 / u32::MAX as f32
        };
        let mut segments = Vec::new();
        let mut meshing = Vec::new();
        for i in 0..200 {
            let at = Vec3::new(next(), next(), next()) * 0.05;
            let along = if i % 7 == 0 { Vec3::ZERO } else { Vec3::new(next() - 0.5, next() - 0.5, next() - 0.5) * 0.02 };
            segments.push(segment(at, at + along, 0.001 + 0.004 * next()));
            meshing.push(Meshing { priority: [1.0, 0.5, 2.0, 0.0][i % 4], ..Meshing::closed([3.0, 2.0][i % 2], 0.004) });
        }
        let frame = Frame::new(&segments, &meshing, 48).unwrap();
        for (i, (s, m)) in segments.iter().zip(&meshing).enumerate() {
            let splat = frame.closed_splat(i as u32, s, m);
            let mut tile = Tile::default();
            for k in 0..TILE {
                (tile.x[k], tile.y[k], tile.z[k]) = (next() * 0.06, next() * 0.06, next() * 0.06);
            }
            let mut measured = TileMeasures::default();
            splat.measure(&tile, &mut measured);
            for k in 0..TILE {
                let d_sq = splat.distance_sq(Vec3::new(tile.x[k], tile.y[k], tile.z[k]));
                assert_eq!(measured.d_sq[k].to_bits(), d_sq.to_bits());
                assert_eq!(measured.nearer[k].to_bits(), (d_sq.sqrt() - splat.closed.radius).to_bits());
                assert_eq!(measured.claim[k].to_bits(), splat.claim(d_sq).to_bits());
            }
        }
    }

    #[test]
    fn the_distance_of_a_segment_to_a_box_is_the_least_along_it() {
        let (lo, hi) = (Vec3::new(-0.5, -0.2, 0.0), Vec3::new(0.5, 0.3, 0.4));
        for i in 0..2000 {
            let at = |k: u32| Vec3::new(random(k), random(k + 1), random(k + 2)) * 3.0 - Vec3::splat(1.5);
            let (a, b) = (at(6 * i), if i % 7 == 0 { at(6 * i) } else { at(6 * i + 3) });
            let brute = (0..=4000)
                .map(|k| {
                    let p = a.lerp(b, k as f32 / 4000.0);
                    (lo - p).max(Vec3::ZERO).max(p - hi).length_squared()
                })
                .fold(f32::INFINITY, f32::min);
            let exact = segment_box_distance_sq(a, b, lo, hi);
            assert!(exact <= brute + 1e-6 && exact >= brute - 2e-3 * brute.sqrt() - 1e-6, "{exact} {brute} {a} {b}");
        }
    }

    #[test]
    fn every_cell_with_a_corner_a_segment_reaches_has_a_slot() {
        let segments: Vec<Segment> = (0..40)
            .map(|i| {
                let from = Vec3::new(random(i), random(i + 100), random(i + 200)) * 2.0;
                segment(from, from + Vec3::new(random(i + 300) - 0.5, random(i + 400), 0.3), 0.02 + 0.05 * random(i + 500))
            })
            .collect();
        let meshing = vec![Meshing::blended(2.0); segments.len()];
        let frame = Frame::new(&segments, &meshing, 80).unwrap();
        let mut grid = frame.grid();
        grid.allocate_all(segments.iter().zip(&meshing).map(|(s, m)| (*s, frame.splat_reach(m, s, false))));
        let [nx, ny, nz] = grid.samples;
        let (mut reached, mut empty) = (0, 0);
        for z in 0..nz {
            for y in 0..ny {
                for x in 0..nx {
                    let p = grid.position(Vec3::new(x as f32, y as f32, z as f32));
                    let near = segments.iter().zip(&meshing).any(|(s, m)| {
                        let reach = frame.splat_reach(m, s, false);
                        crate::spatial::distance_sq_to_segment(p, s.start, s.end) < reach * reach
                    });
                    if near {
                        reached += 1;
                        for o in CORNERS {
                            let c = [x.wrapping_sub(o[0]), y.wrapping_sub(o[1]), z.wrapping_sub(o[2])];
                            if c[0] < nx && c[1] < ny && c[2] < nz {
                                assert_ne!(grid.slot(c), EMPTY, "cell {c:?} next to {:?}", [x, y, z]);
                            }
                        }
                    } else if grid.index([x, y, z]).is_none() {
                        empty += 1;
                    }
                }
            }
        }
        assert!(reached > 10_000 && empty > 10_000, "{reached} {empty}");
    }

    #[test]
    fn reaches_allocated_at_once_get_the_slots_of_one_after_another_and_no_more_room() {
        let segments: Vec<Segment> = (0..40)
            .map(|i| {
                let from = Vec3::new(random(i), random(i + 100), random(i + 200)) * 2.0;
                segment(from, from + Vec3::new(random(i + 300), random(i + 400), 0.1), 0.02 + 0.05 * random(i + 500))
            })
            .collect();
        let meshing = vec![Meshing::blended(2.0); segments.len()];
        let frame = Frame::new(&segments, &meshing, 80).unwrap();
        let reaches: Vec<_> = segments.iter().zip(&meshing).map(|(s, m)| (*s, frame.splat_reach(m, s, false))).collect();
        let mut one_by_one = frame.grid();
        for (s, reach) in &reaches {
            one_by_one.allocate(s, *reach);
        }
        let mut at_once = frame.grid();
        at_once.allocate_all(reaches.iter().copied());
        assert_eq!(at_once.block_slots, one_by_one.block_slots);
        assert_eq!(at_once.density.len(), one_by_one.density.len());
        for len in [at_once.density.capacity(), at_once.tube.capacity(), at_once.owner.capacity(), at_once.run_k.capacity()] {
            assert_eq!(len, at_once.density.len());
        }
        // the grid of a tissue has no density and no strands until it is meshed
        let mut tissue = frame.tissue_grid();
        tissue.allocate_all(reaches.iter().copied());
        assert!(tissue.density.is_empty() && tissue.strand.is_empty() && tissue.run.is_empty() && tissue.run_k.is_empty());
        assert_eq!(tissue.tube.len(), at_once.tube.len());
        tissue.start_meshing();
        assert_eq!((tissue.density.len(), tissue.strand.len(), tissue.run.len()), (at_once.density.len(), at_once.tube.len(), at_once.tube.len()));
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

    /// The blur as it was before it left out rows of zeros, every sum over all its taps
    fn blur_every_row(values: &mut [f32], samples: [usize; 3], sigma: f32) {
        let weights = blur_weights(sigma);
        if weights.is_empty() {
            return;
        }
        let radius = weights.len() / 2;
        let [nx, ny, nz] = samples;
        let taps = |k: usize, len: usize| radius.saturating_sub(k)..weights.len().min(len + radius - k);
        let line_of = |values: &[f32], start: usize, len: usize, stride: usize| -> Vec<f32> {
            (0..len).map(|k| values[start + k * stride]).collect()
        };
        for (len, stride) in [(nx, 1), (ny, nx), (nz, nx * ny)] {
            let source = values.to_vec();
            for outer in 0..values.len() / (len * stride) {
                for inner in 0..stride {
                    let start = outer * len * stride + inner;
                    let line = line_of(&source, start, len, stride);
                    for k in 0..len {
                        let mut sum = 0.0;
                        for o in taps(k, len) {
                            sum += weights[o] * line[k + o - radius];
                        }
                        values[start + k * stride] = sum;
                    }
                }
            }
        }
    }

    #[test]
    fn leaving_out_rows_of_zeros_changes_no_bit_of_the_blur() {
        let samples = [37, 29, 31];
        let n = samples[0] * samples[1] * samples[2];
        for (round, sigma) in [0.8f32, 2.3, 4.6].into_iter().enumerate() {
            // a few blobs in empty space, with negative zeros among the zeros
            let mut values = vec![0.0f32; n];
            for i in 0..n {
                let p = Vec3::new((i % 37) as f32, (i / 37 % 29) as f32, (i / (37 * 29)) as f32);
                let blob = (0..4).any(|b| {
                    let c = Vec3::new(37.0, 29.0, 31.0) * Vec3::new(random(round as u32 * 40 + b), random(b + 7), random(b + 13));
                    p.distance(c) < 3.0 + 3.0 * random(b + 21)
                });
                values[i] = if blob { random(i as u32 + 1000 * round as u32) } else if i % 7 == 0 { -0.0 } else { 0.0 };
            }
            assert!(values.chunks_exact(37).filter(|r| r.iter().all(|&v| v == 0.0)).count() > 100);
            let mut expected = values.clone();
            blur_every_row(&mut expected, samples, sigma);
            let bits = |v: &[f32]| v.iter().map(|v| v.to_bits()).collect::<Vec<_>>();
            // only some rows wanted, as a blob of the weight of one radius has them
            let wanted: Vec<bool> = (0..29 * 31).map(|r| (r % 29 < 12) && (r / 29 > 8 && r / 29 < 20)).collect();
            let mut some = values.clone();
            blur_rows(&mut some, samples, sigma, Some(&wanted), &mut Vec::new());
            for (r, w) in wanted.iter().enumerate().filter(|(_, w)| **w) {
                assert_eq!(bits(&some[r * 37..(r + 1) * 37]), bits(&expected[r * 37..(r + 1) * 37]), "row {} {}", r, w);
            }
            assert!(some.iter().all(|v| v.is_finite()));
            blur(&mut values, samples, sigma);
            assert_eq!(bits(&values), bits(&expected), "sigma {}", sigma);
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

    /// The distance transform as it was before it left out lines, every line gathered one sample at a time
    fn distance_transform_every_line(d: &mut [f32], feature: &mut [u32], samples: [usize; 3]) {
        let [nx, ny, nz] = samples;
        let n = d.len();
        let (mut v, mut z, mut out) = (Vec::new(), Vec::new(), Vec::new());
        for (len, stride) in [(nx, 1), (ny, nx), (nz, nx * ny)] {
            for start in (0..n / (len * stride)).flat_map(|outer| (0..stride).map(move |inner| outer * len * stride + inner)) {
                let mut line_d: Vec<f32> = (0..len).map(|k| d[start + k * stride]).collect();
                let mut line_f: Vec<u32> = (0..len).map(|k| feature[start + k * stride]).collect();
                transform_line(&mut line_d, Some(&mut line_f), (0, len), &mut v, &mut z, &mut out);
                for k in 0..len {
                    d[start + k * stride] = line_d[k];
                    feature[start + k * stride] = line_f[k];
                }
            }
        }
    }

    #[test]
    fn a_constant_line_is_its_own_transform() {
        let (mut v, mut z, mut out) = (Vec::new(), Vec::new(), Vec::new());
        let values = [0.0, 1.0, 4.0, 0.3, -0.7, -17.389, -255.5 * 255.5, -UNCHANGED_LARGEST, UNCHANGED_LARGEST, 12345.678];
        for c in values {
            for len in (1..=300).chain([511, 512, 777, UNCHANGED_LONGEST]) {
                let line = vec![c; len];
                assert!(unchanged_line(&line));
                let mut d = line.clone();
                let mut feature: Vec<u32> = (0..len as u32).map(|i| i * 3 + 1).collect();
                transform_line(&mut d, Some(&mut feature), (0, len), &mut v, &mut z, &mut out);
                assert!(d.iter().all(|x| x.to_bits() == c.to_bits()), "value {} length {}", c, len);
                assert!(feature.iter().enumerate().all(|(i, &f)| f == i as u32 * 3 + 1), "value {} length {}", c, len);
            }
        }
        assert!(!unchanged_line(&[1.0, 1.0, 2.0]) && !unchanged_line(&[f32::INFINITY, 0.0]));
        assert!(!unchanged_line(&vec![0.0; UNCHANGED_LONGEST + 1]) && !unchanged_line(&[UNCHANGED_LARGEST * 2.0; 4]));
    }

    #[test]
    fn leaving_out_unchanged_lines_changes_no_bit_of_the_distance_transform() {
        let samples = [41, 23, 29];
        let n = samples[0] * samples[1] * samples[2];
        let at = |i: usize| Vec3::new((i % 41) as f32, (i / 41 % 23) as f32, (i / (41 * 23)) as f32);
        let blob = |i: usize, round: u32| {
            (0..3).any(|b| {
                let c = Vec3::new(41.0, 23.0, 29.0) * Vec3::new(random(round * 40 + b), random(round * 40 + b + 7), random(b + 13));
                at(i).distance(c) < 2.0 + 4.0 * random(b + 21)
            })
        };
        for round in 0..3 {
            // as the closing has them: balls of different sizes everywhere but near the tubes
            // and far from them all of the largest radius
            let balls: Vec<f32> = (0..n)
                .map(|i| match (blob(i, round), at(i).z > 20.0) {
                    (true, _) => f32::INFINITY,
                    (false, true) => -7.5f32.powi(2),
                    (false, false) => -(1.0 + 5.0 * random(i as u32)).powi(2),
                })
                .collect();
            // as the depth has them: seeds at 0 everywhere but in the tissue
            let depth: Vec<f32> = (0..n).map(|i| if blob(i, round) { f32::INFINITY } else { 0.0 }).collect();
            // as the grown balls have them: balls in the tissue only
            let grown: Vec<f32> = (0..n).map(|i| if blob(i, round) { -(3.0 * random(i as u32)).powi(2) } else { f32::INFINITY }).collect();
            for d in [balls, depth, grown] {
                let feature: Vec<u32> = (0..n as u32).map(|i| if d[i as usize].is_finite() { i } else { EMPTY }).collect();
                let (mut expected_d, mut expected_f) = (d.clone(), feature.clone());
                distance_transform_every_line(&mut expected_d, &mut expected_f, samples);
                let (mut got_d, mut got_f) = (d.clone(), feature.clone());
                distance_transform(&mut got_d, Some(&mut got_f), &LineSpans::whole(samples), false);
                let bits = |v: &[f32]| v.iter().map(|v| v.to_bits()).collect::<Vec<_>>();
                assert_eq!(bits(&got_d), bits(&expected_d));
                assert_eq!(got_f, expected_f);
                let mut without = d.clone();
                distance_transform(&mut without, None, &LineSpans::whole(samples), false);
                assert_eq!(bits(&without), bits(&expected_d));
            }
        }
    }

    #[test]
    fn a_transform_over_a_box_of_blocks_is_that_of_the_box_alone_or_of_the_whole_grid_in_it() {
        let samples: [usize; 3] = [41, 23, 29];
        let blocks = samples.map(|n| n.div_ceil(BLOCK));
        let (lo, hi) = ([1, 1, 0], [4, 3, 2]);
        let in_box = |at: [usize; 3], lo: [usize; 3], hi: [usize; 3]| (0..3).all(|a| lo[a] <= at[a] && at[a] < hi[a]);
        let spans = LineSpans::of_blocks(samples, |i| {
            in_box([i % blocks[0], i / blocks[0] % blocks[1], i / (blocks[0] * blocks[1])], lo, hi)
        });
        let (lo, hi) = ([0, 1, 2].map(|a| lo[a] * BLOCK), [0, 1, 2].map(|a| (hi[a] * BLOCK).min(samples[a])));
        let size = [0, 1, 2].map(|a| hi[a] - lo[a]);
        let n = samples[0] * samples[1] * samples[2];
        let at = |i: usize| [i % samples[0], i / samples[0] % samples[1], i / (samples[0] * samples[1])];
        let d: Vec<f32> =
            (0..n).map(|i| if random(i as u32) < 0.05 { -(3.0 * random(i as u32 + 7)).powi(2) } else { f32::INFINITY }).collect();
        let feature: Vec<u32> = (0..n as u32).map(|i| if d[i as usize].is_finite() { i } else { EMPTY }).collect();
        let (mut got_d, mut got_f) = (d.clone(), feature.clone());
        distance_transform(&mut got_d, Some(&mut got_f), &spans, false);

        let inner: Vec<usize> = (0..n).filter(|&i| in_box(at(i), lo, hi)).collect();
        let (mut box_d, mut box_f): (Vec<f32>, Vec<u32>) = inner.iter().map(|&i| (d[i], feature[i])).unzip();
        distance_transform(&mut box_d, Some(&mut box_f), &LineSpans::whole(size), false);
        for (k, &i) in inner.iter().enumerate() {
            assert_eq!(got_d[i].to_bits(), box_d[k].to_bits());
            assert_eq!(got_f[i], box_f[k]);
        }
        for i in (0..n).filter(|&i| !in_box(at(i), lo, hi)) {
            assert_eq!(got_d[i].to_bits(), d[i].to_bits());
            assert_eq!(got_f[i], feature[i]);
        }

        let (mut exact_d, mut exact_f) = (d.clone(), feature.clone());
        distance_transform(&mut exact_d, Some(&mut exact_f), &spans, true);
        let (mut whole_d, mut whole_f) = (d.clone(), feature.clone());
        distance_transform(&mut whole_d, Some(&mut whole_f), &LineSpans::whole(samples), false);
        for &i in &inner {
            assert_eq!(exact_d[i].to_bits(), whole_d[i].to_bits());
            assert_eq!(exact_f[i], whole_f[i]);
        }
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
            let (power, centre) = grown_balls(&radius, &LineSpans::whole(samples), scale);
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
        // at infinity everything is covered that is thinner than the slab
        let f = field(f32::INFINITY);
        assert!(f.covers(at(5.0), 0.1) && !f.covers(at(5.0), 0.5));
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
    fn a_field_writes_its_strands_only_as_far_as_they_show() {
        // strands thinner and thicker than a cell, crossing the tissue and each other, at several cell sizes
        let mut segments = body();
        for k in 0..8u32 {
            let from = Vec3::new(random(k) - 0.5, random(k + 10), random(k + 20) - 0.5) * 0.5;
            let to = from + Vec3::new(random(k + 30) - 0.5, random(k + 40), random(k + 50) - 0.5) * 0.8;
            segments.push(segment(from, to, 0.002 + 0.04 * random(k + 60)));
        }
        let strand = |i: usize| i >= 16;
        let meshing = vec![Meshing::closed(3.0, 0.18); segments.len()];
        for resolution in [40, 64, 97] {
            let frame = Frame::new(&segments, &meshing, resolution).unwrap();
            let field = TissueField::new(&frame, &segments, &meshing, 5.0).unwrap();
            let reused = field.surface(&segments, &meshing, strand);
            let fresh = surface_in(&frame, &segments, &meshing, strand);
            assert!(reused.mesh.face_count() > 0);
            assert_eq!(reused, fresh, "resolution {}", resolution);
        }
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
