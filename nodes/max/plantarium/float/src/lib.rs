use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;
use nodarium_utils::log;

nodarium_definition_file!("src/input.json");

#[nodarium_execute]
pub fn execute(_value: *const i32) -> Vec<i32> {
    log!("Duuuude");
    vec![32]
}
