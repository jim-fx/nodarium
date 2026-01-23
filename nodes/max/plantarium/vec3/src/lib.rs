use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;
use nodarium_utils::concat_args;
use nodarium_utils::log;
use nodarium_utils::read_i32_slice;

nodarium_definition_file!("src/input.json");

#[nodarium_execute]
pub fn execute(x: (i32, i32), y: (i32, i32), z: (i32, i32)) -> Vec<i32> {
    log!("vec3 x: {:?}", x);
    concat_args(vec![
        read_i32_slice(x).as_slice(),
        read_i32_slice(y).as_slice(),
        read_i32_slice(z).as_slice(),
    ])
}
