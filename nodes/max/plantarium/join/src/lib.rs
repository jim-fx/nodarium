use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;
use nodarium_utils::{
    concat_arg_vecs,
    geometry::{is_path, path_id, path_link, set_path_link, split_items, PathLink},
};
use std::collections::HashSet;

nodarium_definition_file!("src/definition.json");

/// Gives the ids of a list of paths a new salt, so they stay unique next to the paths joined
/// before them. The links inside the list stay intact, parents are salted the same way.
fn resalt_ids(path: &mut [i32], salt: i32) {
    let link = path_link(path);
    set_path_link(
        path,
        PathLink {
            id: path_id(link.id, salt, 0),
            parent_id: if link.has_parent() {
                path_id(link.parent_id, salt, 0)
            } else {
                link.parent_id
            },
            ..link
        },
    );
}

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let mut output: Vec<Vec<i32>> = Vec::new();
    // ids are made of the node that created the path, so lists from different nodes don't
    // clash and keep their ids. Only the same list joined twice does.
    let mut seen: HashSet<i32> = HashSet::new();

    for (slot, arg) in args.iter().enumerate() {
        // an unconnected input is empty, a wrapped list is at least [0, n, 1, n]
        // anything not framed as a list is no geometry, e.g. a mesh from an old graph
        if arg.len() < 4 || arg[0] != 0 {
            continue;
        }

        let items: Vec<Vec<i32>> = split_items(arg).iter().map(|item| item.to_vec()).collect();
        let clashes = items
            .iter()
            .any(|item| is_path(item) && seen.contains(&path_link(item).id));

        for mut item in items {
            if is_path(&item) {
                if clashes {
                    resalt_ids(&mut item, slot as i32 + 1);
                }
                seen.insert(path_link(&item).id);
            }
            output.push(item);
        }
    }

    concat_arg_vecs(output)
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use nodarium_utils::{
        geometry::{create_multiple_paths, path_link, set_path_link, PathLink, NO_PATH},
        split_args, wrap_arg,
    };

    /// Two paths of the node `creator`, the second grows on the first.
    fn list_with_branch(creator: i32) -> Vec<i32> {
        let mut data = create_multiple_paths(2, 3, 1, creator);
        let first = path_link(&data[2..]).id;
        let start = 2 + (3 * 4 + 7 + 4);
        set_path_link(
            &mut data[start..],
            PathLink::child(creator, first, 0.5, 0),
        );
        data
    }

    fn links(result: &[i32]) -> Vec<PathLink> {
        split_args(result).iter().map(|p| path_link(p)).collect()
    }

    #[test]
    fn lists_of_different_nodes_keep_their_ids() {
        let a = list_with_branch(1);
        let b = list_with_branch(2);
        let result = __nodarium_execute_inner(&[&a, &b, &[], &[]]);
        let joined = links(&result);

        let expected: Vec<PathLink> = links(&a).into_iter().chain(links(&b)).collect();
        assert_eq!(joined, expected);
        let mut ids: Vec<i32> = joined.iter().map(|l| l.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 4);
    }

    #[test]
    fn the_same_list_joined_twice_gets_unique_ids_and_keeps_its_links() {
        let a = list_with_branch(1);
        let result = __nodarium_execute_inner(&[&a, &a, &[], &[]]);
        let links = links(&result);

        assert_eq!(links.len(), 4);
        let mut ids: Vec<i32> = links.iter().map(|l| l.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 4);

        // the first copy is untouched, the second points at its own first path
        assert_eq!(links[0], path_link(&split_args(&a)[0]));
        assert_eq!(links[3].parent_id, links[2].id);
        assert_eq!(links[3].parent_alpha, 0.5);
        assert_eq!(links[3].creator, 1);
    }

    #[test]
    fn paths_without_a_parent_keep_having_none() {
        let a = list_with_branch(1);
        let result = __nodarium_execute_inner(&[&a, &a, &[], &[]]);
        let links = links(&result);

        assert!(!links[0].has_parent());
        assert!(!links[2].has_parent());
    }

    #[test]
    fn single_geometries_are_joined_into_a_list() {
        let leaf = |vertices: i32| {
            let mut data = vec![1, vertices, 1, NO_PATH, 0, 1, 2];
            data.extend(vec![0; vertices as usize * 6]);
            wrap_arg(&data)
        };
        let (a, b) = (leaf(3), leaf(4));

        let result = __nodarium_execute_inner(&[&a, &[0, 1, 1, 1], &b, &[0, 1, 1, 1]]);

        assert_eq!(split_args(&result), vec![a.as_slice(), b.as_slice()]);
    }

    #[test]
    fn a_mesh_without_faces_is_left_out() {
        // [tag, vertices, faces, flags, positions, normals, groups]. Read as a list, the words
        // of its positions frame an item that looks like a path
        let mut mesh = vec![14, 3, 0, 0];
        mesh.extend(vec![0; 3 * 3 * 2 + 3]);
        mesh[5] = 1;
        mesh[7] = 15;
        mesh[22] = 1;
        mesh[23] = 1;
        let paths = create_multiple_paths(2, 3, 1, 4);

        let result = __nodarium_execute_inner(&[&mesh, &paths, &[], &[]]);

        assert_eq!(result, __nodarium_execute_inner(&[&paths, &[], &[], &[]]));
    }
}
