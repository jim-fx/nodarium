use std::borrow::Cow;

use glam::Vec3;
use nodarium_utils::{
    concat_arg_vecs,
    geometry::{is_path, wrap_path},
    reads_place, split_args, Ctx,
};

use crate::density::Segment;

/// How the surface around a path looks
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PathStyle {
    /// Where two paths reach a vertex, the one with the larger kernel times priority owns it
    pub priority: f32,
    /// Strength of the fibrous relief
    pub fibre_strength: f32,
    /// How much longer than wide the relief is stretched along the path
    pub fibre_stretch: f32,
}

impl PathStyle {
    /// Fibrous like a stipe, fills into one body with the other paths
    pub const BODY: PathStyle = PathStyle { priority: 1.0, fibre_strength: 1.0, fibre_stretch: 6.0 };
}

/// What decides the material group of a path
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathGroup {
    /// The graph node that made the path
    Creator(i32),
}

/// A path to mesh: its points as `x, y, z, radius`, the values at its points, its group and how
/// its surface looks
#[derive(Clone, Debug, PartialEq)]
pub struct MeshPath<'a> {
    pub points: Cow<'a, [f32]>,
    /// An RGBA8 color per point as `set_color` writes them, empty for a path without colors
    pub colors: Cow<'a, [u32]>,
    /// The fibres per point as `set_fibres` writes them, empty for the fibres of the mesh node
    pub fibres: Cow<'a, [f32]>,
    /// The fill per point as `set_fill` writes it, empty for the fill of the mesh node
    pub fill: Cow<'a, [f32]>,
    pub group: PathGroup,
    pub style: PathStyle,
}

impl MeshPath<'_> {
    pub fn point(&self, i: usize) -> Vec3 {
        Vec3::from_slice(&self.points[i * 4..i * 4 + 3])
    }

    pub fn len(&self) -> usize {
        self.points.len() / 4
    }

    pub fn is_empty(&self) -> bool {
        self.points.len() < 4
    }
}

/// A list inside a list starts with a group, its count then points at index 2. A path never has
/// a count of 1, it counts its header and points.
fn is_list(item: &[i32]) -> bool {
    item.len() > 4 && item[0] == 0 && item[1] == 1
}

fn collect_paths<'a>(list: &'a [i32], style: PathStyle, out: &mut Vec<MeshPath<'a>>) {
    for item in split_args(list) {
        if is_list(item) {
            collect_paths(item, style, out);
        } else if is_path(item) {
            let path = wrap_path(item);
            let group = PathGroup::Creator(path.link.creator);
            out.push(MeshPath {
                points: Cow::Borrowed(path.points),
                colors: Cow::Borrowed(path.colors),
                fibres: Cow::Borrowed(path.fibres),
                fill: Cow::Borrowed(path.fill),
                group,
                style,
            });
        }
    }
}

/// Every path of a list, also those in lists inside it, in the group of its creator
pub fn read_paths(list: &[i32], style: PathStyle) -> Vec<MeshPath<'_>> {
    let mut out = Vec::new();
    if list.len() >= 4 {
        collect_paths(list, style, &mut out);
    }
    out
}

fn count_paths(list: &[i32]) -> usize {
    split_args(list).iter().map(|item| if is_list(item) { count_paths(item) } else { is_path(item) as usize }).sum()
}

fn give_values(list: &[i32], arg: &[i32], along: bool, at: &mut PathAt, give: fn(&[i32], &[f32]) -> Vec<i32>) -> Vec<i32> {
    let items = split_args(list)
        .into_iter()
        .map(|item| {
            if is_list(item) {
                return give_values(item, arg, along, at, give);
            }
            if !is_path(item) {
                return item.to_vec();
            }
            at.ctx.set_path(item, at.rank, at.count);
            at.rank += 1;
            let points = wrap_path(item).length;
            let values: Vec<f32> = if along {
                let alphas = (0..points).map(|i| i as f32 / (points - 1).max(1) as f32);
                at.ctx.float_along(arg, alphas).into_iter().map(|v| v.max(0.0)).collect()
            } else {
                vec![at.ctx.float(arg).max(0.0); points]
            };
            give(item, &values)
        })
        .collect();
    concat_arg_vecs(items)
}

