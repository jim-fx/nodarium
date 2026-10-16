use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;

nodarium_definition_file!("src/input.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    // args[i] holds the value of the i-th input of src/input.json
    vec![]
}
