use std::convert::TryInto;
use std::f32::consts::PI;

use glam::Vec3;
use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;
use nodarium_utils::encode_float;
use nodarium_utils::Ctx;
use nodarium_utils::log;
use nodarium_utils::wrap_arg;
use nodarium_utils::{split_args, decode_float};

nodarium_definition_file!("src/definition.json");

fn calculate_y(x: f32) -> f32 {
    let term1 = (x * PI * 2.0).sin().abs();
    let term2 = (x * 2.0 * PI + (PI / 2.0)).sin() / 2.0;
    term1 + term2
}

// Helper vector math functions
fn vec_sub(a: &[f32; 3], b: &[f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn vec_cross(a: &[f32; 3], b: &[f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn vec_normalize(v: &[f32; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len == 0.0 { [0.0, 0.0, 0.0] } else { [v[0]/len, v[1]/len, v[2]/len] }
}

/// Area weighted vertex normals for a `slice_count x width_resolution` grid
fn grid_normals(positions: &[[f32; 3]], slice_count: usize, width_resolution: usize) -> Vec<[f32; 3]> {
    let mut normals = vec![[0.0f32; 3]; positions.len()];

    for i in 0..(slice_count - 1) {
        for j in 0..(width_resolution - 1) {
            let a = i * width_resolution + j;
            let b = a + 1;
            let c = (i + 1) * width_resolution + j;
            let d = c + 1;

            let n1 = vec_cross(&vec_sub(&positions[b], &positions[a]), &vec_sub(&positions[c], &positions[a]));
            let n2 = vec_cross(&vec_sub(&positions[d], &positions[b]), &vec_sub(&positions[c], &positions[b]));

            for (idxs, n) in [([a, b, c], n1), ([b, d, c], n2)] {
                for idx in idxs {
                    normals[idx][0] += n[0];
                    normals[idx][1] += n[1];
                    normals[idx][2] += n[2];
                }
            }
        }
    }

    normals.iter().map(vec_normalize).collect()
}

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let mut ctx = Ctx::default();
    // A connected shape node wraps the path in another bracket, the default value does not
    let input_path = match split_args(args[0]).first() {
        Some(path) if path.len() > 1 => *path,
        _ => args[0],
    };
    let width_resolution = ctx.int(args[3]).max(3) as usize;
    let path_length = input_path.len().saturating_sub(4) / 2;
    assert!(path_length >= 2, "leaf shape needs at least two points, got {}", path_length);

    let slice_count = path_length;
    let segs = (slice_count - 1).max(1) as f32;

    // alpha goes along the leaf, so every setting can change from base to tip
    let slice_alphas = || (0..slice_count).map(|i| i as f32 / segs);
    let size = ctx.float_along(args[1], slice_alphas());
    let thickness = ctx.float_along(args[2], slice_alphas());
    let y_curve = ctx.float_along(args[4], slice_alphas());
    let y_twist = ctx.float_along(args[5], slice_alphas());
    let x_curve = ctx.float_along(args[6], slice_alphas());

    // Without thickness the leaf stays a single sheet, otherwise a second sheet
    // is added on the back side and both meet along the rim.
    let solid = thickness.iter().any(|&t| t > 0.0);
    let layers = if solid { 2 } else { 1 };

    let sheet_faces = (slice_count - 1) * (width_resolution - 1) * 2;
    let position_amount = slice_count * width_resolution;
    let face_amount = sheet_faces * layers;
    let vertex_amount = position_amount * layers;

    let out_length = 
        3  // metadata
        + face_amount * 3 // indices
        + vertex_amount * 3  // positions
        + vertex_amount * 3; // normals

    let mut out = vec![0 as i32; out_length];

    log!("face_amount={:?} position_amount={:?}", face_amount, position_amount);

    out[0] = 1;
    out[1] = vertex_amount.try_into().unwrap();
    out[2] = face_amount.try_into().unwrap();
    let mut offset = 3;

    // Writing Indices
    let mut idx = 0;
    for layer in 0..layers {
        let layer_base = (layer * position_amount) as i32;
        for i in 0..(slice_count - 1) {
            let base0 = layer_base + (i * width_resolution) as i32;
            let base1 = layer_base + ((i + 1) * width_resolution) as i32;

            for j in 0..(width_resolution - 1) {
                let a = base0 + j as i32;
                let b = base0 + j as i32 + 1;
                let c = base1 + j as i32;
                let d = base1 + j as i32 + 1;

                // the back sheet is wound the other way so it faces outward
                let (t1, t2) = if layer == 0 {
                    ([a, b, c], [b, d, c])
                } else {
                    ([a, c, b], [b, c, d])
                };

                out[offset + idx..offset + idx + 3].copy_from_slice(&t1);
                out[offset + idx + 3..offset + idx + 6].copy_from_slice(&t2);

                idx += 6;
            }
        }
    }

    offset += face_amount * 3;

    // Writing Positions
    let width = 50.0;
    let leaf_length: f32 = 100.0;
    let mut positions = vec![[0.0f32; 3]; position_amount];
    let mut sections = vec![0.0f32; position_amount];

    // Pre-compute a local frame (center, normal=local-Y, binormal=local-X) for
    // each slice by walking the FK chain. At each step we bend around the
    // current binormal (curls the leaf) and twist around the current tangent
    // (rotates the bend plane → spiral).
    let mut centers: Vec<Vec3> = Vec::with_capacity(slice_count);
    let mut frame_n: Vec<Vec3> = Vec::with_capacity(slice_count);
    let mut frame_b: Vec<Vec3> = Vec::with_capacity(slice_count);

    let mut tangent = Vec3::new(0.0, 0.0, 1.0);
    let mut normal = Vec3::new(0.0, 1.0, 0.0);
    let mut binormal = Vec3::new(1.0, 0.0, 0.0);

    let pz_first = decode_float(input_path[2 + 1]);
    let mut center = Vec3::new(0.0, 0.0, pz_first - leaf_length);

    for i in 0..slice_count {
        centers.push(center);
        frame_n.push(normal);
        frame_b.push(binormal);

        if i + 1 < slice_count {
            let bend_per_step = y_curve[i] / segs;
            let twist_per_step = y_twist[i] / segs;
            let pz_curr = decode_float(input_path[2 + i * 2 + 1]);
            let pz_next = decode_float(input_path[2 + (i + 1) * 2 + 1]);
            let seg_len = pz_next - pz_curr;

            center = center + tangent * seg_len;

            // Bend around binormal — tilts tangent toward normal
            let (sin_b, cos_b) = bend_per_step.sin_cos();
            let new_t = tangent * cos_b + normal * sin_b;
            let new_n = -tangent * sin_b + normal * cos_b;
            tangent = new_t;
            normal = new_n;

            // Twist around tangent — rotates normal/binormal so the next bend
            // happens in a rotated plane
            let (sin_tw, cos_tw) = twist_per_step.sin_cos();
            let new_n2 = normal * cos_tw + binormal * sin_tw;
            let new_b = -normal * sin_tw + binormal * cos_tw;
            normal = new_n2;
            binormal = new_b;
        }
    }

    for i in 0..slice_count {
        let ax = i as f32 / segs;
        let px = decode_float(input_path[2 + i * 2 + 0]);
        let hw = width - px; // half-width at this slice

        let c = centers[i];
        let n = frame_n[i];
        let b = frame_b[i];

        for j in 0..width_resolution {
            let alpha = j as f32 / (width_resolution - 1) as f32;
            // Signed cross-section parameter, -1 (left edge) → +1 (right edge)
            let t = 2.0 * alpha - 1.0;
            let py_local = calculate_y(alpha - 0.5) * 5.0 * (ax * PI).sin();

            // X-curl: each cross-section traces a circular arc with curvature
            // x_curve / hw. Because theta = x_curve * t is signed around the
            // midrib, sin/cos give a mirrored arc (left and right edges curl
            // the same direction).
            let theta = x_curve[i] * t;
            let (sin_t, cos_t) = theta.sin_cos();
            let (b_arc, n_arc) = if x_curve[i].abs() < 0.0001 {
                (t * hw, 0.0)
            } else {
                let r = hw / x_curve[i];
                (r * sin_t, r * (1.0 - cos_t))
            };
            // Cross-section bulge follows the rotated local frame
            let b_total = b_arc - py_local * sin_t;
            let n_total = n_arc + py_local * cos_t;

            let world = c + b * b_total + n * n_total;

            let pos_idx = i * width_resolution + j;
            positions[pos_idx] = [world.x, world.y, world.z];
            // keep the cross-section parameter for the thickness profile below
            sections[pos_idx] = t;

        }
    }

    // Writing Positions & Normals
    let write_vec = |out: &mut Vec<i32>, at: usize, v: [f32; 3]| {
        out[at + 0] = encode_float(v[0]);
        out[at + 1] = encode_float(v[1]);
        out[at + 2] = encode_float(v[2]);
    };

    let mid_normals = grid_normals(&positions, slice_count, width_resolution);

    let mut sheets: Vec<(Vec<[f32; 3]>, Vec<[f32; 3]>)> = Vec::with_capacity(layers);
    if solid {
        // Offset the midsurface along its normal. The offset follows a lens
        // profile across the leaf and fades out at both ends, so the front and
        // back sheet meet along the whole rim.
        let offset_sheet = |side: f32| -> Vec<[f32; 3]> {
            (0..position_amount)
                .map(|k| {
                    let i = k / width_resolution;
                    let ax = i as f32 / segs;
                    let t = sections[k];
                    let profile = (1.0 - t * t).max(0.0).sqrt() * (ax * PI).sin().max(0.0).sqrt();
                    let h = thickness[i].max(0.0) * 0.5 * profile * side;
                    let n = mid_normals[k];
                    [
                        positions[k][0] + n[0] * h,
                        positions[k][1] + n[1] * h,
                        positions[k][2] + n[2] * h,
                    ]
                })
                .collect()
        };
        let front = offset_sheet(1.0);
        let back = offset_sheet(-1.0);
        let front_normals = grid_normals(&front, slice_count, width_resolution);
        let back_normals = grid_normals(&back, slice_count, width_resolution)
            .into_iter()
            .map(|n| [-n[0], -n[1], -n[2]])
            .collect();
        sheets.push((front, front_normals));
        sheets.push((back, back_normals));
    } else {
        sheets.push((positions, mid_normals));
    }

    let normals_offset = offset + vertex_amount * 3;
    for (layer, (sheet_positions, sheet_normals)) in sheets.iter().enumerate() {
        for k in 0..position_amount {
            let v = layer * position_amount + k;
            let s = size[k / width_resolution];
            let p = sheet_positions[k];
            write_vec(&mut out, offset + v * 3, [p[0] * s, p[1] * s, p[2] * s]);
            write_vec(&mut out, normals_offset + v * 3, sheet_normals[k]);
        }
    }

    wrap_arg(&out)
}

