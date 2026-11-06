use glam::Vec3;

use crate::mesh::Mesh;

const BLOCK: usize = 8;
const BLOCK_SAMPLES: usize = BLOCK * BLOCK * BLOCK;
const EMPTY: u32 = u32::MAX;

/// Thinner segments would fall between the samples and vanish
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
/// inside, below a half so the blur doesn't shrink the tissue below its outer hyphae
const SURFACE_SHARE: f32 = 0.3;

/// Hyphae of a tissue that reach this many closing radii out of it stay as strands
const STRAND_REACH: f32 = 3.0;

/// How much a closed segment counts for the owner of a sample past its kernel
const FILL_OWNER_WEIGHT: f32 = 1e-3;

#[derive(Clone, Copy)]
struct Closed {
    radius: f32,
    reach: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Segment {
    pub start: Vec3,
    pub end: Vec3,
    pub radius: f32,
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
    /// Distance to the nearest axis of the other segments, the closing keeps away from them
    guard: Vec<f32>,
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
                        self.guard.resize(self.density.len(), f32::INFINITY);
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

    /// Adds the kernel of one segment to every sample it reaches.
    ///
    /// A closed segment instead adds its distance to `tube` out to `closed.reach`
    /// and only uses the kernel to claim samples. Past the kernel it claims them
    /// with a tiny weight, so that the closing has an owner where it fills a gap,
    /// but no blended segment loses a sample to it.
    fn splat(&mut self, index: u32, priority: f32, segment: &Segment, reach: f32, closed: Option<Closed>) {
        let kernel_reach = reach;
        let reach = closed.map_or(reach, |c| c.reach);
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
                    let kernel = |reach: f32| {
                        let k = (1.0 - d_sq / (reach * reach)).max(0.0);
                        k * k * k
                    };
                    let k = match closed {
                        Some(c) => {
                            self.tube[i] = self.tube[i].min(d_sq.sqrt() - c.radius);
                            if d_sq < kernel_reach * kernel_reach {
                                kernel(kernel_reach)
                            } else {
                                FILL_OWNER_WEIGHT * kernel(reach)
                            }
                        }
                        None => {
                            let k = kernel(reach);
                            self.density[i] += k;
                            k
                        }
                    };
                    if k * priority > self.owner_k[i] {
                        self.owner_k[i] = k * priority;
                        self.owner[i] = index;
                    }
                }
            }
        }
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
        for start in (0..n).filter(|&start| start / stride % len == 0) {
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

/// Morphological closing of the tubes of the closed segments with a ball of
/// radius `rho`, blurred by `sigma` and added to the density so that its
/// surface lies at `threshold`.
///
/// Outside the closing are the points within `rho` of a point whose distance
/// to the tubes is more than `rho`, so an empty ball of radius `rho` fits
/// around them. Gaps the ball can't pass through fill, while convex parts keep
/// their size. The closing stays `guard` away from the axes of the other segments.
fn close_tubes(grid: &mut SparseGrid, rho: f32, guard: f32, sigma: f32, threshold: f32) {
    let [nx, ny, nz] = grid.samples;
    let n = nx * ny * nz;
    let tube_at = |grid: &SparseGrid, i: usize| grid.index([i % nx, i / nx % ny, i / (nx * ny)]).map(|s| grid.tube[s]);

    // squared distance in cells to the nearest sample an empty ball fits around, and that sample
    let mut d = vec![f32::INFINITY; n];
    let mut feature = vec![EMPTY; n];
    for i in 0..n {
        if tube_at(grid, i).is_none_or(|t| t > rho) {
            d[i] = 0.0;
            feature[i] = i as u32;
        }
    }
    distance_transform(&mut d, &mut feature, grid.samples);

    // share of every sample inside the closing, ramping from 0 to 1 over two cells across its surface
    let (rho_cells, guard_cells) = (rho / grid.cell, guard / grid.cell);
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
        // signed distance to the closed surface in cells, positive outside
        let outside = if tube > rho_cells || feature[i] == EMPTY {
            tube
        } else {
            let f = feature[i] as usize;
            // the seed lies this far inside the space the empty balls fit in, which places the surface between samples
            let excess = tube_at(grid, f).filter(|t| t.is_finite()).map_or(0.0, |t| t / grid.cell - rho_cells);
            rho_cells - (position(i).distance(position(f)) - excess).max(0.0)
        };
        let allowed = grid.guard[slot] / grid.cell - guard_cells;
        let closing = (0.5 - 0.5 * outside).min(0.5 + 0.5 * allowed);
        solid[i] = closing.max(0.5 - 0.5 * tube).clamp(0.0, 1.0);
    }
    blur(&mut solid, grid.samples, sigma / grid.cell);

    let strands = strands(grid, &solid, STRAND_REACH * rho);
    for i in 0..n {
        if let Some(slot) = grid.index([i % nx, i / nx % ny, i / (nx * ny)]) {
            let mut share = solid[i] / SURFACE_SHARE;
            if strands[i] {
                share = share.max(2.0 * (0.5 - 0.5 * grid.tube[slot] / grid.cell).clamp(0.0, 1.0));
            }
            grid.density[slot] += threshold * share;
        }
    }
}

