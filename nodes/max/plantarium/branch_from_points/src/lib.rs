use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{
    concat_arg_vecs, decode_points,
    geometry::{
        create_path, is_path, path_link, set_path_link, wrap_path, wrap_path_mut, PathLink,
    },
    group_points_by_parent, int, split_args, Ctx, Point,
};

nodarium_definition_file!("src/definition.json");

/// A branch growing from `point`, with up to `max_resolution` points.
fn grow(ctx: &mut Ctx, args: &[&[i32]], point: &Point, max_resolution: usize, reference_length: f32, creator: i32) -> Vec<i32> {
    ctx.alpha = point.alpha;

    let length = ctx.float(args[2]);
    let taper = ctx.float(args[5]);
    // a branch as long as the longest path gets the full resolution, shorter ones get fewer
    // points, no matter which layer they grow on
    let resolution = if reference_length > 0.0 {
        ((max_resolution as f32 * length / reference_length).round() as usize)
            .clamp(4, max_resolution)
    } else {
        max_resolution
    };
    // alpha goes along the branch, so the thickness can follow a curve
    let alphas = (0..resolution).map(|j| j as f32 / (resolution - 1) as f32);
    let thickness = ctx.float_along(args[3], alphas);

    let up_angle = ctx.float(args[4]).to_radians();
    let direction = (point.direction * up_angle.cos() + point.tangent * up_angle.sin())
        .try_normalize()
        .unwrap_or(point.direction);

    // a branch is always one layer deeper than the path it grows on
    let mut data = create_path(resolution, point.depth + 1);
    // the id only depends on this node, the parent and the number of the point, so it stays
    // when other points come or go
    set_path_link(
        &mut data,
        PathLink::child(creator, point.parent_id, point.alpha, point.index),
    );
    let branch = wrap_path_mut(&mut data);
    for j in 0..resolution {
        let a = j as f32 / (resolution - 1) as f32;
        let p = point.position + direction * a * length;
        branch.points[j * 4..j * 4 + 4].copy_from_slice(&[
            p.x,
            p.y,
            p.z,
            point.scale * thickness[j] * (1.0 - a * taper),
        ]);
    }
    data
}

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let mut ctx = Ctx::default();
    let plant = split_args(args[0]);
    let points = decode_points(args[1]);
    let max_resolution = int(args[6]).max(4) as usize;
    let creator = int(args[7]);

    // the longest path sets the point density, so branches on every level share it
    let reference_length = plant
        .iter()
        .filter(|p| is_path(p))
        .map(|p| wrap_path(p).get_length())
        .fold(0.0_f32, f32::max);

    let groups = group_points_by_parent(&points);
    let mut branches: Vec<Vec<Vec<i32>>> = vec![Vec::new(); groups.len()];

    for (rank, (parent_id, group)) in groups.iter().enumerate() {
        // the random values of the branches start from the path they grow on, a point whose
        // path is not in the plant has nothing to start from
        match plant
            .iter()
            .find(|p| is_path(p) && path_link(p).id == *parent_id)
        {
            Some(parent) => ctx.set_path(parent, rank, groups.len()),
            None => ctx.seed_path(*parent_id),
        }
        for point in group {
            branches[rank].push(grow(&mut ctx, args, point, max_resolution, reference_length, creator));
        }
    }

    // like the branch node, the branches of a path come right before it
    let mut output: Vec<Vec<i32>> = Vec::new();
    for item in plant.iter() {
        if is_path(item) {
            let id = path_link(item).id;
            if let Some(rank) = groups.iter().position(|(parent_id, _)| *parent_id == id) {
                output.append(&mut branches[rank]);
            }
        }
        output.push(item.to_vec());
    }
    // points on a path that is not in the plant still grow their branches
    for group in branches.iter_mut() {
        output.append(group);
    }

    concat_arg_vecs(output)
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use glam::Vec3;
    use nodarium_utils::{
        concat_arg_vecs, encode_float, encode_points,
        geometry::{create_multiple_paths, path_link, wrap_multiple_paths, wrap_path, PathLink},
        split_args, Point,
    };

    const BRANCH_NODE: i32 = 9;

    fn stem() -> Vec<i32> {
        let mut stems = create_multiple_paths(1, 5, 1, 3);
        for stem in wrap_multiple_paths(&mut stems).iter_mut() {
            for i in 0..5 {
                stem.points[i * 4..i * 4 + 4].copy_from_slice(&[0.0, i as f32, 0.0, 0.2]);
            }
        }
        stems
    }

    fn point(parent_id: i32, index: i32, y: f32) -> Point {
        Point {
            position: Vec3::new(0.0, y, 0.0),
            direction: Vec3::X,
            tangent: Vec3::Y,
            scale: 0.2,
            parent_id,
            alpha: y / 4.0,
            depth: 1,
            index,
        }
    }

    fn run(plant: &[i32], points: &[Point], angle: f32) -> Vec<i32> {
        __nodarium_execute_inner(&[
            plant,
            &encode_points(points),
            &[encode_float(1.0)],
            &[encode_float(1.0)],
            &[encode_float(angle)],
            &[encode_float(1.0)],
            &[8],
            &[BRANCH_NODE],
        ])
    }

    #[test]
    fn a_branch_grows_from_every_point_along_its_direction() {
        let plant = stem();
        let stem_id = path_link(&split_args(&plant)[0]).id;
        let result = run(&plant, &[point(stem_id, 0, 1.0), point(stem_id, 1, 3.0)], 0.0);
        let items = split_args(&result);

        // the branches come before the stem they grow on
        assert_eq!(items.len(), 3);
        assert_eq!(path_link(items[2]).id, stem_id);

        for (item, y) in items[..2].iter().zip([1.0, 3.0]) {
            let branch = wrap_path(item);
            let base = Vec3::from_slice(&branch.points[..3]);
            let n = branch.length - 1;
            let tip = Vec3::from_slice(&branch.points[n * 4..n * 4 + 3]);
            assert!((base - Vec3::new(0.0, y, 0.0)).length() < 1e-5);
            assert!((tip - Vec3::new(1.0, y, 0.0)).length() < 1e-5);
            // the thickness starts at the thickness of the stem and tapers to nothing
            assert!((branch.points[3] - 0.2).abs() < 1e-5);
            assert!(branch.points[n * 4 + 3].abs() < 1e-5);
            assert_eq!(branch.depth, 2);
        }
    }

    #[test]
    fn a_branch_is_linked_to_the_path_of_its_point() {
        let plant = stem();
        let stem_id = path_link(&split_args(&plant)[0]).id;
        let result = run(&plant, &[point(stem_id, 4, 2.0)], 0.0);

        let link = path_link(split_args(&result)[0]);
        assert_eq!(link, PathLink::child(BRANCH_NODE, stem_id, 0.5, 4));
    }

    #[test]
    fn the_angle_tilts_the_branch_toward_the_tip() {
        let plant = stem();
        let stem_id = path_link(&split_args(&plant)[0]).id;
        let result = run(&plant, &[point(stem_id, 0, 1.0)], 90.0);
        let branch = wrap_path(split_args(&result)[0]);
        let n = branch.length - 1;
        let tip = Vec3::from_slice(&branch.points[n * 4..n * 4 + 3]);

        assert!((tip - Vec3::new(0.0, 2.0, 0.0)).length() < 1e-5);
    }

    #[test]
    fn without_points_the_plant_stays_as_it_is() {
        let plant = stem();
        assert_eq!(run(&plant, &[], 0.0), plant);
        assert_eq!(
            split_args(&run(&concat_arg_vecs(vec![]), &[], 0.0)).len(),
            0
        );
    }
}
