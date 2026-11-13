//! Tubes under a cap that open in pores, the sponge of a Boletus, made from a pattern.
//!
//! The tubes are one layer under the cap, from inside the flesh down to the pore surface. It
//! lies on the cap like the gills of [`crate::gills`]: the pattern is a disc around the cap
//! axis, its radius the distance along the underside from the axis, so a pore stays on the same
//! spot of the tissue while the cap opens. Where the pattern is 1 are the walls between the
//! tubes, where it is 0 a tube opens and the pore surface goes up into it by the pore depth.
//! The depth of the layer is read along the cap, alpha 0 at the stipe to 1 at the margin, and
//! for every stipe, like the depth of the gills.
//!
//! The pore surface is a grid of rings around the axis, every ring with as many vertices as it
//! needs to show the pattern. The top inside the flesh is a coarser grid, and walls at the
//! stipe and at the margin close the layer.

use crate::gills::{DepthAlong, Spot, Underside, EMBED};
use crate::{smooth_normals, write_mesh, Cap, Mesh, MeshBuild};
use glam::Vec3;
use nodarium_utils::{abi::Output, split_args, Ctx, Pattern};
use std::f32::consts::TAU;

/// Vertices of the pore surface along one spacing of the pattern
const POINTS_PER_SPACING: f32 = 5.0;
/// A cap gets at most about this many vertices on its pore surface, a finer pattern on a large
/// cap is shown coarser
const MAX_VERTICES: f32 = 400_000.0;
/// The top inside the flesh is this many times coarser than the pore surface
const TOP_COARSER: f32 = 6.0;
/// Where no ray reaches a spot of the grid, the underside is looked for this many steps further in
const LOOK_IN: usize = 8;

/// The inputs of the pores node as the host sends them, read for every stipe, the depth also
/// along the cap
pub struct PoreInputs<'a> {
    /// The rays of the caps and their stipes
    pub cap: &'a [i32],
    pub pattern: &'a [i32],
    pub depth: &'a [i32],
    pub free_distance: &'a [i32],
    pub pore_depth: &'a [i32],
}

/// A ring of vertices around the axis: their indices and angles, sorted by angle
type Ring = Vec<(u32, f32)>;

/// Closes the band between rings `a` and `b` with triangles. Seen with the angle growing to
/// the right and `b` above `a`, the faces look at the viewer.
fn zip(mesh: &mut impl MeshBuild, a: &Ring, b: &Ring) {
    let (n, m) = (a.len(), b.len());
    let angle = |ring: &Ring, k: usize| ring[k % ring.len()].1 + TAU * (k / ring.len()) as f32;
    let (mut j, mut k) = (0, 0);
    while j < n || k < m {
        if k == m || (j < n && angle(a, j + 1) <= angle(b, k + 1)) {
            mesh.push_face([a[j % n].0, a[(j + 1) % n].0, b[k % m].0]);
            j += 1;
        } else {
            mesh.push_face([a[j % n].0, b[(k + 1) % m].0, b[k % m].0]);
            k += 1;
        }
    }
}

/// The sizes of the pores of one cap
struct Params {
    depth: DepthAlong,
    free_distance: f32,
    pore_depth: f32,
}

/// Where along the cap every ring of a surface from `lo` to `hi` about `step` apart is, and how
/// many vertices it has
fn ring_sizes(lo: f32, hi: f32, step: f32) -> impl ExactSizeIterator<Item = (f32, usize)> {
    let count = (((hi - lo) / step).ceil() as usize).max(1) + 1;
    (0..count).map(move |i| {
        let s = lo + (hi - lo) * i as f32 / (count - 1) as f32;
        (s, ((TAU * s / step).ceil() as usize).max(8))
    })
}

/// The faces [`zip`] closes between neighbouring rings of these sizes, one for every vertex of
/// both rings of a band
fn band_faces(sizes: &[usize]) -> usize {
    sizes.windows(2).map(|w| w[0] + w[1]).sum()
}

/// The rings of a surface from `lo` to `hi` along the cap about `step` apart, every vertex
/// placed by `place(angle, s)`. `None` when a spot has no underside.
fn rings(mesh: &mut impl MeshBuild, lo: f32, hi: f32, step: f32, place: impl Fn(f32, f32) -> Option<Vec3>) -> Option<Vec<Ring>> {
    let sizes = ring_sizes(lo, hi, step);
    let mut rings = Vec::with_capacity(sizes.len());
    for (s, n) in sizes {
        let mut ring = Vec::with_capacity(n);
        for j in 0..n {
            let angle = TAU * (j as f32 + 0.5) / n as f32;
            let p = place(angle, s)?;
            ring.push((mesh.vertex_count() as u32, angle));
            mesh.push_vertex(p, 0);
        }
        rings.push(ring);
    }
    Some(rings)
}

