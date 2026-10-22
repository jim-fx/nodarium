use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;
use nodarium_utils::{
    concat_arg_vecs,
    geometry::{path_link, set_path_link, PathLink, NO_PARENT},
    split_args,
};

nodarium_definition_file!("src/definition.json");

/// Shifts the ids of a list of paths, so they stay unique next to the paths joined before them.
fn shift_ids(path: &mut [i32], offset: i32) {
    let link = path_link(path);
    set_path_link(
        path,
        PathLink {
            id: link.id + offset,
            parent_id: if link.has_parent() {
                link.parent_id + offset
            } else {
                NO_PARENT
            },
            parent_alpha: link.parent_alpha,
        },
    );
}

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let mut output: Vec<Vec<i32>> = Vec::new();
    // every list numbers its paths from the start, the next list continues after the last id
    let mut next_id = 0;

    for arg in args.iter() {
        // an unconnected input is empty, a wrapped list is at least [0, n, 1, n]
        if arg.len() < 4 {
            continue;
        }

        let mut highest_id = -1;
        for item in split_args(arg) {
            let mut item = item.to_vec();
            // type 0 is a path, the header holds type, depth and the link
            if item.len() > 7 && item[2] == 0 {
                highest_id = highest_id.max(path_link(&item).id);
                shift_ids(&mut item, next_id);
            }
            output.push(item);
        }
        next_id += highest_id + 1;
    }

    concat_arg_vecs(output)
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use nodarium_utils::{
        geometry::{create_multiple_paths, path_link, set_path_link, PathLink},
        split_args,
    };

    fn list_with_branch() -> Vec<i32> {
        let mut data = create_multiple_paths(2, 3, 1);
        // path 1 grows on path 0
        let start = 2 + (3 * 4 + 5 + 4);
        set_path_link(
            &mut data[start..],
            PathLink {
                id: 1,
                parent_id: 0,
                parent_alpha: 0.5,
            },
        );
        data
    }

    #[test]
    fn ids_of_the_second_list_continue_after_the_first() {
        let a = list_with_branch();
        let b = list_with_branch();
        let result = __nodarium_execute_inner(&[&a, &b, &[], &[]]);
        let links: Vec<PathLink> = split_args(&result).iter().map(|p| path_link(p)).collect();

        assert_eq!(links.len(), 4);
        let ids: Vec<i32> = links.iter().map(|l| l.id).collect();
        assert_eq!(ids, vec![0, 1, 2, 3]);
        assert_eq!(links[1].parent_id, 0);
        assert_eq!(links[3].parent_id, 2);
        assert_eq!(links[3].parent_alpha, 0.5);
    }

    #[test]
    fn paths_without_a_parent_keep_having_none() {
        let a = list_with_branch();
        let b = list_with_branch();
        let result = __nodarium_execute_inner(&[&a, &b, &[], &[]]);
        let links: Vec<PathLink> = split_args(&result).iter().map(|p| path_link(p)).collect();

        assert!(!links[0].has_parent());
        assert!(!links[2].has_parent());
    }
}
