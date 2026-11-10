use std::borrow::Cow;

use glam::Vec3;
use nodarium_utils::{
    encode_float,
    geometry::{is_path, wrap_path},
    split_args,
};

use crate::density::Segment;
use crate::settings::PathSettings;

pub const LINES_TAG: i32 = 12;
const LINE_WORDS: usize = 7;

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

/// A path to mesh: its points as `x, y, z, radius`, its group and how its surface looks
#[derive(Clone, Debug, PartialEq)]
pub struct MeshPath<'a> {
    pub points: Cow<'a, [f32]>,
    pub group: PathGroup,
    pub style: PathStyle,
    /// Values of the mesh node the path brings with it, see [`crate::apply_settings`]
    pub settings: PathSettings,
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
            out.push(MeshPath { points: Cow::Borrowed(path.points), group, style, settings: PathSettings::default() });
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

/// A small material group per path: creators numbered in the order of their ids, so a node
/// added later gets the next number and the others keep theirs
pub fn group_numbers(paths: &[MeshPath]) -> Vec<u32> {
    let creator = |p: &MeshPath| match p.group {
        PathGroup::Creator(c) => c,
    };
    let mut creators: Vec<i32> = paths.iter().map(creator).collect();
    creators.sort_unstable();
    creators.dedup();
    paths.iter().map(|p| creators.binary_search(&creator(p)).unwrap() as u32).collect()
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

/// The runs of a path whose points `keep` accepts, each run becomes a path of its own
pub fn retain_points<'a>(paths: &[MeshPath<'a>], keep: impl Fn(Vec3) -> bool) -> Vec<MeshPath<'a>> {
    let mut out = Vec::new();
    for path in paths {
        let mut run: Vec<f32> = Vec::new();
        for i in 0..path.len() {
            if keep(path.point(i)) {
                run.extend_from_slice(&path.points[i * 4..i * 4 + 4]);
            } else if !run.is_empty() {
                out.push(MeshPath { points: Cow::Owned(std::mem::take(&mut run)), ..path.clone() });
            }
        }
        if !run.is_empty() {
            out.push(MeshPath { points: Cow::Owned(run), ..path.clone() });
        }
    }
    out
}

/// The paths as line segments for the viewer `[12, n, ...n × (x0, y0, z0, x1, y1, z1, group)]`,
/// in the groups a mesh of the same paths has
pub fn path_lines(paths: &[MeshPath]) -> Vec<i32> {
    let groups = group_numbers(paths);
    let n: usize = paths.iter().map(|p| p.len().saturating_sub(1)).sum();
    let mut words = Vec::with_capacity(2 + n * LINE_WORDS);
    words.extend([LINES_TAG, n as i32]);
    for (path, &group) in paths.iter().zip(&groups) {
        for i in 1..path.len() {
            let (a, b) = (path.point(i - 1), path.point(i));
            words.extend([a.x, a.y, a.z, b.x, b.y, b.z].map(encode_float));
            words.push(group as i32);
        }
    }
    words
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

    #[test]
    fn lines_carry_the_groups_of_the_mesh() {
        let words = concat_arg_vecs(vec![list(1, 8), list(1, 3)]);
        let paths = read_paths(&words, PathStyle::BODY);
        let words = path_lines(&paths);
        assert_eq!(&words[..2], &[LINES_TAG, 4]);
        assert_eq!(words.len(), 2 + 4 * LINE_WORDS);
        assert_eq!(words[2 + LINE_WORDS - 1], 1);
        assert_eq!(words[2 + 3 * LINE_WORDS - 1], 0);
    }

    #[test]
    fn retaining_points_splits_paths() {
        let points = vec![0.0, 0.0, -1.0, 0.1, 0.0, 1.0, 1.0, 0.1, 0.0, 2.0, -1.0, 0.1, 0.0, 3.0, -1.0, 0.1];
        let path = MeshPath {
            points: Cow::Owned(points),
            group: PathGroup::Creator(1),
            style: PathStyle::BODY,
            settings: PathSettings::default(),
        };
        let kept = retain_points(&[path], |p| p.z <= 0.0);
        assert_eq!(kept.iter().map(|p| p.len()).collect::<Vec<_>>(), [1, 2]);
    }
}