/// The pore layer of one cap, worked out before any cap is meshed, so the mesh of all of them
/// is reserved at its size
struct Layer<'a> {
    underside: Underside,
    pattern: &'a Pattern,
    depth: DepthAlong,
    pore_depth: f32,
    free: f32,
    lo: f32,
    hi: f32,
    step: f32,
}

impl<'a> Layer<'a> {
    /// `None` when the cap has no pore layer
    fn new(mut underside: Underside, pattern: &'a Pattern, p: Params) -> Option<Layer<'a>> {
        let deepest = p.depth.0.iter().fold(0.0f32, |a, &b| a.max(b));
        if deepest <= 0.0 {
            return None;
        }
        let free = underside.frame.stipe_radius + p.free_distance.max(0.0);
        let hi = underside.length - underside.margin_radius;
        if hi <= free {
            return None;
        }
        underside.anchor = free;
        let area = std::f32::consts::PI * (hi * hi - free * free);
        let step = (pattern.spacing.max(1e-5) / POINTS_PER_SPACING).max((area / MAX_VERTICES).sqrt());
        let mut layer =
            Layer { underside, pattern, depth: p.depth, pore_depth: p.pore_depth.max(0.0), free, lo: free, hi, step };
        // where the underside slopes down to the stipe, the layer starts where it is the free
        // distance from the axis
        let (mut lo, mut up) = (free, hi);
        if !layer.clear(lo) {
            for _ in 0..24 {
                let mid = 0.5 * (lo + up);
                if layer.clear(mid) {
                    up = mid;
                } else {
                    lo = mid;
                }
            }
            lo = up;
        }
        if hi - lo < step {
            return None;
        }
        layer.lo = lo;
        Some(layer)
    }

    fn spot(&self, angle: f32, s: f32) -> Option<Spot> {
        (0..=LOOK_IN).find_map(|k| self.underside.at(angle, (s - k as f32 * self.step).max(self.free)))
    }

    fn from_axis(&self, q: Vec3) -> f32 {
        let frame = &self.underside.frame;
        let rel = q - frame.hinge;
        (rel - frame.axis * rel.dot(frame.axis)).length()
    }

    fn clear(&self, s: f32) -> bool {
        (0..8).all(|k| {
            let angle = TAU * k as f32 / 8.0;
            self.spot(angle, s).is_none_or(|spot| self.from_axis(spot.position - spot.normal * spot.radius) >= self.free)
        })
    }

    /// The tubes hang from the underside, between straight down from it and straight down the
    /// axis. Where they would reach in closer to the stipe than the free distance, like on a
    /// button whose cap closes around the stipe, they are shorter.
    fn bottom_at(&self, angle: f32, s: f32, wall: f32) -> Option<Vec3> {
        let (start, axis) = (self.lo, self.underside.frame.axis);
        let spot = self.spot(angle, s)?;
        let depth = self.depth.at((s - start) / (self.hi - start));
        let opening = self.pore_depth.min(depth) * (1.0 - wall);
        let down = (spot.normal + axis).try_normalize().unwrap_or(spot.normal);
        let base = spot.position - spot.normal * spot.radius;
        let mut length = depth - opening;
        if self.from_axis(base - down * length) < self.free {
            let (mut short, mut long) = (0.0, length);
            for _ in 0..16 {
                let mid = 0.5 * (short + long);
                if self.from_axis(base - down * mid) < self.free {
                    long = mid;
                } else {
                    short = mid;
                }
            }
            length = short;
        }
        Some(base - down * length)
    }

    fn keep_free(&self, q: Vec3, angle: f32) -> Vec3 {
        let frame = &self.underside.frame;
        let rel = q - frame.hinge;
        let flat = rel - frame.axis * rel.dot(frame.axis);
        let d = flat.length();
        if d < self.free {
            q + flat.try_normalize().unwrap_or_else(|| frame.out(angle)) * (self.free - d)
        } else {
            q
        }
    }

    /// The vertex counts of the rings of the pore surface and of the top inside the flesh
    fn ring_counts(&self) -> (Vec<usize>, Vec<usize>) {
        let counts = |step: f32| ring_sizes(self.lo, self.hi, step).map(|(_, n)| n).collect();
        (counts(self.step), counts(TOP_COARSER * self.step))
    }

    /// The vertices and faces [`Layer::add`] adds
    fn size(&self) -> (usize, usize) {
        let (bottom, top) = self.ring_counts();
        let vertices = bottom.iter().sum::<usize>() + top.iter().sum::<usize>();
        let walls = top[0] + bottom[0] + bottom[bottom.len() - 1] + top[top.len() - 1];
        (vertices, band_faces(&bottom) + band_faces(&top) + walls)
    }

    /// Adds the layer to `mesh`, nothing when a spot of it has no underside
    fn add(&self, mesh: &mut impl MeshBuild) {
        let start = mesh.vertex_count();
        let surfaces = rings(mesh, self.lo, self.hi, self.step, |angle, s| {
            let wall = self.pattern.sample(angle, s).unwrap_or(1.0).clamp(0.0, 1.0);
            Some(self.keep_free(self.bottom_at(angle, s, wall)?, angle))
        })
        .and_then(|bottom| {
            let top = rings(mesh, self.lo, self.hi, TOP_COARSER * self.step, |angle, s| {
                let spot = self.spot(angle, s)?;
                Some(self.keep_free(spot.position - spot.normal * ((1.0 - EMBED) * spot.radius), angle))
            })?;
            Some((bottom, top))
        });
        let Some((bottom, top)) = surfaces else {
            mesh.truncate_vertices(start);
            return;
        };
        for pair in bottom.windows(2) {
            zip(mesh, &pair[0], &pair[1]);
        }
        for pair in top.windows(2) {
            zip(mesh, &pair[1], &pair[0]);
        }
        zip(mesh, &top[0], &bottom[0]);
        zip(mesh, bottom.last().unwrap(), top.last().unwrap());
    }
}

/// The pore layers of every cap of `inputs.cap` that has one
fn layers<'a>(inputs: &PoreInputs, pattern: &'a Pattern) -> Vec<Layer<'a>> {
    let mut ctx = Ctx::default();
    let items = split_args(inputs.cap);
    let caps = Cap::all(&items);
    let mut layers = Vec::new();
    for (rank, cap) in caps.iter().enumerate() {
        ctx.set_path(items[cap.stipe], rank, caps.len());
        ctx.alpha = cap.frame.alpha;
        let params = Params {
            depth: DepthAlong::read(&mut ctx, inputs.depth),
            free_distance: ctx.float(inputs.free_distance),
            pore_depth: ctx.float(inputs.pore_depth),
        };
        let mut rays: Vec<usize> = cap.rays.iter().map(|r| r.path).collect();
        rays.sort_unstable();
        let rays: Vec<&[i32]> = rays.into_iter().map(|i| items[i]).collect();
        if let Some(underside) = Underside::new(cap.frame, &rays) {
            layers.extend(Layer::new(underside, pattern, params));
        }
    }
    layers
}

