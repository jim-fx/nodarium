use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;
use nodarium_utils::read_i32_slice;

nodarium_definition_file!("src/input.json");

#[nodarium_execute]
pub fn execute(input: (i32, i32), _res: (i32, i32)) -> Vec<i32> {
    let inp = read_i32_slice(input);
    return inp;
}
