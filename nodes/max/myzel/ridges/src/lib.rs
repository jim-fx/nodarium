//! False gills under a cap, one blunt fold for every stripe of a pattern, see
//! `myzel_core::gills`.

use myzel_core::{gill_mesh, GillInputs, GillProfile};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::int;

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    gill_mesh(&inputs(args), GillProfile::Fold).encode()
}

/// The gills with their paths, which only the tests look at
#[cfg(test)]
fn build(args: &[&[i32]]) -> myzel_core::Gills {
    myzel_core::gills(&inputs(args), GillProfile::Fold)
}

/// args follow the key order of the inputs in definition.json
fn inputs<'a>(args: &[&'a [i32]]) -> GillInputs<'a> {
    GillInputs {
        cap: args[0],
        pattern: args[1],
        depth: args[2],
        free_distance: None,
        decurrent: args[3],
        thickness: args[4],
        edge_wave: args[5],
        seed: int(args[6]),
        creator: int(args[7]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use myzel_core::{testing, Gills};
    use nodarium_utils::{encode_float, geometry::wrap_path, Pattern};
    use std::f32::consts::TAU;

    /// 24 stripes around the axis from 4 mm to 3 cm out
    fn pattern() -> Vec<i32> {
        let (from, to, rings) = (0.004, 0.03, 60);
        let rings = (0..rings)
            .map(|i| {
                let r = from + (to - from) * i as f32 / (rings - 1) as f32;
                let n = (TAU * r / 0.0002) as usize;
                (0..n).map(|j| 0.5 + 0.5 * (24.0 * nodarium_utils::ring_angle(j, n)).cos()).collect()
            })
            .collect();
        Pattern { inner: from, outer: to, spacing: TAU * 0.01 / 24.0, rings }.encode()
    }

    fn run(depth: f32, decurrent: f32, thickness: f32) -> Gills {
        let cap = testing::list(&testing::grow(&testing::cone()));
        let pattern = pattern();
        build(&[
            &cap,
            &pattern,
            &[encode_float(depth)],
            &[encode_float(decurrent)],
            &[encode_float(thickness)],
            &[encode_float(0.3)],
            &[2],
            &[9],
        ])
    }

    fn points(gills: &Gills) -> Vec<[f32; 3]> {
        gills.paths.iter().flat_map(|p| wrap_path(p).points.chunks_exact(4).map(|q| [q[0], q[1], q[2]]).collect::<Vec<_>>()).collect()
    }

    #[test]
    fn reads_its_inputs_in_order() {
        let folds = run(0.002, 0.0, 0.001);
        assert!(folds.mesh.face_count() > 0);
        // the folds reach the stipe, there is no free distance
        let closest = points(&folds).iter().map(|q| q[0].hypot(q[2])).fold(f32::MAX, f32::min);
        assert!(closest < testing::STIPE_RADIUS + 0.001, "the folds start {closest} from the axis");
        // decurrent folds run down the stipe
        let lowest = |g: &Gills| points(g).iter().filter(|q| q[0].hypot(q[2]) < testing::STIPE_RADIUS + 0.0005).map(|q| q[1]).fold(f32::MAX, f32::min);
        assert!(lowest(&run(0.002, 0.02, 0.001)) < lowest(&folds) - 0.005);
        // the depth and the width are read
        assert_ne!(run(0.0005, 0.0, 0.001).mesh, folds.mesh);
        assert_ne!(run(0.002, 0.0, 0.0005).mesh, folds.mesh);
    }
}
