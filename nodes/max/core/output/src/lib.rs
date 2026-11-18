use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{
    abi::{self, Output},
    concat_args,
    geometry::{is_geometry, split_items},
};

nodarium_definition_file!("src/definition.json");

const INSTANCES_TAG: i32 = 2;

fn is_shown(item: &[i32]) -> bool {
    is_geometry(item) || item.get(2) == Some(&INSTANCES_TAG)
}

/// `[0, 1, ...items, 1, 1]`, the list the viewer reads. The input is the largest buffer of a
/// build, so it is framed where the host copied it in, and only copied when it can't be.
#[nodarium_execute(arena)]
pub fn execute(args: &[&[i32]]) -> Output {
    let input = args[0];
    if is_geometry(input) {
        return abi::frame_input(input, &[0, 1], &[1, 1]).map_or_else(|| concat_args(vec![input]).into(), Output::from);
    }
    let items = split_items(input);
    if items.iter().all(|item| is_shown(item)) {
        return abi::frame_input(input, &[], &[]).map_or_else(|| input.to_vec().into(), Output::from);
    }
    concat_args(items.into_iter().filter(|item| is_shown(item)).collect()).into()
}

#[cfg(test)]
mod tests {
    use super::{__nodarium_execute_inner, Output};
    use nodarium_utils::{
        abi, concat_args,
        geometry::{create_instance_data, create_multiple_paths, split_items, Mesh},
        split_args,
    };

    fn run(input: &[i32]) -> Vec<i32> {
        unsafe { __nodarium_execute_inner(&[input]).words() }.to_vec()
    }

    fn triangle() -> Vec<i32> {
        Mesh {
            faces: vec![0, 1, 2],
            positions: vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            normals: [0.0, 0.0, 1.0].repeat(3),
            colors: Vec::new(),
            groups: vec![0; 3],
            creators: vec![12],
            paths: vec![3, -1, 0, 12, 0, 1],
        }
        .encode()
    }

    /// `words` in a buffer the host allocated for the call
    fn copied_in(words: &[i32]) -> &'static [i32] {
        let ptr = abi::alloc(words.len() * 4);
        unsafe {
            std::ptr::copy_nonoverlapping(words.as_ptr(), ptr, words.len());
            std::slice::from_raw_parts(ptr, words.len())
        }
    }

    #[test]
    fn a_geometry_becomes_a_list_of_it() {
        assert_eq!(run(&triangle()), concat_args(vec![&triangle()]));
    }

    #[test]
    fn a_geometry_the_host_copied_in_is_framed_where_it_is() {
        abi::reset();
        let input = copied_in(&triangle());
        let output = __nodarium_execute_inner(&[input]);
        assert!(matches!(output, Output::InPlace(_)));
        assert_eq!(unsafe { output.words() }, concat_args(vec![&triangle()]));
        abi::reset();
    }

    #[test]
    fn a_list_of_geometry_and_instances_is_passed_on_where_it_is() {
        let instances = create_instance_data(3, 1, 2);
        let list = concat_args(vec![&triangle(), &instances]);
        assert_eq!(run(&list), list);
        abi::reset();
        let output = __nodarium_execute_inner(&[copied_in(&list)]);
        assert!(matches!(output, Output::InPlace(_)));
        assert_eq!(unsafe { output.words() }, &list[..]);
        abi::reset();
    }

    #[test]
    fn anything_but_geometry_and_instances_is_left_out() {
        let paths = create_multiple_paths(2, 3, 1, 9);
        let mut items: Vec<&[i32]> = split_args(&paths);
        let geometry = triangle();
        items.push(&geometry);
        let result = run(&concat_args(items));
        assert_eq!(split_items(&result), vec![geometry.as_slice()]);
    }

    #[test]
    fn nothing_connected_shows_nothing() {
        assert!(split_items(&run(&[0, 1, 1, 1])).is_empty());
    }
}
