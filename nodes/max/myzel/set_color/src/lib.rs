use myzel_core::{color_value, read_color, with_setting, Setting};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    with_setting(args[0], Setting::Color, color_value(read_color(args[1])))
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use myzel_core::{
        apply_settings, color_value, encode_color, opaque, read_paths, read_settings, PathStyle, Setting, SettingItem,
    };
    use nodarium_utils::geometry::create_multiple_paths;

    const EMPTY: [i32; 4] = [0, 1, 1, 1];

    #[test]
    fn the_paths_pass_through_with_one_setting() {
        let paths = create_multiple_paths(3, 4, 1, 12);
        let pink = [217, 160, 150];
        let out = __nodarium_execute_inner(&[&paths, &encode_color(pink)]);
        assert_eq!(read_paths(&out, PathStyle::BODY), read_paths(&paths, PathStyle::BODY));
        let settings = read_settings(&out);
        assert_eq!(settings, vec![SettingItem { setting: Setting::Color, value: color_value(pink), creators: vec![12] }]);
        let mut read = read_paths(&out, PathStyle::BODY);
        apply_settings(&mut read, &settings);
        assert!(read.iter().all(|p| p.settings.color == Some(opaque(pink))));
    }

    #[test]
    fn nothing_connected_gives_nothing() {
        assert_eq!(__nodarium_execute_inner(&[&EMPTY, &encode_color([217, 160, 150])]), EMPTY);
    }
}
