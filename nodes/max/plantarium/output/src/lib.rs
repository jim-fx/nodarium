use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;
use nodarium_utils::{
    concat_args,
    geometry::{extrude_path, split_items, wrap_path},
    int, log,
};

nodarium_definition_file!("src/inputs.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    log!("WASM(output) args: {:?}", args);

    assert_eq!(args.len(), 2, "Expected 2 arguments, got {}", args.len());
    let inputs = split_items(args[0]);

    let resolution = int(args[1]) as usize;

    log!("inputs: {}, resolution: {}", inputs.len(), resolution);

    let mut output: Vec<Vec<i32>> = Vec::new();
    for arg in inputs {
        if arg.get(2) == Some(&0) {
            // a path is passed on and also extruded into geometry
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

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use nodarium_utils::{
        concat_args,
        geometry::{create_multiple_paths, is_geometry, NO_PATH},
        split_args, wrap_arg,
    };

    fn triangle() -> Vec<i32> {
        let mut data = vec![1, 3, 1, NO_PATH, 0, 1, 2];
        data.extend(vec![0; 18]);
        wrap_arg(&data)
    }

    #[test]
    fn a_single_geometry_is_passed_on() {
        let geometry = triangle();
        let result = __nodarium_execute_inner(&[&geometry, &[8]]);
        assert_eq!(split_args(&result), vec![geometry.as_slice()]);
    }

    #[test]
    fn every_item_of_a_list_is_passed_on_and_paths_are_extruded() {
        let geometry = triangle();
        let paths = create_multiple_paths(1, 3, 1, 0);
        let path = split_args(&paths)[0].to_vec();
        let list = concat_args(vec![&geometry, &path]);

        let result = __nodarium_execute_inner(&[&list, &[8]]);
        let items = split_args(&result);

        assert_eq!(items.len(), 3);
        assert_eq!(items[0], geometry.as_slice());
        assert_eq!(items[1], path.as_slice());
        assert!(is_geometry(items[2]));
    }

    #[test]
    fn an_unconnected_input_gives_an_empty_list() {
        let result = __nodarium_execute_inner(&[&[0, 1, 1, 1], &[8]]);
        assert!(split_args(&result).is_empty());
    }
}