/// The vertices and faces of the mesh of `layers`, a layer that has a spot without underside has
/// fewer
fn layers_size(layers: &[Layer]) -> (usize, usize) {
    layers.iter().map(Layer::size).fold((0, 0), |(v, f), (lv, lf)| (v + lv, f + lf))
}

/// The pores under every cap of `inputs.cap`, see the module docs
pub fn pores(inputs: &PoreInputs) -> Mesh {
    let mut mesh = Mesh::default();
    let Some(pattern) = Pattern::decode(inputs.pattern) else {
        return mesh;
    };
    let layers = layers(inputs, &pattern);
    // a mesh that grows by doubling holds up to twice its size, see `Mesh::reserve_exact`
    let (vertices, faces) = layers_size(&layers);
    mesh.reserve_exact(vertices, faces);
    for layer in &layers {
        layer.add(&mut mesh);
    }
    smooth_normals(&mut mesh);
    mesh
}

/// [`pores`] written straight into the result
pub fn write_pores(inputs: &PoreInputs) -> Output {
    let pattern = Pattern::decode(inputs.pattern);
    let layers = pattern.as_ref().map_or_else(Vec::new, |pattern| layers(inputs, pattern));
    let (vertices, faces) = layers_size(&layers);
    write_mesh(vertices, faces, |mesh| {
        for layer in &layers {
            layer.add(mesh);
        }
    })
}

#[cfg(test)]
mod tests;
