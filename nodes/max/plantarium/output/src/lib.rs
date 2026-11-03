use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;
use nodarium_utils::{
    concat_args,
    geometry::{extrude_path, remesh_paths, split_items, wrap_path},
    int, log,
};

nodarium_definition_file!("src/inputs.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    log!("WASM(output) args: {:?}", args);

    let inputs = split_items(args[0]);

    let resolution = int(args[1]) as usize;
    // older callers leave the last input out
    let remesh = args.get(2).map_or(0, |a| int(a));

    if remesh > 0 {
        let levels = if remesh == 1 { 1 } else { usize::MAX };
        let paths: Vec<_> = inputs
            .iter()
            .filter(|arg| arg.get(2) == Some(&0))
            .map(|arg| wrap_path(arg))
            .collect();
        let (geometries, stats) = remesh_paths(&paths, resolution, levels);
        log!("remesh: {:?}", stats);
        let mut output: Vec<&[i32]> = inputs.clone();
        output.extend(geometries.iter().map(|g| g.as_slice()));
        return concat_args(output);
    }

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
        geometry::{
            create_multiple_paths, create_path, geometry_path_id, is_geometry, set_path_link,
            wrap_path_mut, PathLink, NO_PATH,
        },
        split_args, wrap_arg,
    };

    fn path(points: &[[f32; 4]], link: PathLink) -> Vec<i32> {
        let mut data = create_path(points.len(), 1);
        set_path_link(&mut data, link);
        let p = wrap_path_mut(&mut data);
        for (i, point) in points.iter().enumerate() {
            p.points[i * 4..i * 4 + 4].copy_from_slice(point);
        }
        data
    }

    fn stem_and_branch() -> (Vec<i32>, PathLink) {
        let stem = PathLink::root(1, 0);
        let branch = PathLink::child(2, stem.id, 0.5, 0);
        let stem_points: Vec<[f32; 4]> = (0..6).map(|i| [0.0, i as f32 * 0.2, 0.0, 0.1]).collect();
        let branch_points: Vec<[f32; 4]> = (0..6)
            .map(|i| [i as f32 * 0.1, 0.5 + i as f32 * 0.08, 0.0, 0.07])
            .collect();
        let list = concat_args(vec![
            &triangle(),
            &path(&stem_points, stem),
            &path(&branch_points, branch),
        ]);
        (list, stem)
    }

    #[test]
    fn remesh_off_gives_the_same_output_as_before() {
        let (list, _) = stem_and_branch();
        assert_eq!(
            __nodarium_execute_inner(&[&list, &[8], &[0]]),
            __nodarium_execute_inner(&[&list, &[8]])
        );
    }

    #[test]
    fn remesh_joins_the_tree_into_one_geometry() {
        let (list, stem) = stem_and_branch();
        let result = __nodarium_execute_inner(&[&list, &[8], &[2]]);
        let items = split_args(&result);

        // the triangle and both paths are passed on, the tree becomes one geometry
        assert_eq!(items.len(), 4);
        assert_eq!(items[..3], split_args(&list)[..]);
        assert!(is_geometry(items[3]));
        assert_eq!(geometry_path_id(items[3]), stem.id);
    }

    #[test]
    fn remesh_of_an_unconnected_input_gives_an_empty_list() {
        let result = __nodarium_execute_inner(&[&[0, 1, 1, 1], &[8], &[2]]);
        assert!(split_args(&result).is_empty());
    }

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
