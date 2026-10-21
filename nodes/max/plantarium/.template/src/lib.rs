use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    // args[i] holds the value of the i-th input of src/definition.json
    vec![]
}