struct PathAt {
    ctx: Ctx,
    rank: usize,
    count: usize,
}

/// Every path of a list, also those in lists inside it, with a value of `arg` at every point,
/// written by `give` (`with_path_fibres`, `with_path_fill`). `arg` is read for every path, with
/// `path_info` and `random` per path, and along it with alpha. Values below 0 are 0, everything
/// else in the list stays as it is.
pub fn with_values_along(list: &[i32], arg: &[i32], give: fn(&[i32], &[f32]) -> Vec<i32>) -> Vec<i32> {
    if list.len() < 4 || list[0] != 0 {
        return list.to_vec();
    }
    let mut at = PathAt { ctx: Ctx::default(), rank: 0, count: count_paths(list) };
    give_values(list, arg, reads_place(arg), &mut at, give)
}

/// A small material group per path: creators numbered in the order of their ids, so a node
/// added later gets the next number and the others keep theirs
pub fn group_numbers(paths: &[MeshPath]) -> Vec<u32> {
    let creators = group_creators(paths);
    paths.iter().map(|p| creators.binary_search(&creator(p)).unwrap() as u32).collect()
}

fn creator(path: &MeshPath) -> i32 {
    match path.group {
        PathGroup::Creator(c) => c,
    }
}

/// The creator of every group of [`group_numbers`], by the number of the group
pub fn group_creators(paths: &[MeshPath]) -> Vec<i32> {
    let mut creators: Vec<i32> = paths.iter().map(creator).collect();
    creators.sort_unstable();
    creators.dedup();
    creators
}

/// The segments between neighbouring points of a path, each with the mean radius of its two
/// points. A path with a single point is a ball.
pub fn path_segments(points: &[f32]) -> Vec<Segment> {
    let point = |i: usize| (Vec3::from_slice(&points[i * 4..i * 4 + 3]), points[i * 4 + 3].abs());
    let n = points.len() / 4;
    if n == 1 {
        let (p, radius) = point(0);
        return vec![Segment { start: p, end: p, radius }];
    }
    (1..n)
        .map(|i| {
            let ((a, ra), (b, rb)) = (point(i - 1), point(i));
            Segment { start: a, end: b, radius: 0.5 * (ra + rb) }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodarium_utils::{
        concat_arg_vecs,
        geometry::{create_multiple_paths, wrap_multiple_paths},
    };

    fn list(amount: usize, creator: i32) -> Vec<i32> {
        let mut paths = create_multiple_paths(amount, 3, 1, creator);
        for (k, path) in wrap_multiple_paths(&mut paths).iter_mut().enumerate() {
            for i in 0..3 {
                path.points[i * 4..i * 4 + 4].copy_from_slice(&[k as f32, i as f32, 0.0, 0.1]);
            }
        }
        paths
    }

    #[test]
    fn reads_the_paths_of_lists_inside_lists() {
        let a = list(2, 5);
        let b = list(1, 9);
        let nested = concat_arg_vecs(vec![a.clone(), b.clone()]);
        let paths = read_paths(&nested, PathStyle::BODY);
        assert_eq!(paths.len(), 3);
        assert_eq!(paths.iter().map(|p| p.group).collect::<Vec<_>>(), [5, 5, 9].map(PathGroup::Creator));
        assert_eq!(paths[2].point(1), Vec3::new(0.0, 1.0, 0.0));
        assert!(read_paths(&[], PathStyle::BODY).is_empty());
    }

    #[test]
    fn creators_get_small_numbers_in_the_order_of_their_ids() {
        let words = concat_arg_vecs(vec![list(1, 900), list(2, -3), list(1, 42)]);
        let paths = read_paths(&words, PathStyle::BODY);
        assert_eq!(group_numbers(&paths), vec![2, 0, 0, 1]);
    }
}
