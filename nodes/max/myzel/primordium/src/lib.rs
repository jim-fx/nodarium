use myzel_core::{grow, read_float, read_floats, read_int, GrowthParams, Schedule};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let count = |arg: &[i32]| read_int(arg).max(0) as usize;
    let params = GrowthParams {
        seed: read_int(args[0]) as u32,
        iterations: count(args[1]),
        initial_tips: count(args[2]),
        knot_radius: read_float(args[3]),
        segment_length: read_float(args[4]),
        hypha_radius: read_float(args[5]),
        persistence: read_float(args[6]),
        branch_probability: read_float(args[7]),
        density_threshold: read_float(args[8]),
        secondary_factor: read_float(args[9]),
        leading_share: read_float(args[10]),
        gravitropic_angle: Schedule(read_floats(args[11])),
        autotropism: Schedule(read_floats(args[12])),
        galvanotropism: Schedule(read_floats(args[13])),
        waist_height: read_float(args[14]),
        max_segments: count(args[15]),
    };
    grow(&params).encode()
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use myzel_core::{GrowthParams, Hyphae};
    use nodarium_utils::encode_float;

    fn curve(points: &[f32]) -> Vec<i32> {
        let mut words = vec![0, points.len() as i32 + 1];
        words.extend(points.iter().map(|&p| encode_float(p)));
        words.extend([1, 1]);
        words
    }

    #[test]
    fn the_defaults_match_the_core_defaults() {
        let d = GrowthParams::default();
        let f = |v: f32| vec![encode_float(v)];
        let args: Vec<Vec<i32>> = vec![
            vec![d.seed as i32],
            vec![d.iterations as i32],
            vec![d.initial_tips as i32],
            f(d.knot_radius),
            f(d.segment_length),
            f(d.hypha_radius),
            f(d.persistence),
            f(d.branch_probability),
            f(d.density_threshold),
            f(d.secondary_factor),
            f(d.leading_share),
            curve(&d.gravitropic_angle.0),
            curve(&d.autotropism.0),
            curve(&d.galvanotropism.0),
            f(d.waist_height),
            vec![d.max_segments as i32],
        ];
        let refs: Vec<&[i32]> = args.iter().map(|a| a.as_slice()).collect();
        let hyphae = Hyphae::decode(&__nodarium_execute_inner(&refs));
        assert_eq!(hyphae, myzel_core::grow(&d));

        // the defaults in the definition are the same as in the core
        let def: serde_json::Value = serde_json::from_str(include_str!("definition.json")).unwrap();
        let value = |key: &str| def["inputs"][key]["value"].clone();
        assert_eq!(value("iterations"), d.iterations);
        assert_eq!(value("initial_tips"), d.initial_tips);
        assert_eq!(value("max_segments"), d.max_segments);
        for (key, v) in [
            ("knot_radius", d.knot_radius),
            ("segment_length", d.segment_length),
            ("hypha_radius", d.hypha_radius),
            ("persistence", d.persistence),
            ("branch_probability", d.branch_probability),
            ("density_threshold", d.density_threshold),
            ("secondary_factor", d.secondary_factor),
            ("leading_share", d.leading_share),
            ("waist_height", d.waist_height),
        ] {
            assert!((value(key).as_f64().unwrap() as f32 - v).abs() < 1e-7, "{}", key);
        }
        for (key, s) in [
            ("gravitropic_angle", &d.gravitropic_angle),
            ("autotropism", &d.autotropism),
            ("galvanotropism", &d.galvanotropism),
        ] {
            let points: Vec<f32> = value(key).as_array().unwrap().iter().map(|v| v.as_f64().unwrap() as f32).collect();
            assert_eq!(&points, &s.0, "{}", key);
        }
    }
}
