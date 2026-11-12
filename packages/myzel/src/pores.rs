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

use crate::gills::{DepthAlong, Underside, EMBED};
use crate::{smooth_normals, Cap, Mesh};
use glam::Vec3;
use nodarium_utils::{split_args, Ctx, Pattern};
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
fn zip(faces: &mut Vec<u32>, a: &Ring, b: &Ring) {
    let (n, m) = (a.len(), b.len());
    let angle = |ring: &Ring, k: usize| ring[k % ring.len()].1 + TAU * (k / ring.len()) as f32;
    let (mut j, mut k) = (0, 0);
    while j < n || k < m {
        if k == m || (j < n && angle(a, j + 1) <= angle(b, k + 1)) {
            faces.extend([a[j % n].0, a[(j + 1) % n].0, b[k % m].0]);
            j += 1;
        } else {
            faces.extend([a[j % n].0, b[(k + 1) % m].0, b[k % m].0]);
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

/// The rings of a surface from `lo` to `hi` along the cap about `step` apart, every vertex
/// placed by `place(angle, s)`. `None` when a spot has no underside.
fn rings(mesh: &mut Mesh, lo: f32, hi: f32, step: f32, place: impl Fn(f32, f32) -> Option<Vec3>) -> Option<Vec<Ring>> {
    let count = (((hi - lo) / step).ceil() as usize).max(1) + 1;
    let mut rings = Vec::with_capacity(count);
    for i in 0..count {
        let s = lo + (hi - lo) * i as f32 / (count - 1) as f32;
        let n = ((TAU * s / step).ceil() as usize).max(8);
        let mut ring = Vec::with_capacity(n);
        for j in 0..n {
            let angle = TAU * (j as f32 + 0.5) / n as f32;
            let p = place(angle, s)?;
            ring.push((mesh.vertex_count() as u32, angle));
            mesh.positions.extend(p.to_array());
            mesh.normals.extend([0.0; 3]);
            mesh.groups.push(0);
        }
        rings.push(ring);
    }
    Some(rings)
}

/// The pore layer of one cap, `None` when it has none
fn cap_pores(underside: &mut Underside, pattern: &Pattern, p: &Params) -> Option<Mesh> {
    let deepest = p.depth.0.iter().fold(0.0f32, |a, &b| a.max(b));
    if deepest <= 0.0 {
        return None;
    }
    let frame = underside.frame;
    let free = frame.stipe_radius + p.free_distance.max(0.0);
    let hi = underside.length - underside.margin_radius;
    if hi <= free {
        return None;
    }
    underside.anchor = free;
    let underside = &*underside;
    let area = std::f32::consts::PI * (hi * hi - free * free);
    let step = (pattern.spacing.max(1e-5) / POINTS_PER_SPACING).max((area / MAX_VERTICES).sqrt());
    let spot = |angle: f32, s: f32| (0..=LOOK_IN).find_map(|k| underside.at(angle, (s - k as f32 * step).max(free)));
    let from_axis = |q: Vec3| {
        let rel = q - frame.hinge;
        (rel - frame.axis * rel.dot(frame.axis)).length()
    };
    let pore_depth = p.pore_depth.max(0.0);
    // the tubes hang from the underside, between straight down from it and straight down the
    // axis. Where they would reach in closer to the stipe than the free distance, like on a
    // button whose cap closes around the stipe, they are shorter.
    let bottom_at = |angle: f32, s: f32, start: f32, wall: f32| {
        let spot = spot(angle, s)?;
        let depth = p.depth.at((s - start) / (hi - start));
        let opening = pore_depth.min(depth) * (1.0 - wall);
        let down = (spot.normal + frame.axis).try_normalize().unwrap_or(spot.normal);
        let base = spot.position - spot.normal * spot.radius;
        let mut length = depth - opening;
        if from_axis(base - down * length) < free {
            let (mut short, mut long) = (0.0, length);
            for _ in 0..16 {
                let mid = 0.5 * (short + long);
                if from_axis(base - down * mid) < free {
                    long = mid;
                } else {
                    short = mid;
                }
            }
            length = short;
        }
        Some(base - down * length)
    };
    // where the underside slopes down to the stipe, the layer starts where it is the free
    // distance from the axis
    let clear = |s: f32| {
        (0..8).all(|k| {
            let angle = TAU * k as f32 / 8.0;
            spot(angle, s).is_none_or(|spot| from_axis(spot.position - spot.normal * spot.radius) >= free)
        })
    };
    let (mut lo, mut up) = (free, hi);
    if !clear(lo) {
        for _ in 0..24 {
            let mid = 0.5 * (lo + up);
            if clear(mid) {
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
    let keep_free = |q: Vec3, angle: f32| {
        let rel = q - frame.hinge;
        let flat = rel - frame.axis * rel.dot(frame.axis);
        let d = flat.length();
        if d < free {
            q + flat.try_normalize().unwrap_or_else(|| frame.out(angle)) * (free - d)
        } else {
            q
        }
    };

    let mut mesh = Mesh::default();
    let bottom = rings(&mut mesh, lo, hi, step, |angle, s| {
        let wall = pattern.sample(angle, s).unwrap_or(1.0).clamp(0.0, 1.0);
        Some(keep_free(bottom_at(angle, s, lo, wall)?, angle))
    })?;
    let top = rings(&mut mesh, lo, hi, TOP_COARSER * step, |angle, s| {
        let spot = spot(angle, s)?;
        Some(keep_free(spot.position - spot.normal * ((1.0 - EMBED) * spot.radius), angle))
    })?;
    let mut faces = Vec::new();
    for pair in bottom.windows(2) {
        zip(&mut faces, &pair[0], &pair[1]);
    }
    for pair in top.windows(2) {
        zip(&mut faces, &pair[1], &pair[0]);
    }
    zip(&mut faces, &top[0], &bottom[0]);
    zip(&mut faces, bottom.last().unwrap(), top.last().unwrap());
    mesh.faces = faces;
    Some(mesh)
}

/// The pores under every cap of `inputs.cap`, see the module docs
pub fn pores(inputs: &PoreInputs) -> Mesh {
    let mut meshes = Vec::new();
    let Some(pattern) = Pattern::decode(inputs.pattern) else {
        return Mesh::default();
    };
    let mut ctx = Ctx::default();
    let items = split_args(inputs.cap);
    let caps = Cap::all(&items);
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
        if let Some(mut underside) = Underside::new(cap.frame, &rays) {
            meshes.extend(cap_pores(&mut underside, &pattern, &params));
        }
    }
    let mut mesh = Mesh::default();
    for m in &meshes {
        mesh.append(m, 0);
    }
    smooth_normals(&mut mesh);
    mesh
}

#[cfg(test)]
mod tests;
