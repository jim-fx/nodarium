use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;
use nodarium_utils::{
    concat_args, evaluate_int,
    geometry::{extrude_path, wrap_path},
    log, split_args,
};

nodarium_definition_file!("src/inputs.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    log!("WASM(output): args: {:?}", args);


    log!("WASM(output) args: {:?}", args);

    assert_eq!(args.len(), 2, "Expected 2 arguments, got {}", args.len());
    let inputs = split_args(args[0]);

    let resolution = evaluate_int(args[1]) as usize;

    log!("inputs: {}, resolution: {}", inputs.len(), resolution);

    let mut output: Vec<Vec<i32>> = Vec::new();
    for arg in inputs {
        let arg_type = arg[2];
        log!("arg_type: {}, \n {:?}", arg_type, arg,);

        if arg_type == 0 {
            // if this is path we need to extrude it
            output.push(arg.to_vec());
            let path_data = wrap_path(arg);
            let geometry = extrude_path(path_data, resolution);
            output.push(geometry);
            continue;
        }

        output.push(arg.to_vec());
    }

    concat_args(output.iter().map(|v| v.as_slice()).collect())
}
