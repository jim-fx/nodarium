use myzel_core::{read_float, with_setting, Setting};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    with_setting(args[0], Setting::Blend, read_float(args[1]))
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use myzel_core::{read_paths, read_settings, HyphaeMeshParams, PathStyle, Setting, SettingItem};
    use nodarium_utils::{concat_arg_vecs, encode_float, geometry::create_multiple_paths};

    #[test]
    fn the_paths_pass_through_with_their_blend() {
        let paths = concat_arg_vecs(vec![create_multiple_paths(2, 4, 1, 3), create_multiple_paths(1, 4, 1, 40)]);
        let out = __nodarium_execute_inner(&[&paths, &[encode_float(1.8)]]);
        assert_eq!(read_paths(&out, PathStyle::LOOSE), read_paths(&paths, PathStyle::LOOSE));
        assert_eq!(read_settings(&out), vec![SettingItem { setting: Setting::Blend, value: 1.8, creators: vec![3, 40] }]);
    }

    #[test]
    fn the_default_is_the_one_of_hyphae_to_mesh() {
        let def: serde_json::Value = serde_json::from_str(include_str!("definition.json")).unwrap();
        assert_eq!(def["inputs"]["blend"]["value"].as_f64().unwrap() as f32, HyphaeMeshParams::default().blend);
    }
}
