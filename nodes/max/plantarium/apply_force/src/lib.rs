use glam::Vec3;
use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;
use nodarium_utils::{
    concat_arg_vecs, decode_forces, decode_points,
    geometry::{
        place_root, translate_path, wrap_path, wrap_path_mut, DeepestPaths, PathStructure,
        PlaceChain,
    },
    int, sample_forces, split_args, Ctx, Force,
};

nodarium_definition_file!("src/definition.json");

/// The `weight` option that bends by how far a segment is from the root of its path. The
/// other one, 0, bends by the place of a segment along its own path.
const BY_PLACE: i32 = 1;

/// Turns a segment towards the push of the forces at its middle, by `weight`
fn turn(forces: &[Force], rest: Vec3, middle: Vec3, weight: f32) -> Vec3 {
    let push = sample_forces(forces, middle);
    (rest + push * weight).try_normalize().unwrap_or(rest)
}

/// Bends one path. Each segment turns towards the force at its middle, by as much as the
/// strength and its place along the path allow. The segments keep their length and the base
/// stays where it is, so the path is built outward from the base.
fn bend(path_data: &mut [i32], forces: &[Force], strengths: &[f32], stiffness: f32) {
    let path = wrap_path_mut(path_data);
    if path.length < 2 {
        return;
    }

    let original: Vec<Vec3> = path
        .points
        .chunks_exact(4)
        .map(|p| Vec3::new(p[0], p[1], p[2]))
        .collect();

    // 0 spreads the bend over the whole path, 1 leaves only the tip to bend
    let exponent = 0.5 + stiffness * 4.0;

    let mut current = original.clone();
    for i in 1..path.length {
        let segment = original[i] - original[i - 1];
        let length = segment.length();
        let rest = segment.try_normalize().unwrap_or(Vec3::Y);

        let alpha = if path.length > 2 {
            (i - 1) as f32 / (path.length - 2) as f32
        } else {
            1.0
        };
        let weight = strengths[i - 1] * alpha.powf(exponent);

        let middle = current[i - 1] + rest * length * 0.5;
        current[i] = current[i - 1] + turn(forces, rest, middle, weight) * length;
    }

    write_points(path.points, &current);
}

fn write_points(points: &mut [f32], positions: &[Vec3]) {
    for (point, position) in points.chunks_exact_mut(4).zip(positions) {
        point[0] = position.x;
        point[1] = position.y;
        point[2] = position.z;
    }
}

fn points_of(path_data: &[i32]) -> Vec<Vec3> {
    wrap_path(path_data)
        .points
        .chunks_exact(4)
        .map(|p| Vec3::new(p[0], p[1], p[2]))
        .collect()
}

