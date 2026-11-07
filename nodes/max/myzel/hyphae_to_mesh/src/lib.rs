use myzel_core::{paths_mesh, read_float, read_int, read_mesh_paths, HyphaeMeshParams, Mesh, PathStyle, MESH_TAG};
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
    };
    let mut paths = read_mesh_paths(args[0], PathStyle::BODY);
    paths.extend(read_mesh_paths(args[1], PathStyle::LOOSE));
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

/// One mesh as a node sends it, or a list of meshes from `join`
fn read_meshes(arg: &[i32]) -> Vec<Mesh> {
    let n = arg.len();
    match arg.first() {
        None => Vec::new(),
        Some(&MESH_TAG) => vec![Mesh::decode(arg)],
        // one item in brackets [0, n + 1, ...item, 1, 1]
        Some(0) if n >= 5 && arg[1] as usize == n - 3 && arg[2] == MESH_TAG => vec![Mesh::decode(&arg[2..n - 2])],
        Some(_) => split_args(arg).into_iter().filter(|item| item.len() < n).flat_map(read_meshes).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use myzel_core::{grow, hyphae_mesh, GrowthParams, HyphaeMeshParams, Mesh};
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
        let hyphae = grow(&GrowthParams { max_segments: 2000, ..GrowthParams::default() });
        let mesh = run(&hyphae.encode(), &[], &d);
        assert!(mesh.face_count() > 0);
        assert!(mesh.tissue_groups);
        assert_eq!(mesh, hyphae_mesh(&hyphae, &d));

        let def: serde_json::Value = serde_json::from_str(include_str!("definition.json")).unwrap();
        let value = |key: &str| def["inputs"][key]["value"].clone();
        assert_eq!(value("resolution_field"), d.resolution);
        for (key, expected) in [("fibres", d.fibres), ("blend", d.blend), ("fill", d.fill)] {
            assert!((value(key).as_f64().unwrap() as f32 - expected).abs() < 1e-7, "{}", key);
        }
    }

    #[test]
    fn every_creator_is_a_group() {
        let d = HyphaeMeshParams::default();
        let mesh = run(&paths(3, 42, 0.0), &[], &d);
        assert!(mesh.face_count() > 0);
        assert!(!mesh.tissue_groups);
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
            tissue_groups: false,
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
    }

    #[test]
    fn nothing_connected_gives_an_empty_mesh() {
        assert_eq!(run(&[], &[], &HyphaeMeshParams::default()), Mesh::default());
    }
}
