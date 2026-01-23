use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;
use nodarium_utils::{ read_f32, encode_float };

nodarium_definition_file!("src/input.json");

#[nodarium_execute]
pub fn execute(a: (i32, i32)) -> Vec<i32> {
    let a_val = read_f32(a.0);
    vec![encode_float(a_val)]
}
