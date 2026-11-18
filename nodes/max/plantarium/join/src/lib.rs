use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;
use nodarium_utils::{
    abi::{self, Output},
    geometry::{is_path, path_id, path_link, set_path_link, split_items, MeshWords, PathLink},
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

enum Item<'a> {
    Words(&'a [i32]),
    Resalted(Vec<i32>),
    /// Where the geometries with groups go, joined into one
    Joined,
}

#[nodarium_execute(arena)]
pub fn execute(args: &[&[i32]]) -> Output {
    let mut items: Vec<Item> = Vec::new();
    let mut grouped: Vec<MeshWords> = Vec::new();
    // ids are made of the node that created the path, so lists from different nodes don't
    // clash and keep their ids. Only the same list joined twice does.
    let mut seen: HashSet<i32> = HashSet::new();

    for (slot, arg) in args.iter().enumerate() {
        // an unconnected input is empty, a wrapped list is at least [0, n, 1, n]
        if arg.len() < 4 || arg[0] != 0 {
            continue;
        }

        let list = split_items(arg);
        let clashes = list.iter().any(|item| is_path(item) && seen.contains(&path_link(item).id));

        for item in list {
            if is_path(item) {
                if clashes {
                    let mut copy = item.to_vec();
                    resalt_ids(&mut copy, slot as i32 + 1);
                    seen.insert(path_link(&copy).id);
                    items.push(Item::Resalted(copy));
                } else {
                    seen.insert(path_link(item).id);
                    items.push(Item::Words(item));
                }
            } else if let Some(geometry) = MeshWords::read(item).filter(|g| g.has_groups()) {
                if grouped.is_empty() {
                    items.push(Item::Joined);
                }
                grouped.push(geometry);
            } else {
                items.push(Item::Words(item));
            }
        }
    }

    let joined = if grouped.is_empty() { 0 } else { MeshWords::join_len(&grouped) };
    let len = 4 + joined
        + items
            .iter()
            .map(|item| match item {
                Item::Words(words) => words.len(),
                Item::Resalted(words) => words.len(),
                Item::Joined => 0,
            })
            .sum::<usize>();
    // the items between [0, 1] and [1, 1], like `concat_arg_vecs`
    abi::write_result(len, |words| {
        words[..2].copy_from_slice(&[0, 1]);
        let mut at = 2;
        let mut last_closing_bracket = 1;
        for item in &items {
            let item: &[i32] = match item {
                Item::Words(words) => words,
                Item::Resalted(words) => words,
                Item::Joined => {
                    MeshWords::join_into(&grouped, &mut words[at..at + joined]);
                    at += joined;
                    last_closing_bracket = at - 1;
                    continue;
                }
            };
            words[at..at + item.len()].copy_from_slice(item);
            at += item.len();
            if item.len() == 1 {
                words[last_closing_bracket] += 1;
            } else {
                last_closing_bracket = at - 1;
                words[last_closing_bracket] = 1;
            }
        }
        words[at..at + 2].copy_from_slice(&[1, 1]);
        len
    })
    .into()
}

#[cfg(test)]
mod tests {
    use nodarium_utils::{
        concat_arg_vecs, concat_args,
        geometry::{create_multiple_paths, path_link, set_path_link, Mesh, PathLink, NO_PATH},
        opaque, split_args, wrap_arg, NO_COLOR,
    };

    fn __nodarium_execute_inner(args: &[&[i32]]) -> Vec<i32> {
        unsafe { super::__nodarium_execute_inner(args).words() }.to_vec()
    }

    /// Two paths of the node `creator`, the second grows on the first.
    fn list_with_branch(creator: i32) -> Vec<i32> {
        let mut data = create_multiple_paths(2, 3, 1, creator);
        let first = path_link(&data[2..]).id;
        let start = 2 + (3 * 4 + 8 + 4);
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
            let mut data = vec![1, vertices, 1, NO_PATH, 0, 0, 1, 2];
            data.extend(vec![0; vertices as usize * 6]);
            wrap_arg(&data)
        };
        let (a, b) = (leaf(3), leaf(4));

        let result = __nodarium_execute_inner(&[&a, &[0, 1, 1, 1], &b, &[0, 1, 1, 1]]);

        assert_eq!(split_args(&result), vec![a.as_slice(), b.as_slice()]);
    }

    #[test]
    fn single_values_count_into_the_bracket_before_them_as_before() {
        let paths = create_multiple_paths(2, 3, 1, 4);
        let list = concat_arg_vecs(vec![split_args(&paths)[0].to_vec(), vec![7], split_args(&paths)[1].to_vec(), vec![8]]);
        let items: Vec<Vec<i32>> = split_args(&list).iter().map(|i| i.to_vec()).collect();
        assert_eq!(__nodarium_execute_inner(&[&list, &[], &[], &[]]), concat_arg_vecs(items));
    }

    fn plate(x: f32, groups: [u32; 3], colored: bool) -> Mesh {
        Mesh {
            faces: vec![0, 1, 2],
            positions: vec![x, 0.0, 0.0, x + 0.1, 0.0, 0.0, x, 0.1, 0.0],
            normals: vec![0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0],
            colors: if colored { vec![opaque([80, 52, 40]); 3] } else { Vec::new() },
            groups: groups.to_vec(),
            creators: Vec::new(),
        }
    }

    /// The geometry with groups of a joined list, and the other items
    fn joined(inputs: [Vec<i32>; 4]) -> (Mesh, Vec<Vec<i32>>) {
        let refs: Vec<&[i32]> = inputs.iter().map(|w| w.as_slice()).collect();
        let out = __nodarium_execute_inner(&refs);
        let (meshes, rest): (Vec<&[i32]>, Vec<&[i32]>) =
            split_args(&out).into_iter().partition(|item| Mesh::try_decode(item).is_some_and(|m| !m.groups.is_empty()));
        assert_eq!(meshes.len(), 1, "one geometry with groups");
        (Mesh::decode(meshes[0]), rest.into_iter().map(|i| i.to_vec()).collect())
    }

    #[test]
    fn geometries_with_groups_become_one_their_groups_one_after_the_other() {
        let (a, b) = (plate(0.0, [0, 1, 1], false), plate(0.5, [0, 0, 2], false));
        let (mesh, rest) = joined([a.encode(), b.encode(), Vec::new(), Vec::new()]);
        assert!(rest.is_empty());
        assert_eq!(mesh.groups, vec![0, 1, 1, 2, 2, 4]);
        assert_eq!(mesh.faces, vec![0, 1, 2, 3, 4, 5]);
        assert_eq!(&mesh.positions[..9], &a.positions[..]);
        assert_eq!(&mesh.positions[9..], &b.positions[..]);
        // a gap is skipped, the same geometry twice is joined twice
        let (a, b) = (plate(0.0, [0, 0, 0], false), plate(0.5, [0, 1, 1], false));
        let (mesh, _) = joined([a.encode(), vec![0, 1, 1, 1], b.encode(), a.encode()]);
        assert_eq!(mesh.groups, vec![0, 0, 0, 1, 2, 2, 3, 3, 3]);
        // and in lists, like the one of another join
        let (mesh, _) = joined([concat_args(vec![&a.encode(), &b.encode()]), a.encode(), Vec::new(), Vec::new()]);
        assert_eq!(mesh.groups, vec![0, 0, 0, 1, 2, 2, 3, 3, 3]);
    }

    #[test]
    fn colors_and_creators_survive() {
        let (painted, plain) = (plate(0.0, [0, 0, 0], true), plate(0.5, [0, 0, 0], false));
        let brown = opaque([80, 52, 40]);
        let (mesh, _) = joined([painted.encode(), plain.made_by(7).encode(), Vec::new(), Vec::new()]);
        assert_eq!(mesh.colors, [[brown; 3], [NO_COLOR; 3]].concat());
        assert_eq!(mesh.creators, vec![i32::MIN, 7]);
    }

    #[test]
    fn paths_and_plain_geometry_stay_items_around_the_joined_one() {
        let paths = create_multiple_paths(2, 3, 1, 4);
        let leaf = {
            let mut data = vec![1, 3, 1, NO_PATH, 0, 0, 1, 2];
            data.extend(vec![0; 18]);
            wrap_arg(&data)
        };
        let mesh = plate(0.0, [0, 1, 1], false).encode();
        let out = __nodarium_execute_inner(&[&paths, &mesh, &leaf, &mesh]);
        let items = split_args(&out);
        assert_eq!(items.len(), 4);
        assert_eq!(&items[..2], &split_args(&paths)[..]);
        assert_eq!(Mesh::decode(items[2]).vertex_count(), 6);
        assert_eq!(items[3], leaf.as_slice());
    }
}
