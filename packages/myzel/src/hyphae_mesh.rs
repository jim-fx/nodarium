use glam::Vec3;

use crate::density::{surface, Segment};
use crate::hyphae::{Hyphae, Tissue};
use crate::mesh::{pack_rgba, Mesh};
use crate::noise::value_noise;

/// sRGB colors with components in [0, 1]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TissueColors {
    /// The youngest cap cuticle
    pub cuticle_young: [f32; 3],
    /// The oldest cap cuticle
    pub cuticle_old: [f32; 3],
    /// Gills at `t = 0`
    pub gill_young: [f32; 3],
    /// Gills at `t = 1`, when the spores are ripe
    pub gill_mature: [f32; 3],
    pub stipe: [f32; 3],
    /// Veil and the ring it leaves on the stipe
    pub veil: [f32; 3],
    /// Cap flesh and the mycelium at the base
    pub flesh: [f32; 3],
}

const fn rgb(r: u8, g: u8, b: u8) -> [f32; 3] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0]
}

impl Default for TissueColors {
    /// A champignon (Agaricus bisporus)
    fn default() -> Self {
        TissueColors {
            cuticle_young: rgb(0xf4, 0xef, 0xe6),
            cuticle_old: rgb(0xc4, 0xa0, 0x76),
            gill_young: rgb(0xe2, 0xa4, 0xa0),
            gill_mature: rgb(0x5c, 0x3e, 0x30),
            stipe: rgb(0xf2, 0xee, 0xe6),
            veil: rgb(0xf8, 0xf6, 0xf0),
            flesh: rgb(0xec, 0xe6, 0xdc),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct HyphaeMeshParams {
    /// Cells along the longest side of the grid
    pub resolution: usize,
    /// How far, in hypha radii, neighbouring hyphae join into one surface
    pub blend: f32,
    /// Maturity from 0 to 1, browns the gills
    pub t: f32,
    /// Strength of the fibrous relief, 0 leaves the surface smooth
    pub fibres: f32,
    pub seed: u32,
    pub colors: TissueColors,
}

impl Default for HyphaeMeshParams {
    fn default() -> Self {
        HyphaeMeshParams { resolution: 64, blend: 3.0, t: 0.0, fibres: 1.0, seed: 0, colors: TissueColors::default() }
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

/// Noise wavelength across the fibres, in cells, smaller relief can't be shown by the mesh
const FIBRE_WAVELENGTH_CELLS: f32 = 2.5;
/// Relief depth relative to the wavelength
const FIBRE_DEPTH: f32 = 0.18;
const SMOOTHING_PASSES: usize = 2;
/// The cuticle is a single layer of hyphae over the cap flesh, so flesh hyphae
/// just below it often reach the surface vertices more strongly. The surface of
/// a cap is cuticle wherever a cuticle hypha comes close.
const CUTICLE_PRIORITY: f32 = 4.0;

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    [0, 1, 2].map(|i| a[i] + (b[i] - a[i]) * t)
}

/// The surface around the hyphae with one tissue color per vertex and a fibrous relief
pub fn hyphae_mesh(hyphae: &Hyphae, params: &HyphaeMeshParams) -> Mesh {
    let segments = hyphae.to_segments();
    let mut s = surface(&segments, params.resolution, params.blend, |i| match hyphae.segments[i].tissue {
        Tissue::CapCuticle => CUTICLE_PRIORITY,
        _ => 1.0,
    });
    if s.mesh.vertex_count() == 0 {
        return s.mesh;
    }
    let owner = |v: usize| &hyphae.segments[s.owners[v] as usize];

    let (oldest, youngest) = hyphae
        .segments
        .iter()
        .filter(|h| h.tissue == Tissue::CapCuticle)
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), h| (lo.min(h.birth), hi.max(h.birth)));
    let c = &params.colors;
    s.mesh.colors = (0..s.mesh.vertex_count())
        .map(|v| {
            let h = owner(v);
            let [r, g, b] = match h.tissue {
                Tissue::CapCuticle => {
                    let age = if youngest > oldest { (youngest - h.birth) / (youngest - oldest) } else { 0.0 };
                    mix(c.cuticle_young, c.cuticle_old, age)
                }
                Tissue::Gill => mix(c.gill_young, c.gill_mature, params.t),
                Tissue::Stipe => c.stipe,
                Tissue::Veil => c.veil,
                Tissue::Cap | Tissue::Mycelium => c.flesh,
            };
            pack_rgba(r, g, b, 1.0)
        })
        .collect();

    if params.fibres > 0.0 {
        add_fibres(&mut s.mesh, &segments, |v| (s.owners[v] as usize, owner(v).tissue), s.cell, params);
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

    fn color_of(c: [f32; 3]) -> u32 {
        pack_rgba(c[0], c[1], c[2], 1.0)
    }

    fn vertex(mesh: &Mesh, v: usize) -> Vec3 {
        Vec3::from_slice(&mesh.positions[v * 3..v * 3 + 3])
    }

    fn smooth(t: f32) -> HyphaeMeshParams {
        HyphaeMeshParams { t, fibres: 0.0, ..HyphaeMeshParams::default() }
    }

    #[test]
    fn the_mesh_is_closed() {
        assert_closed(&hyphae_mesh(&bundle(), &smooth(0.0)));
        assert_closed(&hyphae_mesh(&bundle(), &HyphaeMeshParams::default()));
    }

    #[test]
    fn colors_match_the_tissues() {
        let c = TissueColors::default();
        let mesh = hyphae_mesh(&bundle(), &smooth(0.0));
        let mut seen = [false; 4];
        for v in 0..mesh.vertex_count() {
            let p = vertex(&mesh, v);
            let radial = Vec3::new(p.x, 0.0, p.z).length();
            let color = mesh.colors[v];
            if p.y < 0.5 && p.y > 0.05 {
                assert_eq!(color, color_of(c.stipe), "stipe vertex at {}", p);
                seen[0] = true;
            } else if p.y > 1.15 && radial < 0.5 {
                let young = Vec3::from(c.cuticle_young);
                let old = Vec3::from(c.cuticle_old);
                let [r, g, b, _] = color.to_le_bytes().map(|b| b as f32 / 255.0);
                let along = (Vec3::new(r, g, b) - young).dot(old - young) / (old - young).length_squared();
                // the middle of the cap is the oldest part, so it is darker
                assert!((along - (1.0 - radial / 0.6)).abs() < 0.3, "cuticle at {} is {}", radial, along);
                seen[1] = true;
            } else if (0.85..0.95).contains(&p.y) && (0.3..0.5).contains(&radial) {
                assert_eq!(color, color_of(c.gill_young), "gill vertex at {}", p);
                seen[2] = true;
            } else if (0.55..0.65).contains(&p.y) && radial > 0.12 {
                assert_eq!(color, color_of(c.veil), "ring vertex at {}", p);
                seen[3] = true;
            }
        }
        assert_eq!(seen, [true; 4]);
    }

    #[test]
    fn gills_brown_as_they_mature() {
        let c = TissueColors::default();
        let (young, mature) = (hyphae_mesh(&bundle(), &smooth(0.0)), hyphae_mesh(&bundle(), &smooth(1.0)));
        let colored = |mesh: &Mesh, color: [f32; 3]| {
            (0..mesh.vertex_count()).filter(|&v| mesh.colors[v] == color_of(color)).collect::<Vec<_>>()
        };
        let gills = colored(&young, c.gill_young);
        assert!(!gills.is_empty());
        assert_eq!(colored(&mature, c.gill_mature), gills);
        for v in (0..young.vertex_count()).filter(|v| !gills.contains(v)) {
            assert_eq!(young.colors[v], mature.colors[v]);
        }
    }

    #[test]
    fn fibres_move_vertices_a_little() {
        let plain = hyphae_mesh(&bundle(), &smooth(0.0));
        let fibrous = hyphae_mesh(&bundle(), &HyphaeMeshParams::default());
        assert_eq!(plain.faces, fibrous.faces);
        assert_eq!(plain.colors, fibrous.colors);
        let cell = surface(&bundle().to_segments(), 64, 3.0, |_| 1.0).cell;
        let moved: Vec<f32> =
            (0..plain.vertex_count()).map(|v| (vertex(&plain, v) - vertex(&fibrous, v)).length()).collect();
        assert!(moved.iter().any(|&d| d > 0.0));
        assert!(moved.iter().all(|&d| d <= FIBRE_DEPTH * FIBRE_WAVELENGTH_CELLS * cell + 1e-6));
    }

    #[test]
    fn same_input_gives_the_same_output() {
        let params = HyphaeMeshParams { t: 0.4, seed: 7, ..HyphaeMeshParams::default() };
        assert_eq!(hyphae_mesh(&bundle(), &params).encode(), hyphae_mesh(&bundle(), &params).encode());
        let other = HyphaeMeshParams { seed: 8, ..params.clone() };
        assert_ne!(hyphae_mesh(&bundle(), &params).positions, hyphae_mesh(&bundle(), &other).positions);
    }

    #[test]
    fn no_hyphae_give_an_empty_mesh() {
        assert_eq!(hyphae_mesh(&Hyphae::default(), &HyphaeMeshParams::default()), Mesh::default());
    }
}
