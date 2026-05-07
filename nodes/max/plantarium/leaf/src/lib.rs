use std::convert::TryInto;
use std::f32::consts::PI;

use glam::Vec3;
use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;
use nodarium_utils::encode_float;
use nodarium_utils::evaluate_float;
use nodarium_utils::evaluate_int;
use nodarium_utils::log;
use nodarium_utils::wrap_arg;
use nodarium_utils::{split_args, decode_float};

nodarium_definition_file!("src/input.json");

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

#[nodarium_execute]
pub fn execute(input: &[i32]) -> Vec<i32> {
    let args = split_args(input);
    let input_path = split_args(args[0])[0];
    let size = evaluate_float(args[1]);
    let width_resolution = evaluate_int(args[2]).max(3) as usize;
    let y_curve = evaluate_float(args[3]);
    let y_twist = evaluate_float(args[4]);
    let x_curve = evaluate_float(args[5]);
    let path_length = (input_path.len() - 4) / 2;

    let slice_count = path_length;
    let face_amount = (slice_count - 1) * (width_resolution - 1) * 2;
    let position_amount = slice_count * width_resolution;

    let out_length = 
        3  // metadata
        + face_amount * 3 // indices
        + position_amount * 3  // positions
        + position_amount * 3; // normals

    let mut out = vec![0 as i32; out_length];

    log!("face_amount={:?} position_amount={:?}", face_amount, position_amount);

    out[0] = 1;
    out[1] = position_amount.try_into().unwrap();
    out[2] = face_amount.try_into().unwrap();
    let mut offset = 3;

    // Writing Indices
    let mut idx = 0;
    for i in 0..(slice_count - 1) {
        let base0 = (i * width_resolution) as i32;
        let base1 = ((i + 1) * width_resolution) as i32;

        for j in 0..(width_resolution - 1) {
            let a = base0 + j as i32;
            let b = base0 + j as i32 + 1;
            let c = base1 + j as i32;
            let d = base1 + j as i32 + 1;

            // triangle 1
            out[offset + idx + 0] = a;
            out[offset + idx + 1] = b;
            out[offset + idx + 2] = c;

            // triangle 2
            out[offset + idx + 3] = b;
            out[offset + idx + 4] = d;
            out[offset + idx + 5] = c;

            idx += 6;
        }
    }

    offset += face_amount * 3;

    // Writing Positions
    let width = 50.0;
    let leaf_length: f32 = 100.0;
    let mut positions = vec![[0.0f32; 3]; position_amount];

    // Pre-compute a local frame (center, normal=local-Y, binormal=local-X) for
    // each slice by walking the FK chain. At each step we bend around the
    // current binormal (curls the leaf) and twist around the current tangent
    // (rotates the bend plane → spiral).
    let segs = (slice_count - 1).max(1) as f32;
    let bend_per_step = y_curve / segs;
    let twist_per_step = y_twist / segs;

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
            let theta = x_curve * t;
            let (sin_t, cos_t) = theta.sin_cos();
            let (b_arc, n_arc) = if x_curve.abs() < 0.0001 {
                (t * hw, 0.0)
            } else {
                let r = hw / x_curve;
                (r * sin_t, r * (1.0 - cos_t))
            };
            // Cross-section bulge follows the rotated local frame
            let b_total = b_arc - py_local * sin_t;
            let n_total = n_arc + py_local * cos_t;

            let world = c + b * b_total + n * n_total;

            let pos_idx = i * width_resolution + j;
            positions[pos_idx] = [world.x, world.y, world.z];

            let flat_idx = offset + pos_idx * 3;
            out[flat_idx + 0] = encode_float(world.x * size);
            out[flat_idx + 1] = encode_float(world.y * size);
            out[flat_idx + 2] = encode_float(world.z * size);
        }
    }

    // Writing Normals
    offset += position_amount * 3;
    let mut normals = vec![[0.0f32; 3]; position_amount];

    for i in 0..(slice_count - 1) {
        for j in 0..(width_resolution - 1) {
            let a = i * width_resolution + j;
            let b = i * width_resolution + j + 1;
            let c = (i + 1) * width_resolution + j;
            let d = (i + 1) * width_resolution + j + 1;

            // triangle 1: a,b,c
            let u = vec_sub(&positions[b], &positions[a]);
            let v = vec_sub(&positions[c], &positions[a]);
            let n1 = vec_cross(&u, &v);

            // triangle 2: b,d,c
            let u2 = vec_sub(&positions[d], &positions[b]);
            let v2 = vec_sub(&positions[c], &positions[b]);
            let n2 = vec_cross(&u2, &v2);

            for &idx in &[a, b, c] {
                normals[idx][0] += n1[0];
                normals[idx][1] += n1[1];
                normals[idx][2] += n1[2];
            }

            for &idx in &[b, d, c] {
                normals[idx][0] += n2[0];
                normals[idx][1] += n2[1];
                normals[idx][2] += n2[2];
            }
        }
    }

    // normalize and write to output
    for i in 0..position_amount {
        let n = vec_normalize(&normals[i]);
        let flat_idx = offset + i * 3;
        out[flat_idx + 0] = encode_float(n[0]);
        out[flat_idx + 1] = encode_float(n[1]);
        out[flat_idx + 2] = encode_float(n[2]);
    }

    wrap_arg(&out)
}

