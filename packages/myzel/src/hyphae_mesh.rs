use glam::Vec3;

use crate::density::{surface, Segment};
use crate::hyphae::{Hyphae, Tissue};
use crate::mesh::Mesh;
use crate::noise::value_noise;

#[derive(Clone, Debug, PartialEq)]
pub struct HyphaeMeshParams {
    /// Cells along the longest side of the grid
    pub resolution: usize,
    /// How far, in hypha radii, neighbouring hyphae join into one surface
    pub blend: f32,
    /// Stipe and cap are tissue: where a ball of this many hypha radii can't
    /// pass between their hyphae the gap fills, 0 shows every hypha
    pub fill: f32,
    /// Strength of the fibrous relief, 0 leaves the surface smooth
    pub fibres: f32,
    pub seed: u32,
}

impl Default for HyphaeMeshParams {
    fn default() -> Self {
        HyphaeMeshParams { resolution: 64, blend: 3.0, fill: 6.0, fibres: 1.0, seed: 0 }
    }
}

/// Relief strength and how much longer than wide its noise is stretched along the hyphae
fn fibre(tissue: Tissue) -> (f32, f32) {
    match tissue {
        Tissue::Stipe => (1.0, 6.0),
        Tissue::Veil => (0.8, 5.0),
        Tissue::Mycelium => (0.8, 3.0),
        Tissue::Cap => (0.3, 2.0),
        // short stretch gives patches, which read as scales
        Tissue::CapCuticle => (0.35, 1.5),
        // gills are thin plates, a deep relief would cut through them
        Tissue::Gill => (0.1, 4.0),
    }
}

/// Noise wavelength across the fibres, in cells. The mesh has about one vertex
/// per cell, shorter noise falls between them and reads as random bumps
const FIBRE_WAVELENGTH_CELLS: f32 = 4.0;
/// Relief depth relative to the wavelength
const FIBRE_DEPTH: f32 = 0.12;
const SMOOTHING_PASSES: usize = 8;
/// The cuticle is a single layer of hyphae over the cap flesh, so flesh hyphae
/// just below it often reach the surface vertices more strongly. The surface of
/// a cap is cuticle wherever a cuticle hypha comes close.
const CUTICLE_PRIORITY: f32 = 4.0;

/// The tissue of stipe and cap fills the space between its hyphae. Gills are
/// thin plates with real gaps between them, and the veil a thin membrane.
fn is_body(tissue: Tissue) -> bool {
    matches!(tissue, Tissue::Mycelium | Tissue::Stipe | Tissue::Cap | Tissue::CapCuticle)
}

/// The surface around the hyphae with a fibrous relief, every vertex is in the group of its tissue
pub fn hyphae_mesh(hyphae: &Hyphae, params: &HyphaeMeshParams) -> Mesh {
    tissue_mesh(
        &hyphae.to_segments(),
        |i| hyphae.segments[i].tissue,
        |i| hyphae.segments[i].tissue as u32,
        params,
    )
}

/// A path for [`paths_mesh`]: its points as `x, y, z, radius` and the material group of its surface
#[derive(Clone, Copy, Debug)]
pub struct MeshPath<'a> {
    pub points: &'a [f32],
    pub group: u32,
}

/// The segments between neighbouring points of a path, each with the mean radius of its two
/// points. A path with a single point is a ball.
pub fn path_segments(points: &[f32]) -> Vec<Segment> {
    let point = |i: usize| (Vec3::from_slice(&points[i * 4..i * 4 + 3]), points[i * 4 + 3].abs());
    let n = points.len() / 4;
    if n == 1 {
        let (p, radius) = point(0);
        return vec![Segment { start: p, end: p, radius }];
    }
    (1..n)
        .map(|i| {
            let ((a, ra), (b, rb)) = (point(i - 1), point(i));
            Segment { start: a, end: b, radius: 0.5 * (ra + rb) }
        })
        .collect()
}

/// [`hyphae_mesh`] for paths, every vertex is in the group of its path. Until paths replace
/// hyphae (#168) and say which tissue they are, all of them are stipe tissue.
pub fn paths_mesh(paths: &[MeshPath], params: &HyphaeMeshParams) -> Mesh {
    let mut segments = Vec::new();
    let mut groups = Vec::new();
    for path in paths.iter().filter(|p| p.points.len() >= 4) {
        let s = path_segments(path.points);
        groups.extend(std::iter::repeat_n(path.group, s.len()));
        segments.extend(s);
    }
    tissue_mesh(&segments, |_| Tissue::Stipe, |i| groups[i], params)
}

