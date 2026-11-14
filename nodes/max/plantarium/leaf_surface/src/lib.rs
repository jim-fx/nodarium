//! Pulls a leaf blade over its veins in stages, each with its own module:
//!
//! 1. `veins`: the vein paths in leaf space, with the frame found from the main veins.
//! 2. `field`: the blade as a field on a grid, the smooth union of the veins, without holes.
//! 3. `outline`: the margin, a closed line with alpha along it. The `margin` input moves the
//!    field in along it, by a share of how far the blade reaches there. The outline of the cut field, with the thin tips of the veins put back
//!    and simplified along its curvature, is what the mesh fills.
//! 4. `mesh`: a constrained Delaunay triangulation of the outline with the veins as chains of
//!    edges, refined until it is fine at the margin and coarse in the middle, flat in leaf space.
//!    `polygon` helps it and `outline` find points in and near the outline.
//! 5. `lift`: the heights of the veins bend the flat mesh, and it is placed like the leaf node.
//! 6. `colors`: a connected `color` is read at every vertex, with alpha vein and the path of the
//!    vein it lies closest to.

mod field;
mod mesh;
mod outline;
mod polygon;
mod veins;

use glam::{Vec2, Vec3};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{
    concat_arg_vecs,
    geometry::{create_geometry_data, is_path, vertex_places, with_geometry_colors, wrap_geometry_data},
    int, opaque, split_args, Ctx,
};

use field::{Cuts, Field};
use polygon::buckets;
use mesh::{vein_mesh, Density, FlatMesh};
use outline::{Margin, Outline};
use veins::{groove, leaf_space, Segment, Vein};

nodarium_definition_file!("src/definition.json");

/// The leaf node is 100 units long at size 1.
const LEAF_LENGTH: f32 = 100.0;

/// The height of the blade over `p`, blended from the veins close to it, and how far `p` lies
/// inside the nearest vein (1 on its middle, 0 at its edge and beyond).
fn heights(veins: &[Vein], p: Vec2, cell: f32) -> (f32, f32) {
    let (mut sum, mut weights, mut vein) = (0.0, 0.0, 0.0_f32);
    let eps = cell * cell;
    for s in veins.iter().flat_map(|v| &v.segments) {
        let (t, d) = s.closest(p);
        let w = 1.0 / (d * d + eps).powi(2);
        sum += w * s.height(t);
        weights += w;
        let width = groove(s.reach(t), cell);
        vein = vein.max(1.0 - (d / width).min(1.0).powi(2));
    }
    (if weights > 0.0 { sum / weights } else { 0.0 }, vein * vein)
}

/// The segments of the veins sorted into a grid by their grooves, for the depth part of `heights`
/// alone: only a vein whose groove reaches `p` sinks it.
struct Grooves<'a> {
    segments: Vec<&'a Segment>,
    origin: Vec2,
    /// The side of a cell of the grid.
    step: f32,
    nx: usize,
    ny: usize,
    starts: Vec<u32>,
    cells: Vec<u32>,
    /// The cell of the field, for the width of a groove.
    cell: f32,
}

