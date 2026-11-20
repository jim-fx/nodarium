use glam::{Quat, Vec3};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{decode_points, encode_points, geometry::NO_PARENT, Ctx, Point};

nodarium_definition_file!("src/definition.json");

/// Where a spiral sits and the axis it turns around
struct Centre {
    position: Option<Vec3>,
    up: Vec3,
}

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let mut ctx = Ctx::default();
    let centres: Vec<Centre> = match decode_points(args[1]).as_slice() {
        [] => vec![Centre { position: None, up: Vec3::Y }],
        points => points
            .iter()
            .map(|p| Centre { position: Some(p.position), up: p.direction.try_normalize().unwrap_or(Vec3::Y) })
            .collect(),
    };

    let mut points = Vec::new();
    for (c, centre) in centres.iter().enumerate() {
        ctx.alpha = c as f32 / (centres.len() - 1).max(1) as f32;
        let amount = ctx.int(args[2]).max(0);
        // the first point sticks out where the first leaf of instance does on an upright path
        let side = centre.up.cross(Vec3::Y);
        let start = side.try_normalize().unwrap_or(Vec3::NEG_Z);
        let mut turn = 0.0;
        for i in 0..amount {
            let alpha = i as f32 / (amount - 1).max(1) as f32;
            ctx.alpha = alpha;
            let origin = Vec3::from_array(ctx.vec3(args[0]));
            let height = ctx.float(args[3]);
            points.push(Point {
                position: centre.position.unwrap_or(origin) + centre.up * height * alpha,
                direction: Quat::from_axis_angle(centre.up, turn) * start,
                tangent: centre.up,
                scale: 1.0,
                parent_id: NO_PARENT,
                alpha,
                depth: 0,
                // what grows on the points takes its id from the index, so it runs on through
                // the spirals of all centres
                index: points.len() as i32,
            });
            turn += ctx.float(args[4]);
        }
    }
    encode_points(&points)
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use glam::Vec3;
    use nodarium_utils::{concat_args, decode_points, encode_float, encode_points, geometry::NO_PARENT, Point};

    fn vec3(v: Vec3) -> Vec<i32> {
        concat_args(vec![&[encode_float(v.x)], &[encode_float(v.y)], &[encode_float(v.z)]])
    }

    fn run(centres: &[Point], amount: i32, height: f32, rotation: f32) -> Vec<Point> {
        let origin = vec3(Vec3::new(0.5, 0.25, -1.0));
        let centres = if centres.is_empty() { Vec::new() } else { encode_points(centres) };
        decode_points(&__nodarium_execute_inner(&[
            &origin,
            &centres,
            &[amount],
            &[encode_float(height)],
            &[encode_float(rotation)],
        ]))
    }

    fn centre(position: Vec3, direction: Vec3) -> Point {
        Point { position, direction, tangent: Vec3::X, scale: 0.3, parent_id: 7, alpha: 0.5, depth: 2, index: 0 }
    }

    #[test]
    fn the_points_turn_by_the_rotation_around_the_origin() {
        let points = run(&[], 5, 0.0, 2.3998);
        assert_eq!(points.len(), 5);
        for (i, point) in points.iter().enumerate() {
            assert!((point.position - Vec3::new(0.5, 0.25, -1.0)).length() < 1e-6);
            assert_eq!(point.tangent, Vec3::Y);
            assert!(point.direction.y.abs() < 1e-6 && (point.direction.length() - 1.0).abs() < 1e-6);
            assert_eq!((point.index, point.parent_id, point.depth, point.scale), (i as i32, NO_PARENT, 0, 1.0));
            assert!((point.alpha - i as f32 / 4.0).abs() < 1e-6);
        }
        // the first one sticks out where instance puts the first leaf on an upright path
        assert!((points[0].direction - Vec3::NEG_Z).length() < 1e-6);
        for pair in points.windows(2) {
            let angle = pair[0].direction.cross(pair[1].direction).dot(Vec3::Y).atan2(pair[0].direction.dot(pair[1].direction));
            assert!((angle.rem_euclid(std::f32::consts::TAU) - 2.3998).abs() < 1e-4, "{angle}");
        }
    }

    #[test]
    fn the_height_lifts_the_points_evenly() {
        let points = run(&[], 3, 0.2, 1.0);
        for (point, y) in points.iter().zip([0.25, 0.35, 0.45]) {
            assert!((point.position.y - y).abs() < 1e-6, "{:?}", point.position);
        }
    }

    #[test]
    fn every_centre_gets_a_spiral_around_its_direction() {
        let centres = [centre(Vec3::new(1.0, 0.0, 0.0), Vec3::X), centre(Vec3::new(0.0, 2.0, 0.0), Vec3::Y * 3.0)];
        let points = run(&centres, 4, 0.3, 2.0);
        assert_eq!(points.len(), 8);
        // the index runs on, so what grows on the points of two centres gets different ids
        assert_eq!(points.iter().map(|p| p.index).collect::<Vec<_>>(), (0..8).collect::<Vec<_>>());
        for (spiral, c) in points.chunks(4).zip(&centres) {
            let up = c.direction.normalize();
            assert!((spiral[0].position - c.position).length() < 1e-6);
            assert!((spiral[3].position - (c.position + up * 0.3)).length() < 1e-6);
            for point in spiral {
                assert_eq!((point.tangent, point.parent_id, point.depth), (up, NO_PARENT, 0));
                assert!(point.direction.dot(up).abs() < 1e-6);
            }
        }
    }

    #[test]
    fn without_amount_there_are_no_points() {
        assert!(run(&[], 0, 0.0, 2.3998).is_empty());
    }
}
