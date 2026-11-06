use myzel_core::{expand, read_float, read_floats, read_hyphae, read_int, ExpandParams, Schedule};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let params = ExpandParams {
        t: read_float(args[1]),
        seed: read_int(args[2]) as u32,
        elongation: Schedule(read_floats(args[3])),
        opening: Schedule(read_floats(args[4])),
        stipe_stretch: read_float(args[5]),
        apical_stretch: read_float(args[6]),
        stipe_thickening: read_float(args[7]),
        cap_spread: read_float(args[8]),
        open_angle: read_float(args[9]),
        gill_deepening: read_float(args[10]),
        inflation: read_float(args[11]),
        veil_extensibility: read_float(args[12]),
        tear_position: read_float(args[13]),
    };
    expand(&read_hyphae(args[0]), &params).encode()
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use myzel_core::{expand, grow, grow_gills, ExpandParams, GillParams, GrowthParams, Hyphae};
    use nodarium_utils::encode_float;

    fn curve(points: &[f32]) -> Vec<i32> {
        let mut words = vec![0, points.len() as i32 + 1];
        words.extend(points.iter().map(|&p| encode_float(p)));
        words.extend([1, 1]);
        words
    }

    #[test]
    fn the_defaults_match_the_core_defaults() {
        let d = ExpandParams::default();
        let f = |v: f32| vec![encode_float(v)];
        let primordium = grow(&GrowthParams { max_segments: 2000, ..GrowthParams::default() });
        let button = grow_gills(&primordium, &GillParams::default()).encode();
        let args: Vec<Vec<i32>> = vec![
            button.clone(),
            f(d.t),
            vec![d.seed as i32],
            curve(&d.elongation.0),
            curve(&d.opening.0),
            f(d.stipe_stretch),
            f(d.apical_stretch),
            f(d.stipe_thickening),
            f(d.cap_spread),
            f(d.open_angle),
            f(d.gill_deepening),
            f(d.inflation),
            f(d.veil_extensibility),
            f(d.tear_position),
        ];
        let refs: Vec<&[i32]> = args.iter().map(|a| a.as_slice()).collect();
        let hyphae = Hyphae::decode(&__nodarium_execute_inner(&refs));
        assert_eq!(hyphae, expand(&Hyphae::decode(&button), &d));

        let def: serde_json::Value = serde_json::from_str(include_str!("definition.json")).unwrap();
        let value = |key: &str| def["inputs"][key]["value"].clone();
        for (key, v) in [
            ("t", d.t),
            ("stipe_stretch", d.stipe_stretch),
            ("apical_stretch", d.apical_stretch),
            ("stipe_thickening", d.stipe_thickening),
            ("cap_spread", d.cap_spread),
            ("open_angle", d.open_angle),
            ("gill_deepening", d.gill_deepening),
            ("inflation", d.inflation),
            ("veil_extensibility", d.veil_extensibility),
            ("tear_position", d.tear_position),
        ] {
            assert!((value(key).as_f64().unwrap() as f32 - v).abs() < 1e-7, "{}", key);
        }
        for (key, s) in [("elongation", &d.elongation), ("opening", &d.opening)] {
            let points: Vec<f32> = value(key).as_array().unwrap().iter().map(|v| v.as_f64().unwrap() as f32).collect();
            assert_eq!(&points, &s.0, "{}", key);
        }
    }
}
