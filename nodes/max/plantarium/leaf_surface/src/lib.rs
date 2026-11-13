//! Pulls a leaf blade over its veins in stages, each with its own module:
//!
//! 1. `veins`: the vein paths in leaf space, with the frame found from the main veins.
//! 2. `field`: the blade as a field on a grid, the smooth union of the veins, without holes.
//! 3. `outline`: the margin, a closed line with alpha along it. The `margin` input moves the
//!    field in along it.
//! 4. `mesh`: triangles inside the margin, flat in leaf space.
//! 5. `lift`: the heights of the veins bend the flat mesh, and it is placed like the leaf node.

mod field;
mod mesh;
mod outline;
mod veins;

use glam::{Vec2, Vec3};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{
    concat_arg_vecs,
    geometry::{create_geometry_data, is_path, wrap_geometry_data},
    int, split_args, Ctx,
};

use field::Field;
use mesh::{marching_squares, FlatMesh};
use outline::Margin;
use veins::{leaf_space, Vein};

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
        let width = (0.15 * s.reach(t)).max(1.5 * cell);
        vein = vein.max(1.0 - (d / width).min(1.0).powi(2));
    }
    (if weights > 0.0 { sum / weights } else { 0.0 }, vein * vein)
}

/// The flat mesh bent by the heights of the veins, the veins sunk by `relief`, in the frame of the
/// leaf node: the base at the origin, along -z, facing +y.
fn lift(mesh: &FlatMesh, veins: &[Vein], relief: f32, cell: f32, scale: f32) -> Vec<Vec3> {
    let bent = veins
        .iter()
        .flat_map(|v| &v.segments)
        .any(|s| s.ha != 0.0 || s.hb != 0.0);
    mesh.positions
        .iter()
        .map(|&p| {
            let (h, vein) = if bent || relief != 0.0 {
                heights(veins, p, cell)
            } else {
                (0.0, 0.0)
            };
            Vec3::new(p.x, h - relief * vein, -p.y) * scale
        })
        .collect()
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
    let cuts = ctx.float_along(args[3], margin.alphas.iter().copied());
    field.cut(&margin.points, &cuts);

    let mesh = marching_squares(&field);
    let positions = lift(&mesh, &veins, relief, field.cell, LEAF_LENGTH * size);
    encode(&positions, &mesh.faces)
}

#[cfg(test)]
mod tests;
