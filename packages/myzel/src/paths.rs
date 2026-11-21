use std::borrow::Cow;
use std::collections::HashMap;

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

/// A path to mesh: its points as `x, y, z, radius`, the values at its points, its group, how
/// its surface looks and the body it belongs to
#[derive(Clone, Debug, PartialEq)]
pub struct MeshPath<'a> {
    /// The id of the path and of the path it grows on, see `PathLink`
    pub id: i32,
    pub parent_id: i32,
    /// Paths of different bodies keep their own surfaces, see [`assign_bodies`]. 0 for all by default.
    pub body: u32,
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
                id: path.link.id,
                parent_id: path.link.parent_id,
                body: 0,
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

/// Puts every path in the body of the guide it grew on. A path whose parent (or the path itself)
/// is one of `guides` belongs to the root of that guide's lineage among them, found by following
/// `parent_id` within `guides`: a cap strand grows on a ray, the ray on its stipe, so it is in the
/// body of the stipe. The roots are numbered from 1 in the order of `guides`, every other path is
/// in body 0. Without guides every path stays in body 0.
pub fn assign_bodies(paths: &mut [MeshPath], guides: &[MeshPath]) {
    let mut parent: HashMap<i32, i32> = HashMap::with_capacity(guides.len());
    for g in guides {
        parent.entry(g.id).or_insert(g.parent_id);
    }
    let root = |start: i32| {
        let mut id = start;
        // a lineage is never longer than the guides, more steps would be a loop
        for _ in 0..guides.len() {
            match parent.get(&id) {
                Some(&up) if up != id && parent.contains_key(&up) => id = up,
                _ => break,
            }
        }
        id
    };
    let mut number: HashMap<i32, u32> = HashMap::new();
    for g in guides {
        let r = root(g.id);
        let next = number.len() as u32 + 1;
        number.entry(r).or_insert(next);
    }
    for path in paths {
        let start = if parent.contains_key(&path.id) { Some(path.id) } else { parent.contains_key(&path.parent_id).then_some(path.parent_id) };
        path.body = start.map_or(0, |s| number[&root(s)]);
    }
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
        let ids = nodarium_utils::geometry::path_link(&a[2..]);
        assert_eq!((paths[0].id, paths[0].parent_id, paths[0].body), (ids.id, ids.parent_id, 0));
        assert!(read_paths(&[], PathStyle::BODY).is_empty());
    }

    /// A path of one point with the given id and parent
    fn linked(id: i32, parent_id: i32) -> MeshPath<'static> {
        MeshPath {
            id,
            parent_id,
            body: 0,
            points: Cow::Owned(vec![0.0, 0.0, 0.0, 0.1]),
            colors: Cow::Owned(Vec::new()),
            fibres: Cow::Owned(Vec::new()),
            fill: Cow::Owned(Vec::new()),
            group: PathGroup::Creator(1),
            style: PathStyle::BODY,
        }
    }

    fn bodies(paths: &[(i32, i32)], guides: &[(i32, i32)]) -> Vec<u32> {
        let mut paths: Vec<MeshPath> = paths.iter().map(|&(id, parent)| linked(id, parent)).collect();
        let guides: Vec<MeshPath> = guides.iter().map(|&(id, parent)| linked(id, parent)).collect();
        assign_bodies(&mut paths, &guides);
        paths.iter().map(|p| p.body).collect()
    }

    #[test]
    fn a_path_belongs_to_the_root_of_its_lineage_in_the_guides() {
        // two stipes 1 and 2, rays 11 and 12 on stipe 1, ray 21 on stipe 2, a ray 13 on ray 11
        let guides = [(1, -1), (11, 1), (2, -1), (12, 1), (21, 2), (13, 11)];
        let strands = [(100, 1), (101, 11), (102, 13), (103, 21), (104, 2), (105, 12)];
        assert_eq!(bodies(&strands, &guides), [1, 1, 1, 2, 2, 1]);
        // the roots are numbered in the order of the guides, also when a ray comes before its stipe
        assert_eq!(bodies(&strands, &[(21, 2), (2, -1), (11, 1), (1, -1)]), [2, 2, 0, 1, 1, 0]);
        // a guide is in its own body
        assert_eq!(bodies(&[(11, 1), (2, -1)], &guides), [1, 2]);
    }

    #[test]
    fn a_path_whose_parent_is_no_guide_is_in_body_0() {
        let guides = [(1, -1), (11, 1)];
        assert_eq!(bodies(&[(100, 7), (101, -1), (102, 11)], &guides), [0, 0, 1]);
        // a ray whose stipe is missing is the root of its own lineage
        assert_eq!(bodies(&[(100, 11), (101, 12)], &[(11, 1), (12, 1)]), [1, 2]);
        assert_eq!(bodies(&[(100, 11)], &[]), [0]);
        // a loop of parents ends
        assert_eq!(bodies(&[(100, 5)], &[(5, 6), (6, 5)]), [1]);
    }

    #[test]
    fn creators_get_small_numbers_in_the_order_of_their_ids() {
        let words = concat_arg_vecs(vec![list(1, 900), list(2, -3), list(1, 42)]);
        let paths = read_paths(&words, PathStyle::BODY);
        assert_eq!(group_numbers(&paths), vec![2, 0, 0, 1]);
    }
}
