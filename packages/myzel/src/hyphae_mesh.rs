use glam::Vec3;

use crate::density::{surface_in, Frame, Meshing, Segment, TissueField};
use crate::hyphae::Hyphae;
use crate::mesh::Mesh;
use crate::noise::value_noise;
use crate::paths::{group_numbers, path_segments, MeshPath, PathStyle};

#[derive(Clone, Debug, PartialEq)]
pub struct HyphaeMeshParams {
    /// Cells along the longest side of the grid
    pub resolution: usize,
    /// How far, in hypha radii, neighbouring hyphae join into one surface
    pub blend: f32,
    /// The body paths are tissue: where a ball of this many path radii can't
    /// pass between them the gap fills, 0 shows every path
    pub fill: f32,
    /// Strength of the fibrous relief, 0 leaves the surface smooth
    pub fibres: f32,
    pub seed: u32,
    /// Body paths that leave the tissue: 0 keeps none of them, the surface is closed
    /// tissue, 1 keeps every one as a strand. In between a strand stays where the
    /// tissue grown by `1 / loose_strands` doesn't cover it, see [`TissueField`].
    pub loose_strands: f32,
}

impl Default for HyphaeMeshParams {
    fn default() -> Self {
        HyphaeMeshParams { resolution: 64, blend: 3.0, fill: 6.0, fibres: 1.0, seed: 0, loose_strands: 0.2 }
    }
}

/// Noise wavelength across the fibres, in cells. The mesh has about one vertex
/// per cell, shorter noise falls between them and reads as random bumps
const FIBRE_WAVELENGTH_CELLS: f32 = 4.0;
/// Relief depth relative to the wavelength
const FIBRE_DEPTH: f32 = 0.12;
const SMOOTHING_PASSES: usize = 8;

/// The surface around the hyphae with a fibrous relief, every vertex is in the group of its tissue
pub fn hyphae_mesh(hyphae: &Hyphae, params: &HyphaeMeshParams) -> Mesh {
    paths_mesh(&hyphae.to_paths(), params)
}

/// The surface around the paths with a fibrous relief along them. The body paths fill into one
/// tissue, see [`HyphaeMeshParams::fill`], and only some of them leave it as strands, see
/// [`HyphaeMeshParams::loose_strands`]. Every vertex is in the group of the path that owns it,
/// see [`group_numbers`].
pub fn paths_mesh(paths: &[MeshPath], params: &HyphaeMeshParams) -> Mesh {
    let mut segments = Vec::new();
    let mut owner = Vec::new();
    let mut meshing = Vec::new();
    for (k, path) in paths.iter().enumerate().filter(|(_, p)| !p.is_empty()) {
        let s = path_segments(&path.points);
        owner.extend(std::iter::repeat_n(k, s.len()));
        let (closed, priority) = (path.style.body, path.style.priority);
        meshing.extend((0..s.len()).map(|j| Meshing { closed, priority, joined: j > 0 }));
        segments.extend(s);
    }
    let style = |i: usize| paths[owner[i]].style;
    let radius = segments.iter().map(|s| s.radius).fold(0.0, f32::max);
    let Some(frame) = Frame::new(&segments, &meshing, params.resolution, params.blend, params.fill * radius) else {
        return Mesh::default();
    };
    let loose = params.loose_strands.clamp(0.0, 1.0);
    let scale = if loose > 0.0 { 1.0 / loose } else { f32::INFINITY };
    let field = (loose < 1.0).then(|| TissueField::new(&frame, &segments, &meshing, scale)).flatten();
    let mut s = match field {
        Some(field) => {
            let strand = strands(paths, &field, segments.len());
            field.surface(&segments, &meshing, |i| strand[i])
        }
        // at 1 every body path keeps its tube wherever it leaves the tissue
        None => surface_in(&frame, &segments, &meshing, |i| loose >= 1.0 && meshing[i].closed),
    };
    if s.mesh.vertex_count() == 0 {
        return s.mesh;
    }

    let (groups, tissues) = group_numbers(paths);
    s.mesh.groups = s.owners.iter().map(|&i| groups[owner[i as usize]]).collect();
    s.mesh.tissue_groups = tissues;

    if params.fibres > 0.0 {
        let owners = &s.owners;
        add_fibres(&mut s.mesh, &segments, |v| (owners[v] as usize, style(owners[v] as usize)), s.cell, params);
    }
    s.mesh
}

