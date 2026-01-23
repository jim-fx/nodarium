use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;
use nodarium_utils::read_i32;

nodarium_definition_file!("src/input.json");

#[nodarium_execute]
pub fn execute(a: (i32, i32)) -> Vec<i32> {
    vec![read_i32(a.0)]
}
