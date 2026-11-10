use myzel_core::{
    apply_settings, paths_mesh, read_float, read_int, read_paths, read_settings, HyphaeMeshParams, Mesh, PathStyle,
    MESH_TAG,
};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::split_args;

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let params = HyphaeMeshParams {
        resolution: read_int(args[2]).clamp(16, 256) as usize,
        fibres: read_float(args[3]).max(0.0),
        seed: read_int(args[4]) as u32,
        blend: read_float(args[5]),
        fill: read_float(args[6]).max(0.0),
        loose_strands: read_float(args[8]).clamp(0.0, 1.0),
    };
    let mut paths = read_paths(args[0], PathStyle::BODY);
    paths.extend(read_paths(args[1], PathStyle::LOOSE));
    // settings from set_blend, set_fibres, set_fill and set_color reach the paths of their nodes in either input
    let settings = [read_settings(args[0]), read_settings(args[1])].concat();
    apply_settings(&mut paths, &settings);
    let mut mesh = paths_mesh(&paths, &params);
    let meshes = read_meshes(args[7]);
    if !meshes.is_empty() {
        // every added mesh gets its own groups after the ones of the paths
        let mut next = mesh.groups.iter().max().map_or(0, |g| g + 1);
        for other in meshes.iter().filter(|m| m.vertex_count() > 0) {
            mesh.append(other, next);
            next += other.groups.iter().max().map_or(0, |g| g + 1);
        }
    }
    mesh.encode()
}

