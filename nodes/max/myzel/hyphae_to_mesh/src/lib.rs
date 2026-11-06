use myzel_core::{hyphae_mesh, read_float, read_hyphae, read_int, HyphaeMeshParams};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let params = HyphaeMeshParams {
        resolution: read_int(args[1]).clamp(16, 256) as usize,
        fibres: read_float(args[2]).max(0.0),
        seed: read_int(args[3]) as u32,
        blend: read_float(args[4]),
    };
    hyphae_mesh(&read_hyphae(args[0]), &params).encode()
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
        ];
        let refs: Vec<&[i32]> = args.iter().map(|a| a.as_slice()).collect();
        let mesh = Mesh::decode(&__nodarium_execute_inner(&refs));
        assert!(mesh.face_count() > 0);
        assert_eq!(mesh, hyphae_mesh(&hyphae, &d));

        let def: serde_json::Value = serde_json::from_str(include_str!("definition.json")).unwrap();
        let value = |key: &str| def["inputs"][key]["value"].clone();
        assert_eq!(value("resolution_field"), d.resolution);
        for (key, expected) in [("fibres", d.fibres), ("blend", d.blend)] {
            assert!((value(key).as_f64().unwrap() as f32 - expected).abs() < 1e-7, "{}", key);
        }
    }
}
