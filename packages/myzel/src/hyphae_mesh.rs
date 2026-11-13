use glam::Vec3;

use crate::color::{linear_rgba, pack_linear, NO_COLOR};
use crate::density::{surface_in, Frame, Meshing, Segment, TissueField};
use crate::mesh::Mesh;
use crate::noise::value_noise;
use crate::paths::{group_numbers, path_segments, MeshPath, PathStyle};
use crate::spatial::{distance_sq_to_segment, TubeTree};

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
    /// Body paths that leave the tissue: 0 keeps none of them that a tissue thicker than
    /// they are could absorb, 1 keeps every one as a strand. In between a strand stays
    /// where the tissue grown by `1 / loose_strands` doesn't cover it, see [`TissueField`].
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

/// The surface around the paths with a fibrous relief along them. The body paths fill into one
/// tissue, see [`HyphaeMeshParams::fill`], and only some of them leave it as strands, see
/// [`HyphaeMeshParams::loose_strands`]. Every vertex is in the group of the path that owns it,
/// see [`group_numbers`].
///
/// A path that brings its own fibres or fill (see [`crate::PathSettings`]) is meshed with
/// them instead of `params`: its gaps fill by its own fill and
/// its relief has its own strength. Where paths of different values meet, the surface and the
/// relief go over from one to the other.
pub fn paths_mesh(paths: &[MeshPath], params: &HyphaeMeshParams) -> Mesh {
    let mut segments = Vec::new();
    let mut owner = Vec::new();
    for (k, path) in paths.iter().enumerate().filter(|(_, p)| !p.is_empty()) {
        let s = path_segments(&path.points);
        owner.extend(std::iter::repeat_n(k, s.len()));
        segments.extend(s);
    }
    // the fill counts in radii of the thickest path
    let radius = segments.iter().map(|s| s.radius).fold(0.0, f32::max);
    let mut meshing = Vec::with_capacity(segments.len());
    for (i, &k) in owner.iter().enumerate() {
        let path = &paths[k];
        let fill = path.settings.fill.unwrap_or(params.fill);
        meshing.push(Meshing {
            closed: true,
            priority: path.style.priority,
            joined: i > 0 && owner[i - 1] == k,
            blend: params.blend,
            close: (fill * radius).max(0.0),
        });
    }
    let style = |i: usize| paths[owner[i]].style;
    let Some(frame) = Frame::new(&segments, &meshing, params.resolution) else {
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

    let groups = group_numbers(paths);
    s.mesh.groups = s.owners.iter().map(|&i| groups[owner[i as usize]]).collect();
    if paths.iter().any(|p| p.settings.color.is_some()) {
        let colors: Vec<u32> = owner.iter().map(|&k| paths[k].settings.color.unwrap_or(NO_COLOR)).collect();
        let cell = s.cell;
        let seam: Vec<f32> = segments.iter().zip(&meshing).map(|(g, m)| seam_width(g, m, cell)).collect();
        s.mesh.colors = blend_colors(&s.mesh, &s.owners, &segments, &colors, &seam);
    }

    // the relief of a path is its fibres relative to those of the node, so it stays the same where they are the same
    let fibres = |i: usize| paths[owner[i]].settings.fibres.unwrap_or(params.fibres).max(0.0);
    let reference = if params.fibres > 0.0 { params.fibres } else { s.owners.iter().map(|&i| fibres(i as usize)).fold(0.0, f32::max) };
    if s.owners.iter().any(|&i| fibres(i as usize) > 0.0) {
        let owners = &s.owners;
        let owner = |v: usize| {
            let i = owners[v] as usize;
            (i, style(i), fibres(i) / reference)
        };
        add_fibres(&mut s.mesh, &segments, owner, s.cell, reference, params.seed);
    }
    s.mesh
}

/// How far a color goes over into the one next to it: as far as the kernel of the segment reaches,
/// where paths blend into one surface, but at least a cell
fn seam_width(segment: &Segment, meshing: &Meshing, cell: f32) -> f32 {
    (meshing.blend.max(1.1) * segment.radius).max(cell)
}

/// A seam fades out over this many seam widths
const SEAM_REACH: f32 = 3.0;

/// The color of every vertex, mixed from the colors of the segments near it. Each color counts by
/// how much farther its nearest tube is than the nearest tube of any color, `exp(-(Δd / w)²)` with
/// the seam width `w` of the two segments: one color away from the others, and an even mix where
/// two colors are equally near, so the color goes over smoothly where the surface goes over from
/// one path to the next, filled gaps included. Colors mix in linear light, a vertex without a
/// color of its own ([`NO_COLOR`]) only lowers the alpha.
fn blend_colors(mesh: &Mesh, owners: &[u32], segments: &[Segment], colors: &[u32], seam: &[f32]) -> Vec<u32> {
    let mut palette: Vec<u32> = Vec::new();
    let slot: Vec<usize> = colors
        .iter()
        .map(|c| {
            palette.iter().position(|p| p == c).unwrap_or_else(|| {
                palette.push(*c);
                palette.len() - 1
            })
        })
        .collect();
    let n = mesh.vertex_count();
    if palette.len() == 1 {
        return vec![palette[0]; n];
    }
    let linear: Vec<[f32; 4]> = palette.iter().map(|&c| linear_rgba(c)).collect();
    let reach = seam.iter().fold(0.0f32, |a, &w| a.max(w)) * SEAM_REACH;
    let mut tubes = vec![Vec::new(); palette.len()];
    for (i, s) in segments.iter().enumerate() {
        tubes[slot[i]].push((s.start, s.end, s.radius, i as u32));
    }
    let trees: Vec<TubeTree> = tubes.into_iter().map(TubeTree::new).collect();
    let tube = |i: usize, p: Vec3| distance_sq_to_segment(p, segments[i].start, segments[i].end).sqrt() - segments[i].radius;

    let mut nearest = vec![(f32::INFINITY, 0usize); palette.len()];
    let mut out = Vec::with_capacity(n);
    for (p, &own) in mesh.positions.chunks_exact(3).zip(owners) {
        let p = Vec3::from_slice(p);
        let own = own as usize;
        nearest.fill((f32::INFINITY, 0));
        let own_slot = slot[own];
        nearest[own_slot] = (tube(own, p), own);
        if let Some((d, i)) = trees[own_slot].nearest(p, nearest[own_slot].0) {
            nearest[own_slot] = (d, i as usize);
        }
        // a color counts up to SEAM_REACH seams farther out than the nearest one, and the seam is
        // at most the widest one, so a color farther than that doesn't count
        let mut closest = nearest[own_slot].0;
        for (k, tree) in trees.iter().enumerate().filter(|&(k, _)| k != own_slot) {
            if let Some((d, i)) = tree.nearest(p, closest + reach) {
                nearest[k] = (d, i as usize);
                closest = closest.min(d);
            }
        }
        let (closest, by) = nearest.iter().copied().fold((f32::INFINITY, own), |a, b| if b.0 < a.0 { b } else { a });
        let mut sum = [0.0f32; 4];
        let mut total = 0.0;
        for (k, &(d, i)) in nearest.iter().enumerate() {
            let w = 0.5 * (seam[i] + seam[by]);
            let x = (d - closest) / w;
            // colors no segment nearby has are infinitely far
            if x.is_nan() || x >= SEAM_REACH {
                continue;
            }
            let weight = (-x * x).exp();
            // premultiplied, so a seam to a vertex without color doesn't fade the color towards white
            let [r, g, b, a] = linear[k];
            for (c, value) in [r, g, b].into_iter().enumerate() {
                sum[c] += value * a * weight;
            }
            sum[3] += a * weight;
            total += weight;
        }
        out.push(if sum[3] > 0.0 {
            pack_linear([sum[0] / sum[3], sum[1] / sum[3], sum[2] / sum[3], sum[3] / total])
        } else {
            NO_COLOR
        });
    }
    out
}

/// Per segment of the paths, in the order [`paths_mesh`] makes them, whether it is a strand that
/// keeps its own tube where it leaves the tissue. Every body path is split into runs of points
/// whose tube the tissue holds and runs of points whose tube sticks out of it, see
/// [`TissueField::holds`]:
///
/// - a run outside that the grown tissue covers completely is a small feature next to a large
///   one, the tissue absorbs it
/// - any other run outside is a strand, from the last point inside to the next one, so it grows
///   out of the tissue
///
/// A path outside the tissue all along, such as a bundle too thin for the blur, is one run. So is
/// a lone path a little thicker than the blur, which wears its tissue down to a core around its
/// axis: it keeps its tube.
fn strands(paths: &[MeshPath], field: &TissueField, count: usize) -> Vec<bool> {
    let mut strand = Vec::with_capacity(count);
    for path in paths.iter().filter(|p| !p.is_empty()) {
        let first = strand.len();
        let n = path.len();
        strand.extend(std::iter::repeat_n(false, path_segments(&path.points).len()));
        let along = |i: usize| path.point((i + 1).min(n - 1)) - path.point(i.saturating_sub(1));
        let inside: Vec<bool> = (0..n).map(|i| field.holds(path.point(i), along(i), path.points[i * 4 + 3].abs())).collect();
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
/// hypha it belongs to, then recomputes the normals so the shading shows it.
/// `owner` gives the segment of a vertex, its style and how strong its relief is
/// relative to `fibres`.
fn add_fibres(
    mesh: &mut Mesh,
    segments: &[Segment],
    owner: impl Fn(usize) -> (usize, PathStyle, f32),
    cell: f32,
    fibres: f32,
    seed: u32,
) {
    let n = mesh.vertex_count();
    let mut direction = Vec::with_capacity(n);
    let mut strength = Vec::with_capacity(n);
    let mut stretch = Vec::with_capacity(n);
    for v in 0..n {
        let (segment, style, scale) = owner(v);
        let s = &segments[segment];
        // a path of one point is a ball, it gets its direction from its neighbours
        direction.push((s.end - s.start).normalize_or_zero());
        strength.push(style.fibre_strength * scale);
        stretch.push(style.fibre_stretch);
    }

    // neighbouring vertices of different hyphae would get unrelated noise, smoothing hides the seams, and where paths
    // of different fibres meet the relief goes over from one strength to the other over a few rings of vertices
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
    let depth = fibres * FIBRE_DEPTH * wavelength;
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
        let offset = normal * (value_noise(q, seed) * depth * strength[v]);
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
    use crate::color::opaque;
    use crate::density::{assert_closed, components, surface};
    use crate::paths::PathGroup;
    use crate::settings::PathSettings;
    use std::borrow::Cow;
    use std::f32::consts::PI;

    const STIPE: i32 = 1;
    const CUTICLE: i32 = 2;
    const GILL: i32 = 3;
    const VEIL: i32 = 4;

    fn strand(paths: &mut Vec<(Vec<f32>, i32)>, from: Vec3, to: Vec3, steps: usize, creator: i32) {
        let points = (0..=steps).flat_map(|i| from.lerp(to, i as f32 / steps as f32).extend(0.02).to_array()).collect();
        paths.push((points, creator));
    }

    /// A stipe bundle from 0 to 1, a cap cuticle disc on top of it, gills under the cap and a
    /// ring around the stipe, each made by its own node
    fn bundle() -> Vec<(Vec<f32>, i32)> {
        let mut paths = Vec::new();
        for i in 0..8 {
            let a = i as f32 / 8.0 * 2.0 * PI;
            let foot = Vec3::new(a.cos(), 0.0, a.sin()) * 0.06;
            strand(&mut paths, foot, foot + Vec3::Y, 10, STIPE);
        }
        for i in 0..16 {
            let a = (i as f32 + 0.5) / 16.0 * 2.0 * PI;
            let out = Vec3::new(a.cos(), 0.0, a.sin());
            for r in 0..6 {
                let (inner, outer) = (r as f32 * 0.1, (r + 1) as f32 * 0.1);
                strand(&mut paths, out * inner + Vec3::Y * 1.1, out * outer + Vec3::Y * 1.1, 1, CUTICLE);
            }
            strand(&mut paths, out * 0.2 + Vec3::Y * 0.98, out * 0.55 + Vec3::Y * 0.98, 4, GILL);
        }
        for i in 0..16 {
            let a = i as f32 / 16.0 * 2.0 * PI;
            let out = Vec3::new(a.cos(), 0.0, a.sin());
            strand(&mut paths, out * 0.09 + Vec3::Y * 0.6, out * 0.14 + Vec3::Y * 0.6, 2, VEIL);
        }
        paths
    }

    fn bundle_mesh(params: &HyphaeMeshParams) -> Mesh {
        let points = bundle();
        let mut paths = mesh_paths(&points, PathStyle::BODY);
        for path in paths.iter_mut().filter(|p| matches!(p.group, PathGroup::Creator(GILL | VEIL))) {
            path.settings.fill = Some(0.0);
            path.settings.fibres = Some(0.1 * params.fibres);
        }
        paths_mesh(&paths, params)
    }

    /// Paths that keep their gaps open: a fill of 0 blends them like any other paths
    fn open(mut paths: Vec<MeshPath>) -> Vec<MeshPath> {
        for p in &mut paths {
            p.settings.fill = Some(0.0);
        }
        paths
    }

    fn vertex(mesh: &Mesh, v: usize) -> Vec3 {
        Vec3::from_slice(&mesh.positions[v * 3..v * 3 + 3])
    }

    fn smooth() -> HyphaeMeshParams {
        HyphaeMeshParams { fibres: 0.0, ..HyphaeMeshParams::default() }
    }

    #[test]
    fn the_mesh_is_closed() {
        assert_closed(&bundle_mesh(&smooth()));
        assert_closed(&bundle_mesh(&HyphaeMeshParams::default()));
    }

    #[test]
    fn groups_match_the_creators() {
        let mesh = bundle_mesh(&smooth());
        assert!(mesh.colors.is_empty());
        let mut seen = [false; 4];
        for v in 0..mesh.vertex_count() {
            let p = vertex(&mesh, v);
            let radial = Vec3::new(p.x, 0.0, p.z).length();
            let group = mesh.groups[v];
            if p.y < 0.5 && p.y > 0.05 {
                assert_eq!(group, (STIPE - 1) as u32, "stipe vertex at {}", p);
                seen[0] = true;
            } else if p.y > 1.12 && radial < 0.5 {
                assert_eq!(group, (CUTICLE - 1) as u32, "cuticle vertex at {}", p);
                seen[1] = true;
            } else if (0.9..0.97).contains(&p.y) && (0.3..0.5).contains(&radial) {
                assert_eq!(group, (GILL - 1) as u32, "gill vertex at {}", p);
                seen[2] = true;
            } else if (0.55..0.65).contains(&p.y) && radial > 0.12 {
                assert_eq!(group, (VEIL - 1) as u32, "ring vertex at {}", p);
                seen[3] = true;
            }
        }
        assert_eq!(seen, [true; 4]);
    }

    #[test]
    fn the_stipe_relief_runs_along_the_paths() {
        let stipe = stipe_points();
        let plain = paths_mesh(&mesh_paths(&stipe, PathStyle::BODY), &smooth());
        let fibrous = paths_mesh(&mesh_paths(&stipe, PathStyle::BODY), &HyphaeMeshParams::default());
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
        let plain = bundle_mesh(&smooth());
        let fibrous = bundle_mesh(&HyphaeMeshParams::default());
        assert_eq!(plain.faces, fibrous.faces);
        assert_eq!(plain.groups, fibrous.groups);
        let d = HyphaeMeshParams::default();
        let segments: Vec<Segment> = bundle().iter().flat_map(|(p, _)| path_segments(p)).collect();
        let cell = surface(&segments, d.resolution, d.blend, d.fill * 0.02, |_| true, |_| false, |_| 1.0).cell;
        let moved: Vec<f32> =
            (0..plain.vertex_count()).map(|v| (vertex(&plain, v) - vertex(&fibrous, v)).length()).collect();
        assert!(moved.iter().any(|&d| d > 0.0));
        assert!(moved.iter().all(|&d| d <= FIBRE_DEPTH * FIBRE_WAVELENGTH_CELLS * cell + 1e-6));
    }

    #[test]
    fn same_input_gives_the_same_output() {
        let params = HyphaeMeshParams { seed: 7, ..HyphaeMeshParams::default() };
        assert_eq!(bundle_mesh(&params).encode(), bundle_mesh(&params).encode());
        let other = HyphaeMeshParams { seed: 8, ..params.clone() };
        assert_ne!(bundle_mesh(&params).positions, bundle_mesh(&other).positions);
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
            .map(|(p, creator)| MeshPath {
                points: Cow::Borrowed(p),
                group: PathGroup::Creator(*creator),
                style,
                settings: PathSettings::default(),
            })
            .collect()
    }

    #[test]
    fn paths_become_one_closed_tissue_in_the_groups_of_their_creators() {
        let points = stipe_points();
        let mesh = paths_mesh(&mesh_paths(&points, PathStyle::BODY), &HyphaeMeshParams::default());
        assert_closed(&mesh);
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
        let loose = paths_mesh(&open(mesh_paths(&points, PathStyle::BODY)), &smooth());
        let inside = (0..loose.vertex_count()).any(|v| {
            let p = vertex(&loose, v);
            (0.2..0.8).contains(&p.y) && Vec3::new(p.x, 0.0, p.z).length() < 0.14
        });
        assert!(inside, "the gaps between loose paths stay open, their inner side shows");
    }

    #[test]
    fn the_same_paths_give_the_same_mesh() {
        let points = stipe_points();
        let params = HyphaeMeshParams { seed: 3, ..HyphaeMeshParams::default() };
        let a = paths_mesh(&mesh_paths(&points, PathStyle::BODY), &params).encode();
        assert_eq!(a, paths_mesh(&mesh_paths(&points, PathStyle::BODY), &params).encode());
    }

    #[test]
    fn no_paths_give_an_empty_mesh() {
        assert_eq!(paths_mesh(&[], &HyphaeMeshParams::default()), Mesh::default());
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
            let mesh = paths_mesh(&open(mesh_paths(&[(path.clone(), 1)], PathStyle::BODY)), &params);
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
            let mesh = paths_mesh(&open(mesh_paths(paths, PathStyle::BODY)), &smooth());
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
        let apart = paths_mesh(&open(mesh_paths(&[([line(Vec3::ZERO, Vec3::Y, 11, r), far].concat(), 1)], PathStyle::BODY)), &smooth());
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

    /// Upright paths of radius `r` 40 mm long that fill a disc 5 mm wide, in rings about 0.9 mm apart
    fn thin_bundle(r: f32) -> Vec<(Vec<f32>, i32)> {
        let mut feet = vec![Vec3::ZERO];
        for ring in 1..=5 {
            let radius = ring as f32 * 0.001;
            let around = (2.0 * PI * radius / 0.0009).round() as usize;
            feet.extend((0..around).map(|i| {
                let a = (i as f32 + 0.5 * ring as f32) / around as f32 * 2.0 * PI;
                Vec3::new(a.cos(), 0.0, a.sin()) * radius
            }));
        }
        feet.into_iter().map(|at| (line(at, at + Vec3::Y * 0.04, 11, r), 1)).collect()
    }

    /// Mean of the outermost distance from the axis per direction and height, in the middle of the length
    fn outer_radius(mesh: &Mesh) -> f32 {
        let mut outer = [[0.0f32; 48]; 8];
        for v in 0..mesh.vertex_count() {
            let p = vertex(mesh, v);
            let along = (p.y - 0.012) / 0.016;
            if (0.0..1.0).contains(&along) {
                let around = ((p.z.atan2(p.x) / (2.0 * PI) + 0.5) * 48.0) as usize % 48;
                let bin = &mut outer[(along * 8.0) as usize][around];
                *bin = bin.max(Vec3::new(p.x, 0.0, p.z).length());
            }
        }
        let all: Vec<f32> = outer.iter().flatten().copied().filter(|&r| r > 0.0).collect();
        all.iter().sum::<f32>() / all.len() as f32
    }

    #[test]
    fn a_bundle_of_thin_paths_keeps_its_size_at_any_resolution() {
        // thinner than 0.75 cells below 128, as tissue and blended: the cells used to swell it by up to 2 mm at 20
        for (r, fill) in [(0.0003, 1.5), (0.0005, 0.0)] {
            let points = thin_bundle(r);
            let radii: Vec<f32> = [256, 128, 64, 32, 20]
                .into_iter()
                .map(|resolution| outer_radius(&mesh_body(&points, &HyphaeMeshParams { resolution, fill, ..loose(0.2) })))
                .collect();
            for radius in &radii[1..] {
                assert!((radius - radii[0]).abs() < 0.00015, "fill {}: {:?} from 256 down to 20", fill, radii);
            }
            let grows = radii.windows(2).all(|w| w[1] > w[0]) && radii[4] - radii[0] > 0.00005;
            assert!(!grows, "fill {}: grows as the cells grow {:?}", fill, radii);
        }
    }

    /// Mean distance from the y axis of the vertices in the middle 40 % of a path from 0 to 40 mm up
    fn mean_radius(mesh: &Mesh) -> f32 {
        let around: Vec<f32> = (0..mesh.vertex_count())
            .map(|v| vertex(mesh, v))
            .filter(|p| (0.012..0.028).contains(&p.y))
            .map(|p| Vec3::new(p.x, 0.0, p.z).length())
            .collect();
        around.iter().sum::<f32>() / around.len() as f32
    }

    #[test]
    fn a_lone_path_keeps_its_radius_at_any_fill() {
        // the blur of fill 1.5 wore the tissue of the path down to a core of 1.6 mm around its axis
        let r = 0.003;
        let points = [(line(Vec3::ZERO, Vec3::Y * 0.04, 11, r), 1)];
        for (fill, loose_strands) in [(0.1, 0.2), (0.5, 0.2), (1.5, 0.2), (1.5, 0.0), (6.0, 0.2)] {
            let radii: Vec<f32> = [128, 64, 32]
                .into_iter()
                .map(|resolution| mean_radius(&mesh_body(&points, &HyphaeMeshParams { resolution, fill, ..loose(loose_strands) })))
                .collect();
            // the blur rounds a tissue out past its outer hyphae, a lone path by up to 0.14 r, and at 32 the path is
            // little more than a cell thick
            for (radius, least) in radii.iter().zip([0.95, 0.95, 0.85]) {
                assert!(
                    (least * r..1.15 * r).contains(radius),
                    "fill {} loose_strands {}: {:?} at 128, 64 and 32",
                    fill,
                    loose_strands,
                    radii
                );
            }
        }
    }

    #[test]
    fn a_thin_open_strand_stays_at_the_lowest_resolution() {
        // a lone blended path of 0.2 mm, the cells are 2.6 mm
        let points = [(line(Vec3::ZERO, Vec3::Y * 0.04, 11, 0.0002), 1)];
        let mesh = paths_mesh(&open(mesh_paths(&points, PathStyle::BODY)), &HyphaeMeshParams { resolution: 16, ..smooth() });
        assert_closed(&mesh);
        assert_eq!(components(&mesh), 1);
        assert!(highest(&mesh) > 0.04, "strand ends at {}", highest(&mesh));
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

    /// The paths of `points` in `style`, those of `creator` with `settings`
    fn with_settings<'a>(
        points: &'a [(Vec<f32>, i32)],
        style: PathStyle,
        creator: i32,
        settings: PathSettings,
    ) -> Vec<MeshPath<'a>> {
        let mut paths = mesh_paths(points, style);
        for p in paths.iter_mut().filter(|p| p.group == PathGroup::Creator(creator)) {
            p.settings = settings;
        }
        paths
    }

    fn set(fibres: f32, fill: f32) -> PathSettings {
        PathSettings { fibres: Some(fibres), fill: Some(fill), color: None }
    }

    #[test]
    fn settings_equal_to_the_node_give_the_same_mesh() {
        // a body of two creators and paths around it, every path with the values of the node
        let mut points = stipe_points();
        points.extend((0..4).map(|i| {
            let a = i as f32 * 1.7;
            let at = Vec3::new(a.cos(), 0.0, a.sin()) * 0.3;
            (line(at, at + Vec3::new(0.0, 0.8, 0.1), 9, 0.015), 50)
        }));
        let paths = |settings: PathSettings| {
            let mut paths = mesh_paths(&points, PathStyle::BODY);
            for p in &mut paths {
                p.settings = settings;
            }
            paths
        };
        for params in [HyphaeMeshParams::default(), HyphaeMeshParams { blend: 2.0, fill: 1.5, fibres: 0.6, seed: 4, ..HyphaeMeshParams::default() }] {
            let plain = paths_mesh(&paths(PathSettings::default()), &params);
            let same = paths_mesh(&paths(set(params.fibres, params.fill)), &params);
            assert!(plain.face_count() > 0);
            assert_eq!(plain.encode(), same.encode());
        }
    }

    /// Two slabs of body paths along z side by side, at x < 0 made by node 1 and at x > 0 by node 2.
    /// Further apart from each other than within each, the closing joins them into one tissue.
    fn two_slabs() -> Vec<(Vec<f32>, i32)> {
        let mut points = Vec::new();
        for x in [-0.25, -0.19, -0.13, -0.07, 0.07, 0.13, 0.19, 0.25] {
            for y in [-0.12, -0.06, 0.0] {
                points.push((line(Vec3::new(x, y, -0.5), Vec3::new(x, y, 0.5), 11, 0.02), if x < 0.0 { 1 } else { 2 }));
            }
        }
        points
    }

    #[test]
    fn fibres_of_a_node_stay_on_its_surface() {
        let points = two_slabs();
        let params = HyphaeMeshParams { resolution: 96, fill: 6.0, fibres: 1.5, ..HyphaeMeshParams::default() };
        let smooth_mesh = paths_mesh(&mesh_paths(&points, PathStyle::BODY), &HyphaeMeshParams { fibres: 0.0, ..params.clone() });
        let uniform = paths_mesh(&mesh_paths(&points, PathStyle::BODY), &params);
        let mesh = paths_mesh(&with_settings(&points, PathStyle::BODY, 1, PathSettings { fibres: Some(0.0), ..PathSettings::default() }), &params);
        assert_eq!(mesh.faces, smooth_mesh.faces);
        assert_closed(&mesh);
        let cell = surface(&path_segments(&points[0].0), 96, 3.0, 0.0, |_| false, |_| false, |_| 1.0).cell;
        // the relief goes over from 0 to 1.5 within the smoothing of the relief, a few rings of vertices
        let seam = SMOOTHING_PASSES as f32 * 2.0 * cell;
        // the noise is the same as with 1.5 everywhere, only its strength changes: per bin across the seam the share of it
        let mut bins = [(0.0f32, 0usize); 8];
        for v in 0..mesh.vertex_count() {
            let before = vertex(&smooth_mesh, v);
            let (moved, full) = ((vertex(&mesh, v) - before).length(), (vertex(&uniform, v) - before).length());
            // the top of the slabs, away from their ends
            if before.y < 0.0 || before.z.abs() > 0.4 {
                continue;
            }
            if before.x < -seam {
                assert_eq!(moved, 0.0, "vertex at {} moved", before);
            }
            assert!(moved <= full + 1e-7, "vertex at {} moved more than with 1.5 everywhere", before);
            if before.x.abs() < 0.2 && full > 1e-6 {
                let bin = (((before.x + 0.2) / 0.05) as usize).min(7);
                bins[bin].0 += moved / full;
                bins[bin].1 += 1;
            }
        }
        let share: Vec<f32> = bins.iter().map(|(sum, n)| sum / *n.max(&1) as f32).collect();
        assert!(share[0] < 0.01 && share[7] > 0.9, "relief across the seam {:?}", share);
        for pair in share.windows(2) {
            assert!(pair[1] >= pair[0], "relief across the seam {:?}", share);
        }
    }

    /// Of a 3 x 3 bundle of upright paths `spacing` apart around `x`, made by `creator`
    fn bundle_at(x: f32, spacing: f32, creator: i32) -> Vec<(Vec<f32>, i32)> {
        (0..9)
            .map(|i| {
                let foot = Vec3::new(x + ((i % 3) as f32 - 1.0) * spacing, 0.0, ((i / 3) as f32 - 1.0) * spacing);
                (line(foot, foot + Vec3::Y, 11, 0.03), creator)
            })
            .collect()
    }

    #[test]
    fn every_node_fills_the_gaps_by_its_own_fill() {
        // the gaps between the hyphae are 0.09 wide: a ball of fill 1 (0.03) passes, one of fill 6 (0.18) doesn't
        let spacing = 0.12;
        let mut points = bundle_at(-0.4, spacing, 1);
        points.extend(bundle_at(0.4, spacing, 2));
        let inner = |mesh: &Mesh, x: f32| {
            (0..mesh.vertex_count())
                .map(|v| vertex(mesh, v))
                .filter(|p| (0.2..0.8).contains(&p.y) && (p.x - x).abs() < 0.5 * spacing && p.z.abs() < 0.5 * spacing)
                .count()
        };
        let params = HyphaeMeshParams { resolution: 96, fill: 6.0, ..smooth() };
        let mesh = paths_mesh(&with_settings(&points, PathStyle::BODY, 1, PathSettings { fill: Some(1.0), ..PathSettings::default() }), &params);
        assert_closed(&mesh);
        assert!(inner(&mesh, -0.4) > 0, "the bundle of fill 1 keeps its inner faces");
        assert_eq!(inner(&mesh, 0.4), 0, "the bundle of fill 6 has none");
        // the same as when the whole mesh has its fill
        let alone = paths_mesh(&mesh_paths(&points, PathStyle::BODY), &HyphaeMeshParams { fill: 1.0, ..params.clone() });
        assert!(inner(&alone, 0.4) > 0);
    }

    #[test]
    fn a_tissue_of_a_larger_fill_is_as_smooth_as_alone() {
        // the slabs of fill 1.5 and 6: away from the seam each top is as high as with its fill alone
        let points = two_slabs();
        let params = HyphaeMeshParams { resolution: 96, fill: 1.5, ..smooth() };
        let top = |mesh: &Mesh, x: f32| {
            (0..mesh.vertex_count())
                .map(|v| vertex(mesh, v))
                .filter(|p| (p.x - x).abs() < 0.02 && p.z.abs() < 0.3)
                .map(|p| p.y)
                .fold(f32::NEG_INFINITY, f32::max)
        };
        let mixed = paths_mesh(&with_settings(&points, PathStyle::BODY, 2, PathSettings { fill: Some(6.0), ..PathSettings::default() }), &params);
        let low = paths_mesh(&mesh_paths(&points, PathStyle::BODY), &params);
        let high = paths_mesh(&mesh_paths(&points, PathStyle::BODY), &HyphaeMeshParams { fill: 6.0, ..params.clone() });
        let cell = surface(&path_segments(&points[0].0), 96, 3.0, 0.0, |_| false, |_| false, |_| 1.0).cell;
        assert!((top(&mixed, 0.25) - top(&high, 0.25)).abs() < 2.0 * cell, "{} vs {}", top(&mixed, 0.25), top(&high, 0.25));
        assert!((top(&mixed, -0.25) - top(&low, -0.25)).abs() < 2.0 * cell, "{} vs {}", top(&mixed, -0.25), top(&low, -0.25));
    }

    fn colored(points: &[(Vec<f32>, i32)], colors: &[(i32, [u8; 3])]) -> Vec<MeshPath<'static>> {
        mesh_paths(points, PathStyle::BODY)
            .into_iter()
            .map(|p| {
                let color = colors.iter().find(|(creator, _)| p.group == PathGroup::Creator(*creator)).map(|(_, c)| opaque(*c));
                MeshPath { points: std::borrow::Cow::Owned(p.points.into_owned()), settings: PathSettings { color, ..p.settings }, ..p }
            })
            .collect()
    }

    const RED: [u8; 3] = [220, 40, 30];
    const BLUE: [u8; 3] = [30, 60, 210];

    #[test]
    fn paths_without_a_color_keep_the_material() {
        let points = two_slabs();
        let params = HyphaeMeshParams { resolution: 96, fill: 6.0, ..HyphaeMeshParams::default() };
        let plain = paths_mesh(&mesh_paths(&points, PathStyle::BODY), &params);
        assert!(plain.colors.is_empty());
        let mesh = paths_mesh(&colored(&points, &[(2, RED)]), &params);
        // the same surface, only node 2 has a color
        assert_eq!(mesh.faces, plain.faces);
        assert_eq!(mesh.positions, plain.positions);
        assert_eq!(mesh.groups, plain.groups);
        for v in 0..mesh.vertex_count() {
            let x = vertex(&mesh, v).x;
            if x < -0.2 {
                assert_eq!(mesh.colors[v], NO_COLOR, "vertex at x {}", x);
            } else if x > 0.2 {
                assert_eq!(mesh.colors[v], opaque(RED), "vertex at x {}", x);
            }
        }
    }

    #[test]
    fn a_color_stays_on_its_group_and_goes_over_smoothly_at_the_seam() {
        let points = two_slabs();
        let params = HyphaeMeshParams { resolution: 96, fill: 6.0, ..HyphaeMeshParams::default() };
        let mesh = paths_mesh(&colored(&points, &[(1, RED), (2, BLUE)]), &params);
        assert_closed(&mesh);
        let [r0, _, b0, _] = linear_rgba(opaque(RED));
        let [r1, _, b1, _] = linear_rgba(opaque(BLUE));
        // red and blue mix along a line, how far along it the color is
        let blue = |v: usize| {
            let [r, _, b, a] = linear_rgba(mesh.colors[v]);
            assert_eq!(a, 1.0);
            ((r - r0) * (r1 - r0) + (b - b0) * (b1 - b0)) / ((r1 - r0).powi(2) + (b1 - b0).powi(2))
        };
        let mut bins = [(0.0f32, 0usize); 8];
        for v in 0..mesh.vertex_count() {
            let p = vertex(&mesh, v);
            if p.x < -0.2 {
                assert_eq!(mesh.colors[v], opaque(RED), "vertex at {}", p);
            } else if p.x > 0.2 {
                assert_eq!(mesh.colors[v], opaque(BLUE), "vertex at {}", p);
            }
            if p.y > 0.0 && p.z.abs() < 0.4 && p.x.abs() < 0.24 {
                let bin = (((p.x + 0.24) / 0.06) as usize).min(7);
                bins[bin].0 += blue(v);
                bins[bin].1 += 1;
            }
        }
        let share: Vec<f32> = bins.iter().map(|(sum, n)| sum / *n.max(&1) as f32).collect();
        assert!(share[0] < 0.01 && share[7] > 0.99, "blue across the seam {:?}", share);
        assert!(share.iter().filter(|&&s| s > 0.05 && s < 0.95).count() >= 2, "no hard edge: {:?}", share);
        for pair in share.windows(2) {
            assert!(pair[1] >= pair[0], "blue across the seam {:?}", share);
        }
        for f in mesh.faces.chunks_exact(3) {
            for (i, j) in [(f[0], f[1]), (f[1], f[2]), (f[2], f[0])] {
                let step = (blue(i as usize) - blue(j as usize)).abs();
                assert!(step < 0.2, "the color jumps by {} between {} and {}", step, vertex(&mesh, i as usize), vertex(&mesh, j as usize));
            }
        }
    }

    #[test]
    fn a_thin_path_takes_the_color_of_a_thick_neighbour_near_it() {
        // a thin red path at y 0 and a ten times thicker blue one whose surface is 0.08 above it
        let segments = [
            Segment { start: Vec3::new(-1.0, 0.0, 0.0), end: Vec3::new(1.0, 0.0, 0.0), radius: 0.01 },
            Segment { start: Vec3::new(-1.0, 0.19, 0.0), end: Vec3::new(1.0, 0.19, 0.0), radius: 0.1 },
        ];
        let seam = [0.01, 0.1];
        let mesh = Mesh { positions: vec![0.0, 0.01, 0.0], ..Mesh::default() };
        let colors = [opaque(RED), opaque(BLUE)];
        let [r, _, b, _] = linear_rgba(blend_colors(&mesh, &[0], &segments, &colors, &seam)[0]);
        let ([r0, _, b0, _], [r1, _, b1, _]) = (linear_rgba(colors[0]), linear_rgba(colors[1]));
        let blue = ((r - r0) * (r1 - r0) + (b - b0) * (b1 - b0)) / ((r1 - r0).powi(2) + (b1 - b0).powi(2));
        // 0.08 farther than the red tube with a seam of 0.055: exp(-(0.08 / 0.055)²) of blue
        assert!(blue > 0.05 && blue < 0.5, "blue share {}", blue);
    }

    #[test]
    fn a_seam_to_paths_without_a_color_fades_the_color_not_its_hue() {
        let points = two_slabs();
        let params = HyphaeMeshParams { resolution: 96, fill: 6.0, ..HyphaeMeshParams::default() };
        let mesh = paths_mesh(&colored(&points, &[(2, RED)]), &params);
        let red = linear_rgba(opaque(RED));
        let mut faded = 0;
        for v in 0..mesh.vertex_count() {
            let [r, g, b, a] = linear_rgba(mesh.colors[v]);
            if a > 0.0 {
                for (c, want) in [r, g, b].into_iter().zip(red) {
                    assert!((c - want).abs() < 0.02, "vertex at {} has {:?}", vertex(&mesh, v), [r, g, b, a]);
                }
            }
            if a > 0.05 && a < 0.95 {
                faded += 1;
            }
        }
        assert!(faded > 10, "only {} vertices at the seam", faded);
    }
}
