use glam::{Vec2, Vec3};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{
    concat_arg_vecs, encode_points,
    geometry::{create_path, orthogonal_to, path_id, set_path_link, wrap_path, wrap_path_mut, PathLink},
    int, split_args, Ctx, Point,
};
use std::f32::consts::PI;

nodarium_definition_file!("src/definition.json");

const GOLDEN_ANGLE: f32 = 2.399_963;

/// The inputs of one umbel, read once at the tip of its path.
struct Umbel {
    amount: i32,
    variation: f32,
    opening: f32,
    shape: f32,
    depth: i32,
    umbellet_scale: f32,
    seed: i32,
    creator: i32,
    max_resolution: usize,
    reference_length: f32,
}

/// Where a ray of an umbel starts and how big it is.
struct Base {
    position: Vec3,
    axis: Vec3,
    parent_id: i32,
    parent_depth: i32,
    level: i32,
    length: f32,
    thickness: f32,
}

/// A number from 0 to 1 that only depends on the seed, the ray and `salt`.
fn random(seed: i32, id: i32, salt: i32) -> f32 {
    path_id(seed, id, salt) as f32 / i32::MAX as f32
}

/// The tip of ray `i` of `amount` for a ray length of 1: how far it is out from the axis (x) and
/// along it (y). The rays fill the disc evenly from the inside out, the last one is the outer
/// ray at `opening`. On a flat top (`shape` 0) all tips are as high as the outer one, on a dome
/// (`shape` 1) all rays are as long.
fn tip_offset(i: i32, amount: i32, opening: f32, shape: f32) -> Vec2 {
    let r = ((i + 1) as f32 / amount as f32).sqrt();
    let flat = Vec2::new(opening.sin() * r, opening.cos());
    let dome = Vec2::new((opening * r).sin(), (opening * r).cos());
    flat.lerp(dome, shape)
}

