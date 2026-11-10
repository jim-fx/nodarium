use myzel_core::read_float;
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{concat_args, encode_float};

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    // args[i] holds the value of the i-th input of src/definition.json,
    // read them with the read_* helpers of myzel_core
    let value = read_float(args[0]);
    concat_args(vec![&[encode_float(value)]])
}