/// One mesh as a node sends it, or a list of meshes from `join`. The input also takes paths, so a
/// list can hold paths, geometry and settings too, these are left out.
fn read_meshes(arg: &[i32]) -> Vec<Mesh> {
    let n = arg.len();
    if arg.first() == Some(&MESH_TAG) {
        return Mesh::try_decode(arg).into_iter().collect();
    }
    if n < 5 || arg[0] != 0 {
        return Vec::new();
    }
    // one item in brackets [0, n + 1, ...item, 1, 1]
    if arg[1] as usize == n - 3 && arg[2] == MESH_TAG {
        return Mesh::try_decode(&arg[2..n - 2]).into_iter().collect();
    }
    // only a list [0, 1, ...items, 1, 1] is split, the count of any other item is its length
    if arg[1] == 1 {
        return split_args(arg).into_iter().filter(|item| item.len() < n).flat_map(read_meshes).collect();
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use myzel_core::{HyphaeMeshParams, Mesh};
    use nodarium_utils::{
        concat_arg_vecs, encode_float,
        geometry::{create_multiple_paths, wrap_multiple_paths},
    };

    fn run(body: &[i32], loose: &[i32], d: &HyphaeMeshParams) -> Mesh {
        run_with_meshes(body, loose, d, &[])
    }

    fn run_with_meshes(body: &[i32], loose: &[i32], d: &HyphaeMeshParams, meshes: &[i32]) -> Mesh {
        let args: Vec<Vec<i32>> = vec![
            body.to_vec(),
            loose.to_vec(),
            vec![d.resolution as i32],
            vec![encode_float(d.fibres)],
            vec![d.seed as i32],
            vec![encode_float(d.blend)],
            vec![encode_float(d.fill)],
            meshes.to_vec(),
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
    fn every_creator_is_a_group() {
        let d = HyphaeMeshParams::default();
        let mesh = run(&paths(3, 42, 0.0), &[], &d);
        assert!(mesh.face_count() > 0);
        assert!(mesh.groups.iter().all(|&g| g == 0));

        // body and loose paths, from lists inside lists, numbered in the order of their creators
        let body = concat_arg_vecs(vec![paths(2, 900, 0.0), paths(2, 17, 0.02)]);
        let mesh = run(&body, &paths(2, 300, 0.04), &d);
        for v in 0..mesh.vertex_count() {
            let x = mesh.positions[v * 3];
            let expected = if x < 0.012 { 2 } else if x < 0.032 { 0 } else { 1 };
            assert_eq!(mesh.groups[v], expected, "vertex at x {}", x);
        }
    }

    #[test]
    fn the_same_paths_give_the_same_mesh() {
        let d = HyphaeMeshParams { seed: 5, ..HyphaeMeshParams::default() };
        assert_eq!(run(&paths(4, 1, 0.0), &paths(2, 2, 0.03), &d), run(&paths(4, 1, 0.0), &paths(2, 2, 0.03), &d));
    }

    #[test]
    fn adds_meshes_with_their_own_groups() {
        let d = HyphaeMeshParams::default();
        let plain = run(&paths(3, 42, 0.0), &[], &d);
        let plate = Mesh {
            faces: vec![0, 1, 2],
            positions: vec![0.1, 0.0, 0.0, 0.2, 0.0, 0.0, 0.1, 0.1, 0.0],
            normals: vec![0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0],
            colors: Vec::new(),
            groups: vec![0, 0, 0],
        };
        let mesh = run_with_meshes(&paths(3, 42, 0.0), &[], &d, &plate.encode());
        // the paths as before, then the plate in the next group
        let v = plain.vertex_count();
        assert_eq!(mesh.vertex_count(), v + 3);
        assert_eq!(&mesh.positions[..v * 3], &plain.positions[..]);
        assert_eq!(&mesh.groups[v..], &[1, 1, 1]);
        assert_eq!(&mesh.faces[mesh.faces.len() - 3..], &[v as u32, v as u32 + 1, v as u32 + 2]);
        // two meshes from a join, one group each
        let joined = concat_arg_vecs(vec![nodarium_utils::wrap_arg(&plate.encode()), nodarium_utils::wrap_arg(&plate.encode())]);
        let mesh = run_with_meshes(&paths(3, 42, 0.0), &[], &d, &joined);
        assert_eq!(&mesh.groups[v..], &[1, 1, 1, 2, 2, 2]);
        // only a mesh
        assert_eq!(run_with_meshes(&[], &[], &d, &plate.encode()), plate);
        // nothing connected, the way the host sends an empty list
        assert_eq!(run_with_meshes(&paths(3, 42, 0.0), &[], &d, &[0, 1, 1, 1]), plain);
    }

    #[test]
    fn paths_in_the_meshes_are_left_out() {
        use nodarium_utils::{split_args, wrap_arg};
        let d = HyphaeMeshParams::default();
        let plain = run(&paths(3, 42, 0.0), &[], &d);
        // 16 paths like the strands of a membrane, the one indexed 14 has a 14 after its header
        let mut strands = paths(16, 14, 0.0);
        for path in wrap_multiple_paths(&mut strands).iter_mut() {
            path.points[0] = f32::from_bits(14);
        }
        assert!(split_args(&strands).iter().any(|item| item.contains(&14)));
        let with_strands = |meshes: &[i32]| run_with_meshes(&paths(3, 42, 0.0), &[], &d, meshes);
        assert_eq!(with_strands(&strands), plain);
        assert_eq!(with_strands(split_args(&strands)[14]), plain);

        // meshes and paths from one join: only the meshes are added, in their order
        let plate = |x: f32| Mesh {
            faces: vec![0, 1, 2],
            positions: vec![x, 0.0, 0.0, x + 0.1, 0.0, 0.0, x, 0.1, 0.0],
            normals: vec![0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0],
            colors: Vec::new(),
            groups: vec![0, 0, 0],
        };
        let mut items: Vec<Vec<i32>> = vec![wrap_arg(&plate(0.1).encode())];
        items.extend(split_args(&strands).iter().map(|item| item.to_vec()));
        items.push(wrap_arg(&plate(0.2).encode()));
        items.push(paths(2, 9, 0.04));
        let mixed = with_strands(&concat_arg_vecs(items));
        let mut expected = plain.clone();
        expected.append(&plate(0.1), 1);
        expected.append(&plate(0.2), 2);
        assert_eq!(mixed, expected);

        // a 14 in front of words that aren't a mesh, wrapped or not
        let not_a_mesh = [14, 2, 0, 0, 5, 9];
        assert_eq!(with_strands(&not_a_mesh), plain);
        assert_eq!(with_strands(&concat_arg_vecs(vec![wrap_arg(&not_a_mesh), wrap_arg(&plate(0.1).encode())])), {
            let mut one = plain.clone();
            one.append(&plate(0.1), 1);
            one
        });
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
        let highest = |m: &Mesh| (0..m.vertex_count()).map(|v| m.positions[v * 3 + 1]).fold(f32::MIN, f32::max);
        let at = |loose_strands: f32| run(&body, &[], &HyphaeMeshParams { loose_strands, ..HyphaeMeshParams::default() });
        // the closing (6 mm) leaves a low fillet where the strand was
        assert!(highest(&at(0.0)) < 0.033, "gone at 0, ends at {}", highest(&at(0.0)));
        assert!(highest(&at(1.0)) > 0.05, "kept at 1, ends at {}", highest(&at(1.0)));
    }

    #[test]
    fn settings_reach_the_paths_of_their_nodes() {
        use myzel_core::{with_setting, Setting, SettingItem};
        use nodarium_utils::{split_args, wrap_arg};
        let d = HyphaeMeshParams::default();
        let (body, loose) = (concat_arg_vecs(vec![paths(3, 42, 0.0), paths(2, 9, 0.02)]), paths(2, 300, 0.04));
        let plain = run(&body, &loose, &d);
        // the values of the node, set on every node's paths: the same mesh
        let mut same_body = body.clone();
        for (setting, value) in [(Setting::Blend, d.blend), (Setting::Fibres, d.fibres), (Setting::Fill, d.fill)] {
            same_body = with_setting(&same_body, setting, value);
        }
        let same_loose = with_setting(&loose, Setting::Blend, d.blend);
        assert_eq!(run(&same_body, &same_loose, &d), plain);
        // no relief on any path is the node without relief, a setting in the loose input reaches the body paths too
        let smooth = run(&body, &loose, &HyphaeMeshParams { fibres: 0.0, ..d.clone() });
        let for_nine = SettingItem { setting: Setting::Fibres, value: 0.0, creators: vec![9] };
        let flat_loose = with_setting(&loose, Setting::Fibres, 0.0);
        let flat_loose: Vec<Vec<i32>> =
            split_args(&flat_loose).iter().map(|i| i.to_vec()).chain([wrap_arg(&for_nine.encode())]).collect();
        let flat_body = concat_arg_vecs(vec![with_setting(&paths(3, 42, 0.0), Setting::Fibres, 0.0), paths(2, 9, 0.02)]);
        let mesh = run(&flat_body, &concat_arg_vecs(flat_loose), &d);
        assert_eq!(mesh.faces, smooth.faces);
        assert_eq!(mesh.positions, smooth.positions);
    }

    #[test]
    fn colors_reach_their_paths_and_meshes() {
        use myzel_core::{color_value, opaque, paint_mesh, with_setting, Setting, NO_COLOR};
        let d = HyphaeMeshParams::default();
        let (white, brown) = ([240, 236, 228], [80, 52, 40]);
        let body = concat_arg_vecs(vec![paths(3, 42, 0.0), paths(2, 9, 0.03)]);
        let plain = run(&body, &[], &d);
        assert!(plain.colors.is_empty());
        let mut plate = Mesh {
            faces: vec![0, 1, 2],
            positions: vec![0.1, 0.0, 0.0, 0.2, 0.0, 0.0, 0.1, 0.1, 0.0],
            normals: vec![0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0],
            colors: Vec::new(),
            groups: vec![0, 0, 0],
        };
        // only the plate has a color, the paths keep the material
        paint_mesh(&mut plate, brown);
        let mesh = run_with_meshes(&body, &[], &d, &plate.encode());
        let v = plain.vertex_count();
        assert_eq!(&mesh.positions[..v * 3], &plain.positions[..]);
        assert!(mesh.colors[..v].iter().all(|&c| c == NO_COLOR));
        assert_eq!(&mesh.colors[v..], &[opaque(brown); 3]);
        // a join of a painted mesh and one without colors, like gill plates and a skin
        let unpainted = Mesh { colors: Vec::new(), ..plate.clone() };
        let gills = myzel_core::paint_meshes(&concat_arg_vecs(vec![nodarium_utils::wrap_arg(&unpainted.encode())]), brown);
        let joined = concat_arg_vecs(
            nodarium_utils::split_args(&gills).iter().map(|i| i.to_vec()).chain([nodarium_utils::wrap_arg(&unpainted.encode())]).collect(),
        );
        let mesh = run_with_meshes(&body, &[], &d, &joined);
        assert_eq!(&mesh.colors[v..], &[[opaque(brown); 3], [NO_COLOR; 3]].concat());
        // node 42 white, node 9 keeps the material away from the seam
        let set = with_setting(&body, Setting::Color, color_value(white));
        let only_42 = concat_arg_vecs(vec![with_setting(&paths(3, 42, 0.0), Setting::Color, color_value(white)), paths(2, 9, 0.03)]);
        let mesh = run(&only_42, &[], &d);
        assert_eq!(mesh.positions, plain.positions);
        for (k, &c) in mesh.colors.iter().enumerate() {
            let x = mesh.positions[k * 3];
            if x < 0.0 {
                assert_eq!(c, opaque(white), "vertex at x {}", x);
            } else if x > 0.035 {
                assert_eq!(c, NO_COLOR, "vertex at x {}", x);
            }
        }
        assert!(run(&set, &[], &d).colors.iter().all(|&c| c == opaque(white)));
    }

    #[test]
    fn nothing_connected_gives_an_empty_mesh() {
        assert_eq!(run(&[], &[], &HyphaeMeshParams::default()), Mesh::default());
    }
}
