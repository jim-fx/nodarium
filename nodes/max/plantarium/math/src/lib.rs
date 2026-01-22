use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;
use nodarium_utils::{read_f32, read_i32, log};

nodarium_definition_file!("src/input.json");

#[nodarium_execute]
pub fn execute(op_type: *const i32, a: *const i32, b: *const i32) -> Vec<i32> {
    let op = unsafe { read_i32(op_type) };
    let a_val = unsafe { read_f32(a) };
    let b_val = unsafe { read_f32(b) };

    log!("op_type: {:?}", op);

    let result = match op {
        0 => a_val + b_val,
        1 => a_val - b_val,
        2 => a_val * b_val,
        3 => a_val / b_val,
        _ => 0.0,
    };

    vec![result.to_bits() as i32]
}
