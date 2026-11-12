//! Thin closed sheets, like gill plates and veils: two faces over the same grid, joined along
//! their border. They stay sharp at any mesh resolution, where paths need to be a few field
//! cells thick to mesh at all.

use crate::Mesh;
use glam::Vec3;

/// The grid of a sheet: `columns` across it, `rows` from its first edge (row 0, e.g. where it is
/// attached) to its last (e.g. a gill or torn edge).
#[derive(Clone, Copy, Debug)]
pub struct SheetGrid {
    pub columns: usize,
    pub rows: usize,
    /// The last column joins the first one: a band that goes around, without side walls
    pub wrap: bool,
}

/// Adds a thin closed sheet over `grid`. `front(column, row)` and `back(column, row)` are the
/// two faces. The front must lie on the side the grid's `column × row` directions point to,
/// then every face looks out of the sheet. With `edge`, the last rows of both faces join through
/// one more vertex per column, which rounds the last edge; without it they join directly.
///
/// The vertices are the front column by column, then the back, then the edge, and every vertex is
/// in group 0 with a zero normal, see [`smooth_normals`].
pub fn add_sheet(
    mesh: &mut Mesh,
    grid: SheetGrid,
    front: impl Fn(usize, usize) -> Vec3,
    back: impl Fn(usize, usize) -> Vec3,
    edge: Option<&dyn Fn(usize) -> Vec3>,
) {
    let SheetGrid { columns: m, rows, wrap } = grid;
    if m < 2 || rows < 2 {
        return;
    }
    let base = mesh.vertex_count() as u32;
    let mut push = |p: Vec3| {
        mesh.positions.extend([p.x, p.y, p.z]);
        mesh.normals.extend([0.0; 3]);
        mesh.groups.push(0);
    };
    for side in [&front as &dyn Fn(usize, usize) -> Vec3, &back] {
        for i in 0..m {
            for k in 0..rows {
                push(side(i, k));
            }
        }
    }
    if let Some(edge) = edge {
        for i in 0..m {
            push(edge(i));
        }
    }
    let a = |i: usize, k: usize| base + (i * rows + k) as u32;
    let b = |i: usize, k: usize| base + ((m + i) * rows + k) as u32;
    let e = |i: usize| base + (2 * m * rows + i) as u32;
    let faces = &mut mesh.faces;
    let mut quad = |p: u32, q: u32, r: u32, t: u32| faces.extend([p, q, r, p, r, t]);
    let last = rows - 1;
    let spans = if wrap { m } else { m - 1 };
    for i in 0..spans {
        let j = (i + 1) % m;
        for k in 0..last {
            quad(a(i, k), a(j, k), a(j, k + 1), a(i, k + 1));
            quad(b(i, k), b(i, k + 1), b(j, k + 1), b(j, k));
        }
        // the first edge
        quad(a(j, 0), a(i, 0), b(i, 0), b(j, 0));
        // the last edge
        if edge.is_some() {
            quad(a(i, last), a(j, last), e(j), e(i));
            quad(e(i), e(j), b(j, last), b(i, last));
        } else {
            quad(a(i, last), a(j, last), b(j, last), b(i, last));
        }
    }
    if wrap {
        return;
    }
    // the side walls of the first and the last column
    for k in 0..last {
        quad(a(0, k), a(0, k + 1), b(0, k + 1), b(0, k));
        quad(a(m - 1, k), b(m - 1, k), b(m - 1, k + 1), a(m - 1, k + 1));
    }
    if edge.is_some() {
        mesh.faces.extend([a(0, last), e(0), b(0, last)]);
        mesh.faces.extend([a(m - 1, last), b(m - 1, last), e(m - 1)]);
    }
}

/// Vertex normals from the faces around every vertex, weighted by their area. On a thin sheet
/// the faces outweigh its narrow edges, so both faces keep their own side's normal.
pub fn smooth_normals(mesh: &mut Mesh) {
    let v = mesh.vertex_count();
    let Mesh { positions, normals, faces, .. } = mesh;
    let p = |i: u32| Vec3::from_slice(&positions[i as usize * 3..i as usize * 3 + 3]);
    // summed in place, a second list of normals would be one more copy of them in memory
    normals.clear();
    normals.resize(v * 3, 0.0);
    for f in faces.chunks_exact(3) {
        let n = (p(f[1]) - p(f[0])).cross(p(f[2]) - p(f[0]));
        for &i in f {
            let sum = &mut normals[i as usize * 3..i as usize * 3 + 3];
            sum[0] += n.x;
            sum[1] += n.y;
            sum[2] += n.z;
        }
    }
    for normal in normals.chunks_exact_mut(3) {
        normal.copy_from_slice(&Vec3::from_slice(normal).try_normalize().unwrap_or(Vec3::Y).to_array());
    }
}

/// Test helpers for meshes made of sheets
pub mod sheet_checks {
    use crate::Mesh;
    use glam::Vec3;
    use std::collections::HashMap;

