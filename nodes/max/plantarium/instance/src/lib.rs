use glam::{Mat4, Quat, Vec3};
use nodarium_macros::{nodarium_execute, nodarium_definition_file};
use nodarium_utils::{
    concat_args,
    geometry::{
        encode_instances, is_collapsed, is_geometry, path_id, pick_geometry, split_items, wrap_path,
        DeepestPaths,
    },
    int, split_args, Ctx,
};

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let mut ctx = Ctx::default();
    let mut inputs = split_args(args[0]);

    let geometries: Vec<&[i32]> = split_items(args[1])
        .into_iter()
        .filter(|item| is_geometry(item))
        .collect();
    if geometries.is_empty() {
        return concat_args(inputs);
    }
    let seed = args[9][0];
    let by_index = int(args[10]) == 1;

    // the transforms of the instances of each geometry and the paths they sit on
    let mut groups: Vec<(Vec<[f32; 16]>, Vec<i32>)> = vec![Default::default(); geometries.len()];

    let depth = ctx.int(args[6]);
    let selected = DeepestPaths::new(&inputs, depth);
    let path_count = selected.count(&inputs);
    let mut path_index = 0;

    for path_data in inputs.iter() {
        // skip everything that is not a path, e.g. instances from a previous instance node
        if !selected.contains(path_data) {
            continue;
        }

        // alpha is where this parent path sits among all paths, from 0 at the first to 1 at the last
        ctx.set_path(path_data, path_index, path_count);
        path_index += 1;
        let amount = ctx.int(args[2]);
        let lowest_instance = ctx.float(args[3]);
        let highest_instance = ctx.float(args[4]);

        let path = wrap_path(path_data);

        // each instance reads the rotation step itself, so a random node gives every
        // instance its own turn around the path
        let mut turn = 0.0;

        for i in 0..amount {
            let alpha = lowest_instance
                + (i as f32 / (amount - 1).max(1) as f32) * (highest_instance - lowest_instance);
            ctx.alpha = alpha;

            let point = path.get_point_at(alpha);
            let tangent = Vec3::from_slice(&path.get_direction_at(alpha)).normalize();
            let size = (point[3] + 0.01) * ctx.float(args[7]);

            // the geometry grows along its -z axis, its face looks along +y.
            // It sticks out sideways from the path, turned around the path by the rotation,
            // and faces up to the light as far as it can
            let side = tangent.cross(Vec3::Y);
            let base = if side.length_squared() > 1e-6 {
                side.normalize()
            } else {
                Vec3::NEG_Z
            };
            let around = Quat::from_axis_angle(tangent, turn) * base;
            turn += ctx.float(args[5]);
            // the angle tilts it along the path, positive toward the tip
            let angle = ctx.float(args[8]).to_radians();
            let pick = if by_index {
                ctx.float(args[11])
            } else {
                ctx.float(args[12])
            };
            let out = around * angle.cos() + tangent * angle.sin();
            let up = Vec3::Y - out * Vec3::Y.dot(out);
            let face = if up.length_squared() > 1e-6 {
                up.normalize()
            } else {
                tangent * angle.cos() - around * angle.sin()
            };
            let back = -out;
            let across = face.cross(back);

            let transform = Mat4::from_cols(
                (across * size).extend(0.0),
                (face * size).extend(0.0),
                (back * size).extend(0.0),
                Vec3::from_slice(&point).extend(1.0),
            );
            if is_collapsed(&transform) {
                continue;
            }

            let hash = path_id(seed, path.link.id, i);
            let choice = pick_geometry(by_index, pick, hash, geometries.len());
            groups[choice].0.push(transform.to_cols_array());
            groups[choice].1.push(path.link.id);
        }
    }

    let instance_data: Vec<Vec<i32>> = geometries
        .iter()
        .zip(&groups)
        .filter(|(_, (transforms, _))| !transforms.is_empty())
        .map(|(geometry, (transforms, path_ids))| encode_instances(geometry, transforms, path_ids))
        .collect();

    inputs.extend(instance_data.iter().map(Vec::as_slice));

    concat_args(inputs)
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use nodarium_utils::{
        concat_args, encode_float,
        geometry::{
            create_multiple_paths, has_instance_colors, path_id, path_link, with_geometry_colors,
            wrap_instance_data, wrap_multiple_paths, NO_PATH,
        },
        split_args, wrap_arg,
    };

    /// Two straight stems of four points each.
    fn stems() -> Vec<i32> {
        let mut stems = create_multiple_paths(2, 4, 1, 6);
        for stem in wrap_multiple_paths(&mut stems).iter_mut() {
            for i in 0..4 {
                stem.points[i * 4..i * 4 + 4].copy_from_slice(&[0.0, i as f32, 0.0, 0.1]);
            }
        }
        stems
    }

    /// A geometry of `vertices` points at the origin and a single face.
    fn geometry(vertices: i32) -> Vec<i32> {
        let mut data = vec![1, vertices, 1, NO_PATH, 0, 1, 2];
        data.extend(vec![0; vertices as usize * 6]);
        wrap_arg(&data)
    }

    fn run(stems: &[i32], geometry: &[i32], amount: i32, seed: i32) -> Vec<i32> {
        run_sized(stems, geometry, amount, seed, 1.0)
    }

    fn run_sized(stems: &[i32], geometry: &[i32], amount: i32, seed: i32, size: f32) -> Vec<i32> {
        run_with(stems, geometry, amount, seed, size, &[])
    }

    /// Runs with the picking inputs appended, so `&[]` is a graph saved before they existed.
    fn run_with(
        stems: &[i32],
        geometry: &[i32],
        amount: i32,
        seed: i32,
        size: f32,
        pick: &[&[i32]],
    ) -> Vec<i32> {
        let (amount, size, seed) = ([amount], [encode_float(size)], [seed]);
        let (zero, one) = ([encode_float(0.0)], [encode_float(1.0)]);
        let mut args: Vec<&[i32]> = vec![
            stems, geometry, &amount, &zero, &one, &zero, &[1], &size, &zero, &seed,
        ];
        let defaults: [&[i32]; 3] = [&[0], &[0], &[0]];
        args.extend((0..3).map(|i| pick.get(i).copied().unwrap_or(defaults[i])));
        __nodarium_execute_inner(&args)
    }

    fn three() -> Vec<i32> {
        concat_args(vec![&geometry(3), &geometry(4), &geometry(5)])
    }

    /// How many instances got the geometry with 3, 4 and 5 vertices.
    fn counts(result: &[i32]) -> [usize; 3] {
        let mut counts = [0; 3];
        for (vertices, ids) in groups(result) {
            counts[vertices - 3] = ids.len();
        }
        counts
    }

    /// The vertex amount and path ids of every instance group in the result.
    fn groups(result: &[i32]) -> Vec<(usize, Vec<i32>)> {
        split_args(result)
            .iter()
            .filter(|item| item[2] == 2)
            .map(|item| {
                let mut data = item.to_vec();
                let instances = wrap_instance_data(&mut data);
                (instances.positions.len() / 3, instances.path_ids.to_vec())
            })
            .collect()
    }

    #[test]
    fn every_instance_knows_the_path_it_sits_on() {
        let stems = stems();
        let stem_ids: Vec<i32> = split_args(&stems).iter().map(|p| path_link(p).id).collect();

        let groups = groups(&run(&stems, &geometry(3), 3, 0));

        assert_eq!(groups.len(), 1);
        let path_ids = &groups[0].1;
        assert_eq!(path_ids.len(), 6);
        assert_eq!(&path_ids[..3], &[stem_ids[0]; 3]);
        assert_eq!(&path_ids[3..], &[stem_ids[1]; 3]);
    }

    #[test]
    fn a_list_with_one_geometry_is_the_same_as_the_geometry() {
        let stems = stems();
        let single = run(&stems, &geometry(3), 3, 0);
        let list = run(&stems, &concat_args(vec![&geometry(3)]), 3, 0);
        assert_eq!(single, list);
    }

    #[test]
    fn every_instance_picks_one_geometry_of_the_list() {
        let stems = stems();
        let list = concat_args(vec![&geometry(3), &geometry(4), &geometry(5)]);

        let groups = groups(&run(&stems, &list, 20, 0));

        let mut vertices: Vec<usize> = groups.iter().map(|(v, _)| *v).collect();
        vertices.sort();
        assert_eq!(vertices, vec![3, 4, 5]);
        let total: usize = groups.iter().map(|(_, ids)| ids.len()).sum();
        assert_eq!(total, 40);
    }

    #[test]
    fn the_seed_decides_which_geometry_an_instance_picks() {
        let stems = stems();
        let list = concat_args(vec![&geometry(3), &geometry(4), &geometry(5)]);

        assert_eq!(run(&stems, &list, 20, 1), run(&stems, &list, 20, 1));
        assert_ne!(run(&stems, &list, 20, 1), run(&stems, &list, 20, 2));
    }

    #[test]
    fn the_paths_stay_in_the_output() {
        let stems = stems();
        let list = concat_args(vec![&geometry(3), &geometry(4)]);

        let result = run(&stems, &list, 5, 0);

        assert_eq!(&split_args(&result)[..2], &split_args(&stems)[..]);
    }

    #[test]
    fn instances_scaled_to_nothing_are_dropped() {
        let stems = stems();
        let list = concat_args(vec![&geometry(3), &geometry(4)]);

        let result = run_sized(&stems, &list, 5, 0, 0.0);

        assert!(groups(&result).is_empty());
        assert_eq!(split_args(&result), split_args(&stems));
    }

    #[test]
    fn an_unconnected_geometry_places_nothing() {
        let stems = stems();
        // the default value `[0, 0, 0]`, and the empty list the runtime sends without one
        for geometry in [&[0, 4, 0, 0, 0, 1, 1][..], &[0, 1, 1, 1][..]] {
            let result = run(&stems, geometry, 3, 0);
            assert!(groups(&result).is_empty());
            assert_eq!(split_args(&result).len(), 2);
        }
    }

    #[test]
    fn the_default_picks_like_before() {
        let stems = stems();
        let mut expected = vec![(3, vec![]), (4, vec![]), (5, vec![])];
        for stem in split_args(&stems) {
            let id = path_link(stem).id;
            for i in 0..50 {
                expected[path_id(7, id, i) as usize % 3].1.push(id);
            }
        }

        let result = run_with(&stems, &three(), 50, 7, 1.0, &[&[0], &[0], &[0]]);

        assert_eq!(groups(&result), expected);
    }

    #[test]
    fn unconnected_picking_inputs_pick_like_before() {
        let stems = stems();
        let list = three();
        let empty: &[i32] = &[0, 1, 1, 1];
        assert_eq!(
            run_with(&stems, &list, 20, 3, 1.0, &[empty, empty, empty]),
            run(&stems, &list, 20, 3)
        );
    }

    #[test]
    fn the_bias_skews_the_random_pick() {
        let stems = stems();
        let list = three();
        let pick = |bias: f32| {
            let bias = [encode_float(bias)];
            counts(&run_with(&stems, &list, 1000, 0, 1.0, &[&[0], &[0], &bias]))
        };

        let even = pick(0.0);
        assert!(even.iter().all(|&c| (550..=780).contains(&c)), "{even:?}");

        let late = pick(1.0);
        assert!(late[0] < late[1] && late[1] < late[2], "{late:?}");
        assert!(late[2] > 1400, "{late:?}");

        let early = pick(-1.0);
        assert!(early[0] > early[1] && early[1] > early[2], "{early:?}");
        assert!(early[0] > 1400, "{early:?}");

        let slight = pick(0.3);
        assert!(slight[2] > even[2] && slight[2] < late[2], "{slight:?}");
        assert_eq!(pick(5.0), late);
    }

    #[test]
    fn the_index_picks_the_geometry() {
        let stems = stems();
        let list = three();
        let pick = |index: f32| {
            let index = [encode_float(index)];
            counts(&run_with(&stems, &list, 10, 0, 1.0, &[&[1], &index]))
        };

        assert_eq!(pick(0.0), [20, 0, 0]);
        assert_eq!(pick(0.3), [20, 0, 0]);
        assert_eq!(pick(0.34), [0, 20, 0]);
        assert_eq!(pick(0.7), [0, 0, 20]);
        assert_eq!(pick(1.0), [0, 0, 20]);
        assert_eq!(pick(-2.0), [20, 0, 0]);
        assert_eq!(pick(7.0), [0, 0, 20]);
        assert_eq!(pick(f32::NAN), [20, 0, 0]);
    }

    #[test]
    fn an_alpha_index_goes_through_the_list_along_the_path() {
        let stems = stems();
        let list = three();
        let alpha = concat_args(vec![&[2], &[0]]);

        let result = run_with(&stems, &list, 9, 0, 1.0, &[&[1], &alpha]);

        assert_eq!(counts(&result), [6, 6, 6]);
        let unconnected: &[i32] = &[0, 1, 1, 1];
        assert_eq!(
            counts(&run_with(&stems, &list, 9, 0, 1.0, &[&[1], unconnected])),
            [18, 0, 0]
        );
    }

    #[test]
    fn instances_bring_the_colors_of_their_geometry() {
        let stems = stems();
        let colored = with_geometry_colors(&geometry(3), &[1, 2, 3]);
        let result = run(&stems, &colored, 2, 0);
        let mut item = split_args(&result).iter().find(|i| i[2] == 2).unwrap().to_vec();
        assert_eq!(wrap_instance_data(&mut item).colors, &[1, 2, 3]);
        assert_eq!(groups(&result), groups(&run(&stems, &geometry(3), 2, 0)));

        let plain = run(&stems, &geometry(3), 2, 0);
        assert!(split_args(&plain).iter().filter(|i| i[2] == 2).all(|i| !has_instance_colors(i)));
    }
}
