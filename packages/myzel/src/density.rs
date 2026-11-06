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

    /// Adds the kernel of one segment to every sample it reaches
    fn splat(&mut self, index: u32, priority: f32, segment: &Segment, reach: f32) {
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
                    let k = 1.0 - d_sq / reach_sq;
                    let k = k * k * k;
                    let i = self.slot([x, y, z]) as usize * BLOCK_SAMPLES + Self::local([x, y, z]);
                    self.density[i] += k;
                    if k * priority > self.owner_k[i] {
                        self.owner_k[i] = k * priority;
                        self.owner[i] = index;
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
    surface(segments, resolution, blend, |_| 1.0).mesh
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
pub fn surface(
    segments: &[Segment],
    resolution: usize,
    blend: f32,
    priority: impl Fn(usize) -> f32,
) -> SegmentSurface {
    if segments.is_empty() {
        return SegmentSurface::default();
    }
    let reach_factor = blend.max(1.1);
    let threshold = (1.0 - 1.0 / (reach_factor * reach_factor)).powi(3);
    let (mut min, mut max) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
    let mut widest = 0.0f32;
    for s in segments {
        min = min.min(s.start.min(s.end));
        max = max.max(s.start.max(s.end));
        widest = widest.max(s.radius);
    }
    let longest = ((max - min) + Vec3::splat(2.0 * reach_factor * widest)).max_element();
    if !longest.is_finite() || longest <= 0.0 {
        return SegmentSurface::default();
    }
    let cell = longest / resolution.max(2) as f32;
    let min_radius = MIN_RADIUS_IN_CELLS * cell;

    // two cells of empty padding keep the surface closed
    let pad = reach_factor * widest.max(min_radius) + 2.0 * cell;
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
    };
    for (i, s) in segments.iter().enumerate() {
        grid.splat(i as u32, priority(i), s, reach_factor * s.radius.max(min_radius));
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
        let s = surface(&pair, 48, 2.0, |_| 1.0);
        assert_eq!(s.owners.len(), s.mesh.vertex_count());
        for (i, &owner) in s.owners.iter().enumerate() {
            assert_eq!(owner, (vertex(&s.mesh, i as u32).x > 0.25) as u32);
        }
    }

    #[test]
    fn priority_lets_a_segment_win_where_both_reach() {
        let pair = [segment(Vec3::ZERO, Vec3::Y, 0.05), segment(Vec3::X * 0.12, Vec3::new(0.12, 1.0, 0.0), 0.05)];
        let even = surface(&pair, 48, 3.0, |_| 1.0);
        let first = surface(&pair, 48, 3.0, |i| if i == 0 { 4.0 } else { 1.0 });
        let count = |s: &SegmentSurface| s.owners.iter().filter(|&&o| o == 0).count();
        assert_eq!(even.mesh, first.mesh);
        assert!(count(&first) > count(&even));
    }

    #[test]
    fn no_segments_give_an_empty_mesh() {
        assert_eq!(mesh_segments(&[], 64, 2.0), Mesh::default());
    }
}
