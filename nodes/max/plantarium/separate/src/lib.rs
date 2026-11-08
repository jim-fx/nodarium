use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{
    concat_arg_vecs,
    geometry::{is_path, path_link},
    split_args,
};

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> (Vec<i32>, Vec<i32>) {
    let items = split_args(args[0]);
    let mut creators: Vec<i32> = split_args(args[1]).iter().filter(|p| is_path(p)).map(|p| path_link(p).creator).collect();
    creators.sort_unstable();
    creators.dedup();
    let (taken, rest): (Vec<&[i32]>, Vec<&[i32]>) =
        items.into_iter().partition(|p| is_path(p) && creators.binary_search(&path_link(p).creator).is_ok());
    let owned = |list: Vec<&[i32]>| concat_arg_vecs(list.into_iter().map(|p| p.to_vec()).collect());
    (owned(taken), owned(rest))
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use nodarium_utils::{
        concat_arg_vecs,
        geometry::{create_path, set_path_link, PathLink},
        split_args,
    };

    fn path(creator: i32, index: i32) -> Vec<i32> {
        let mut data = create_path(3, 2);
        set_path_link(&mut data, PathLink::child(creator, 1, 1.0, index));
        data
    }

    #[test]
    fn takes_out_the_paths_of_the_same_nodes() {
        let (a, b) = (vec![path(4, 0), path(4, 1)], vec![path(5, 0)]);
        let joined = concat_arg_vecs(vec![a[0].clone(), b[0].clone(), a[1].clone(), path(4, 7)]);
        let (taken, rest) = __nodarium_execute_inner(&[&joined, &concat_arg_vecs(a.clone())]);
        assert_eq!(split_args(&taken), vec![a[0].as_slice(), a[1].as_slice(), path(4, 7).as_slice()]);
        assert_eq!(split_args(&rest), vec![b[0].as_slice()]);
    }
}
