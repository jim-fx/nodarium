use myzel_core::{read_float, read_int, reaction_diffusion, ReactionDiffusionParams};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    reaction_diffusion(&ReactionDiffusionParams {
        radius: read_float(args[0]),
        inner: read_float(args[1]),
        spacing: read_float(args[2]),
        alignment: read_float(args[3]),
        irregularity: read_float(args[4]),
        seed: read_int(args[5]) as u32,
        steps: read_int(args[6]).max(0) as usize,
    })
    .encode()
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use myzel_core::{reaction_diffusion, Pattern, ReactionDiffusionParams};
    use nodarium_utils::encode_float;

    #[test]
    fn the_defaults_match_the_core_defaults() {
        let d = ReactionDiffusionParams { steps: 300, ..ReactionDiffusionParams::default() };
        let args: Vec<Vec<i32>> = vec![
            vec![encode_float(d.radius)],
            vec![encode_float(d.inner)],
            vec![encode_float(d.spacing)],
            vec![encode_float(d.alignment)],
            vec![encode_float(d.irregularity)],
            vec![d.seed as i32],
            vec![d.steps as i32],
        ];
        let refs: Vec<&[i32]> = args.iter().map(|a| a.as_slice()).collect();
        assert_eq!(Pattern::decode(&__nodarium_execute_inner(&refs)), reaction_diffusion(&d));

        let d = ReactionDiffusionParams::default();
        let def: serde_json::Value = serde_json::from_str(include_str!("definition.json")).unwrap();
        let value = |key: &str| def["inputs"][key]["value"].as_f64().unwrap() as f32;
        for (key, expected) in [
            ("radius", d.radius),
            ("inner", d.inner),
            ("spacing", d.spacing),
            ("alignment", d.alignment),
            ("irregularity", d.irregularity),
            ("steps", d.steps as f32),
        ] {
            assert!((value(key) - expected).abs() < 1e-7, "{}", key);
        }
    }
}
