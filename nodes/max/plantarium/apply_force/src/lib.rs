use glam::Vec3;
use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;
use nodarium_utils::{
    concat_arg_vecs, decode_forces,
    geometry::{translate_path, wrap_path, wrap_path_mut, PathStructure},
    int, sample_forces, split_args, Ctx, Force,
};

nodarium_definition_file!("src/definition.json");

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
        let push = sample_forces(forces, middle);
        let direction = (rest + push * weight).try_normalize().unwrap_or(rest);

        current[i] = current[i - 1] + direction * length;
    }

    for (point, position) in path.points.chunks_exact_mut(4).zip(&current) {
        point[0] = position.x;
        point[1] = position.y;
        point[2] = position.z;
    }
}

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let mut ctx = Ctx::default();

    let original: Vec<Vec<i32>> = split_args(args[0]).iter().map(|p| p.to_vec()).collect();
    let forces = decode_forces(args[1]);
    let depth = int(args[4]).max(1);

    let is_path = |p: &Vec<i32>| p.len() > 7 && p[2] == 0;
    let max_depth = original
        .iter()
        .filter(|p| is_path(p))
        .map(|p| p[3])
        .max()
        .unwrap_or(0);
    let affected: Vec<bool> = original
        .iter()
        .map(|p| is_path(p) && p[3] >= max_depth - depth + 1)
        .collect();
    let affected_count = affected.iter().filter(|a| **a).count();

    // The inputs are read in the order of the list, so random values don't depend on which
    // paths the structure visits first.
    let mut path_index = 0;
    let settings: Vec<Option<(Vec<f32>, f32)>> = original
        .iter()
        .zip(&affected)
        .map(|(path_data, affected)| {
            if !affected {
                return None;
            }
            // alpha is where this path sits among all bent paths, from 0 at the first to 1 at the last
            ctx.set_path(path_data, path_index, affected_count);
            path_index += 1;

            let length = wrap_path(path_data).length;
            let stiffness = ctx.float(args[3]).clamp(0.0, 1.0);
            // alpha goes along the path, so the strength can change from base to tip
            let alphas = (0..length).map(|i| i as f32 / (length - 1).max(1) as f32);
            let strengths = ctx.float_along(args[2], alphas);
            Some((strengths, stiffness))
        })
        .collect();

    let mut current = original.clone();
    let structure = PathStructure::new(&original);
    for &index in structure.order() {
        // the base moves with the spot on its parent first, then the path bends from there
        let shift = structure.shift(index, &original, &current);
        translate_path(&mut current[index], shift);

        if let Some((strengths, stiffness)) = &settings[index] {
            if !forces.is_empty() {
                bend(&mut current[index], &forces, strengths, *stiffness);
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
        concat_arg_vecs, encode_float,
        geometry::{
            create_path, interpolate_along_path, set_path_link, wrap_path, wrap_path_mut, PathLink,
        },
        split_args, Force, ForceKind,
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

    fn run(paths: Vec<Vec<i32>>, forces: &[Force], depth: i32, stiffness: f32) -> Vec<Vec<i32>> {
        let plants = concat_arg_vecs(paths);
        let force_list = concat_arg_vecs(forces.iter().map(|f| f.encode()).collect());
        let result = __nodarium_execute_inner(&[
            &plants,
            if forces.is_empty() { &[] } else { &force_list },
            &[encode_float(1.0)],
            &[encode_float(stiffness)],
            &[depth],
        ]);
        split_args(&result).iter().map(|p| p.to_vec()).collect()
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

    fn flat(path_data: &[i32]) -> Vec<f32> {
        wrap_path(path_data).points.to_vec()
    }
}