/// Per segment of the paths, in the order [`paths_mesh`] makes them, whether it is a strand that
/// keeps its own tube where it leaves the tissue. Every body path is split into runs of points
/// outside and inside the tissue:
///
/// - a run outside that the grown tissue covers completely is a small feature next to a large
///   one, the tissue absorbs it
/// - any other run outside is a strand, from the last point inside to the next one, so it grows
///   out of the tissue
///
/// A path outside the tissue all along, such as a bundle too thin for the blur, is one run.
fn strands(paths: &[MeshPath], field: &TissueField, count: usize) -> Vec<bool> {
    let mut strand = Vec::with_capacity(count);
    for path in paths.iter().filter(|p| !p.is_empty()) {
        let first = strand.len();
        let n = path.len();
        strand.extend(std::iter::repeat_n(false, path_segments(&path.points).len()));
        if !path.style.body {
            continue;
        }
        let inside: Vec<bool> = (0..n).map(|i| field.inside(path.point(i))).collect();
        let mut start = 0;
        for end in 1..=n {
            if end < n && inside[end] == inside[start] {
                continue;
            }
            let run = start..end;
            start = end;
            if inside[run.start] || run.clone().all(|i| field.covers(path.point(i), path.points[i * 4 + 3].abs())) {
                continue;
            }
            // segment j joins the points j and j + 1, a path of one point is one ball
            let segments = if n == 1 { 0..1 } else { run.start.saturating_sub(1)..run.end.min(n - 1) };
            strand[first + segments.start..first + segments.end].fill(true);
        }
    }
    strand
}