fn grow(
    ctx: &mut Ctx,
    args: &[&[i32]],
    umbel: &Umbel,
    base: &Base,
    paths: &mut Vec<Vec<i32>>,
    tips: &mut Vec<Point>,
) {
    let side = orthogonal_to(base.axis);
    let around = base.axis.cross(side);

    for i in 0..umbel.amount {
        let link = PathLink::child(umbel.creator, base.parent_id, 1.0, i);
        let jitter = (random(umbel.seed, link.id, 0) - 0.5) * PI / umbel.amount as f32;
        let turn = i as f32 * GOLDEN_ANGLE + jitter;
        let out = side * turn.cos() + around * turn.sin();

        let length = base.length * (1.0 - umbel.variation * random(umbel.seed, link.id, 1));
        let offset = tip_offset(i, umbel.amount, umbel.opening, umbel.shape);
        let ray = (out * offset.x + base.axis * offset.y) * length;
        let tip = base.position + ray;
        let direction = ray.try_normalize().unwrap_or(base.axis);

        let resolution = if umbel.reference_length > 0.0 {
            ((umbel.max_resolution as f32 * ray.length() / umbel.reference_length).round()
                as usize)
                .clamp(4, umbel.max_resolution)
        } else {
            umbel.max_resolution
        };
        let alphas = (0..resolution).map(|j| j as f32 / (resolution - 1) as f32);
        let thickness = ctx.float_along(args[6], alphas);

        let depth = base.parent_depth + 1;
        let mut data = create_path(resolution, depth);
        set_path_link(&mut data, link);
        let path = wrap_path_mut(&mut data);
        for j in 0..resolution {
            let p = base.position + ray * (j as f32 / (resolution - 1) as f32);
            path.points[j * 4..j * 4 + 4]
                .copy_from_slice(&[p.x, p.y, p.z, thickness[j] * base.thickness]);
        }

        if base.level < umbel.depth {
            // umbellets on a flat umbel face up with it, on a dome they follow their ray
            let axis = base
                .axis
                .lerp(direction, umbel.shape.clamp(0.0, 1.0))
                .try_normalize()
                .unwrap_or(base.axis);
            let umbellet = Base {
                position: tip,
                axis,
                parent_id: link.id,
                parent_depth: depth,
                level: base.level + 1,
                length: base.length * umbel.umbellet_scale,
                thickness: base.thickness * umbel.umbellet_scale,
            };
            grow(ctx, args, umbel, &umbellet, paths, tips);
        } else {
            tips.push(Point {
                position: tip,
                direction: (out - direction * out.dot(direction))
                    .try_normalize()
                    .unwrap_or_else(|| orthogonal_to(direction)),
                tangent: direction,
                scale: thickness[resolution - 1] * base.thickness,
                parent_id: link.id,
                alpha: 1.0,
                depth,
                index: 0,
            });
        }

        paths.push(data);
    }
}

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> (Vec<i32>, Vec<i32>) {
    let mut ctx = Ctx::default();
    let plant = split_args(args[0]);
    let layers = ctx.int(args[10]);

    let is_path = |p: &[i32]| p.len() > 2 && p[2] == 0;
    let max_depth = plant
        .iter()
        .filter(|p| is_path(p))
        .map(|p| p[3])
        .max()
        .unwrap_or(0);
    let selected = |p: &[i32]| is_path(p) && p[3] > max_depth - layers && wrap_path(p).length > 1;
    let count = plant.iter().filter(|p| selected(p)).count();
    let reference_length = plant
        .iter()
        .filter(|p| is_path(p))
        .map(|p| wrap_path(p).get_length())
        .fold(0.0_f32, f32::max);

    let mut output: Vec<Vec<i32>> = Vec::new();
    let mut tips = Vec::new();
    let mut rank = 0;

    for item in plant.iter() {
        if selected(item) {
            ctx.set_path(item, rank, count);
            rank += 1;
            // the umbel sits at the tip of its path
            ctx.alpha = 1.0;

            let umbel = Umbel {
                amount: ctx.int(args[1]).max(0),
                variation: ctx.float(args[3]).clamp(0.0, 1.0),
                opening: ctx.float(args[4]).to_radians(),
                shape: ctx.float(args[5]),
                depth: ctx.int(args[7]).max(1),
                umbellet_scale: ctx.float(args[8]),
                seed: int(args[9]),
                creator: int(args[12]),
                max_resolution: int(args[11]).max(4) as usize,
                reference_length,
            };

            let path = wrap_path(item);
            let n = path.length;
            let position = Vec3::from_slice(&path.points[(n - 1) * 4..(n - 1) * 4 + 3]);
            let before = Vec3::from_slice(&path.points[(n - 2) * 4..(n - 2) * 4 + 3]);
            let base = Base {
                position,
                axis: (position - before).try_normalize().unwrap_or(Vec3::Y),
                parent_id: path.link.id,
                parent_depth: path.depth,
                level: 1,
                length: ctx.float(args[2]),
                thickness: 1.0,
            };
            grow(&mut ctx, args, &umbel, &base, &mut output, &mut tips);
        }
        output.push(item.to_vec());
    }

    (concat_arg_vecs(output), encode_points(&tips))
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use glam::Vec3;
    use nodarium_utils::{
        decode_points, encode_float,
        geometry::{create_multiple_paths, path_link, wrap_multiple_paths, wrap_path},
        split_args, Point,
    };

    const UMBEL_NODE: i32 = 11;

    /// A straight stem from (0, 0, 0) up to (0, 1, 0).
    fn stem() -> Vec<i32> {
        let mut stems = create_multiple_paths(1, 3, 1, 3);
        for stem in wrap_multiple_paths(&mut stems).iter_mut() {
            for i in 0..3 {
                stem.points[i * 4..i * 4 + 4].copy_from_slice(&[0.0, i as f32 / 2.0, 0.0, 0.1]);
            }
        }
        stems
    }

    struct Inputs {
        amount: i32,
        variation: f32,
        shape: f32,
        depth: i32,
        seed: i32,
    }

    impl Default for Inputs {
        fn default() -> Self {
            Self {
                amount: 8,
                variation: 0.0,
                shape: 0.0,
                depth: 1,
                seed: 1,
            }
        }
    }

    fn run(plant: &[i32], inputs: Inputs) -> (Vec<i32>, Vec<Point>) {
        let (paths, points) = __nodarium_execute_inner(&[
            plant,
            &[inputs.amount],
            &[encode_float(0.5)],
            &[encode_float(inputs.variation)],
            &[encode_float(60.0)],
            &[encode_float(inputs.shape)],
            &[encode_float(0.01)],
            &[inputs.depth],
            &[encode_float(0.2)],
            &[inputs.seed],
            &[1],
            &[8],
            &[UMBEL_NODE],
        ]);
        (paths, decode_points(&points))
    }

    const TOP: Vec3 = Vec3::new(0.0, 1.0, 0.0);

    #[test]
    fn every_ray_ends_in_a_point() {
        let (paths, points) = run(&stem(), Inputs::default());
        let items = split_args(&paths);

        assert_eq!(items.len(), 9);
        assert_eq!(points.len(), 8);
        for (item, point) in items[..8].iter().zip(&points) {
            let ray = wrap_path(item);
            let n = ray.length;
            assert_eq!(Vec3::from_slice(&ray.points[..3]), TOP);
            let tip = Vec3::from_slice(&ray.points[(n - 1) * 4..(n - 1) * 4 + 3]);
            assert!((tip - point.position).length() < 1e-5);
            assert!(point.direction.dot(point.tangent).abs() < 1e-5);
        }
    }

    #[test]
    fn a_flat_umbel_has_its_tips_at_one_height() {
        let (_, points) = run(&stem(), Inputs::default());
        let height = 0.5 * 60_f32.to_radians().cos();

        for point in &points {
            assert!((point.position.y - TOP.y - height).abs() < 1e-5, "{:?}", point.position);
        }
        let widest = points
            .iter()
            .map(|p| (p.position - TOP).length())
            .fold(0.0, f32::max);
        assert!((widest - 0.5).abs() < 1e-5);
    }

    #[test]
    fn a_domed_umbel_has_rays_of_one_length() {
        let (_, points) = run(&stem(), Inputs { shape: 1.0, ..Default::default() });
        let flat = run(&stem(), Inputs::default()).1;

        for (dome, flat) in points.iter().zip(&flat) {
            assert!(((dome.position - TOP).length() - 0.5).abs() < 1e-5);
            assert!(dome.position.y >= flat.position.y - 1e-5);
        }
    }

    #[test]
    fn a_compound_umbel_has_an_umbellet_on_every_ray() {
        let plant = stem();
        let stem_id = path_link(split_args(&plant)[0]).id;
        let (paths, points) = run(&plant, Inputs { amount: 5, depth: 2, ..Default::default() });
        let items = split_args(&paths);

        assert_eq!(points.len(), 25);
        assert_eq!(items.len(), 1 + 5 + 25);

        let rays: Vec<i32> = items
            .iter()
            .map(|p| path_link(p))
            .filter(|l| l.parent_id == stem_id)
            .map(|l| l.id)
            .collect();
        assert_eq!(rays.len(), 5);
        for item in items.iter().filter(|p| wrap_path(p).depth == 3) {
            let link = path_link(item);
            assert!(rays.contains(&link.parent_id));
            assert_eq!(link.creator, UMBEL_NODE);
            assert_eq!(link.parent_alpha, 1.0);
        }
        for point in &points {
            assert_eq!(point.depth, 3);
            assert!(items.iter().any(|p| path_link(p).id == point.parent_id));
        }
    }

    #[test]
    fn rays_are_children_of_their_path() {
        let plant = stem();
        let stem_link = path_link(split_args(&plant)[0]);
        let (paths, points) = run(&plant, Inputs::default());
        let items = split_args(&paths);

        assert_eq!(path_link(items[8]), stem_link);
        for (i, (item, point)) in items[..8].iter().zip(&points).enumerate() {
            let link = path_link(item);
            assert_eq!(link.parent_id, stem_link.id);
            assert_eq!(link.parent_alpha, 1.0);
            assert_eq!(link.index, i as i32);
            assert_eq!(wrap_path(item).depth, 2);
            assert_eq!(point.parent_id, link.id);
        }
    }

    #[test]
    fn the_seed_changes_the_jitter() {
        let tips = |seed| run(&stem(), Inputs { seed, ..Default::default() }).1;
        let (a, b) = (tips(1), tips(2));

        assert_eq!(a, tips(1));
        assert!(a.iter().zip(&b).any(|(a, b)| (a.position - b.position).length() > 1e-4));
        // the jitter only turns the rays, a flat top stays flat
        for (a, b) in a.iter().zip(&b) {
            assert!((a.position.y - b.position.y).abs() < 1e-5);
        }
    }

    #[test]
    fn an_unconnected_plant_gives_nothing() {
        let (paths, points) = run(&[0, 1, 1, 1], Inputs::default());

        assert!(split_args(&paths).is_empty());
        assert!(points.is_empty());
    }
}
