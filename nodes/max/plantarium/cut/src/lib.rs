use glam::{Vec4, Vec4Swizzles};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{
    concat_arg_vecs,
    geometry::{
        is_path, path_link, resample_up_to, set_path_link, wrap_path, wrap_path_mut, DeepestPaths,
        PointValues,
    },
    split_args, Ctx,
};
use std::collections::{HashMap, HashSet};

nodarium_definition_file!("src/definition.json");

/// Where along its length (0 to 1) a path first rises above `height`, 0 when its base lies
/// above it, nothing when it stays below
fn crossing(points: &[Vec4], height: f32) -> Option<f32> {
    if points[0].y > height {
        return Some(0.0);
    }
    let lengths: Vec<f32> = points.windows(2).map(|w| (w[1].xyz() - w[0].xyz()).length()).collect();
    let total: f32 = lengths.iter().sum();
    if total <= 0.0 {
        return None;
    }
    let mut along = 0.0;
    for (i, w) in points.windows(2).enumerate() {
        if w[1].y > height {
            let t = (height - w[0].y) / (w[1].y - w[0].y);
            return Some((along + t * lengths[i]) / total);
        }
        along += lengths[i];
    }
    None
}

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let mut ctx = Ctx::default();
    let mut items: Vec<Vec<i32>> = split_args(args[0]).iter().map(|p| p.to_vec()).collect();
    let depth = ctx.int(args[2]);
    let selected = DeepestPaths::new(&items, depth);
    let path_count = selected.count(&items);

    // parents come before their children, so a path knows what happened to its parent
    let mut order: Vec<usize> = (0..items.len()).filter(|i| is_path(&items[*i])).collect();
    order.sort_by_key(|i| items[*i][3]);

    let mut removed: HashSet<i32> = HashSet::new();
    let mut cuts: HashMap<i32, f32> = HashMap::new();
    let mut keep = vec![true; items.len()];
    let mut path_index = 0;

    for index in order {
        let mut link = path_link(&items[index]);
        if removed.contains(&link.parent_id) {
            removed.insert(link.id);
            keep[index] = false;
            continue;
        }
        if let Some(&cut) = cuts.get(&link.parent_id) {
            if link.parent_alpha > cut {
                removed.insert(link.id);
                keep[index] = false;
                continue;
            }
            // the parent now ends at the cut, the spot keeps its place on it
            link.parent_alpha /= cut;
            set_path_link(&mut items[index], link);
        }

        if !selected.contains(&items[index]) {
            continue;
        }
        let path = wrap_path(&items[index]);
        if path.length < 2 {
            continue;
        }
        ctx.set_path(&items[index], path_index, path_count);
        path_index += 1;
        let height = ctx.float(args[1]);

        let points: Vec<Vec4> = path.points.chunks_exact(4).map(Vec4::from_slice).collect();
        let Some(alpha) = crossing(&points, height) else {
            continue;
        };
        if alpha <= 0.0 {
            removed.insert(link.id);
            keep[index] = false;
            continue;
        }
        let shortened = resample_up_to(&points, alpha);
        // the colors and other values of the part that is left spread over the shorter path
        let values = path.has_values().then(|| {
            let n = shortened.len();
            let shares: Vec<f32> = (0..n).map(|i| alpha * i as f32 / (n - 1).max(1) as f32).collect();
            PointValues::at_lengths(&path, &shares)
        });
        let mut target = wrap_path_mut(&mut items[index]);
        for (i, point) in shortened.iter().enumerate() {
            target.points[i * 4..i * 4 + 4].copy_from_slice(&point.to_array());
        }
        if let Some(values) = values {
            values.write_into(&mut target);
        }
        cuts.insert(link.id, alpha);
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
    use super::*;
    use nodarium_utils::{
        encode_float,
        geometry::{create_path, PathLink},
    };

    const CREATOR: i32 = 3;

    /// A path of 11 points from `from` along `step`
    fn path(link: PathLink, depth: i32, from: [f32; 3], step: [f32; 3]) -> Vec<i32> {
        let mut data = create_path(11, depth);
        set_path_link(&mut data, link);
        let path = wrap_path_mut(&mut data);
        for i in 0..11 {
            let k = i as f32;
            path.points[i * 4..i * 4 + 4].copy_from_slice(&[
                from[0] + step[0] * k,
                from[1] + step[1] * k,
                from[2] + step[2] * k,
                0.01,
            ]);
        }
        data
    }

    fn run(paths: &[Vec<i32>], height: f32) -> Vec<Vec<i32>> {
        let input = concat_arg_vecs(paths.to_vec());
        let out = __nodarium_execute_inner(&[&input, &[encode_float(height)], &[10]]);
        split_args(&out).iter().map(|p| p.to_vec()).collect()
    }

    fn top(path: &[i32]) -> f32 {
        wrap_path(path).points.chunks_exact(4).map(|p| p[1]).fold(f32::MIN, f32::max)
    }

    #[test]
    fn paths_end_at_the_height() {
        let stem = PathLink::root(CREATOR, 0);
        let leaning = PathLink::root(CREATOR, 1);
        let out = run(
            &[path(stem, 1, [0.0; 3], [0.0, 0.1, 0.0]), path(leaning, 1, [0.5, 0.0, 0.0], [0.1, 0.1, 0.0])],
            0.55,
        );
        assert_eq!(out.len(), 2);
        for p in &out {
            assert!((top(p) - 0.55).abs() < 1e-5);
            assert_eq!(wrap_path(p).length, 11);
        }
    }

    #[test]
    fn a_path_below_the_height_stays() {
        let input = vec![path(PathLink::root(CREATOR, 0), 1, [0.0; 3], [0.0, 0.01, 0.0])];
        assert_eq!(run(&input, 0.5), input);
    }

    #[test]
    fn what_grew_above_the_cut_is_gone_and_the_rest_keeps_its_place() {
        let stem = PathLink::root(CREATOR, 0);
        let low = PathLink::child(CREATOR + 1, stem.id, 0.3, 0);
        let high = PathLink::child(CREATOR + 1, stem.id, 0.8, 1);
        let out = run(
            &[
                path(stem, 1, [0.0; 3], [0.0, 0.1, 0.0]),
                path(low, 2, [0.0, 0.3, 0.0], [0.01, 0.0, 0.0]),
                path(high, 2, [0.0, 0.8, 0.0], [0.01, 0.0, 0.0]),
            ],
            0.5,
        );
        assert_eq!(out.len(), 2);
        let low = path_link(&out[1]);
        // the stem is half as long, the branch at 0.3 of it now sits at 0.6
        assert!((low.parent_alpha - 0.6).abs() < 1e-5);
    }

    #[test]
    fn a_path_that_starts_above_the_height_is_gone() {
        let out = run(&[path(PathLink::root(CREATOR, 0), 1, [0.0, 1.0, 0.0], [0.0, 0.1, 0.0])], 0.5);
        assert!(out.is_empty());
    }
}
