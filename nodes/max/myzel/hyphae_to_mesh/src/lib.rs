use myzel_core::{hyphae_mesh, read_color, read_float, read_hyphae, read_int, HyphaeMeshParams, TissueColors};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let params = HyphaeMeshParams {
        resolution: read_int(args[1]).clamp(16, 256) as usize,
        t: read_float(args[2]).clamp(0.0, 1.0),
        fibres: read_float(args[3]).max(0.0),
        seed: read_int(args[4]) as u32,
        colors: TissueColors {
            cuticle_young: read_color(args[5]),
            cuticle_old: read_color(args[6]),
            gill_young: read_color(args[7]),
            gill_mature: read_color(args[8]),
            stipe: read_color(args[9]),
            veil: read_color(args[10]),
            flesh: read_color(args[11]),
        },
        blend: read_float(args[12]),
    };
    hyphae_mesh(&read_hyphae(args[0]), &params).encode()
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use myzel_core::{grow, hyphae_mesh, GrowthParams, HyphaeMeshParams, Mesh, TissueColors};
    use nodarium_utils::encode_float;

    fn color(c: [f32; 3]) -> Vec<i32> {
        let [r, g, b] = c.map(|v| (v * 255.0).round() as i32);
        vec![0, 4, r, g, b, 1, 1]
    }

    #[test]
    fn the_defaults_match_the_core_defaults() {
        let d = HyphaeMeshParams::default();
        let c = TissueColors::default();
        let hyphae = grow(&GrowthParams { max_segments: 2000, ..GrowthParams::default() });
        let args: Vec<Vec<i32>> = vec![
            hyphae.encode(),
            vec![d.resolution as i32],
            vec![encode_float(d.t)],
            vec![encode_float(d.fibres)],
            vec![d.seed as i32],
            color(c.cuticle_young),
            color(c.cuticle_old),
            color(c.gill_young),
            color(c.gill_mature),
            color(c.stipe),
            color(c.veil),
            color(c.flesh),
            vec![encode_float(d.blend)],
        ];
        let refs: Vec<&[i32]> = args.iter().map(|a| a.as_slice()).collect();
        let mesh = Mesh::decode(&__nodarium_execute_inner(&refs));
        assert!(mesh.face_count() > 0);
        assert_eq!(mesh, hyphae_mesh(&hyphae, &d));

        let def: serde_json::Value = serde_json::from_str(include_str!("definition.json")).unwrap();
        let value = |key: &str| def["inputs"][key]["value"].clone();
        assert_eq!(value("resolution_field"), d.resolution);
        for (key, expected) in [("t", d.t), ("fibres", d.fibres), ("blend", d.blend)] {
            assert!((value(key).as_f64().unwrap() as f32 - expected).abs() < 1e-7, "{}", key);
        }
        for (key, expected) in [
            ("cuticle_young", c.cuticle_young),
            ("cuticle_old", c.cuticle_old),
            ("gill_young", c.gill_young),
            ("gill_mature", c.gill_mature),
            ("stipe", c.stipe),
            ("veil", c.veil),
            ("flesh", c.flesh),
        ] {
            assert_eq!(value(key), serde_json::json!(color(expected)[2..5]), "{}", key);
        }
    }
}
