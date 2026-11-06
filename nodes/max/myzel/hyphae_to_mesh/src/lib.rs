use myzel_core::{hyphae_mesh, paths_mesh, read_float, read_hyphae, read_int, HyphaeMeshParams, MeshPath, HYPHAE_TAG};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{
    geometry::{is_path, wrap_path},
    split_args,
};

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let params = HyphaeMeshParams {
        resolution: read_int(args[1]).clamp(16, 256) as usize,
        fibres: read_float(args[2]).max(0.0),
        seed: read_int(args[3]) as u32,
        blend: read_float(args[4]),
        fill: read_float(args[5]).max(0.0),
    };
    if args[0].first() == Some(&HYPHAE_TAG) {
        return hyphae_mesh(&read_hyphae(args[0]), &params).encode();
    }
    let items = split_args(args[0]);
    let paths: Vec<MeshPath> = items
        .iter()
        .filter(|item| is_path(item))
        .map(|item| {
            let path = wrap_path(item);
            MeshPath { points: path.points, group: path.link.creator as u32 }
        })
        .collect();
    paths_mesh(&paths, &params).encode()
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use myzel_core::{grow, hyphae_mesh, GrowthParams, HyphaeMeshParams, Mesh};
    use nodarium_utils::encode_float;

    #[test]
    fn the_defaults_match_the_core_defaults() {
        let d = HyphaeMeshParams::default();
        let hyphae = grow(&GrowthParams { max_segments: 2000, ..GrowthParams::default() });
        let args: Vec<Vec<i32>> = vec![
            hyphae.encode(),
            vec![d.resolution as i32],
            vec![encode_float(d.fibres)],
            vec![d.seed as i32],
            vec![encode_float(d.blend)],
            vec![encode_float(d.fill)],
        ];
        let refs: Vec<&[i32]> = args.iter().map(|a| a.as_slice()).collect();
        let mesh = Mesh::decode(&__nodarium_execute_inner(&refs));
        assert!(mesh.face_count() > 0);
        assert_eq!(mesh, hyphae_mesh(&hyphae, &d));

        let def: serde_json::Value = serde_json::from_str(include_str!("definition.json")).unwrap();
        let value = |key: &str| def["inputs"][key]["value"].clone();
        assert_eq!(value("resolution_field"), d.resolution);
        for (key, expected) in [("fibres", d.fibres), ("blend", d.blend), ("fill", d.fill)] {
            assert!((value(key).as_f64().unwrap() as f32 - expected).abs() < 1e-7, "{}", key);
        }
    }

    #[test]
    fn paths_are_meshed_in_the_group_of_their_creator() {
        use nodarium_utils::geometry::{create_multiple_paths, wrap_multiple_paths};
        let mut paths = create_multiple_paths(3, 8, 1, 42);
        for (k, path) in wrap_multiple_paths(&mut paths).iter_mut().enumerate() {
            for i in 0..8 {
                path.points[i * 4..i * 4 + 4].copy_from_slice(&[k as f32 * 0.003, i as f32 * 0.005, 0.0, 0.001]);
            }
        }
        let d = HyphaeMeshParams::default();
        let args: Vec<Vec<i32>> = vec![
            paths,
            vec![d.resolution as i32],
            vec![encode_float(d.fibres)],
            vec![d.seed as i32],
            vec![encode_float(d.blend)],
            vec![encode_float(d.fill)],
        ];
        let refs: Vec<&[i32]> = args.iter().map(|a| a.as_slice()).collect();
        let mesh = Mesh::decode(&__nodarium_execute_inner(&refs));
        assert!(mesh.face_count() > 0);
        assert!(mesh.groups.iter().all(|&g| g == 42));
    }
}