/// How one path is bent, read in the order of the list
enum Setting {
    /// The strength at every point and the stiffness
    AlongPath(Vec<f32>, f32),
    /// The path with its lead-in from the root and the weight of every segment
    ByPlace(PlaceChain, Vec<f32>),
}

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let mut ctx = Ctx::default();

    let original: Vec<Vec<i32>> = split_args(args[0]).iter().map(|p| p.to_vec()).collect();
    let forces = decode_forces(args[1]);
    let mode = int(args[4]);
    let roots = decode_points(args[6]);
    let depth = int(args[7]).max(1);

    let selected = DeepestPaths::new(&original, depth);
    let affected: Vec<bool> = original.iter().map(|p| selected.contains(p)).collect();
    let affected_count = affected.iter().filter(|a| **a).count();
    let structure = PathStructure::new(&original);

    // The inputs are read in the order of the list, so random values don't depend on which
    // paths the structure visits first.
    let mut path_index = 0;
    let settings: Vec<Option<Setting>> = original
        .iter()
        .enumerate()
        .map(|(index, path_data)| {
            if !affected[index] {
                return None;
            }
            // alpha is where this path sits among all bent paths, from 0 at the first to 1 at the last
            ctx.set_path(path_data, path_index, affected_count);
            path_index += 1;

            let length = wrap_path(path_data).length;
            let stiffness = ctx.float(args[3]).clamp(0.0, 1.0);
            if mode == BY_PLACE && length >= 2 {
                let reach = ctx.float(args[5]).max(0.0);
                let root = place_root(&structure, &original, index, &roots)?;
                let chain = PlaceChain::new(&points_of(path_data), root, reach);
                // alpha is how far a segment is into the reach, the same for every path there
                let strengths = ctx.float_along(args[2], chain.alphas.iter().copied());
                let exponent = 0.5 + stiffness * 4.0;
                let weights = strengths
                    .iter()
                    .zip(&chain.alphas)
                    .map(|(strength, alpha)| strength * alpha.powf(exponent))
                    .collect();
                return Some(Setting::ByPlace(chain, weights));
            }
            // alpha goes along the path, so the strength can change from base to tip
            let alphas = (0..length).map(|i| i as f32 / (length - 1).max(1) as f32);
            let strengths = ctx.float_along(args[2], alphas);
            Some(Setting::AlongPath(strengths, stiffness))
        })
        .collect();

    let mut current = original.clone();
    for &index in structure.order() {
        // the base moves with the spot on its parent first, then the path bends from there
        let shift = structure.shift(index, &original, &current);
        match &settings[index] {
            Some(Setting::ByPlace(chain, weights)) if !forces.is_empty() => {
                let start = chain.points[0] + shift;
                let bent = chain.bend(start, |k, rest, middle| {
                    turn(&forces, rest, middle, weights[k])
                });
                write_points(wrap_path_mut(&mut current[index]).points, &bent);
            }
            setting => {
                translate_path(&mut current[index], shift);
                if let Some(Setting::AlongPath(strengths, stiffness)) = setting {
                    if !forces.is_empty() {
                        bend(&mut current[index], &forces, strengths, *stiffness);
                    }
                }
            }
        }
    }

    concat_arg_vecs(current)
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use glam::Vec3;
    use nodarium_utils::{
        concat_arg_vecs, concat_args, encode_float, encode_points,
        geometry::{
            create_path, interpolate_along_path, set_path_link, wrap_path, wrap_path_mut, PathLink,
        },
        split_args, Force, ForceKind, Point,
    };

    fn point_force(position: Vec3, strength: f32, radius: f32) -> Force {
        Force {
            kind: ForceKind::Point,
            position,
            strength,
            radius,
            falloff: 0.0,
        }
    }

    fn path(id: i32, depth: i32, parent: Option<(i32, f32)>, points: &[Vec3]) -> Vec<i32> {
        let mut data = create_path(points.len(), depth);
        let (parent_id, parent_alpha) = parent.unwrap_or((-1, 0.0));
        set_path_link(
            &mut data,
            PathLink {
                id,
                parent_id,
                parent_alpha,
                ..PathLink::root(0, 0)
            },
        );
        let path = wrap_path_mut(&mut data);
        for (i, p) in points.iter().enumerate() {
            path.points[i * 4..i * 4 + 4].copy_from_slice(&[p.x, p.y, p.z, 0.1]);
        }
        data
    }

    /// A vertical stem with 5 points, 1 apart
    fn stem(id: i32, depth: i32, parent: Option<(i32, f32)>, base: Vec3) -> Vec<i32> {
        let points: Vec<Vec3> = (0..5).map(|i| base + Vec3::Y * i as f32).collect();
        path(id, depth, parent, &points)
    }

    const ALONG_PATH: i32 = 0;
    const BY_PLACE: i32 = 1;

    fn run(paths: Vec<Vec<i32>>, forces: &[Force], depth: i32, stiffness: f32) -> Vec<Vec<i32>> {
        run_with(
            paths,
            forces,
            depth,
            stiffness,
            ALONG_PATH,
            0.0,
            &[encode_float(1.0)],
        )
    }

    fn run_with(
        paths: Vec<Vec<i32>>,
        forces: &[Force],
        depth: i32,
        stiffness: f32,
        weight: i32,
        reach: f32,
        strength: &[i32],
    ) -> Vec<Vec<i32>> {
        run_rooted(
            paths,
            forces,
            depth,
            stiffness,
            weight,
            reach,
            strength,
            &[],
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn run_rooted(
        paths: Vec<Vec<i32>>,
        forces: &[Force],
        depth: i32,
        stiffness: f32,
        weight: i32,
        reach: f32,
        strength: &[i32],
        roots: &[Point],
    ) -> Vec<Vec<i32>> {
        let roots = if roots.is_empty() {
            vec![0, 1, 1, 1]
        } else {
            encode_points(roots)
        };
        let plants = concat_arg_vecs(paths);
        let force_list = concat_arg_vecs(forces.iter().map(|f| f.encode()).collect());
        let result = __nodarium_execute_inner(&[
            &plants,
            if forces.is_empty() { &[] } else { &force_list },
            strength,
            &[encode_float(stiffness)],
            &[weight],
            &[encode_float(reach)],
            &roots,
            &[depth],
        ]);
        split_args(&result).iter().map(|p| p.to_vec()).collect()
    }

    /// A stipe from (0, -2, 0) up to the origin, and paths that grow from its tip along x: a
    /// long one from the origin and a short one that is the outer part of it, like a full ray
    /// of a cap and a ray the cap adds further out.
    fn long_and_short() -> (Vec<i32>, Vec<i32>, Vec<i32>) {
        let stipe = path(
            0,
            1,
            None,
            &[
                Vec3::new(0.0, -2.0, 0.0),
                Vec3::new(0.0, -1.0, 0.0),
                Vec3::ZERO,
            ],
        );
        let along_x = |from: usize| (from..9).map(|i| Vec3::X * i as f32).collect::<Vec<_>>();
        let long = path(1, 2, Some((0, 1.0)), &along_x(0));
        let short = path(2, 2, Some((0, 1.0)), &along_x(4));
        (stipe, long, short)
    }

    /// The largest distance between the points the short path shares with the long one
    fn gap(result: &[Vec<i32>]) -> f32 {
        let (long, short) = (points(&result[1]), points(&result[2]));
        long[4..]
            .iter()
            .zip(&short)
            .map(|(a, b)| a.distance(*b))
            .fold(0.0, f32::max)
    }

    /// A curve from the alpha node: 0.2 up to alpha 0.4, then up to 2 at alpha 1
    fn strength_curve() -> Vec<i32> {
        let alpha = concat_args(vec![&[2], &[0]]);
        let words: Vec<i32> = [0.0, 0.1, 0.4, 0.1, 1.0, 1.0]
            .iter()
            .map(|v| encode_float(*v))
            .collect();
        let mut parts: Vec<&[i32]> = vec![&[3], &alpha];
        let (min, max) = ([encode_float(0.0)], [encode_float(2.0)]);
        parts.push(&min);
        parts.push(&max);
        parts.extend(words.chunks(1));
        concat_args(parts)
    }

    fn points(path_data: &[i32]) -> Vec<Vec3> {
        wrap_path(path_data)
            .points
            .chunks_exact(4)
            .map(|p| Vec3::new(p[0], p[1], p[2]))
            .collect()
    }

    fn tip(path_data: &[i32]) -> Vec3 {
        *points(path_data).last().unwrap()
    }

    #[test]
    fn a_path_bends_towards_a_point_force() {
        let force = point_force(Vec3::new(4.0, 4.0, 0.0), 1.0, 20.0);
        let result = run(vec![stem(0, 1, None, Vec3::ZERO)], &[force], 1, 0.0);

        assert!(tip(&result[0]).x > 0.5, "tip is {:?}", tip(&result[0]));
    }

    #[test]
    fn a_negative_strength_bends_away() {
        let force = point_force(Vec3::new(4.0, 4.0, 0.0), -1.0, 20.0);
        let result = run(vec![stem(0, 1, None, Vec3::ZERO)], &[force], 1, 0.0);

        assert!(tip(&result[0]).x < -0.5, "tip is {:?}", tip(&result[0]));
    }

    #[test]
    fn the_base_and_the_segment_lengths_stay() {
        let force = point_force(Vec3::new(4.0, 2.0, 1.0), 2.0, 20.0);
        let result = run(
            vec![stem(0, 1, None, Vec3::new(1.0, 0.0, 1.0))],
            &[force],
            1,
            0.3,
        );

        let bent = points(&result[0]);
        assert_eq!(bent[0], Vec3::new(1.0, 0.0, 1.0));
        for pair in bent.windows(2) {
            assert!(((pair[1] - pair[0]).length() - 1.0).abs() < 1e-4);
        }
    }

    #[test]
    fn without_forces_nothing_changes() {
        let paths = vec![stem(0, 1, None, Vec3::ZERO)];
        let result = run(paths.clone(), &[], 1, 0.0);
        assert_eq!(result[0], paths[0]);
    }

    #[test]
    fn paths_outside_of_the_radius_stay() {
        let paths = vec![stem(0, 1, None, Vec3::ZERO)];
        let force = point_force(Vec3::new(50.0, 2.0, 0.0), 1.0, 3.0);
        let result = run(paths.clone(), &[force], 1, 0.0);
        assert_eq!(result[0], paths[0]);
    }

    #[test]
    fn a_stiffer_path_bends_less_at_the_base() {
        let force = point_force(Vec3::new(4.0, 4.0, 0.0), 1.0, 20.0);
        let soft = run(vec![stem(0, 1, None, Vec3::ZERO)], &[force], 1, 0.0);
        let stiff = run(vec![stem(0, 1, None, Vec3::ZERO)], &[force], 1, 1.0);

        assert!(points(&soft[0])[2].x > points(&stiff[0])[2].x);
    }

    #[test]
    fn opposite_forces_cancel() {
        let paths = vec![stem(0, 1, None, Vec3::ZERO)];
        let left = point_force(Vec3::new(-3.0, 2.0, 0.0), 1.0, 20.0);
        let right = point_force(Vec3::new(3.0, 2.0, 0.0), 1.0, 20.0);
        let result = run(paths, &[left, right], 1, 0.0);

        assert!(
            tip(&result[0]).x.abs() < 1e-3,
            "tip is {:?}",
            tip(&result[0])
        );
    }

    #[test]
    fn only_the_selected_layers_bend() {
        let trunk = stem(0, 1, None, Vec3::ZERO);
        let branch = stem(1, 2, Some((0, 0.5)), Vec3::new(0.0, 2.0, 0.0));
        let force = point_force(Vec3::new(4.0, 6.0, 0.0), 1.0, 30.0);

        let result = run(vec![trunk.clone(), branch], &[force], 1, 0.0);

        assert_eq!(result[0], trunk);
        assert!(tip(&result[1]).x > 0.1);
    }

    #[test]
    fn a_branch_keeps_touching_a_bent_stem() {
        let trunk = stem(0, 1, None, Vec3::ZERO);
        let attach = 0.5;
        let on_trunk = interpolate_along_path(&flat(&trunk), attach).0.truncate();
        let branch = path(1, 2, Some((0, attach)), &[on_trunk, on_trunk + Vec3::X]);
        let force = point_force(Vec3::new(-6.0, 3.0, 2.0), 2.0, 50.0);

        let result = run(vec![trunk.clone(), branch], &[force], 2, 0.2);

        let bent_trunk = flat(&result[0]);
        assert_ne!(bent_trunk, flat(&trunk));
        let spot = interpolate_along_path(&bent_trunk, attach).0.truncate();
        let base = points(&result[1])[0];
        assert!(
            (base - spot).length() < 1e-4,
            "base {:?} spot {:?}",
            base,
            spot
        );
    }

    #[test]
    fn deeper_structures_keep_touching_through_every_level() {
        let trunk = stem(0, 1, None, Vec3::ZERO);
        let attach = 0.5;
        let on_trunk = interpolate_along_path(&flat(&trunk), attach).0.truncate();
        let branch = path(
            1,
            2,
            Some((0, attach)),
            &(0..5)
                .map(|i| on_trunk + Vec3::X * i as f32)
                .collect::<Vec<_>>(),
        );
        let on_branch = interpolate_along_path(&flat(&branch), attach).0.truncate();
        let twig = path(2, 3, Some((1, attach)), &[on_branch, on_branch + Vec3::Y]);
        let force = point_force(Vec3::new(-6.0, 3.0, 2.0), 2.0, 50.0);

        // the twig comes first in the list, the order must not matter
        let result = run(vec![twig, branch, trunk], &[force], 3, 0.2);

        let spot = interpolate_along_path(&flat(&result[1]), attach)
            .0
            .truncate();
        let base = points(&result[0])[0];
        assert!(
            (base - spot).length() < 1e-4,
            "base {:?} spot {:?}",
            base,
            spot
        );
    }

    #[test]
    fn by_place_a_short_path_bends_like_the_long_path_it_lies_on() {
        let (stipe, long, short) = long_and_short();
        let forces = [
            point_force(Vec3::new(5.0, 6.0, 1.0), 1.0, 50.0),
            point_force(Vec3::new(7.0, -2.0, -3.0), 0.5, 6.0),
        ];
        let paths = vec![stipe, long, short];
        let one = [encode_float(1.0)];

        // along each path the short one bends from its own base
        let along = run_with(paths.clone(), &forces, 1, 0.3, ALONG_PATH, 0.0, &one);
        assert!(gap(&along) > 0.5, "gap along each path {}", gap(&along));

        for reach in [0.0, 3.0, 6.0, 20.0] {
            let result = run_with(paths.clone(), &forces, 1, 0.3, BY_PLACE, reach, &one);
            assert_ne!(points(&result[1]), points(&paths[1]));
            assert!(
                gap(&result) < 1e-5,
                "gap {} with reach {}",
                gap(&result),
                reach
            );
        }
    }

    #[test]
    fn by_place_the_strength_follows_the_place_not_the_point() {
        let (stipe, long, short) = long_and_short();
        let force = point_force(Vec3::new(5.0, 6.0, 1.0), 1.0, 50.0);
        let result = run_with(
            vec![stipe, long, short],
            &[force],
            1,
            0.0,
            BY_PLACE,
            8.0,
            &strength_curve(),
        );
        assert!(gap(&result) < 1e-5, "gap {}", gap(&result));
    }

    #[test]
    fn by_place_a_path_does_not_depend_on_the_other_paths() {
        let (stipe, long, short) = long_and_short();
        let force = point_force(Vec3::new(5.0, 6.0, 1.0), 1.0, 50.0);
        let one = [encode_float(1.0)];
        let alone = run_with(
            vec![stipe.clone(), short.clone()],
            &[force],
            1,
            0.3,
            BY_PLACE,
            6.0,
            &one,
        );
        let together = run_with(
            vec![long, short, stipe],
            &[force],
            1,
            0.3,
            BY_PLACE,
            6.0,
            &one,
        );
        assert_eq!(alone[1], together[1]);
    }

    #[test]
    fn by_place_the_segments_keep_their_length() {
        let (stipe, long, short) = long_and_short();
        let force = point_force(Vec3::new(5.0, 6.0, 1.0), 3.0, 50.0);
        let one = [encode_float(1.0)];
        let result = run_with(
            vec![stipe, long, short],
            &[force],
            1,
            0.3,
            BY_PLACE,
            4.0,
            &one,
        );
        for path in &result[1..] {
            for pair in points(path).windows(2) {
                assert!(((pair[1] - pair[0]).length() - 1.0).abs() < 1e-4);
            }
        }
        // the base of the long path sits on its root and stays
        assert!(points(&result[1])[0].length() < 1e-5);
        // the short one starts away from it and moves with the long one
        assert!(points(&result[2])[0].distance(Vec3::X * 4.0) > 0.1);
    }

    #[test]
    fn by_place_a_root_point_stands_in_for_a_missing_parent() {
        let (stipe, long, short) = long_and_short();
        let force = point_force(Vec3::new(5.0, 6.0, 1.0), 1.0, 50.0);
        let one = [encode_float(1.0)];
        let with_stipe = run_with(
            vec![stipe, long.clone(), short.clone()],
            &[force],
            1,
            0.3,
            BY_PLACE,
            6.0,
            &one,
        );

        // only the rays, and the hinge at the top of the stipe they grow on
        let hinge = Point {
            position: Vec3::ZERO,
            direction: Vec3::X,
            tangent: Vec3::Y,
            scale: 0.1,
            parent_id: 0,
            alpha: 1.0,
            depth: 1,
            index: 0,
        };
        let rays = vec![long, short];
        let rooted = run_rooted(
            rays.clone(),
            &[force],
            1,
            0.3,
            BY_PLACE,
            6.0,
            &one,
            &[hinge],
        );
        let unrooted = run_with(rays, &[force], 1, 0.3, BY_PLACE, 6.0, &one);

        let both = |r: &[Vec<i32>]| [points(&r[0]), points(&r[1])].concat();
        for (a, b) in both(&rooted).iter().zip(both(&with_stipe[1..])) {
            assert!(a.distance(b) < 1e-4, "{:?} {:?}", a, b);
        }
        // without the hinge each ray grows from its own base and the short one bends on its own
        let (long, short) = (points(&unrooted[0]), points(&unrooted[1]));
        assert!(long[8].distance(short[4]) > 0.5);
    }

    #[test]
    fn by_place_without_a_reach_the_stiffness_does_nothing() {
        let (stipe, long, _) = long_and_short();
        let force = point_force(Vec3::new(5.0, 6.0, 1.0), 1.0, 50.0);
        let one = [encode_float(1.0)];
        let soft = run_with(
            vec![stipe.clone(), long.clone()],
            &[force],
            1,
            0.0,
            BY_PLACE,
            0.0,
            &one,
        );
        let stiff = run_with(vec![stipe, long], &[force], 1, 1.0, BY_PLACE, 0.0, &one);
        assert_eq!(soft, stiff);
    }

    #[test]
    fn by_place_a_branch_keeps_touching_a_bent_stem() {
        let trunk = stem(0, 1, None, Vec3::ZERO);
        let attach = 0.5;
        let on_trunk = interpolate_along_path(&flat(&trunk), attach).0.truncate();
        let branch = path(
            1,
            2,
            Some((0, attach)),
            &[on_trunk, on_trunk + Vec3::X, on_trunk + Vec3::X * 2.0],
        );
        let force = point_force(Vec3::new(-6.0, 3.0, 2.0), 2.0, 50.0);
        let one = [encode_float(1.0)];

        let result = run_with(
            vec![trunk.clone(), branch.clone()],
            &[force],
            2,
            0.2,
            BY_PLACE,
            3.0,
            &one,
        );

        let spot = interpolate_along_path(&flat(&result[0]), attach)
            .0
            .truncate();
        let base = points(&result[1])[0];
        assert!(
            (base - spot).length() < 1e-4,
            "base {:?} spot {:?}",
            base,
            spot
        );
        assert_ne!(points(&result[1])[2] - base, Vec3::X * 2.0);
    }

    #[test]
    fn by_place_without_forces_nothing_changes() {
        let (stipe, long, short) = long_and_short();
        let paths = vec![stipe, long, short];
        let result = run_with(
            paths.clone(),
            &[],
            1,
            0.3,
            BY_PLACE,
            3.0,
            &[encode_float(1.0)],
        );
        assert_eq!(result, paths);
    }

    fn flat(path_data: &[i32]) -> Vec<f32> {
        wrap_path(path_data).points.to_vec()
    }
}
