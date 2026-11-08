use myzel_core::{color_value, paint_meshes, read_color, with_setting, Setting};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> (Vec<i32>, Vec<i32>) {
    let color = read_color(args[2]);
    (with_setting(args[0], Setting::Color, color_value(color)), paint_meshes(args[1], color))
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use myzel_core::{
        apply_settings, color_value, encode_color, opaque, read_paths, read_settings, Mesh, PathStyle, Setting,
        SettingItem,
    };
    use nodarium_utils::geometry::create_multiple_paths;

    const EMPTY: [i32; 4] = [0, 1, 1, 1];

    fn plate() -> Mesh {
        Mesh {
            faces: vec![0, 1, 2],
            positions: vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            normals: vec![0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0],
            colors: Vec::new(),
            groups: vec![0, 0, 0],
            tissue_groups: false,
        }
    }

    #[test]
    fn the_paths_pass_through_with_their_color() {
        let paths = create_multiple_paths(3, 4, 1, 12);
        let pink = [217, 160, 150];
        let (out, mesh) = __nodarium_execute_inner(&[&paths, &EMPTY, &encode_color(pink)]);
        assert_eq!(read_paths(&out, PathStyle::BODY), read_paths(&paths, PathStyle::BODY));
        let settings = read_settings(&out);
        assert_eq!(settings, vec![SettingItem { setting: Setting::Color, value: color_value(pink), creators: vec![12] }]);
        let mut read = read_paths(&out, PathStyle::BODY);
        apply_settings(&mut read, &settings);
        assert!(read.iter().all(|p| p.settings.color == Some(opaque(pink))));
        assert_eq!(mesh, EMPTY);
        // nothing connected
        assert_eq!(__nodarium_execute_inner(&[&EMPTY, &EMPTY, &encode_color(pink)]).0, EMPTY);
    }

    #[test]
    fn a_mesh_gets_the_color_all_over() {
        let brown = [74, 50, 38];
        let (paths, mesh) = __nodarium_execute_inner(&[&EMPTY, &plate().encode(), &encode_color(brown)]);
        assert_eq!(paths, EMPTY);
        let mesh = Mesh::decode(&mesh);
        assert_eq!(mesh.colors, vec![opaque(brown); 3]);
        assert_eq!(Mesh { colors: Vec::new(), ..mesh }, plate());
    }
}
