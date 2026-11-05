use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use glam::Vec2;
use myzel_core::{read_float, read_floats, Field};
use nodarium_utils::split_args;

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    // a connected shape node wraps the points in another bracket, the value on the node does not
    let profile = match split_args(args[0]).first() {
        Some(inner) if inner.len() > 1 => *inner,
        _ => args[0],
    };
    let points = read_floats(profile);
    assert!(points.len() >= 4, "revolve needs a shape with at least 2 points, got {}", points.len() / 2);
    let height = read_float(args[1]);
    let radius = read_float(args[2]);

    // the shape editor draws the left half of a 100 x 100 box, mirrored at x = 50, with y going down
    let to_profile = |x: f32, y: f32| Vec2::new((50.0 - x) / 50.0 * radius, (100.0 - y) / 100.0 * height);
    let outline = points.chunks_exact(2).map(|p| to_profile(p[0], p[1]));
    let first = to_profile(50.0, points[1]);
    let last = to_profile(50.0, points[points.len() - 1]);
    let profile: Vec<Vec2> = std::iter::once(first).chain(outline).chain(std::iter::once(last)).collect();
    Field::revolve(&profile).encode()
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    #[allow(unused_imports)]
    use glam::Vec3;
    #[allow(unused_imports)]
    use myzel_core::{eval, Field};
    #[allow(unused_imports)]
    use nodarium_utils::{decode_float, encode_float};

    #[allow(dead_code)]
    fn vec3(x: f32, y: f32, z: f32) -> Vec<i32> {
        vec![0, 4, encode_float(x), encode_float(y), encode_float(z), 1, 1]
    }

    fn shape(points: &[f32]) -> Vec<i32> {
        let mut words = vec![0, points.len() as i32 + 1];
        words.extend(points.iter().map(|&v| encode_float(v)));
        words.extend([1, 1]);
        words
    }

    #[test]
    fn revolves_a_box_into_a_cylinder() {
        let profile = shape(&[0.0, 100.0, 0.0, 0.0]);
        let words = __nodarium_execute_inner(&[&profile, &[encode_float(2.0)], &[encode_float(0.5)]]);
        assert!((eval(&words, Vec3::new(1.0, 1.0, 0.0)) - 0.5).abs() < 1e-5);
        assert!((eval(&words, Vec3::new(0.0, 3.0, 0.0)) - 1.0).abs() < 1e-5);
        assert!(eval(&words, Vec3::new(0.0, 1.0, 0.0)) < 0.0);
    }

    #[test]
    fn reads_a_connected_shape_node() {
        let profile = shape(&[0.0, 100.0, 0.0, 0.0]);
        let wrapped = nodarium_utils::concat_args(vec![&profile]);
        let direct = __nodarium_execute_inner(&[&profile, &[encode_float(2.0)], &[encode_float(0.5)]]);
        let connected = __nodarium_execute_inner(&[&wrapped, &[encode_float(2.0)], &[encode_float(0.5)]]);
        assert_eq!(direct, connected);
    }
}
