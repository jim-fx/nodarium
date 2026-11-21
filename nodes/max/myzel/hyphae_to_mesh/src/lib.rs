use myzel_core::{assign_bodies, paths_mesh, read_float, read_int, read_paths, HyphaeMeshParams, PathStyle};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let params = HyphaeMeshParams {
        resolution: read_int(args[2]).clamp(16, 256) as usize,
        fibres: read_float(args[3]).max(0.0),
        seed: read_int(args[4]) as u32,
        fill: read_float(args[5]).max(0.0),
        ..HyphaeMeshParams::default()
    };
    let mut paths = read_paths(args[0], PathStyle::BODY);
    assign_bodies(&mut paths, &read_paths(args[1], PathStyle::BODY));
    paths_mesh(&paths, &params).encode()
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use myzel_core::{HyphaeMeshParams, Mesh};
    use nodarium_utils::{
        concat_arg_vecs, encode_float,
        geometry::{
            create_multiple_paths, create_path, set_path_link, with_path_fibres, with_path_fill, wrap_multiple_paths, wrap_path,
            wrap_path_mut, PathLink, NO_PARENT,
        },
        split_args,
    };

    /// What an input without a link gets
    const UNCONNECTED: [i32; 4] = [0, 1, 1, 1];

    fn run(paths: &[i32], d: &HyphaeMeshParams) -> Mesh {
        run_bodies(paths, &UNCONNECTED, d)
    }

    fn run_bodies(paths: &[i32], bodies: &[i32], d: &HyphaeMeshParams) -> Mesh {
        let args: Vec<Vec<i32>> = vec![
            paths.to_vec(),
            bodies.to_vec(),
            vec![d.resolution as i32],
            vec![encode_float(d.fibres)],
            vec![d.seed as i32],
            vec![encode_float(d.fill)],
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
        for (key, expected) in [("fibres", d.fibres), ("fill", d.fill)] {
            assert!((value(key).as_f64().unwrap() as f32 - expected).abs() < 1e-7, "{}", key);
        }
    }

    #[test]
    fn paths_and_bodies_are_the_only_data_inputs() {
        let def: serde_json::Value = serde_json::from_str(include_str!("definition.json")).unwrap();
        let mut data: Vec<&String> =
            def["inputs"].as_object().unwrap().iter().filter(|(_, v)| v["type"] == "path").map(|(k, _)| k).collect();
        data.sort();
        assert_eq!(data, ["bodies", "paths"]);
    }

    /// An upright path of 8 points at `x`, 1 mm thick, made by `creator` on the path `parent`
    fn upright(creator: i32, index: i32, parent: i32, x: f32) -> Vec<i32> {
        let mut path = create_path(8, 1);
        let link = if parent == NO_PARENT { PathLink::root(creator, index) } else { PathLink::child(creator, parent, 0.0, index) };
        set_path_link(&mut path, link);
        let data = wrap_path_mut(&mut path);
        for i in 0..8 {
            data.points[i * 4..i * 4 + 4].copy_from_slice(&[x, i as f32 * 0.005, 0.0, 0.001]);
        }
        path
    }

    fn id(path: &[i32]) -> i32 {
        wrap_path(path).link.id
    }

    /// Two stipes as guides, and two strands 3 mm apart on each, 1.5 mm between the bundles
    fn two_bundles() -> (Vec<i32>, Vec<i32>) {
        let (a, b) = (upright(10, 0, NO_PARENT, 0.0), upright(10, 1, NO_PARENT, 0.02));
        let strands = concat_arg_vecs(vec![
            upright(20, 0, id(&a), 0.0),
            upright(20, 1, id(&a), 0.003),
            upright(20, 2, id(&b), 0.0065),
            upright(20, 3, id(&b), 0.0095),
        ]);
        (strands, concat_arg_vecs(vec![a, b]))
    }

    /// A closing of 1.5 mm fills the 1.5 mm between the bundles, a larger one would blur them away
    fn fine() -> HyphaeMeshParams {
        HyphaeMeshParams { resolution: 128, fill: 1.5, ..HyphaeMeshParams::default() }
    }

    /// The number of separate pieces of a mesh
    fn pieces(mesh: &Mesh) -> usize {
        let mut parent: Vec<usize> = (0..mesh.vertex_count()).collect();
        fn find(parent: &mut [usize], i: usize) -> usize {
            let mut i = i;
            while parent[i] != i {
                parent[i] = parent[parent[i]];
                i = parent[i];
            }
            i
        }
        for f in mesh.faces.chunks_exact(3) {
            for (a, b) in [(f[0], f[1]), (f[1], f[2])] {
                let (a, b) = (find(&mut parent, a as usize), find(&mut parent, b as usize));
                parent[a] = b;
            }
        }
        (0..mesh.vertex_count()).filter(|&v| find(&mut parent, v) == v).count()
    }

    #[test]
    fn bundles_of_two_bodies_keep_their_own_surfaces() {
        let (strands, stipes) = two_bundles();
        let one = run(&strands, &fine());
        assert_eq!(pieces(&one), 1, "without bodies the gap between the bundles fills");
        let two = run_bodies(&strands, &stipes, &fine());
        assert_eq!(pieces(&two), 2, "every body has a surface of its own");
        assert!(two.groups.iter().all(|&g| g == 0));
        assert_eq!(two.creators, one.creators);
        // nothing of the surface in the middle of the gap
        assert!(two.positions.chunks_exact(3).filter(|p| (0.005..0.015).contains(&p[1])).all(|p| (p[0] - 0.00475).abs() > 0.0003));
    }

    #[test]
    fn strands_on_rays_of_one_stipe_are_one_body() {
        let stipe = upright(10, 0, NO_PARENT, 0.0);
        let rays = [upright(11, 0, id(&stipe), 0.0), upright(11, 1, id(&stipe), 0.006)];
        let strands = concat_arg_vecs(vec![upright(20, 0, id(&rays[0]), 0.0), upright(20, 1, id(&rays[1]), 0.0035)]);
        let guides = concat_arg_vecs(vec![stipe, rays[0].clone(), rays[1].clone()]);
        let mesh = run_bodies(&strands, &guides, &fine());
        assert_eq!(pieces(&mesh), 1);
        assert_eq!(mesh, run(&strands, &fine()));
    }

    #[test]
    fn strands_whose_parent_is_no_body_share_one() {
        let (strands, _) = two_bundles();
        let others = concat_arg_vecs(vec![upright(10, 5, NO_PARENT, 0.1), upright(10, 6, NO_PARENT, 0.2)]);
        assert_eq!(run_bodies(&strands, &others, &fine()), run(&strands, &fine()));
        // the strands of a stipe keep apart from those that grew on no body, which fill into one
        let (a, b) = (upright(10, 0, NO_PARENT, 0.0), upright(10, 1, NO_PARENT, 0.02));
        let strands = concat_arg_vecs(vec![
            upright(20, 0, id(&a), 0.0),
            upright(20, 1, id(&a), 0.003),
            upright(20, 2, id(&b), 0.0065),
            upright(20, 3, 12345, 0.0095),
            upright(20, 4, NO_PARENT, 0.0125),
        ]);
        let mesh = run_bodies(&strands, &concat_arg_vecs(vec![a]), &fine());
        assert_eq!(pieces(&mesh), 2);
    }

    #[test]
    fn without_bodies_the_mesh_stays_as_it_was() {
        let (strands, _) = two_bundles();
        let plain = run(&strands, &fine());
        assert_eq!(run_bodies(&strands, &[], &fine()), plain);
        assert_eq!(run_bodies(&strands, &concat_arg_vecs(vec![]), &fine()), plain);
    }

    #[test]
    fn the_same_bodies_give_the_same_mesh() {
        let (strands, stipes) = two_bundles();
        assert_eq!(run_bodies(&strands, &stipes, &fine()), run_bodies(&strands, &stipes, &fine()));
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

    /// Every path of a list with `value` at every point, written by `give`
    fn with_value(list: &[i32], value: f32, give: fn(&[i32], &[f32]) -> Vec<i32>) -> Vec<i32> {
        myzel_core::with_values_along(list, &[encode_float(value)], give)
    }

    #[test]
    fn the_values_of_the_points_reach_the_mesh() {
        let d = HyphaeMeshParams::default();
        let all = concat_arg_vecs(vec![paths(3, 42, 0.0), paths(2, 9, 0.02), paths(2, 300, 0.04)]);
        let plain = run(&all, &d);
        // the values of the node at every point: the same mesh
        let same = with_value(&with_value(&all, d.fibres, with_path_fibres), d.fill, with_path_fill);
        assert_eq!(run(&same, &d), plain);
        // no relief on any path is the node without relief
        let smooth = run(&all, &HyphaeMeshParams { fibres: 0.0, ..d.clone() });
        let mesh = run(&with_value(&all, 0.0, with_path_fibres), &d);
        assert_eq!(mesh.faces, smooth.faces);
        assert_eq!(mesh.positions, smooth.positions);
    }

    #[test]
    fn a_fill_of_zero_keeps_the_gaps_between_paths_open() {
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
        assert!(inner(&run(&with_value(&ring(5), 0.0, with_path_fill), &d)), "the inner side of the paths shows");
    }

    #[test]
    fn the_colors_of_the_points_color_the_surface() {
        use myzel_core::{opaque, NO_COLOR};
        use nodarium_utils::geometry::with_path_colors;
        let d = HyphaeMeshParams::default();
        let white = [240, 236, 228];
        let colored = |list: Vec<i32>| {
            concat_arg_vecs(split_args(&list).iter().map(|path| with_path_colors(path, &[opaque(white); 8])).collect())
        };
        let body = concat_arg_vecs(vec![paths(3, 42, 0.0), paths(2, 9, 0.03)]);
        let plain = run(&body, &d);
        assert!(plain.colors.is_empty());
        // node 42 white, node 9 keeps the material away from the seam
        let only_42 = concat_arg_vecs(vec![colored(paths(3, 42, 0.0)), paths(2, 9, 0.03)]);
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
        let all = concat_arg_vecs(vec![colored(paths(3, 42, 0.0)), colored(paths(2, 9, 0.03))]);
        assert!(run(&all, &d).colors.iter().all(|&c| c == opaque(white)));
    }

    #[test]
    fn nothing_connected_gives_an_empty_mesh() {
        assert_eq!(run(&[], &HyphaeMeshParams::default()), Mesh::default());
    }
}