fn tissue_mesh(
    segments: &[Segment],
    tissue: impl Fn(usize) -> Tissue,
    group: impl Fn(usize) -> u32,
    params: &HyphaeMeshParams,
) -> Mesh {
    let radius = segments.iter().map(|s| s.radius).fold(0.0, f32::max);
    let mut s = surface(
        segments,
        params.resolution,
        params.blend,
        params.fill * radius,
        |i| is_body(tissue(i)),
        |i| match tissue(i) {
            Tissue::CapCuticle => CUTICLE_PRIORITY,
            _ => 1.0,
        },
    );
    if s.mesh.vertex_count() == 0 {
        return s.mesh;
    }

    s.mesh.groups = s.owners.iter().map(|&i| group(i as usize)).collect();

    if params.fibres > 0.0 {
        let owners = &s.owners;
        add_fibres(&mut s.mesh, segments, |v| (owners[v] as usize, tissue(owners[v] as usize)), s.cell, params);
    }
    s.mesh
}

/// Moves every vertex along its normal by noise that is stretched along the
/// hypha it belongs to, then recomputes the normals so the shading shows it
fn add_fibres(
    mesh: &mut Mesh,
    segments: &[Segment],
    owner: impl Fn(usize) -> (usize, Tissue),
    cell: f32,
    params: &HyphaeMeshParams,
) {
    let n = mesh.vertex_count();
    let mut direction = Vec::with_capacity(n);
    let mut strength = Vec::with_capacity(n);
    let mut stretch = Vec::with_capacity(n);
    for v in 0..n {
        let (segment, tissue) = owner(v);
        let s = &segments[segment];
        // a root is a point, it gets its direction from its neighbours
        direction.push((s.end - s.start).normalize_or_zero());
        let (a, k) = fibre(tissue);
        strength.push(a);
        stretch.push(k);
    }

    // neighbouring vertices of different hyphae would get unrelated noise, smoothing hides the seams
    for _ in 0..SMOOTHING_PASSES {
        let mut d = direction.clone();
        let mut a = strength.clone();
        let mut k = stretch.clone();
        let mut count = vec![1.0f32; n];
        for f in mesh.faces.chunks_exact(3) {
            for (i, j) in [(f[0], f[1]), (f[1], f[2]), (f[2], f[0])] {
                let (i, j) = (i as usize, j as usize);
                // a hypha has no front and back, so opposite directions count as the same
                let dj = if direction[i].dot(direction[j]) < 0.0 { -direction[j] } else { direction[j] };
                d[i] += dj;
                a[i] += strength[j];
                k[i] += stretch[j];
                count[i] += 1.0;
            }
        }
        for v in 0..n {
            direction[v] = d[v].normalize_or_zero();
            strength[v] = a[v] / count[v];
            stretch[v] = k[v] / count[v];
        }
    }

    let wavelength = FIBRE_WAVELENGTH_CELLS * cell;
    let depth = params.fibres * FIBRE_DEPTH * wavelength;
    let centre = {
        let p = &mesh.positions;
        let (mut lo, mut hi) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
        for v in p.chunks_exact(3) {
            lo = lo.min(Vec3::from_slice(v));
            hi = hi.max(Vec3::from_slice(v));
        }
        (lo + hi) * 0.5
    };
    for v in 0..n {
        let p = Vec3::from_slice(&mesh.positions[v * 3..v * 3 + 3]);
        let normal = Vec3::from_slice(&mesh.normals[v * 3..v * 3 + 3]);
        // close to the centre the noise coordinates stay small, so direction changes shift them less
        let q = (p - centre) / wavelength;
        let d = direction[v];
        let q = q - d * q.dot(d) * (1.0 - 1.0 / stretch[v]);
        let offset = normal * (value_noise(q, params.seed) * depth * strength[v]);
        mesh.positions[v * 3..v * 3 + 3].copy_from_slice(&(p + offset).to_array());
    }

    let mut normals = vec![Vec3::ZERO; n];
    for f in mesh.faces.chunks_exact(3) {
        let [a, b, c] = [f[0], f[1], f[2]].map(|i| Vec3::from_slice(&mesh.positions[i as usize * 3..i as usize * 3 + 3]));
        let area_normal = (b - a).cross(c - a);
        for &i in f {
            normals[i as usize] += area_normal;
        }
    }
    for (v, normal) in normals.iter().enumerate() {
        let old = Vec3::from_slice(&mesh.normals[v * 3..v * 3 + 3]);
        let normal = normal.try_normalize().unwrap_or(old);
        mesh.normals[v * 3..v * 3 + 3].copy_from_slice(&normal.to_array());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::density::assert_closed;
    use crate::hyphae::{flags, HyphaSegment};
    use std::f32::consts::PI;

    fn add(h: &mut Hyphae, parent: Option<u32>, end: Vec3, birth: f32, tissue: Tissue) -> u32 {
        h.push(HyphaSegment { end, radius: 0.02, parent, birth, tissue, flags: flags::STANDARD })
    }

    fn strand(h: &mut Hyphae, from: Vec3, to: Vec3, steps: usize, birth: f32, tissue: Tissue) {
        let mut parent = add(h, None, from, birth, tissue);
        for i in 1..=steps {
            parent = add(h, Some(parent), from.lerp(to, i as f32 / steps as f32), birth, tissue);
        }
    }

    /// A stipe bundle from 0 to 1, a cap cuticle disc on top of it that is
    /// older in the middle, gills under the cap and a ring around the stipe
    fn bundle() -> Hyphae {
        let mut h = Hyphae::default();
        for i in 0..8 {
            let a = i as f32 / 8.0 * 2.0 * PI;
            let foot = Vec3::new(a.cos(), 0.0, a.sin()) * 0.06;
            strand(&mut h, foot, foot + Vec3::Y, 10, 0.0, Tissue::Stipe);
        }
        for i in 0..16 {
            let a = (i as f32 + 0.5) / 16.0 * 2.0 * PI;
            let out = Vec3::new(a.cos(), 0.0, a.sin());
            for r in 0..6 {
                let (inner, outer) = (r as f32 * 0.1, (r + 1) as f32 * 0.1);
                let root = add(&mut h, None, out * inner + Vec3::Y * 1.1, r as f32, Tissue::CapCuticle);
                add(&mut h, Some(root), out * outer + Vec3::Y * 1.1, r as f32, Tissue::CapCuticle);
            }
            strand(&mut h, out * 0.2 + Vec3::Y * 0.98, out * 0.55 + Vec3::Y * 0.98, 4, 0.0, Tissue::Gill);
        }
        for i in 0..16 {
            let a = i as f32 / 16.0 * 2.0 * PI;
            let out = Vec3::new(a.cos(), 0.0, a.sin());
            strand(&mut h, out * 0.09 + Vec3::Y * 0.6, out * 0.14 + Vec3::Y * 0.6, 2, 0.0, Tissue::Veil);
        }
        h
    }

    fn vertex(mesh: &Mesh, v: usize) -> Vec3 {
        Vec3::from_slice(&mesh.positions[v * 3..v * 3 + 3])
    }

    fn smooth() -> HyphaeMeshParams {
        HyphaeMeshParams { fibres: 0.0, ..HyphaeMeshParams::default() }
    }

    #[test]
    fn the_mesh_is_closed() {
        assert_closed(&hyphae_mesh(&bundle(), &smooth()));
        assert_closed(&hyphae_mesh(&bundle(), &HyphaeMeshParams::default()));
    }

    #[test]
    fn groups_match_the_tissues() {
        let mesh = hyphae_mesh(&bundle(), &smooth());
        assert!(mesh.colors.is_empty());
        let mut seen = [false; 4];
        for v in 0..mesh.vertex_count() {
            let p = vertex(&mesh, v);
            let radial = Vec3::new(p.x, 0.0, p.z).length();
            let group = mesh.groups[v];
            if p.y < 0.5 && p.y > 0.05 {
                assert_eq!(group, Tissue::Stipe as u32, "stipe vertex at {}", p);
                seen[0] = true;
            } else if p.y > 1.12 && radial < 0.5 {
                assert_eq!(group, Tissue::CapCuticle as u32, "cuticle vertex at {}", p);
                seen[1] = true;
            } else if (0.85..0.95).contains(&p.y) && (0.3..0.5).contains(&radial) {
                assert_eq!(group, Tissue::Gill as u32, "gill vertex at {}", p);
                seen[2] = true;
            } else if (0.55..0.65).contains(&p.y) && radial > 0.12 {
                assert_eq!(group, Tissue::Veil as u32, "ring vertex at {}", p);
                seen[3] = true;
            }
        }
        assert_eq!(seen, [true; 4]);
    }

    #[test]
    fn the_stipe_relief_runs_along_the_hyphae() {
        let mut h = Hyphae::default();
        for i in 0..24 {
            let a = i as f32 / 24.0 * 2.0 * PI;
            let foot = Vec3::new(a.cos(), 0.0, a.sin()) * 0.15;
            strand(&mut h, foot, foot + Vec3::Y, 10, 0.0, Tissue::Stipe);
        }
        let plain = hyphae_mesh(&h, &smooth());
        let fibrous = hyphae_mesh(&h, &HyphaeMeshParams::default());
        let offset = |v: usize| (vertex(&fibrous, v) - vertex(&plain, v)).dot(Vec3::from_slice(&plain.normals[v * 3..v * 3 + 3]));
        // how much the relief changes per length, along the stipe and around it
        let (mut along, mut around) = ([0.0f32; 2], [0.0f32; 2]);
        for f in plain.faces.chunks_exact(3) {
            for (a, b) in [(f[0], f[1]), (f[1], f[2]), (f[2], f[0])] {
                let (a, b) = (a as usize, b as usize);
                let (pa, pb) = (vertex(&plain, a), vertex(&plain, b));
                if !(0.2..0.8).contains(&pa.y) {
                    continue;
                }
                let edge = pb - pa;
                let change = (offset(a) - offset(b)).abs() / edge.length();
                let sums = if edge.y.abs() > 0.9 * edge.length() { &mut along } else if edge.y.abs() < 0.1 * edge.length() { &mut around } else { continue };
                sums[0] += change;
                sums[1] += 1.0;
            }
        }
        let (along, around) = (along[0] / along[1], around[0] / around[1]);
        assert!(along < 0.5 * around, "relief changes {} along and {} around the stipe", along, around);
    }

    #[test]
    fn fibres_move_vertices_a_little() {
        let plain = hyphae_mesh(&bundle(), &smooth());
        let fibrous = hyphae_mesh(&bundle(), &HyphaeMeshParams::default());
        assert_eq!(plain.faces, fibrous.faces);
        assert_eq!(plain.groups, fibrous.groups);
        let h = bundle();
        let d = HyphaeMeshParams::default();
        let cell = surface(&h.to_segments(), d.resolution, d.blend, d.fill * 0.02, |i| is_body(h.segments[i].tissue), |_| 1.0).cell;
        let moved: Vec<f32> =
            (0..plain.vertex_count()).map(|v| (vertex(&plain, v) - vertex(&fibrous, v)).length()).collect();
        assert!(moved.iter().any(|&d| d > 0.0));
        assert!(moved.iter().all(|&d| d <= FIBRE_DEPTH * FIBRE_WAVELENGTH_CELLS * cell + 1e-6));
    }

    #[test]
    fn same_input_gives_the_same_output() {
        let params = HyphaeMeshParams { seed: 7, ..HyphaeMeshParams::default() };
        assert_eq!(hyphae_mesh(&bundle(), &params).encode(), hyphae_mesh(&bundle(), &params).encode());
        let other = HyphaeMeshParams { seed: 8, ..params.clone() };
        assert_ne!(hyphae_mesh(&bundle(), &params).positions, hyphae_mesh(&bundle(), &other).positions);
    }

    #[test]
    fn paths_become_one_closed_tissue_in_the_groups_of_their_creators() {
        let mut paths: Vec<(Vec<f32>, u32)> = Vec::new();
        for i in 0..24 {
            let a = i as f32 / 24.0 * 2.0 * PI;
            let foot = Vec3::new(a.cos(), 0.0, a.sin()) * 0.15;
            let points = (0..=10).flat_map(|j| (foot + Vec3::Y * j as f32 / 10.0).extend(0.02).to_array()).collect();
            paths.push((points, if i < 12 { 7 } else { 31 }));
        }
        let paths: Vec<MeshPath> = paths.iter().map(|(points, group)| MeshPath { points, group: *group }).collect();
        let mesh = paths_mesh(&paths, &HyphaeMeshParams::default());
        assert_closed(&mesh);
        assert!(mesh.groups.contains(&7) && mesh.groups.contains(&31));
        assert!(mesh.groups.iter().all(|&g| g == 7 || g == 31));
        // the bundle is one tissue, not 24 tubes: nothing of the surface lies deep inside it
        for v in 0..mesh.vertex_count() {
            let p = vertex(&mesh, v);
            if (0.2..0.8).contains(&p.y) {
                assert!(Vec3::new(p.x, 0.0, p.z).length() > 0.12, "vertex inside the bundle at {}", p);
            }
        }
    }

    #[test]
    fn path_segments_carry_the_mean_radius() {
        let s = path_segments(&[0.0, 0.0, 0.0, 0.2, 0.0, 1.0, 0.0, 0.1, 0.0, 2.0, 0.0, 0.0]);
        assert_eq!(s.len(), 2);
        assert_eq!(s[1].start, Vec3::Y);
        assert!((s[0].radius - 0.15).abs() < 1e-6 && (s[1].radius - 0.05).abs() < 1e-6);
        assert_eq!(path_segments(&[1.0, 2.0, 3.0, 0.1]).len(), 1);
    }

    #[test]
    fn no_hyphae_give_an_empty_mesh() {
        assert_eq!(hyphae_mesh(&Hyphae::default(), &HyphaeMeshParams::default()), Mesh::default());
    }
}
