use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;

nodarium_definition_file!("src/input.json");

#[nodarium_execute]
pub fn execute(_input: *const i32, _res: *const i32) -> Vec<i32> {
    return vec![0];
}
