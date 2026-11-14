use glam::{Mat4, Vec3};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{
    concat_args, decode_points,
    geometry::{encode_instances, is_geometry, is_path, path_id, path_link, pick_geometry, split_items},
    group_points_by_parent, int, split_args, Ctx, Point,
};

nodarium_definition_file!("src/definition.json");

/// Where and how an instance sits on `point`. The geometry grows along its -z axis, its face
/// looks along +y. It sticks out along the direction of the point and faces up to the light as
/// far as it can.
fn transform(point: &Point, size: f32, angle: f32) -> Mat4 {
    // the angle tilts it along the path, positive toward the tip
    let out = point.direction * angle.cos() + point.tangent * angle.sin();
    let up = Vec3::Y - out * Vec3::Y.dot(out);
    let face = if up.length_squared() > 1e-6 {
        up.normalize()
    } else {
        point.tangent * angle.cos() - point.direction * angle.sin()
    };
    let back = -out;
    let across = face.cross(back);

    Mat4::from_cols(
        (across * size).extend(0.0),
        (face * size).extend(0.0),
        (back * size).extend(0.0),
        point.position.extend(1.0),
    )
}

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let mut ctx = Ctx::default();
    let mut plant = split_args(args[0]);
    let points = decode_points(args[1]);

    // like instance, a link that brings no geometry places nothing
    let geometries: Vec<&[i32]> = split_items(args[2]).into_iter().filter(|item| is_geometry(item)).collect();
    if geometries.is_empty() {
        return concat_args(plant);
    }
    let seed = args.get(5).map_or(0, |s| s[0]);
    let by_index = args.get(6).is_some_and(|p| int(p) == 1);
    let pick_input = if by_index { args.get(7) } else { args.get(8) };

    // the transforms of the instances of each geometry and the paths they sit on
    let mut placed: Vec<(Vec<[f32; 16]>, Vec<i32>)> = vec![Default::default(); geometries.len()];

    let groups = group_points_by_parent(&points);
    for (rank, (parent_id, group)) in groups.iter().enumerate() {
        // the random values of the instances start from the path they sit on
        match plant
            .iter()
            .find(|p| is_path(p) && path_link(p).id == *parent_id)
        {
            Some(parent) => ctx.set_path(parent, rank, groups.len()),
            None => ctx.seed_path(*parent_id),
        }
        for point in group {
            ctx.alpha = point.alpha;
            let size = (point.scale + 0.01) * ctx.float(args[3]);
            let angle = ctx.float(args[4]).to_radians();
            let pick = pick_input.map_or(0.0, |p| ctx.float(p));
            let hash = path_id(seed, point.parent_id, point.index);
            let choice = pick_geometry(by_index, pick, hash, geometries.len());
            placed[choice].0.push(transform(point, size, angle).to_cols_array());
            placed[choice].1.push(point.parent_id);
        }
    }

    let instance_data: Vec<Vec<i32>> = geometries
        .iter()
        .zip(&placed)
        .filter(|(_, (transforms, _))| !transforms.is_empty())
        .map(|(geometry, (transforms, path_ids))| encode_instances(geometry, transforms, path_ids))
        .collect();
    plant.extend(instance_data.iter().map(Vec::as_slice));
    concat_args(plant)
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use glam::{Mat4, Vec3};
    use nodarium_utils::{
        concat_args, encode_float, encode_points,
        geometry::{
            create_multiple_paths, has_instance_colors, path_link, with_geometry_colors,
            wrap_instance_data, NO_PATH,
        },
        split_args, wrap_arg, Point,
    };

    fn triangle() -> Vec<i32> {
        concat_args(vec![&[
            1, 3, 1, NO_PATH, 0, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ]])
    }

    #[test]
    fn an_instance_sits_on_every_point_and_knows_its_path() {
        let plant = create_multiple_paths(1, 4, 1, 6);
        let stem_id = path_link(split_args(&plant)[0]).id;
        let points: Vec<Point> = (0..3)
            .map(|i| Point {
                position: Vec3::new(0.0, i as f32, 0.0),
                direction: Vec3::X,
                tangent: Vec3::Y,
                scale: 0.09,
                parent_id: stem_id,
                alpha: i as f32 / 2.0,
                depth: 1,
                index: i,
            })
            .collect();

        let result = __nodarium_execute_inner(&[
            &plant,
            &encode_points(&points),
            &triangle(),
            &[encode_float(10.0)],
            &[encode_float(0.0)],
        ]);

        let items = split_args(&result);
        assert_eq!(items.len(), 2);
        let mut instance_data = items[1].to_vec();
        let instances = wrap_instance_data(&mut instance_data);
        assert_eq!(instances.instance_amount, 3);
        assert_eq!(instances.path_ids, &[stem_id; 3]);

        for (i, point) in points.iter().enumerate() {
            let matrix = Mat4::from_cols_slice(&instances.instances[i * 16..i * 16 + 16]);
            // it sits on the point and sticks out along its direction (the geometry grows along -z)
            assert!((matrix.w_axis.truncate() - point.position).length() < 1e-5);
            let out = -matrix.z_axis.truncate();
            assert!((out.normalize() - Vec3::X).length() < 1e-5);
            // the size is the thickness of the path at the point times the size input
            assert!((out.length() - 1.0).abs() < 1e-5);
        }
    }

    #[test]
    fn the_angle_tilts_the_instance_toward_the_tip() {
        let plant = create_multiple_paths(1, 4, 1, 6);
        let stem_id = path_link(split_args(&plant)[0]).id;
        let point = Point {
            position: Vec3::ZERO,
            direction: Vec3::X,
            tangent: Vec3::Y,
            scale: 0.09,
            parent_id: stem_id,
            alpha: 0.5,
            depth: 1,
            index: 0,
        };

        let result = __nodarium_execute_inner(&[
            &plant,
            &encode_points(&[point]),
            &triangle(),
            &[encode_float(10.0)],
            &[encode_float(90.0)],
        ]);

        let mut instance_data = split_args(&result)[1].to_vec();
        let instances = wrap_instance_data(&mut instance_data);
        let matrix = Mat4::from_cols_slice(&instances.instances[..16]);
        let out = -matrix.z_axis.truncate();
        assert!((out.normalize() - Vec3::Y).length() < 1e-5);
        // pointing straight up, its face turns away from the direction of the point
        let face = matrix.y_axis.truncate().normalize();
        assert!((face - Vec3::NEG_X).length() < 1e-5);
    }

    #[test]
    fn the_instances_bring_the_colors_of_the_geometry() {
        let plant = create_multiple_paths(1, 4, 1, 6);
        let stem_id = path_link(split_args(&plant)[0]).id;
        let point = Point {
            position: Vec3::ZERO,
            direction: Vec3::X,
            tangent: Vec3::Y,
            scale: 0.09,
            parent_id: stem_id,
            alpha: 0.5,
            depth: 1,
            index: 0,
        };
        let run = |geometry: &[i32]| {
            let result = __nodarium_execute_inner(&[
                &plant,
                &encode_points(&[point]),
                geometry,
                &[encode_float(10.0)],
                &[encode_float(0.0)],
            ]);
            split_args(&result)[1].to_vec()
        };

        let mut colored = run(&with_geometry_colors(&triangle(), &[4, 5, 6]));
        assert_eq!(wrap_instance_data(&mut colored).colors, &[4, 5, 6]);
        assert!(!has_instance_colors(&run(&triangle())));
    }

    #[test]
    fn a_link_without_geometry_places_nothing() {
        let plant = create_multiple_paths(1, 4, 1, 6);
        let stem_id = path_link(split_args(&plant)[0]).id;
        let point = Point {
            position: Vec3::ZERO,
            direction: Vec3::X,
            tangent: Vec3::Y,
            scale: 0.09,
            parent_id: stem_id,
            alpha: 0.5,
            depth: 1,
            index: 0,
        };
        let run = |geometry: &[i32]| {
            __nodarium_execute_inner(&[
                &plant,
                &encode_points(&[point]),
                geometry,
                &[encode_float(10.0)],
                &[encode_float(0.0)],
            ])
        };
        // a path, a list of paths and nothing connected pass the plant on as it is
        for geometry in [plant.clone(), split_args(&plant)[0].to_vec(), vec![0, 1, 1, 1]] {
            assert_eq!(split_args(&run(&geometry)), split_args(&plant));
        }
        // of a list of the same geometry, whichever is picked places it
        let framed = wrap_arg(&triangle()[2..triangle().len() - 2]);
        let list = concat_args(vec![&framed, &framed]);
        assert_eq!(run(&list), run(&triangle()));
    }

    /// A geometry of `vertices` points at the origin and a single face.
    fn geometry(vertices: i32) -> Vec<i32> {
        let mut data = vec![1, vertices, 1, NO_PATH, 0, 1, 2];
        data.extend(vec![0; vertices as usize * 6]);
        wrap_arg(&data)
    }

    /// `amount` points along a stem, and how many instances of the geometries with 3, 4 and 5
    /// vertices sit on them.
    fn picked(amount: i32, seed: i32, pick: &[&[i32]]) -> [usize; 3] {
        let plant = create_multiple_paths(1, 4, 1, 6);
        let stem_id = path_link(split_args(&plant)[0]).id;
        let points: Vec<Point> = (0..amount)
            .map(|i| Point {
                position: Vec3::new(0.0, i as f32, 0.0),
                direction: Vec3::X,
                tangent: Vec3::Y,
                scale: 0.09,
                parent_id: stem_id,
                alpha: i as f32 / (amount - 1).max(1) as f32,
                depth: 1,
                index: i,
            })
            .collect();
        let list = concat_args(vec![&geometry(3), &geometry(4), &geometry(5)]);
        let points = encode_points(&points);
        let (size, angle, seed) = ([encode_float(10.0)], [encode_float(0.0)], [seed]);
        let mut args: Vec<&[i32]> = vec![&plant, &points, &list, &size, &angle, &seed];
        args.extend(pick);
        let result = __nodarium_execute_inner(&args);
        let mut counts = [0; 3];
        for item in split_args(&result).iter().skip(1) {
            let mut data = item.to_vec();
            let instances = wrap_instance_data(&mut data);
            counts[instances.positions.len() / 3 - 3] += instances.instance_amount;
        }
        counts
    }

    #[test]
    fn every_instance_picks_a_geometry_of_a_list_at_random() {
        let counts = picked(300, 1, &[]);
        assert_eq!(counts.iter().sum::<usize>(), 300);
        assert!(counts.iter().all(|&c| c > 60), "{counts:?}");
        assert_ne!(counts, picked(300, 2, &[]), "another seed picks others");
        assert_eq!(counts, picked(300, 1, &[]));
    }

    #[test]
    fn the_bias_favours_the_later_geometries() {
        let (zero, bias) = ([encode_float(0.0)], [encode_float(1.0)]);
        let counts = picked(300, 1, &[&[0], &zero, &bias]);
        assert!(counts[2] > 200 && counts[0] < 20, "{counts:?}");
    }

    #[test]
    fn by_index_alpha_goes_through_the_list_along_the_path() {
        let alpha = concat_args(vec![&[2], &[0]]);
        let (zero, one) = ([encode_float(0.0)], [encode_float(1.0)]);
        assert_eq!(picked(9, 1, &[&[1], &alpha, &zero]), [3, 3, 3]);
        assert_eq!(picked(9, 1, &[&[1], &one, &zero]), [0, 0, 9]);
    }
}