/// The blur wears away thin parts, so the hyphae there stay as they are. Of the
/// hyphae outside the blurred tissue only those reaching more than `reach` away
/// from it are kept, as whole strands: a stipe too thin for the blur or a strand
/// leaving the tissue. Shorter stubs, such as a tip that ends just outside the
/// tissue or a hypha lying on it, would show as dots and dashes on a smooth surface.
fn strands(grid: &SparseGrid, solid: &[f32], reach: f32) -> Vec<bool> {
    let [nx, ny, nz] = grid.samples;
    let n = solid.len();
    let near_tube = |i: usize| {
        grid.index([i % nx, i / nx % ny, i / (nx * ny)]).is_some_and(|s| grid.tube[s] < grid.cell)
    };
    let candidate: Vec<bool> = (0..n).map(|i| solid[i] < SURFACE_SHARE && near_tube(i)).collect();
    let mut keep = vec![false; n];
    if !candidate.contains(&true) {
        return keep;
    }

    let mut d = vec![f32::INFINITY; n];
    let mut feature = vec![EMPTY; n];
    for i in 0..n {
        if solid[i] >= SURFACE_SHARE {
            d[i] = 0.0;
        }
    }
    distance_transform(&mut d, &mut feature, grid.samples);
    let reach_sq = (reach / grid.cell).powi(2);

    // breadth first from the far samples back to the tissue, a strand leaving it is about `reach` long until it is far
    let steps = (reach / grid.cell).ceil() as usize + 2;
    let mut front: Vec<usize> = (0..n).filter(|&i| candidate[i] && d[i] > reach_sq).collect();
    for &i in &front {
        keep[i] = true;
    }
    let mut next = Vec::new();
    for _ in 0..steps {
        for &i in &front {
            let (x, y, z) = (i % nx, i / nx % ny, i / (nx * ny));
            for dz in z.saturating_sub(1)..=(z + 1).min(nz - 1) {
                for dy in y.saturating_sub(1)..=(y + 1).min(ny - 1) {
                    for dx in x.saturating_sub(1)..=(x + 1).min(nx - 1) {
                        let j = dx + nx * (dy + ny * dz);
                        if candidate[j] && !keep[j] {
                            keep[j] = true;
                            next.push(j);
                        }
                    }
                }
            }
        }
        std::mem::swap(&mut front, &mut next);
        next.clear();
    }
    keep
}

