//! Thin gills under a cap, one plate for every stripe of a pattern, see `myzel_core::gills`.

use myzel_core::{gill_mesh, GillInputs, GillProfile};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::int;

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    gill_mesh(&inputs(args), GillProfile::Plate).encode()
}

/// The gills with their paths, which only the tests look at
#[cfg(test)]
fn build(args: &[&[i32]]) -> myzel_core::Gills {
    myzel_core::gills(&inputs(args), GillProfile::Plate)
}

/// args follow the key order of the inputs in definition.json
fn inputs<'a>(args: &[&'a [i32]]) -> GillInputs<'a> {
    GillInputs {
        cap: args[0],
        pattern: args[1],
        depth: args[2],
        free_distance: Some(args[3]),
        decurrent: args[4],
        thickness: args[5],
        edge_wave: args[6],
        seed: int(args[7]),
        creator: int(args[8]),
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

    fn run(free_distance: f32, decurrent: f32) -> Gills {
        let cap = testing::list(&testing::grow(&testing::cone()));
        let pattern = pattern();
        build(&[
            &cap,
            &pattern,
            &[encode_float(0.003)],
            &[encode_float(free_distance)],
            &[encode_float(decurrent)],
            &[encode_float(0.0003)],
            &[encode_float(0.3)],
            &[2],
            &[9],
        ])
    }

    /// How close to the axis of the test cap the gills come, and how low they reach on the stipe
    fn extent(gills: &Gills) -> (f32, f32) {
        let points: Vec<[f32; 3]> =
            gills.paths.iter().flat_map(|p| wrap_path(p).points.chunks_exact(4).map(|q| [q[0], q[1], q[2]]).collect::<Vec<_>>()).collect();
        let from_axis = |q: &[f32; 3]| q[0].hypot(q[2]);
        let closest = points.iter().map(from_axis).fold(f32::MAX, f32::min);
        let lowest = points.iter().filter(|q| from_axis(q) < testing::STIPE_RADIUS + 0.0005).map(|q| q[1]).fold(f32::MAX, f32::min);
        (closest, lowest)
    }

    #[test]
    fn reads_its_inputs_in_order() {
        let free = run(0.004, 0.0);
        assert!(free.mesh.face_count() > 0);
        let (closest_free, _) = extent(&free);
        let (closest_attached, high) = extent(&run(0.0, 0.0));
        assert!(closest_free > closest_attached + 0.003, "free {closest_free}, attached {closest_attached}");
        // decurrent gills run down the stipe
        let (_, low) = extent(&run(0.0, 0.02));
        assert!(low < high - 0.005, "decurrent gills reach {low}, attached ones {high}");
    }
}
