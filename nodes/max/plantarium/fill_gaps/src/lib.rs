use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{concat_arg_vecs, geometry::fill_gaps, int, split_args, Ctx};

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let mut ctx = Ctx::default();
    let items = split_args(args[0]);
    let spacing = ctx.float(args[1]).max(0.0);
    let seed = int(args[2]);
    let added = fill_gaps(&items, spacing, seed);
    concat_arg_vecs(items.iter().map(|p| p.to_vec()).chain(added).collect())
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use nodarium_utils::{
        concat_arg_vecs, encode_float,
        geometry::{create_path, set_path_link, wrap_path_mut, PathLink},
        split_args,
    };
    use std::f32::consts::TAU;

    fn ring() -> Vec<i32> {
        let paths = (0..8)
            .map(|k| {
                let angle = TAU * k as f32 / 8.0;
                let mut data = create_path(12, 2);
                set_path_link(&mut data, PathLink::child(5, 99, 1.0, k));
                let path = wrap_path_mut(&mut data);
                for i in 0..12 {
                    let r = 0.03 * i as f32 / 11.0;
                    path.points[i * 4..i * 4 + 4].copy_from_slice(&[r * angle.cos(), -0.2 * r, r * angle.sin(), 0.001]);
                }
                data
            })
            .collect();
        concat_arg_vecs(paths)
    }

    #[test]
    fn keeps_the_paths_and_adds_new_ones() {
        let input = ring();
        let none = __nodarium_execute_inner(&[&input, &[encode_float(0.0)], &[1]]);
        assert_eq!(split_args(&none), split_args(&input));
        let filled = __nodarium_execute_inner(&[&input, &[encode_float(1.0)], &[1]]);
        let paths = split_args(&filled);
        assert!(paths.len() > 8);
        assert_eq!(&paths[..8], split_args(&input).as_slice());
    }
}
