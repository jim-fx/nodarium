use myzel_core::read_field;
use nodarium_macros::{nodarium_definition_file, nodarium_execute};

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    // args[i] holds the value of the i-th input of src/definition.json,
    // read them with the read_* helpers of myzel_core
    let field = read_field(args[0]);
    field.encode()
}
