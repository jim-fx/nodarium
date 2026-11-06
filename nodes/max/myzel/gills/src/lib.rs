use myzel_core::{grow_gills, read_float, read_hyphae, read_int, GillParams};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let count = |arg: &[i32]| read_int(arg).max(0) as usize;
    let params = GillParams {
        seed: read_int(args[1]) as u32,
        gill_count: count(args[2]),
        lamellulae: count(args[3]),
        gill_depth: read_float(args[4]),
        veil: read_int(args[5]) == 0,
        veil_attachment: read_float(args[6]),
        max_segments: count(args[7]),
    };
    grow_gills(&read_hyphae(args[0]), &params).encode()
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use myzel_core::{grow, grow_gills, GillParams, GrowthParams, Hyphae};
    use nodarium_utils::encode_float;

    #[test]
    fn the_defaults_match_the_core_defaults() {
        let d = GillParams::default();
        let primordium = grow(&GrowthParams { max_segments: 2000, ..GrowthParams::default() }).encode();
        let args: Vec<Vec<i32>> = vec![
            primordium.clone(),
            vec![d.seed as i32],
            vec![d.gill_count as i32],
            vec![d.lamellulae as i32],
            vec![encode_float(d.gill_depth)],
            vec![if d.veil { 0 } else { 1 }],
            vec![encode_float(d.veil_attachment)],
            vec![d.max_segments as i32],
        ];
        let refs: Vec<&[i32]> = args.iter().map(|a| a.as_slice()).collect();
        let hyphae = Hyphae::decode(&__nodarium_execute_inner(&refs));
        assert_eq!(hyphae, grow_gills(&Hyphae::decode(&primordium), &d));

        let def: serde_json::Value = serde_json::from_str(include_str!("definition.json")).unwrap();
        let value = |key: &str| def["inputs"][key]["value"].clone();
        assert_eq!(value("gill_count"), d.gill_count);
        assert_eq!(value("lamellulae"), d.lamellulae);
        assert_eq!(value("max_segments"), d.max_segments);
        assert_eq!(value("veil"), 0);
        assert!((value("gill_depth").as_f64().unwrap() as f32 - d.gill_depth).abs() < 1e-7);
        assert!((value("veil_attachment").as_f64().unwrap() as f32 - d.veil_attachment).abs() < 1e-7);
    }
}
