use myzel_core::margin_points;
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{encode_points, split_args};

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    encode_points(&margin_points(&split_args(args[0])))
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use myzel_core::testing::{cone, frame, grow, list, place, points, rays};
    use nodarium_utils::{decode_points, geometry::path_link};

    #[test]
    fn the_tips_of_the_rays_in_a_ring_around_the_axis() {
        let c = cone();
        let paths = grow(&c);
        let margin = decode_points(&__nodarium_execute_inner(&[&list(&paths)]));
        let rays = rays(&paths);
        assert_eq!(margin.len(), rays.len());
        let f = frame(&paths);
        let angles: Vec<f32> = margin.iter().map(|m| f.angle(m.position)).collect();
        assert!(angles.windows(2).all(|w| w[1] >= w[0]));
        for m in &margin {
            let ray = rays.iter().find(|r| path_link(r).id == m.parent_id).unwrap();
            assert_eq!(m.position, *points(ray).last().unwrap());
            assert!((place(m.position).x - c.radius).abs() < 1e-4);
        }
    }

    #[test]
    fn no_margin_without_the_stipe() {
        let only_rays = rays(&grow(&cone()));
        assert!(decode_points(&__nodarium_execute_inner(&[&list(&only_rays)])).is_empty());
        assert!(decode_points(&__nodarium_execute_inner(&[&[]])).is_empty());
    }
}
