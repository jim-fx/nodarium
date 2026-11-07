use myzel_core::{read_float, with_setting, Setting};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    with_setting(args[0], Setting::Fill, read_float(args[1]).max(0.0))
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use myzel_core::{read_paths, read_settings, HyphaeMeshParams, PathStyle, Setting, SettingItem};
    use nodarium_utils::{encode_float, geometry::create_multiple_paths};

    #[test]
    fn the_paths_pass_through_with_their_fill() {
        let paths = create_multiple_paths(2, 4, 1, 7);
        let out = __nodarium_execute_inner(&[&paths, &[encode_float(-2.0)]]);
        assert_eq!(read_paths(&out, PathStyle::BODY), read_paths(&paths, PathStyle::BODY));
        // a fill below 0 is 0
        assert_eq!(read_settings(&out), vec![SettingItem { setting: Setting::Fill, value: 0.0, creators: vec![7] }]);
    }

    #[test]
    fn the_default_is_the_one_of_hyphae_to_mesh() {
        let def: serde_json::Value = serde_json::from_str(include_str!("definition.json")).unwrap();
        assert_eq!(def["inputs"]["fill"]["value"].as_f64().unwrap() as f32, HyphaeMeshParams::default().fill);
    }
}