    /// Whether every edge belongs to exactly two faces that walk it once each way: the mesh is
    /// closed and its faces are wound consistently. Returns the first edge that isn't.
    pub fn open_edge(mesh: &Mesh) -> Option<(u32, u32)> {
        let mut edges = HashMap::new();
        for f in mesh.faces.chunks_exact(3) {
            for (a, b) in [(f[0], f[1]), (f[1], f[2]), (f[2], f[0])] {
                *edges.entry((a, b)).or_insert(0) += 1;
            }
        }
        edges.iter().find(|(&(a, b), &n)| n != 1 || !edges.contains_key(&(b, a))).map(|(&e, _)| e)
    }

    /// The volume a closed mesh encloses, positive when its faces look outwards
    pub fn volume(mesh: &Mesh) -> f32 {
        let p = |i: u32| Vec3::from_slice(&mesh.positions[i as usize * 3..i as usize * 3 + 3]);
        mesh.faces.chunks_exact(3).map(|f| p(f[0]).dot(p(f[1]).cross(p(f[2])))).sum::<f32>() / 6.0
    }
}

#[cfg(test)]
mod tests {
    use super::sheet_checks::{open_edge, volume};
    use super::*;

    /// A flat sheet in the xz plane, 1 × 1, `thickness` thick
    fn square(columns: usize, rows: usize, thickness: f32, edge: bool) -> Mesh {
        let mut mesh = Mesh::default();
        let mid = |i: usize, k: usize| Vec3::new(i as f32 / (columns - 1) as f32, 0.0, k as f32 / (rows - 1) as f32);
        // column × row = x × z = -y
        let normal = Vec3::NEG_Y;
        let grid = SheetGrid { columns, rows, wrap: false };
        let rim = |i: usize| mid(i, rows - 1) + Vec3::Z * (0.5 * thickness);
        add_sheet(
            &mut mesh,
            grid,
            |i, k| mid(i, k) + normal * (0.5 * thickness),
            |i, k| mid(i, k) - normal * (0.5 * thickness),
            if edge { Some(&rim) } else { None },
        );
        smooth_normals(&mut mesh);
        mesh
    }

    #[test]
    fn a_sheet_is_closed_and_looks_outwards() {
        for edge in [false, true] {
            let mesh = square(5, 4, 0.01, edge);
            assert_eq!(open_edge(&mesh), None);
            // a 1 × 1 plate 0.01 thick, a little more with the rounded edge
            let v = volume(&mesh);
            assert!(v > 0.0099 && v < 0.0101 + if edge { 0.0001 } else { 0.0 }, "volume {v}");
            assert!(mesh.groups.iter().all(|&g| g == 0));
        }
    }

    #[test]
    fn the_faces_keep_their_own_side_s_normal() {
        let mesh = square(6, 6, 0.01, false);
        let rows = 6;
        // the middle of the front and the back
        let (front, back) = (2 * rows + 2, (6 + 2) * rows + 2);
        let n = |v: usize| Vec3::from_slice(&mesh.normals[v * 3..v * 3 + 3]);
        assert!(n(front).distance(Vec3::NEG_Y) < 1e-4);
        assert!(n(back).distance(Vec3::Y) < 1e-4);
        // and at the border too, the narrow edges don't tilt them much
        assert!(n(0).dot(Vec3::NEG_Y) > 0.9);
    }

    #[test]
    fn a_band_that_goes_around_has_no_side_walls() {
        // a cylinder band of radius 1 and height 1, 0.02 thick, rows from the top down
        let columns = 64;
        let mut mesh = Mesh::default();
        let at = |i: usize, k: usize, r: f32| {
            let angle = std::f32::consts::TAU * i as f32 / columns as f32;
            Vec3::new(angle.cos() * r, -(k as f32) / 3.0, angle.sin() * r)
        };
        // column × row = around × down, which points outwards
        add_sheet(&mut mesh, SheetGrid { columns, rows: 4, wrap: true }, |i, k| at(i, k, 1.01), |i, k| at(i, k, 0.99), None);
        assert_eq!(open_edge(&mesh), None);
        let v = volume(&mesh);
        let expected = std::f32::consts::PI * (1.01f32.powi(2) - 0.99f32.powi(2));
        assert!((v - expected).abs() < 0.01 * expected, "volume {v}, expected {expected}");
    }

    #[test]
    fn too_small_grids_add_nothing() {
        let mut mesh = Mesh::default();
        add_sheet(&mut mesh, SheetGrid { columns: 1, rows: 5, wrap: false }, |_, _| Vec3::ZERO, |_, _| Vec3::ZERO, None);
        add_sheet(&mut mesh, SheetGrid { columns: 5, rows: 1, wrap: false }, |_, _| Vec3::ZERO, |_, _| Vec3::ZERO, None);
        assert_eq!(mesh, Mesh::default());
    }
}
