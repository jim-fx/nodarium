//! Tubes under a cap that open in pores, see `myzel_core::pores`.

use myzel_core::{write_pores, PoreInputs};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::abi::Output;

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Output {
    write_pores(&inputs(args))
}

/// args follow the key order of the inputs in definition.json
fn inputs<'a>(args: &[&'a [i32]]) -> PoreInputs<'a> {
    PoreInputs { cap: args[0], pattern: args[1], depth: args[2], free_distance: args[3], pore_depth: args[4] }
}

#[cfg(test)]
mod tests {
    use super::*;
    use myzel_core::{pores, testing, Mesh};
    use nodarium_utils::{encode_float, Pattern};

    /// Walls in rings 2 mm apart around the axis
    fn pattern() -> Vec<i32> {
        let rings = (0..401).map(|i| vec![if (i / 10) % 2 == 0 { 1.0 } else { 0.0 }; 16]).collect();
        Pattern { inner: 0.0, outer: 0.04, spacing: 0.002, rings }.encode()
    }

    fn run(depth: f32, free_distance: f32, pore_depth: f32) -> Mesh {
        let cap = testing::list(&testing::grow(&testing::cone()));
        let pattern = pattern();
        let args: [&[i32]; 5] =
            [&cap, &pattern, &[encode_float(depth)], &[encode_float(free_distance)], &[encode_float(pore_depth)]];
        let words = unsafe { __nodarium_execute_inner(&args).words() }.to_vec();
        // written straight into the result like the mesh is encoded
        assert_eq!(words, pores(&inputs(&args)).encode());
        Mesh::decode(&words)
    }

    fn lowest(mesh: &Mesh) -> f32 {
        mesh.positions.chunks_exact(3).map(|p| p[1]).fold(f32::MAX, f32::min)
    }

    fn closest(mesh: &Mesh) -> f32 {
        mesh.positions.chunks_exact(3).map(|p| p[0].hypot(p[2])).fold(f32::MAX, f32::min)
    }

    #[test]
    fn reads_its_inputs_in_order() {
        let base = run(0.004, 0.001, 0.0);
        assert!(base.face_count() > 0);
        assert!(lowest(&run(0.008, 0.001, 0.0)) < lowest(&base) - 0.002);
        assert!(closest(&run(0.004, 0.004, 0.0)) > closest(&base) + 0.002);
        // open pores leave the surface where the walls are and move it up into the tubes
        let open = run(0.004, 0.001, 0.002);
        assert_eq!(open.vertex_count(), base.vertex_count());
        assert_ne!(open.positions, base.positions);
        assert!(run(0.0, 0.001, 0.002).face_count() == 0);
    }
}
