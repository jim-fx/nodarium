use myzel_core::with_values_along;
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::geometry::with_path_fill;

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    with_values_along(args[0], args[1], with_path_fill)
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use myzel_core::{read_paths, HyphaeMeshParams, PathStyle};
    use nodarium_utils::{encode_float, geometry::create_multiple_paths};

    #[test]
    fn the_paths_pass_through_with_their_fill() {
        let paths = create_multiple_paths(2, 4, 1, 7);
        let out = __nodarium_execute_inner(&[&paths, &[encode_float(-2.0)]]);
        let (before, after) = (read_paths(&paths, PathStyle::BODY), read_paths(&out, PathStyle::BODY));
        assert_eq!(after.len(), 2);
        for (a, b) in after.iter().zip(&before) {
            assert_eq!((&a.points, a.group), (&b.points, b.group));
            // a fill below 0 is 0
            assert_eq!(&*a.fill, &[0.0; 4]);
            assert!(a.fibres.is_empty());
        }
    }

    #[test]
    fn the_default_is_the_one_of_hyphae_to_mesh() {
        let def: serde_json::Value = serde_json::from_str(include_str!("definition.json")).unwrap();
        assert_eq!(def["inputs"]["fill"]["value"].as_f64().unwrap() as f32, HyphaeMeshParams::default().fill);
    }
}
