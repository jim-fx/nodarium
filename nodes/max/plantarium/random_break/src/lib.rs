use glam::{Vec4, Vec4Swizzles};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{
    concat_arg_vecs,
    geometry::{is_path, path_id, path_link, set_path_link, wrap_path, wrap_path_mut, DeepestPaths},
    split_args, Ctx,
};
use std::collections::{HashMap, HashSet};

nodarium_definition_file!("src/definition.json");

/// A break this close to the base would leave a path without length.
const MIN_BREAK: f32 = 0.01;

/// A number from 0 to 1 that only depends on the seed, the path and `salt`.
fn random(seed: i32, id: i32, salt: i32) -> f32 {
    path_id(seed, id, salt) as f32 / i32::MAX as f32
}

/// Resamples `points` to cover the path from its base up to `alpha` along its length, with the
/// same number of points, so the broken path keeps its resolution.
fn cut(points: &[Vec4], alpha: f32) -> Vec<Vec4> {
    let mut distances = Vec::with_capacity(points.len());
    let mut sum = 0.0;
    for (i, point) in points.iter().enumerate() {
        if i > 0 {
            sum += (point.xyz() - points[i - 1].xyz()).length();
        }
        distances.push(sum);
    }
    let end = sum * alpha;
    let last = points.len() - 1;

    let mut segment = 0;
    (0..points.len())
        .map(|k| {
            let target = end * k as f32 / last as f32;
            while segment < last - 1 && distances[segment + 1] < target {
                segment += 1;
            }
            let span = distances[segment + 1] - distances[segment];
            let t = if span > 0.0 {
                ((target - distances[segment]) / span).clamp(0.0, 1.0)
            } else {
                0.0
            };
            points[segment].lerp(points[segment + 1], t)
        })
        .collect()
}

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let mut ctx = Ctx::default();
    let mut items: Vec<Vec<i32>> = split_args(args[0]).iter().map(|p| p.to_vec()).collect();
    let seed = args[4][0];
    let depth = ctx.int(args[5]);

    let selected = DeepestPaths::new(&items, depth);
    let path_count = selected.count(&items);

    // parents come before their children, so a path knows what happened to its parent
    let mut order: Vec<usize> = (0..items.len()).filter(|i| is_path(&items[*i])).collect();
    order.sort_by_key(|i| items[*i][3]);

    let mut removed: HashSet<i32> = HashSet::new();
    let mut breaks: HashMap<i32, f32> = HashMap::new();
    let mut keep = vec![true; items.len()];
    let mut path_index = 0;

    for index in order {
        let mut link = path_link(&items[index]);

        if removed.contains(&link.parent_id) {
            removed.insert(link.id);
            keep[index] = false;
            continue;
        }
        if let Some(&b) = breaks.get(&link.parent_id) {
            if link.parent_alpha > b {
                removed.insert(link.id);
                keep[index] = false;
                continue;
            }
            // the parent now ends at the break, the spot keeps its place on it
            link.parent_alpha /= b;
            set_path_link(&mut items[index], link);
        }

        if !selected.contains(&items[index]) {
            continue;
        }
        let path = wrap_path(&items[index]);
        if path.length < 2 || path.get_length() <= 0.0 {
            continue;
        }

        ctx.set_path(&items[index], path_index, path_count);
        path_index += 1;

        let chance = ctx.float(args[1]);
        if random(seed, link.id, 0) >= chance {
            continue;
        }
        let lowest = ctx.float(args[2]);
        let highest = ctx.float(args[3]);
        let alpha = (lowest + random(seed, link.id, 1) * (highest - lowest)).clamp(MIN_BREAK, 1.0);
        if alpha >= 1.0 {
            continue;
        }

        let points: Vec<Vec4> = path.points.chunks_exact(4).map(Vec4::from_slice).collect();
        let broken = cut(&points, alpha);
        let target = wrap_path_mut(&mut items[index]).points;
        for (i, point) in broken.iter().enumerate() {
            target[i * 4..i * 4 + 4].copy_from_slice(&point.to_array());
        }
        breaks.insert(link.id, alpha);
    }

    let kept = items
        .into_iter()
        .zip(keep)
        .filter_map(|(item, keep)| keep.then_some(item))
        .collect();
    concat_arg_vecs(kept)
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use nodarium_utils::{
        concat_arg_vecs, encode_float,
        geometry::{create_path, path_link, set_path_link, wrap_path, wrap_path_mut, PathLink},
        split_args,
    };

    /// A straight path up the y axis from `base`, `height` long with `resolution` points.
    fn straight(link: PathLink, depth: i32, base: [f32; 3], height: f32) -> Vec<i32> {
        let resolution = 9;
        let mut path = create_path(resolution, depth);
        set_path_link(&mut path, link);
        let points = wrap_path_mut(&mut path).points;
        for i in 0..resolution {
            let y = base[1] + height * i as f32 / (resolution - 1) as f32;
            points[i * 4..i * 4 + 4].copy_from_slice(&[base[0], y, base[2], 0.1]);
        }
        path
    }

    fn run(paths: Vec<Vec<i32>>, chance: f32, lowest: f32, highest: f32, depth: i32) -> Vec<Vec<i32>> {
        let plant = concat_arg_vecs(paths);
        let out = __nodarium_execute_inner(&[
            &plant,
            &[encode_float(chance)],
            &[encode_float(lowest)],
            &[encode_float(highest)],
            &[3],
            &[depth],
        ]);
        split_args(&out).iter().map(|p| p.to_vec()).collect()
    }

    fn tip_y(path: &[i32]) -> f32 {
        let p = wrap_path(path);
        p.points[(p.length - 1) * 4 + 1]
    }

    #[test]
    fn a_broken_path_ends_at_the_break_and_keeps_its_resolution() {
        let stem = straight(PathLink::root(1, 0), 1, [0.0; 3], 10.0);
        let out = run(vec![stem], 1.0, 0.5, 0.5, 1);
        assert_eq!(out.len(), 1);
        let path = wrap_path(&out[0]);
        assert_eq!(path.length, 9);
        assert!((tip_y(&out[0]) - 5.0).abs() < 1e-4);
        for i in 0..path.length {
            let y = path.points[i * 4 + 1];
            assert!((y - 5.0 * i as f32 / 8.0).abs() < 1e-4, "point {} at {}", i, y);
        }
    }

    #[test]
    fn nothing_breaks_without_a_chance() {
        let stem = straight(PathLink::root(1, 0), 1, [0.0; 3], 10.0);
        let out = run(vec![stem.clone()], 0.0, 0.5, 0.5, 1);
        assert_eq!(out, vec![stem]);
    }

    #[test]
    fn branches_above_the_break_are_gone_and_the_ones_below_stay_in_place() {
        let stem_link = PathLink::root(1, 0);
        let stem = straight(stem_link, 1, [0.0; 3], 10.0);
        let low = PathLink::child(2, stem_link.id, 0.2, 0);
        let high = PathLink::child(2, stem_link.id, 0.8, 1);
        let twig = PathLink::child(4, high.id, 0.5, 0);
        let paths = vec![
            stem,
            straight(low, 2, [0.0, 2.0, 0.0], 1.0),
            straight(high, 2, [0.0, 8.0, 0.0], 1.0),
            straight(twig, 3, [0.0, 8.5, 0.0], 0.5),
        ];

        // the stem is the only one on depth 1, so a depth of 3 reaches it
        let out = run(paths, 1.0, 0.5, 0.5, 3);
        let ids: Vec<i32> = out.iter().map(|p| path_link(p).id).collect();
        assert!(ids.contains(&low.id));
        assert!(!ids.contains(&high.id));
        assert!(!ids.contains(&twig.id));

        let branch = out.iter().find(|p| path_link(p).id == low.id).unwrap();
        let alpha = path_link(branch).parent_alpha;
        assert!((alpha - 0.4).abs() < 1e-5, "parent alpha is {}", alpha);
    }

    #[test]
    fn the_same_seed_breaks_the_same_paths() {
        let paths: Vec<Vec<i32>> = (0..20)
            .map(|i| straight(PathLink::root(1, i), 1, [i as f32, 0.0, 0.0], 10.0))
            .collect();
        let a = run(paths.clone(), 0.5, 0.2, 0.9, 1);
        let b = run(paths, 0.5, 0.2, 0.9, 1);
        assert_eq!(a, b);

        let broken = a.iter().filter(|p| tip_y(p) < 9.99).count();
        assert!(broken > 3 && broken < 17, "{} of 20 broke", broken);
    }
}
