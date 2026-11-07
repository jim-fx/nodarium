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

/// How much a closed segment counts for the owner of a sample past its kernel
const FILL_OWNER_WEIGHT: f32 = 1e-3;


#[derive(Clone, Copy)]
struct Closed {
    radius: f32,
    reach: f32,
    /// The segment keeps its own tube where it leaves the tissue
    strand: bool,
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
    /// Signed distance to the nearest tube of the closed segments that are strands
    strand: Vec<f32>,
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
                        self.strand.resize(self.density.len(), f32::INFINITY);
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
                            let tube = d_sq.sqrt() - c.radius;
                            self.tube[i] = self.tube[i].min(tube);
                            if c.strand {
                                self.strand[i] = self.strand[i].min(tube);
                            }
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
/// radius `rho`, blurred by `sigma`: per sample its share of tissue, the
/// surface of the tissue is where it reaches [`SURFACE_SHARE`].
///
/// Outside the closing are the points within `rho` of a point whose distance
/// to the tubes is more than `rho`, so an empty ball of radius `rho` fits
/// around them. Gaps the ball can't pass through fill, while convex parts keep
/// their size. The closing stays `guard` away from the axes of the other segments.
fn closing(grid: &SparseGrid, rho: f32, guard: f32, sigma: f32) -> Vec<f32> {
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
    solid
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
    pub fn new(
        frame: &Frame,
        segments: &[Segment],
        closed: impl Fn(usize) -> bool,
        priority: impl Fn(usize) -> f32,
        scale: f32,
    ) -> Option<TissueField> {
        if !frame.closing {
            return None;
        }
        let mut grid = frame.grid();
        for (i, s) in segments.iter().enumerate() {
            if closed(i) {
                let radius = s.radius.max(frame.min_radius);
                let c = Closed { radius, reach: frame.closed_reach(radius), strand: false };
                grid.splat(i as u32, priority(i), s, frame.reach_factor * radius, Some(c));
            }
        }
        frame.guard(&mut grid, segments, &closed);
        let solid = closing(&grid, frame.close, GUARD * frame.close, SMOOTHING * frame.close);

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
    /// `closed` and `priority` must be the same as for the field.
    pub fn surface(
        self,
        segments: &[Segment],
        closed: impl Fn(usize) -> bool,
        strand: impl Fn(usize) -> bool,
        priority: impl Fn(usize) -> f32,
    ) -> SegmentSurface {
        let TissueField { frame, mut grid, solid, .. } = self;
        for (i, s) in segments.iter().enumerate() {
            let radius = s.radius.max(frame.min_radius);
            if !closed(i) {
                grid.splat(i as u32, priority(i), s, frame.reach_factor * radius, None);
            } else if strand(i) {
                // the tube and the owners are there already, splatting them again changes nothing
                let c = Closed { radius, reach: frame.closed_reach(radius), strand: true };
                grid.splat(i as u32, priority(i), s, frame.reach_factor * radius, Some(c));
            }
        }
        add_tissue(&mut grid, &solid, frame.threshold);
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
    reach_factor: f32,
    /// Density of the surface
    threshold: f32,
    /// Radius of the closing
    close: f32,
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
    pub fn new(
        segments: &[Segment],
        resolution: usize,
        blend: f32,
        close: f32,
        closed: &impl Fn(usize) -> bool,
    ) -> Option<Frame> {
        if segments.is_empty() {
            return None;
        }
        let reach_factor = blend.max(1.1);
        let threshold = (1.0 - 1.0 / (reach_factor * reach_factor)).powi(3);
        let close = close.max(0.0);
        let closing = close > 0.0 && (0..segments.len()).any(closed);
        let (mut min, mut max) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
        let mut widest = 0.0f32;
        for (i, s) in segments.iter().enumerate() {
            min = min.min(s.start.min(s.end));
            max = max.max(s.start.max(s.end));
            widest = widest.max(if closing && closed(i) { s.radius + close } else { reach_factor * s.radius });
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
        Some(Frame { reach_factor, threshold, close, closing, cell, min_radius, origin, samples })
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
            strand: Vec::new(),
            guard: Vec::new(),
        }
    }

    /// A closed segment reaches a little past the closing, so every sample near the closed surface knows its distance
    fn closed_reach(&self, radius: f32) -> f32 {
        radius + self.close + 2.0 * self.cell
    }

    /// Keeps the closing out of the gaps between the segments that aren't closed
    fn guard(&self, grid: &mut SparseGrid, segments: &[Segment], closed: &impl Fn(usize) -> bool) {
        for (i, s) in segments.iter().enumerate() {
            if !closed(i) {
                grid.guard(s, GUARD * self.close + 2.0 * self.cell);
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
pub fn surface(
    segments: &[Segment],
    resolution: usize,
    blend: f32,
    close: f32,
    closed: impl Fn(usize) -> bool,
    strand: impl Fn(usize) -> bool,
    priority: impl Fn(usize) -> f32,
) -> SegmentSurface {
    match Frame::new(segments, resolution, blend, close, &closed) {
        Some(frame) => surface_in(&frame, segments, closed, strand, priority),
        None => SegmentSurface::default(),
    }
}

/// [`surface`] on the grid of `frame`, which the segments must lie in. Meshes of segments
/// taken from the same set, on the frame of the whole set, are sampled at the same places.
pub fn surface_in(
    frame: &Frame,
    segments: &[Segment],
    closed: impl Fn(usize) -> bool,
    strand: impl Fn(usize) -> bool,
    priority: impl Fn(usize) -> f32,
) -> SegmentSurface {
    if segments.is_empty() {
        return SegmentSurface::default();
    }
    let mut grid = frame.grid();
    for (i, s) in segments.iter().enumerate() {
        let radius = s.radius.max(frame.min_radius);
        if frame.closing && closed(i) {
            let c = Closed { radius, reach: frame.closed_reach(radius), strand: strand(i) };
            grid.splat(i as u32, priority(i), s, frame.reach_factor * radius, Some(c));
        } else {
            grid.splat(i as u32, priority(i), s, frame.reach_factor * radius, None);
        }
    }
    if frame.closing {
        frame.guard(&mut grid, segments, &closed);
        let solid = closing(&grid, frame.close, GUARD * frame.close, SMOOTHING * frame.close);
        add_tissue(&mut grid, &solid, frame.threshold);
    }
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
        let frame = Frame::new(&slab, 96, 3.0, close, &|_| true).unwrap();
        let field = |scale: f32| TissueField::new(&frame, &slab, |_| true, |_| 1.0, scale).unwrap();
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
        let frame = Frame::new(&segments, 64, 3.0, 0.18, &closed).unwrap();
        let field = TissueField::new(&frame, &segments, closed, priority, 5.0).unwrap();
        let reused = field.surface(&segments, closed, strand, priority);
        let fresh = surface_in(&frame, &segments, closed, strand, priority);
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