/// Moves every vertex along its normal by noise that is stretched along the
/// hypha it belongs to, then recomputes the normals so the shading shows it
fn add_fibres(
    mesh: &mut Mesh,
    segments: &[Segment],
    owner: impl Fn(usize) -> (usize, PathStyle),
    cell: f32,
    params: &HyphaeMeshParams,
) {
    let n = mesh.vertex_count();
    let mut direction = Vec::with_capacity(n);
    let mut strength = Vec::with_capacity(n);
    let mut stretch = Vec::with_capacity(n);
    for v in 0..n {
        let (segment, style) = owner(v);
        let s = &segments[segment];
        // a path of one point is a ball, it gets its direction from its neighbours
        direction.push((s.end - s.start).normalize_or_zero());
        strength.push(style.fibre_strength);
        stretch.push(style.fibre_stretch);
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
    use crate::density::{assert_closed, components, surface};
    use crate::hyphae::{flags, HyphaSegment, Tissue};
    use crate::paths::PathGroup;
    use std::borrow::Cow;
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
            } else if (0.9..0.97).contains(&p.y) && (0.3..0.5).contains(&radial) {
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
        let body = |i: usize| PathStyle::of_tissue(h.segments[i].tissue).body;
        let cell = surface(&h.to_segments(), d.resolution, d.blend, d.fill * 0.02, body, |_| false, |_| 1.0).cell;
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

    /// 24 straight paths from y 0 to 1 around the y axis, the first half made by node 31, the other by 7
    fn stipe_points() -> Vec<(Vec<f32>, i32)> {
        (0..24)
            .map(|i| {
                let a = i as f32 / 24.0 * 2.0 * PI;
                let foot = Vec3::new(a.cos(), 0.0, a.sin()) * 0.15;
                let points = (0..=10).flat_map(|j| (foot + Vec3::Y * j as f32 / 10.0).extend(0.02).to_array()).collect();
                (points, if i < 12 { 31 } else { 7 })
            })
            .collect()
    }

    fn mesh_paths(points: &[(Vec<f32>, i32)], style: PathStyle) -> Vec<MeshPath<'_>> {
        points
            .iter()
            .map(|(p, creator)| MeshPath { points: Cow::Borrowed(p), group: PathGroup::Creator(*creator), style })
            .collect()
    }

    #[test]
    fn paths_become_one_closed_tissue_in_the_groups_of_their_creators() {
        let points = stipe_points();
        let mesh = paths_mesh(&mesh_paths(&points, PathStyle::BODY), &HyphaeMeshParams::default());
        assert_closed(&mesh);
        assert!(!mesh.tissue_groups);
        // creator 7 comes first in the order of the ids
        for v in 0..mesh.vertex_count() {
            let p = vertex(&mesh, v);
            let angle = p.z.atan2(p.x).rem_euclid(2.0 * PI);
            if (0.2..0.8).contains(&p.y) && (0.3..2.8).contains(&angle) {
                assert_eq!(mesh.groups[v], 1, "vertex of creator 31 at {}", p);
            }
            if (0.2..0.8).contains(&p.y) && (3.5..6.0).contains(&angle) {
                assert_eq!(mesh.groups[v], 0, "vertex of creator 7 at {}", p);
            }
        }
        assert!(mesh.groups.iter().all(|&g| g < 2));
        // the bundle is one tissue, not 24 tubes: nothing of the surface lies deep inside it
        for v in 0..mesh.vertex_count() {
            let p = vertex(&mesh, v);
            if (0.2..0.8).contains(&p.y) {
                assert!(Vec3::new(p.x, 0.0, p.z).length() > 0.12, "vertex inside the bundle at {}", p);
            }
        }
    }

    #[test]
    fn loose_paths_keep_their_gaps() {
        let points = stipe_points();
        let loose = paths_mesh(&mesh_paths(&points, PathStyle::LOOSE), &smooth());
        let inside = (0..loose.vertex_count()).any(|v| {
            let p = vertex(&loose, v);
            (0.2..0.8).contains(&p.y) && Vec3::new(p.x, 0.0, p.z).length() < 0.14
        });
        assert!(inside, "the gaps between loose paths stay open, their inner side shows");
    }

    /// The hyphae that are the same as the paths: a root at the first point, then one segment per point
    fn equivalent_hyphae(points: &[(Vec<f32>, i32)], tissue: Tissue) -> Hyphae {
        let mut h = Hyphae::default();
        for (p, _) in points {
            let mut parent = None;
            for c in p.chunks(4) {
                parent = Some(h.push(HyphaSegment {
                    end: Vec3::new(c[0], c[1], c[2]),
                    radius: c[3],
                    parent,
                    birth: 0.0,
                    tissue,
                    flags: flags::STANDARD,
                }));
            }
        }
        h
    }

    #[test]
    fn paths_and_the_same_hyphae_give_the_same_mesh() {
        let points = stipe_points();
        let hyphae = equivalent_hyphae(&points, Tissue::Stipe);
        let style = PathStyle::of_tissue(Tissue::Stipe);
        for params in [smooth(), HyphaeMeshParams::default()] {
            let from_paths = paths_mesh(&mesh_paths(&points, style), &params);
            let from_hyphae = hyphae_mesh(&hyphae, &params);
            assert!(from_paths.face_count() > 0);
            assert_eq!(from_paths.faces, from_hyphae.faces);
            assert_eq!(from_paths.positions, from_hyphae.positions);
            assert_eq!(from_paths.normals, from_hyphae.normals);
            // paths number their creators, hyphae keep their tissue
            assert!(from_hyphae.tissue_groups && from_hyphae.groups.iter().all(|&g| g == Tissue::Stipe as u32));
        }
    }

    #[test]
    fn hyphae_as_paths_mesh_like_their_segments() {
        // the paths leave out the balls at the roots of the hyphae, the surface stays about where it was
        let h = bundle();
        // every hypha leaving the tissue stays, as the segments do
        let params = HyphaeMeshParams { loose_strands: 1.0, ..smooth() };
        let mesh = hyphae_mesh(&h, &params);
        let style = |i: usize| PathStyle::of_tissue(h.segments[i].tissue);
        let (resolution, blend, close) = (params.resolution, params.blend, params.fill * 0.02);
        // a segment that continues its parent of the same tissue is joined to it, as in a path
        let meshing: Vec<Meshing> = (0..h.len())
            .map(|i| {
                let s = &h.segments[i];
                let joined = i > 0 && s.parent == Some(i as u32 - 1) && h.segments[i - 1].tissue == s.tissue;
                Meshing { closed: style(i).body, priority: style(i).priority, joined }
            })
            .collect();
        let segments = h.to_segments();
        let frame = Frame::new(&segments, &meshing, resolution, blend, close).unwrap();
        let old = surface_in(&frame, &segments, &meshing, |_| true);
        let bounds = |m: &Mesh| {
            m.positions.chunks(3).fold((Vec3::INFINITY, Vec3::NEG_INFINITY), |(lo, hi), p| {
                (lo.min(Vec3::from_slice(p)), hi.max(Vec3::from_slice(p)))
            })
        };
        let ((lo, hi), (old_lo, old_hi)) = (bounds(&mesh), bounds(&old.mesh));
        assert!((lo - old_lo).abs().max_element() < 2.0 * old.cell, "{} {}", lo, old_lo);
        assert!((hi - old_hi).abs().max_element() < 2.0 * old.cell, "{} {}", hi, old_hi);
        let ratio = mesh.vertex_count() as f32 / old.mesh.vertex_count() as f32;
        assert!((0.9..1.1).contains(&ratio), "{} vertices, {} before", mesh.vertex_count(), old.mesh.vertex_count());
    }

    #[test]
    fn the_same_paths_give_the_same_mesh() {
        let points = stipe_points();
        let params = HyphaeMeshParams { seed: 3, ..HyphaeMeshParams::default() };
        let a = paths_mesh(&mesh_paths(&points, PathStyle::BODY), &params).encode();
        assert_eq!(a, paths_mesh(&mesh_paths(&points, PathStyle::BODY), &params).encode());
    }

    #[test]
    fn no_hyphae_give_an_empty_mesh() {
        assert_eq!(hyphae_mesh(&Hyphae::default(), &HyphaeMeshParams::default()), Mesh::default());
    }

    /// A straight path from `a` to `b` of `points` points with radius `r`
    fn line(a: Vec3, b: Vec3, points: usize, r: f32) -> Vec<f32> {
        (0..points).flat_map(|i| a.lerp(b, i as f32 / (points - 1) as f32).extend(r).to_array()).collect()
    }

    /// A body of 16 paths in a ring of radius 0.15 from y 0 to 1, as `density`'s tests have it, and more paths
    fn body_and(more: &[Vec<f32>]) -> Vec<(Vec<f32>, i32)> {
        let mut paths: Vec<(Vec<f32>, i32)> = (0..16)
            .map(|i| {
                let a = i as f32 / 16.0 * 2.0 * PI;
                let at = Vec3::new(a.cos(), 0.0, a.sin()) * 0.15;
                (line(at, at + Vec3::Y, 11, 0.03), 1)
            })
            .collect();
        paths.extend(more.iter().map(|p| (p.clone(), 2)));
        paths
    }

    #[test]
    fn a_loose_path_keeps_its_radius_where_its_segments_meet() {
        // segments of 2 r: summed, the kernels of neighbouring segments swelled the path to 1.66 r at blend 3
        let (r, length) = (0.05, 1.0);
        let path = line(Vec3::ZERO, Vec3::Y * length, 11, r);
        for blend in [1.5, 2.4, 3.0] {
            let params = HyphaeMeshParams { resolution: 128, blend, ..smooth() };
            let mesh = paths_mesh(&mesh_paths(&[(path.clone(), 1)], PathStyle::LOOSE), &params);
            assert_closed(&mesh);
            let cell = surface(&path_segments(&path), 128, blend, 0.0, |_| false, |_| false, |_| 1.0).cell;
            for v in 0..mesh.vertex_count() {
                let p = vertex(&mesh, v);
                let d = (p - Vec3::Y * p.y.clamp(0.0, length)).length();
                assert!((d - r).abs() < 1.4 * cell, "blend {}: vertex {} from the axis of a path of radius {}", blend, d, r);
            }
        }
    }

    #[test]
    fn a_loose_path_that_comes_back_blends_with_itself() {
        // up one arm and down the other, closer than blend 3 joins: like two paths, the gap between the arms fills
        let (r, gap) = (0.03, 0.075);
        let up = line(Vec3::ZERO, Vec3::Y, 11, r);
        let down = line(Vec3::new(gap, 1.0, 0.0), Vec3::new(gap, 0.0, 0.0), 11, r);
        let between = |paths: &[(Vec<f32>, i32)]| {
            let mesh = paths_mesh(&mesh_paths(paths, PathStyle::LOOSE), &smooth());
            assert_closed(&mesh);
            (0..mesh.vertex_count())
                .map(|v| vertex(&mesh, v))
                .filter(|p| (0.3..0.7).contains(&p.y) && (p.x - 0.5 * gap).abs() < 0.25 * gap && p.z.abs() < 0.5 * r)
                .count()
        };
        assert_eq!(between(&[(up.clone(), 1), (down.clone(), 1)]), 0, "two paths");
        assert_eq!(between(&[([up, down].concat(), 1)]), 0, "one path");
        // too far apart for blend 3, the arms stay apart
        let far = line(Vec3::new(4.0 * r, 1.0, 0.0), Vec3::new(4.0 * r, 0.0, 0.0), 11, r);
        let apart = paths_mesh(&mesh_paths(&[([line(Vec3::ZERO, Vec3::Y, 11, r), far].concat(), 1)], PathStyle::LOOSE), &smooth());
        let inner = |p: Vec3| (0.3..0.7).contains(&p.y) && (0.5 * r..3.5 * r).contains(&p.x) && p.z.abs() < 0.5 * r;
        assert!((0..apart.vertex_count()).map(|v| vertex(&apart, v)).any(inner), "the inner sides of the arms show");
    }

    fn scaled(paths: &[(Vec<f32>, i32)], k: f32) -> Vec<(Vec<f32>, i32)> {
        paths.iter().map(|(p, c)| (p.iter().map(|v| v * k).collect(), *c)).collect()
    }

    fn loose(loose_strands: f32) -> HyphaeMeshParams {
        HyphaeMeshParams { loose_strands, ..smooth() }
    }

    fn mesh_body(paths: &[(Vec<f32>, i32)], params: &HyphaeMeshParams) -> Mesh {
        let mesh = paths_mesh(&mesh_paths(paths, PathStyle::BODY), params);
        assert_closed(&mesh);
        mesh
    }

    fn highest(mesh: &Mesh) -> f32 {
        (0..mesh.vertex_count()).map(|v| vertex(mesh, v).y).fold(f32::NEG_INFINITY, f32::max)
    }

    #[test]
    fn a_strand_leaving_the_tissue_is_gone_at_0_and_kept_at_1() {
        // the scene of `a_strand_keeps_its_tube_out_of_the_tissue`: the closing has radius 6 r = 0.18
        let (r, close) = (0.03, 0.18);
        let rim = Vec3::new(0.15, 1.0, 0.0);
        let paths = body_and(&[line(rim, rim + Vec3::Y * 4.0 * close, 8, r)]);
        let gone = mesh_body(&paths, &loose(0.0));
        assert_eq!(components(&gone), 1);
        // the hyphae of the body end at 1, the closing leaves a low fillet where the strand was
        assert!(highest(&gone) < 1.0 + 0.5 * close, "strand ends at {}", highest(&gone));
        let kept = mesh_body(&paths, &loose(1.0));
        assert_eq!(components(&kept), 1);
        assert!(highest(&kept) > 1.0 + 4.0 * close + 0.5 * r, "strand ends at {}", highest(&kept));
    }

    #[test]
    fn a_tip_just_outside_the_tissue_leaves_no_spike() {
        let r = 0.03;
        let rim = Vec3::new(0.15, 1.0, 0.0);
        let tipped = mesh_body(&body_and(&[line(rim, rim + Vec3::Y * 3.0 * r, 3, r)]), &loose(0.2));
        assert_eq!(components(&tipped), 1);
        // as its own tube the tip would reach 4 r above the hyphae of the body
        assert!(highest(&tipped) < 1.0 + r, "tip ends at {}", highest(&tipped));
    }

    #[test]
    fn strands_stay_by_the_thickness_of_the_tissue_next_to_them() {
        // the same tip, 3 mm out of the top of a stipe 20 mm wide and of a lone bundle 1 mm wide
        let (r, tip) = (0.0001, 0.003);
        let column = |width: f32, height: f32| {
            let rings = (width / 2.0 / (6.0 * r)).ceil() as usize;
            let mut paths: Vec<Vec<f32>> = vec![line(Vec3::ZERO, Vec3::Y * height, 12, r)];
            for ring in 1..=rings {
                let radius = width / 2.0 * ring as f32 / rings as f32 - r;
                let around = (2.0 * PI * radius / (6.0 * r)).ceil() as usize;
                for i in 0..around {
                    let a = i as f32 / around as f32 * 2.0 * PI;
                    let at = Vec3::new(a.cos(), 0.0, a.sin()) * radius;
                    paths.push(line(at, at + Vec3::Y * height, 12, r));
                }
            }
            paths.push(line(Vec3::Y * height, Vec3::Y * (height + tip), 5, r));
            paths.into_iter().map(|p| (p, 1)).collect::<Vec<_>>()
        };
        let params = HyphaeMeshParams { resolution: 96, fill: 6.0, ..smooth() };
        let stipe = mesh_body(&column(0.02, 0.03), &params);
        assert!(highest(&stipe) < 0.03 + 0.5 * tip, "the stipe ends at {}", highest(&stipe));
        let bundle = mesh_body(&column(0.001, 0.01), &params);
        assert!(highest(&bundle) > 0.01 + 0.9 * tip, "the bundle ends at {}", highest(&bundle));
    }

    #[test]
    fn free_strands_stay_by_their_distance_to_the_tissue() {
        // upright strands next to the body, a strand is absorbed while it is closer than (1 / 0.2 - 1) R
        let free = |x: f32| line(Vec3::new(0.15 + x, 0.0, 0.0), Vec3::new(0.15 + x, 1.0, 0.0), 11, 0.03);
        let paths = body_and(&[free(0.3), free(1.0), free(2.0)]);
        let count = |l: f32| components(&mesh_body(&paths, &HyphaeMeshParams { resolution: 96, ..loose(l) }));
        assert_eq!(count(0.0), 1);
        assert_eq!(count(0.2), 1 + 2);
        assert_eq!(count(1.0), 1 + 3);
    }

    #[test]
    fn the_same_strands_stay_at_any_size() {
        let free = |x: f32, h: f32| line(Vec3::new(0.15 + x, 0.0, 0.0), Vec3::new(0.15 + x, h, 0.0), 11, 0.03);
        let rim = Vec3::new(0.15, 1.0, 0.0);
        let paths = body_and(&[free(0.3, 1.0), free(1.0, 1.0), line(rim, rim + Vec3::new(0.1, 0.6, 0.0), 8, 0.03)]);
        let params = HyphaeMeshParams { resolution: 96, ..loose(0.2) };
        let full = mesh_body(&paths, &params);
        let half = mesh_body(&scaled(&paths, 0.5), &params);
        assert_eq!(components(&full), components(&half));
        assert!((highest(&half) - 0.5 * highest(&full)).abs() < 1e-3, "{} vs {}", highest(&half), highest(&full));
        let ratio = half.vertex_count() as f32 / full.vertex_count() as f32;
        assert!((0.98..1.02).contains(&ratio), "{} vertices, {} at full size", half.vertex_count(), full.vertex_count());
    }

    #[test]
    fn a_thin_body_bundle_without_tissue_stays() {
        // narrower than the blur, there is no tissue to absorb it, even at 0
        let bundle: Vec<(Vec<f32>, i32)> = (0..4)
            .map(|i| {
                let a = i as f32 / 4.0 * 2.0 * PI;
                let at = Vec3::new(a.cos(), 0.0, a.sin()) * 0.03;
                (line(at, at + Vec3::Y, 11, 0.02), 1)
            })
            .collect();
        for l in [0.0, 0.2] {
            let mesh = mesh_body(&bundle, &HyphaeMeshParams { fill: 15.0, ..loose(l) });
            assert!(highest(&mesh) > 1.0, "bundle ends at {}", highest(&mesh));
        }
    }

    #[test]
    fn an_arc_outside_the_tissue_is_absorbed_or_kept_whole() {
        // a path that leaves the top of the body and comes back in further round
        let (r, close) = (0.03, 0.18);
        let arc: Vec<f32> = (0..=12)
            .flat_map(|i| {
                let t = i as f32 / 12.0;
                let a = t * PI;
                Vec3::new(0.12 * a.cos(), 0.9 + 0.5 * a.sin(), 0.12 * a.sin()).extend(r).to_array()
            })
            .collect();
        let paths = body_and(&[arc]);
        let gone = mesh_body(&paths, &loose(0.0));
        assert_eq!(components(&gone), 1);
        // only the low fillets of the closing show where it leaves and enters the body
        assert!(highest(&gone) < 1.0 + 0.1, "arc reaches {}", highest(&gone));
        let kept = mesh_body(&paths, &loose(1.0));
        assert_eq!(components(&kept), 1);
        assert!(highest(&kept) > 1.4 - close, "arc reaches {}", highest(&kept));
    }
}
