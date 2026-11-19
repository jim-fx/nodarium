use myzel_core::with_values_along;
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::geometry::with_path_fibres;

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    with_values_along(args[0], args[1], with_path_fibres)
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use myzel_core::{read_paths, HyphaeMeshParams, PathStyle};
    use nodarium_utils::{
        concat_args, encode_float,
        geometry::{create_multiple_paths, with_path_colors, wrap_multiple_paths},
        split_args,
    };

    fn strands(amount: usize, creator: i32) -> Vec<i32> {
        let mut paths = create_multiple_paths(amount, 4, 1, creator);
        for (k, path) in wrap_multiple_paths(&mut paths).iter_mut().enumerate() {
            for i in 0..4 {
                path.points[i * 4..i * 4 + 4].copy_from_slice(&[k as f32, i as f32, 0.0, 0.1]);
            }
        }
        paths
    }

    #[test]
    fn the_paths_pass_through_with_their_fibres() {
        let paths = strands(3, 12);
        let out = __nodarium_execute_inner(&[&paths, &[encode_float(1.5)]]);
        let (before, after) = (read_paths(&paths, PathStyle::BODY), read_paths(&out, PathStyle::BODY));
        assert_eq!(after.len(), 3);
        for (a, b) in after.iter().zip(&before) {
            assert_eq!((&a.points, a.group), (&b.points, b.group));
            assert_eq!(&*a.fibres, &[1.5; 4]);
            assert!(a.fill.is_empty());
        }
        // nothing connected
        assert_eq!(__nodarium_execute_inner(&[&[0, 1, 1, 1], &[encode_float(1.5)]]), vec![0, 1, 1, 1]);
    }

    #[test]
    fn colors_stay_and_fibres_are_read_along_the_path() {
        let colored: Vec<Vec<i32>> = split_args(&strands(1, 4)).iter().map(|p| with_path_colors(p, &[7; 4])).collect();
        let colored = concat_args(colored.iter().map(|p| p.as_slice()).collect());
        // the alpha node, from 0 at the base to 1 at the tip
        let alpha = concat_args(vec![&[2], &[0]]);
        let out = __nodarium_execute_inner(&[&colored, &alpha]);
        let path = &read_paths(&out, PathStyle::BODY)[0];
        assert_eq!(&*path.colors, &[7; 4]);
        assert_eq!(&*path.fibres, &[0.0, 1.0 / 3.0, 2.0 / 3.0, 1.0]);
    }

    #[test]
    fn the_default_is_the_one_of_hyphae_to_mesh() {
        let def: serde_json::Value = serde_json::from_str(include_str!("definition.json")).unwrap();
        assert_eq!(def["inputs"]["fibres"]["value"].as_f64().unwrap() as f32, HyphaeMeshParams::default().fibres);
    }
}
