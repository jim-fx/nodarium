use std::convert::TryInto;
use std::f32::consts::PI;

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
    let mut positions = vec![[0.0f32; 3]; position_amount];
    for i in 0..slice_count {
        let ax = i as f32 / (slice_count -1) as f32;

        let px = decode_float(input_path[2 + i * 2 + 0]);
        let pz = decode_float(input_path[2 + i * 2 + 1]);


        for j in 0..width_resolution {
            let alpha = j as f32 / (width_resolution - 1) as f32;
            let x = 2.0 * (-px * (alpha - 0.5) + alpha * width);
            let py = calculate_y(alpha-0.5)*5.0*(ax*PI).sin();
            let pz_val = pz - 100.0;

            let pos_idx = i * width_resolution + j;
            positions[pos_idx] = [x - width, py, pz_val];

            let flat_idx = offset + pos_idx * 3;
            out[flat_idx + 0] = encode_float((x - width) * size);
            out[flat_idx + 1] = encode_float(py * size);
            out[flat_idx + 2] = encode_float(pz_val * size);
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