/// Separable gaussian blur of a dense grid, `sigma` in samples
fn blur(values: &mut [f32], samples: [usize; 3], sigma: f32) {
    if sigma < 0.25 {
        return;
    }
    let radius = (3.0 * sigma).ceil() as isize;
    let weights: Vec<f32> = (-radius..=radius).map(|i| (-(i * i) as f32 / (2.0 * sigma * sigma)).exp()).collect();
    let total: f32 = weights.iter().sum();
    let weights: Vec<f32> = weights.iter().map(|w| w / total).collect();
    let [nx, ny, nz] = samples;
    let mut line = Vec::new();
    for (len, stride) in [(nx, 1), (ny, nx), (nz, nx * ny)] {
        for start in 0..values.len() {
            if start / stride % len != 0 {
                continue;
            }
            line.clear();
            line.extend((0..len).map(|k| values[start + k * stride]));
            for k in 0..len {
                let mut sum = 0.0;
                for (w, offset) in weights.iter().zip(-radius..=radius) {
                    let j = k as isize + offset;
                    if j >= 0 && (j as usize) < len {
                        sum += w * line[j as usize];
                    }
                }
                values[start + k * stride] = sum;
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
    surface(segments, resolution, blend, 0.0, |_| false, |_| 1.0).mesh
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

/// [`mesh_segments`] that also tells which segment every vertex belongs to.
/// A segment owns a vertex where its kernel times `priority(index)` is the largest.
///
/// The segments for which `closed(index)` holds are not blended but form a
/// tissue: the morphological closing of their tubes with a ball of radius
/// `close`, blurred a little. Gaps between them that the ball can't pass
/// through fill, while convex parts and wider gaps stay about as they are, so
/// a loose bundle of segments gets one smooth surface without swelling where
/// it is dense. The closing keeps out of the gaps between the other segments.
pub fn surface(
    segments: &[Segment],
    resolution: usize,
    blend: f32,
    close: f32,
    closed: impl Fn(usize) -> bool,
    priority: impl Fn(usize) -> f32,
) -> SegmentSurface {
    if segments.is_empty() {
        return SegmentSurface::default();
    }
    let reach_factor = blend.max(1.1);
    let threshold = (1.0 - 1.0 / (reach_factor * reach_factor)).powi(3);
    let close = close.max(0.0);
    let closing = close > 0.0 && (0..segments.len()).any(&closed);
    let is_closed = |i: usize| closing && closed(i);
    let (mut min, mut max) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
    let mut widest = 0.0f32;
    for (i, s) in segments.iter().enumerate() {
        min = min.min(s.start.min(s.end));
        max = max.max(s.start.max(s.end));
        widest = widest.max(if is_closed(i) { s.radius + close } else { reach_factor * s.radius });
    }
    let longest = ((max - min) + Vec3::splat(2.0 * widest)).max_element();
    if !longest.is_finite() || longest <= 0.0 {
        return SegmentSurface::default();
    }
    let cell = longest / resolution.max(2) as f32;
    let min_radius = MIN_RADIUS_IN_CELLS * cell;

    // two cells of empty padding keep the surface closed
    // a closed segment reaches a little past `close`, so every sample near the closed surface knows its distance
    let closed_reach = |radius: f32| radius + close + 2.0 * cell;
    let pad = widest.max(reach_factor * min_radius) + 4.0 * cell;
    let origin = min - Vec3::splat(pad);
    let size = max - min + Vec3::splat(2.0 * pad);
    let samples = [0, 1, 2].map(|a| (size[a] / cell).ceil() as usize + 1);
    let blocks = samples.map(|s| s.div_ceil(BLOCK));
    let mut grid = SparseGrid {
        origin,
        cell,
        samples,
        blocks,
        block_slots: vec![EMPTY; blocks[0] * blocks[1] * blocks[2]],
        density: Vec::new(),
        owner: Vec::new(),
        owner_k: Vec::new(),
        tube: Vec::new(),
        guard: Vec::new(),
    };
    for (i, s) in segments.iter().enumerate() {
        let radius = s.radius.max(min_radius);
        if is_closed(i) {
            let closed = Closed { radius, reach: closed_reach(radius) };
            grid.splat(i as u32, priority(i), s, reach_factor * radius, Some(closed));
        } else {
            grid.splat(i as u32, priority(i), s, reach_factor * radius, None);
        }
    }
    if closing {
        let guard = GUARD * close;
        for (i, s) in segments.iter().enumerate() {
            if !is_closed(i) {
                grid.guard(s, guard + 2.0 * cell);
            }
        }
        close_tubes(&mut grid, close, guard, SMOOTHING * close, threshold);
    }

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
                    cell_of(&grid, c).map(|i| vertex_of_cell[i]).filter(|&i| i != EMPTY)
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

    fn components(mesh: &Mesh) -> usize {
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
        let s = surface(&pair, 48, 2.0, 0.0, |_| false, |_| 1.0);
        assert_eq!(s.owners.len(), s.mesh.vertex_count());
        for (i, &owner) in s.owners.iter().enumerate() {
            assert_eq!(owner, (vertex(&s.mesh, i as u32).x > 0.25) as u32);
        }
    }

    #[test]
    fn priority_lets_a_segment_win_where_both_reach() {
        let pair = [segment(Vec3::ZERO, Vec3::Y, 0.05), segment(Vec3::X * 0.12, Vec3::new(0.12, 1.0, 0.0), 0.05)];
        let even = surface(&pair, 48, 3.0, 0.0, |_| false, |_| 1.0);
        let first = surface(&pair, 48, 3.0, 0.0, |_| false, |i| if i == 0 { 4.0 } else { 1.0 });
        let count = |s: &SegmentSurface| s.owners.iter().filter(|&&o| o == 0).count();
        assert_eq!(even.mesh, first.mesh);
        assert!(count(&first) > count(&even));
    }

    fn closed_surface(segments: &[Segment], close: f32, closed: impl Fn(usize) -> bool) -> SegmentSurface {
        surface(segments, 64, 3.0, close, closed, |_| 1.0)
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
        let pair = [segment(Vec3::ZERO, Vec3::Y, 0.03), segment(Vec3::X * 0.5, Vec3::new(0.5, 1.0, 0.0), 0.03)];
        let closed = closed_surface(&pair, 0.2, |_| true);
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
                .filter(|p| (0.7..0.85).contains(&p.y) && p.z.abs() < 0.08 && p.x.abs() < 0.08 - r + s.cell)
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
    fn a_strand_leaving_the_tissue_stays() {
        let (r, close) = (0.03, 0.18);
        let mut segments = body();
        let rim = segments[0].end;
        segments.push(segment(rim, rim + Vec3::Y * 4.0 * close, r));
        let s = closed_surface(&segments, close, |_| true);
        assert_closed(&s.mesh);
        assert_eq!(components(&s.mesh), 1);
        assert!(highest(&s) > 1.0 + 4.0 * close + 0.5 * r, "strand ends at {}", highest(&s));
    }

    #[test]
    fn a_thin_closed_bundle_does_not_vanish() {
        // narrower than the blur, it stays as its hyphae
        let r = 0.02;
        let bundle: Vec<Segment> = (0..4)
            .map(|i| {
                let a = i as f32 / 4.0 * 2.0 * PI;
                let at = Vec3::new(a.cos(), 0.0, a.sin()) * 0.03;
                segment(at, at + Vec3::Y, r)
            })
            .collect();
        let s = closed_surface(&bundle, 0.3, |_| true);
        assert_closed(&s.mesh);
        assert!(highest(&s) > 1.0, "bundle ends at {}", highest(&s));
        let lowest = (0..s.mesh.vertex_count()).map(|v| vertex(&s.mesh, v as u32).y).fold(f32::INFINITY, f32::min);
        assert!(lowest < 0.0, "bundle starts at {}", lowest);
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
            let s = surface(&segments, 96, 3.0, close, |_| true, |_| 1.0);
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