impl<'a> Grooves<'a> {
    fn new(veins: &'a [Vein], cell: f32) -> Grooves<'a> {
        let segments: Vec<&Segment> = veins.iter().flat_map(|v| &v.segments).collect();
        // a little wider, for the rounding of the distances
        let reach = |s: &Segment| 1.01 * groove(s.ra.max(s.rb), cell);
        let lo = segments.iter().fold(Vec2::MAX, |m, s| m.min(s.a.min(s.b) - reach(s)));
        let hi = segments.iter().fold(Vec2::MIN, |m, s| m.max(s.a.max(s.b) + reach(s)));
        let step = (4.0 * cell).max((hi - lo).max_element() / 256.0);
        let (nx, ny) = (((hi.x - lo.x) / step) as usize + 1, ((hi.y - lo.y) / step) as usize + 1);
        let key = |p: Vec2| {
            let k = ((p - lo) / step).floor();
            ((k.x.max(0.0) as usize).min(nx - 1), (k.y.max(0.0) as usize).min(ny - 1))
        };
        let each = |visit: &mut dyn FnMut(usize, u32)| {
            for (k, s) in segments.iter().enumerate() {
                let ((x0, y0), (x1, y1)) = (key(s.a.min(s.b) - reach(s)), key(s.a.max(s.b) + reach(s)));
                for y in y0..=y1 {
                    for x in x0..=x1 {
                        visit(y * nx + x, k as u32);
                    }
                }
            }
        };
        let (starts, cells) = buckets(nx * ny, each);
        Grooves {
            segments,
            origin: lo,
            step,
            nx,
            ny,
            starts,
            cells,
            cell,
        }
    }

    fn depth(&self, p: Vec2) -> f32 {
        let k = ((p - self.origin) / self.step).floor();
        if self.segments.is_empty() || k.x < 0.0 || k.y < 0.0 || k.x >= self.nx as f32 || k.y >= self.ny as f32 {
            return 0.0;
        }
        let c = k.y as usize * self.nx + k.x as usize;
        let mut vein = 0.0_f32;
        for &k in &self.cells[self.starts[c] as usize..self.starts[c + 1] as usize] {
            let s = self.segments[k as usize];
            let (t, d) = s.closest(p);
            let width = groove(s.reach(t), self.cell);
            vein = vein.max(1.0 - (d / width).min(1.0).powi(2));
        }
        vein * vein
    }
}

/// The flat mesh bent by the heights of the veins, the veins sunk by `relief`, in the frame of the
/// leaf node: the base at the origin, along -z, facing +y.
fn lift(mesh: &FlatMesh, veins: &[Vein], relief: f32, cell: f32, scale: f32) -> Vec<Vec3> {
    // turned flat with rotate, veins keep heights of about a millionth of their length, from
    // rounding, which would cost a blend of every segment at every vertex for nothing
    let span = veins.iter().fold(0.0_f32, |m, v| m.max((v.max - v.min).max_element()));
    let bent = veins
        .iter()
        .flat_map(|v| &v.segments)
        .any(|s| s.ha.abs().max(s.hb.abs()) > 1e-4 * span);
    let grooves = (!bent && relief != 0.0).then(|| Grooves::new(veins, cell));
    mesh.positions
        .iter()
        .map(|&p| {
            let (h, vein) = if let Some(grooves) = &grooves {
                (0.0, grooves.depth(p))
            } else if bent {
                heights(veins, p, cell)
            } else {
                (0.0, 0.0)
            };
            Vec3::new(p.x, h - relief * vein, -p.y) * scale
        })
        .collect()
}

/// The vein whose groove every vertex of the flat mesh lies deepest in, and how deep: 0 on the
/// vein, 1 at the edge of its groove and beyond.
fn nearest_veins(mesh: &FlatMesh, veins: &[Vein], cell: f32) -> Vec<(usize, f32)> {
    mesh.positions
        .iter()
        .map(|&p| {
            let mut best = (0, f32::MAX);
            for (k, vein) in veins.iter().enumerate() {
                let outside = vein.lower_bound(p) + vein.reach;
                if outside >= best.1 * groove(vein.reach, cell) {
                    continue;
                }
                for s in &vein.segments {
                    let (t, d) = s.closest(p);
                    let share = d / groove(s.reach(t), cell);
                    if share < best.1 {
                        best = (k, share);
                    }
                }
            }
            (best.0, best.1.min(1.0))
        })
        .collect()
}

/// The color of every vertex, read like `set_color` reads it on a leaf, with alpha vein and the
/// path of the nearest vein, so `path_info` tells the midrib from the side veins.
fn colors(ctx: &mut Ctx, color: &[i32], paths: &[&[i32]], veins: &[Vein], nearest: &[(usize, f32)], positions: &[Vec3]) -> Vec<u32> {
    let flat: Vec<f32> = positions.iter().flat_map(|p| p.to_array()).collect();
    let places = vertex_places(&flat);
    let mut colors = vec![0; positions.len()];
    for (k, vein) in veins.iter().enumerate() {
        let vertices: Vec<usize> = (0..positions.len()).filter(|&i| nearest[i].0 == k).collect();
        if vertices.is_empty() {
            continue;
        }
        ctx.set_path(paths[vein.path], k, veins.len());
        let at = vertices.iter().map(|&i| (places[i].0, places[i].1, nearest[i].1));
        for (&i, c) in vertices.iter().zip(ctx.colors_at_veins(color, at)) {
            colors[i] = opaque(c);
        }
    }
    colors
}

fn encode(positions: &[Vec3], faces: &[[u32; 3]]) -> Vec<i32> {
    let mut normals = vec![Vec3::ZERO; positions.len()];
    for f in faces {
        let [a, b, c] = f.map(|i| positions[i as usize]);
        let n = (b - a).cross(c - a);
        for &i in f {
            normals[i as usize] += n;
        }
    }

    let mut out = create_geometry_data(positions.len(), faces.len());
    let geometry = wrap_geometry_data(&mut out);
    for (i, f) in faces.iter().enumerate() {
        geometry.faces[i * 3..i * 3 + 3].copy_from_slice(&f.map(|v| v as i32));
    }
    for (i, (p, n)) in positions.iter().zip(&normals).enumerate() {
        geometry.positions[i * 3..i * 3 + 3].copy_from_slice(&p.to_array());
        let n = n.try_normalize().unwrap_or(Vec3::Y);
        geometry.normals[i * 3..i * 3 + 3].copy_from_slice(&n.to_array());
    }
    out
}

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let mut ctx = Ctx::default();
    let paths: Vec<&[i32]> = split_args(args[0]).into_iter().filter(|p| is_path(p)).collect();
    let size = ctx.float(args[1]);
    let smooth = ctx.float(args[2]).max(0.0);
    let relief = ctx.float(args[4]);
    let resolution = int(args[5]).clamp(8, 512) as usize;

    let veins = leaf_space(&paths);
    let Some(mut field) = Field::blade(&veins, smooth, resolution) else {
        return concat_arg_vecs(vec![]);
    };

    let margin = Margin::of(&field);
    let along = ctx.float_along(args[3], margin.alphas.iter().copied());
    let reaches = if along.iter().any(|&s| s != 0.0) {
        field.reaches(&margin.points)
    } else {
        vec![0.0; along.len()]
    };
    let cuts = Cuts::new(&margin.points, &along, &reaches, field.cell);
    field.cut(&cuts);

    let density = Density::new(field.cell);
    let outline = Outline::of(&field, &veins, &cuts, &density);
    let mesh = vein_mesh(&outline, &veins, &density);
    let positions = lift(&mesh, &veins, relief, field.cell, LEAF_LENGTH * size);
    let geometry = encode(&positions, &mesh.faces);
    // an unconnected color, the blade keeps the viewer's material
    match args.get(6).filter(|c| **c != [0, 1, 1, 1]) {
        Some(color) => {
            let nearest = nearest_veins(&mesh, &veins, field.cell);
            with_geometry_colors(&geometry, &colors(&mut ctx, color, &paths, &veins, &nearest, &positions))
        }
        None => geometry,
    }
}

#[cfg(test)]
mod tests;
