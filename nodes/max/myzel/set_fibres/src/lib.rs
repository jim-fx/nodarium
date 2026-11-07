use myzel_core::{read_float, with_setting, Setting};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    with_setting(args[0], Setting::Fibres, read_float(args[1]).max(0.0))
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use myzel_core::{read_paths, read_settings, HyphaeMeshParams, PathStyle, Setting, SettingItem};
    use nodarium_utils::{encode_float, geometry::create_multiple_paths};

    #[test]
    fn the_paths_pass_through_with_their_fibres() {
        let paths = create_multiple_paths(3, 4, 1, 12);
        let out = __nodarium_execute_inner(&[&paths, &[encode_float(1.5)]]);
        assert_eq!(read_paths(&out, PathStyle::BODY), read_paths(&paths, PathStyle::BODY));
        assert_eq!(read_settings(&out), vec![SettingItem { setting: Setting::Fibres, value: 1.5, creators: vec![12] }]);
        // nothing connected
        assert_eq!(__nodarium_execute_inner(&[&[0, 1, 1, 1], &[encode_float(1.5)]]), vec![0, 1, 1, 1]);
    }

    #[test]
    fn the_default_is_the_one_of_hyphae_to_mesh() {
        let def: serde_json::Value = serde_json::from_str(include_str!("definition.json")).unwrap();
        assert_eq!(def["inputs"]["fibres"]["value"].as_f64().unwrap() as f32, HyphaeMeshParams::default().fibres);
    }
}
