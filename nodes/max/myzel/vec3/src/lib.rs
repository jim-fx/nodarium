use myzel_core::read_float;
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::encode_float;

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let [x, y, z] = [0, 1, 2].map(|i| encode_float(read_float(args[i])));
    // the same framing the host uses for a vec3 value
    vec![0, 4, x, y, z, 1, 1]
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use glam::Vec3;
    use myzel_core::read_vec3;
    use nodarium_utils::encode_float;

    #[test]
    fn combines_into_a_vec3_value() {
        let f = |v: f32| vec![encode_float(v)];
        let output = __nodarium_execute_inner(&[&f(1.0), &f(2.0), &f(3.0)]);
        assert_eq!(read_vec3(&output), Vec3::new(1.0, 2.0, 3.0));
    }
}
