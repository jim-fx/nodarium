use myzel_core::{
    apply_settings, paths_mesh, read_float, read_int, read_paths, read_settings, HyphaeMeshParams, PathStyle,
};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let params = HyphaeMeshParams {
        resolution: read_int(args[1]).clamp(16, 256) as usize,
        fibres: read_float(args[2]).max(0.0),
        seed: read_int(args[3]) as u32,
        blend: read_float(args[4]),
        fill: read_float(args[5]).max(0.0),
        loose_strands: read_float(args[6]).clamp(0.0, 1.0),
    };
    let mut paths = read_paths(args[0], PathStyle::BODY);
    apply_settings(&mut paths, &read_settings(args[0]));
    paths_mesh(&paths, &params).encode()
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use myzel_core::{HyphaeMeshParams, Mesh};
    use nodarium_utils::{
        concat_arg_vecs, encode_float,
        geometry::{create_multiple_paths, wrap_multiple_paths},
    };

    fn run(paths: &[i32], d: &HyphaeMeshParams) -> Mesh {
        let args: Vec<Vec<i32>> = vec![
            paths.to_vec(),
            vec![d.resolution as i32],
            vec![encode_float(d.fibres)],
            vec![d.seed as i32],
            vec![encode_float(d.blend)],
            vec![encode_float(d.fill)],
            vec![encode_float(d.loose_strands)],
        ];
        let refs: Vec<&[i32]> = args.iter().map(|a| a.as_slice()).collect();
        Mesh::decode(&__nodarium_execute_inner(&refs))
    }

    /// `amount` upright paths side by side made by `creator`, starting at `x`
    fn paths(amount: usize, creator: i32, x: f32) -> Vec<i32> {
        let mut paths = create_multiple_paths(amount, 8, 1, creator);
        for (k, path) in wrap_multiple_paths(&mut paths).iter_mut().enumerate() {
            for i in 0..8 {
                path.points[i * 4..i * 4 + 4].copy_from_slice(&[x + k as f32 * 0.003, i as f32 * 0.005, 0.0, 0.001]);
            }
        }
        paths
    }

    #[test]
    fn the_defaults_match_the_core_defaults() {
        let d = HyphaeMeshParams::default();
        let def: serde_json::Value = serde_json::from_str(include_str!("definition.json")).unwrap();
        let value = |key: &str| def["inputs"][key]["value"].clone();
        assert_eq!(value("resolution_field"), d.resolution);
        for (key, expected) in [("fibres", d.fibres), ("blend", d.blend), ("fill", d.fill), ("loose_strands", d.loose_strands)] {
            assert!((value(key).as_f64().unwrap() as f32 - expected).abs() < 1e-7, "{}", key);
        }
    }

    #[test]
    fn paths_are_the_only_data_input() {
        let def: serde_json::Value = serde_json::from_str(include_str!("definition.json")).unwrap();
        let data: Vec<&String> =
            def["inputs"].as_object().unwrap().iter().filter(|(_, v)| v["type"] == "path").map(|(k, _)| k).collect();
        assert_eq!(data, ["paths"]);
    }

    #[test]
    fn every_creator_is_a_group() {
        let d = HyphaeMeshParams::default();
        let mesh = run(&paths(3, 42, 0.0), &d);
        assert!(mesh.face_count() > 0);
        assert!(mesh.groups.iter().all(|&g| g == 0));

        // lists inside lists, numbered in the order of their creators
        let all = concat_arg_vecs(vec![paths(2, 900, 0.0), paths(2, 17, 0.02), paths(2, 300, 0.04)]);
        let mesh = run(&all, &d);
        for v in 0..mesh.vertex_count() {
            let x = mesh.positions[v * 3];
            let expected = if x < 0.012 { 2 } else if x < 0.032 { 0 } else { 1 };
            assert_eq!(mesh.groups[v], expected, "vertex at x {}", x);
        }
    }

    #[test]
    fn the_same_paths_give_the_same_mesh() {
        let d = HyphaeMeshParams { seed: 5, ..HyphaeMeshParams::default() };
        let all = concat_arg_vecs(vec![paths(4, 1, 0.0), paths(2, 2, 0.03)]);
        assert_eq!(run(&all, &d), run(&all, &d));
    }

    #[test]
    fn loose_strands_keeps_a_strand_that_leaves_the_body() {
        // a stipe of 16 paths 30 mm high and a strand that leaves its top for 20 mm
        let mut body = create_multiple_paths(17, 11, 1, 7);
        for (k, path) in wrap_multiple_paths(&mut body).iter_mut().enumerate() {
            let a = k as f32 / 16.0 * std::f32::consts::TAU;
            let (x, z) = (0.005 * a.cos(), 0.005 * a.sin());
            for i in 0..11 {
                let t = i as f32 / 10.0;
                let point = if k < 16 { [x, 0.03 * t, z, 0.001] } else { [0.005, 0.03 + 0.02 * t, 0.0, 0.001] };
                path.points[i * 4..i * 4 + 4].copy_from_slice(&point);
            }
        }
        let top = |m: &Mesh| (0..m.vertex_count()).map(|v| m.positions[v * 3 + 1]).fold(f32::MIN, f32::max);
        let at = |loose_strands: f32| run(&body, &HyphaeMeshParams { loose_strands, ..HyphaeMeshParams::default() });
        // the closing (6 mm) leaves a low fillet where the strand was
        assert!(top(&at(0.0)) < 0.033, "gone at 0, ends at {}", top(&at(0.0)));
        assert!(top(&at(1.0)) > 0.05, "kept at 1, ends at {}", top(&at(1.0)));
    }

    #[test]
    fn settings_reach_the_paths_of_their_nodes() {
        use myzel_core::{with_setting, Setting};
        let d = HyphaeMeshParams::default();
        let all = concat_arg_vecs(vec![paths(3, 42, 0.0), paths(2, 9, 0.02), paths(2, 300, 0.04)]);
        let plain = run(&all, &d);
        // the values of the node, set on every node's paths: the same mesh
        let mut same = all.clone();
        for (setting, value) in [(Setting::Fibres, d.fibres), (Setting::Fill, d.fill)] {
            same = with_setting(&same, setting, value);
        }
        assert_eq!(run(&same, &d), plain);
        // no relief on any path is the node without relief
        let smooth = run(&all, &HyphaeMeshParams { fibres: 0.0, ..d.clone() });
        let flat = with_setting(&all, Setting::Fibres, 0.0);
        let mesh = run(&flat, &d);
        assert_eq!(mesh.faces, smooth.faces);
        assert_eq!(mesh.positions, smooth.positions);
    }

    #[test]
    fn a_fill_of_zero_keeps_the_gaps_between_paths_open() {
        use myzel_core::{with_setting, Setting};
        // 12 paths on a ring 8 mm wide, 3 mm apart: the default fill closes them into a tube
        let ring = |creator: i32| {
            let mut data = create_multiple_paths(12, 8, 1, creator);
            for (k, path) in wrap_multiple_paths(&mut data).iter_mut().enumerate() {
                let a = k as f32 / 12.0 * std::f32::consts::TAU;
                for i in 0..8 {
                    path.points[i * 4..i * 4 + 4].copy_from_slice(&[0.004 * a.cos(), i as f32 * 0.005, 0.004 * a.sin(), 0.001]);
                }
            }
            data
        };
        let d = HyphaeMeshParams { fibres: 0.0, ..HyphaeMeshParams::default() };
        let inner = |m: &Mesh| {
            (0..m.vertex_count()).any(|v| {
                let (x, y, z) = (m.positions[v * 3], m.positions[v * 3 + 1], m.positions[v * 3 + 2]);
                (0.01..0.03).contains(&y) && (x * x + z * z).sqrt() < 0.003
            })
        };
        assert!(!inner(&run(&ring(5), &d)), "closed tissue has no inside");
        assert!(inner(&run(&with_setting(&ring(5), Setting::Fill, 0.0), &d)), "the inner side of the paths shows");
    }

    #[test]
    fn colors_reach_their_paths() {
        use myzel_core::{color_value, opaque, with_setting, Setting, NO_COLOR};
        let d = HyphaeMeshParams::default();
        let white = [240, 236, 228];
        let body = concat_arg_vecs(vec![paths(3, 42, 0.0), paths(2, 9, 0.03)]);
        let plain = run(&body, &d);
        assert!(plain.colors.is_empty());
        // node 42 white, node 9 keeps the material away from the seam
        let set = with_setting(&body, Setting::Color, color_value(white));
        let only_42 = concat_arg_vecs(vec![with_setting(&paths(3, 42, 0.0), Setting::Color, color_value(white)), paths(2, 9, 0.03)]);
        let mesh = run(&only_42, &d);
        assert_eq!(mesh.positions, plain.positions);
        for (k, &c) in mesh.colors.iter().enumerate() {
            let x = mesh.positions[k * 3];
            if x < 0.0 {
                assert_eq!(c, opaque(white), "vertex at x {}", x);
            } else if x > 0.035 {
                assert_eq!(c, NO_COLOR, "vertex at x {}", x);
            }
        }
        assert!(run(&set, &d).colors.iter().all(|&c| c == opaque(white)));
    }

    #[test]
    fn nothing_connected_gives_an_empty_mesh() {
        assert_eq!(run(&[], &HyphaeMeshParams::default()), Mesh::default());
    }
}
