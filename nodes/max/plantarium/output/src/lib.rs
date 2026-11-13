use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;
use nodarium_utils::{
    geometry::{extrude_path_into, extrude_path_len, split_items, wrap_path, COLORED_PATH},
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

    // a path is passed on and also extruded into geometry
    let is_path = |arg: &[i32]| matches!(arg.get(2), Some(&0) | Some(&COLORED_PATH));

    // the geometry is the bulk of the result, it is extruded straight into it instead of being
    // copied over from a buffer of its own
    let len = 4 + inputs
        .iter()
        .map(|arg| {
            arg.len()
                + if is_path(arg) {
                    extrude_path_len(&wrap_path(arg), resolution)
                } else {
                    0
                }
        })
        .sum::<usize>();

    // every item comes with its own brackets, so the list is just the items between [0, 1] and [1, 1]
    let mut output = Vec::with_capacity(len);
    output.extend_from_slice(&[0, 1]);
    for arg in inputs {
        output.extend_from_slice(arg);
        if is_path(arg) {
            extrude_path_into(wrap_path(arg), resolution, &mut output);
        }
    }
    output.extend_from_slice(&[1, 1]);
    output
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use nodarium_utils::{
        concat_args,
        geometry::{create_multiple_paths, is_geometry, with_path_colors, wrap_geometry_data, NO_PATH},
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

    #[test]
    fn a_path_with_colors_gives_geometry_with_colors() {
        let paths = create_multiple_paths(1, 3, 1, 0);
        let path = with_path_colors(split_args(&paths)[0], &[7, 8, 9]);
        let result = __nodarium_execute_inner(&[&concat_args(vec![&path]), &[4]]);
        let items = split_args(&result);
        assert_eq!(items[0], path.as_slice());
        let mut geometry = items[1].to_vec();
        let colors = wrap_geometry_data(&mut geometry).colors.to_vec();
        assert_eq!(colors.len(), 3 * 4 + 4 + 1);
        assert_eq!(&colors[4..8], &[8; 4]);
    }
}
