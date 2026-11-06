use std::borrow::Cow;

use glam::Vec3;
use nodarium_utils::{
    encode_float,
    geometry::{is_path, wrap_path},
    split_args,
};

use crate::density::Segment;
use crate::hyphae::{flags, Hyphae, Tissue};

pub const LINES_TAG: i32 = 12;
const LINE_WORDS: usize = 7;
/// Groups below this are tissues in a mesh that has hyphae in it
pub const TISSUE_COUNT: u32 = 6;

/// How the surface around a path looks
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PathStyle {
    /// Body tissue fills the gaps between its paths, other paths keep theirs
    pub body: bool,
    /// Where two paths reach a vertex, the one with the larger kernel times priority owns it
    pub priority: f32,
    /// Strength of the fibrous relief
    pub fibre_strength: f32,
    /// How much longer than wide the relief is stretched along the path
    pub fibre_stretch: f32,
}

/// The cuticle is a single layer of hyphae over the cap flesh, so flesh hyphae
/// just below it often reach the surface vertices more strongly. The surface of
/// a cap is cuticle wherever a cuticle hypha comes close.
const CUTICLE_PRIORITY: f32 = 4.0;

impl PathStyle {
    /// Fibrous like a stipe, fills into one body with the other body paths
    pub const BODY: PathStyle = PathStyle { body: true, priority: 1.0, fibre_strength: 1.0, fibre_stretch: 6.0 };
    /// Thin plates and membranes like gills and the veil: a deep relief would cut through them
    pub const LOOSE: PathStyle = PathStyle { body: false, priority: 1.0, fibre_strength: 0.1, fibre_stretch: 4.0 };

    pub fn of_tissue(tissue: Tissue) -> PathStyle {
        let (body, priority, fibre_strength, fibre_stretch) = match tissue {
            Tissue::Stipe => (true, 1.0, 1.0, 6.0),
            Tissue::Veil => (false, 1.0, 0.8, 5.0),
            Tissue::Mycelium => (true, 1.0, 0.8, 3.0),
            Tissue::Cap => (true, 1.0, 0.3, 2.0),
            // short stretch gives patches, which read as scales
            Tissue::CapCuticle => (true, CUTICLE_PRIORITY, 0.35, 1.5),
            Tissue::Gill => (false, 1.0, 0.1, 4.0),
        };
        PathStyle { body, priority, fibre_strength, fibre_stretch }
    }
}

/// What decides the material group of a path
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathGroup {
    /// The graph node that made the path
    Creator(i32),
    /// Hyphae grown by the old nodes keep their tissue as the group
    Tissue(Tissue),
}

/// A path to mesh: its points as `x, y, z, radius`, its group and how its surface looks
#[derive(Clone, Debug, PartialEq)]
pub struct MeshPath<'a> {
    pub points: Cow<'a, [f32]>,
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
            out.push(MeshPath { points: Cow::Borrowed(path.points), group: PathGroup::Creator(path.link.creator), style });
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

impl Hyphae {
    /// The hyphae as paths: a path follows the first child of every segment as long as the
    /// tissue stays the same, other children start a path at their parent's end. Every path is in
    /// the group of its tissue and looks like it.
    pub fn to_paths(&self) -> Vec<MeshPath<'static>> {
        let mut paths: Vec<(Vec<f32>, Tissue)> = Vec::new();
        // the path that ends with the end of a segment and has no continuation yet
        let mut open: Vec<Option<usize>> = vec![None; self.len()];
        for (i, s) in self.segments.iter().enumerate() {
            let point = [s.end.x, s.end.y, s.end.z, s.radius];
            let parent = s.parent.filter(|_| s.flags & flags::BROKEN == 0);
            if let Some(p) = parent {
                let p = p as usize;
                match open[p] {
                    Some(k) if paths[k].1 == s.tissue => {
                        open[p] = None;
                        paths[k].0.extend(point);
                        open[i] = Some(k);
                    }
                    _ => {
                        let start = self.segments[p].end;
                        let mut points = vec![start.x, start.y, start.z, s.radius];
                        points.extend(point);
                        paths.push((points, s.tissue));
                        open[i] = Some(paths.len() - 1);
                    }
                }
            } else {
                paths.push((point.to_vec(), s.tissue));
                open[i] = Some(paths.len() - 1);
            }
        }
        paths
            .into_iter()
            .map(|(points, tissue)| MeshPath {
                points: Cow::Owned(points),
                group: PathGroup::Tissue(tissue),
                style: PathStyle::of_tissue(tissue),
            })
            .collect()
    }
}

/// The paths or the hyphae of an input, all of them in `style`. Hyphae keep the look of their
/// tissue, loose hyphae don't fill into a body.
pub fn read_mesh_paths(arg: &[i32], style: PathStyle) -> Vec<MeshPath<'_>> {
    if arg.first() == Some(&crate::hyphae::HYPHAE_TAG) {
        let mut paths = Hyphae::decode(arg).to_paths();
        if !style.body {
            for p in &mut paths {
                p.style.body = false;
            }
        }
        return paths;
    }
    read_paths(arg, style)
}

/// A small material group per path: creators numbered in the order of their ids, so a node
/// added later gets the next number and the others keep theirs. Tissues keep their own number,
/// creators then come after the tissues. The flag tells whether there are tissue groups.
pub fn group_numbers(paths: &[MeshPath]) -> (Vec<u32>, bool) {
    let tissues = paths.iter().any(|p| matches!(p.group, PathGroup::Tissue(_)));
    let mut creators: Vec<i32> = paths
        .iter()
        .filter_map(|p| match p.group {
            PathGroup::Creator(c) => Some(c),
            PathGroup::Tissue(_) => None,
        })
        .collect();
    creators.sort_unstable();
    creators.dedup();
    let offset = if tissues { TISSUE_COUNT } else { 0 };
    let numbers = paths
        .iter()
        .map(|p| match p.group {
            PathGroup::Tissue(t) => t as u32,
            PathGroup::Creator(c) => offset + creators.binary_search(&c).unwrap() as u32,
        })
        .collect();
    (numbers, tissues)
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
    let (groups, _) = group_numbers(paths);
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
    use crate::hyphae::HyphaSegment;
    use nodarium_utils::{
        concat_arg_vecs,
        geometry::{create_multiple_paths, wrap_multiple_paths},
    };

    fn segment(end: Vec3, parent: Option<u32>, tissue: Tissue) -> HyphaSegment {
        HyphaSegment { end, radius: 0.01, parent, birth: 0.0, tissue, flags: 0 }
    }

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
        assert_eq!(group_numbers(&paths), (vec![2, 0, 0, 1], false));
    }

    #[test]
    fn hyphae_follow_their_first_child_until_the_tissue_changes() {
        let mut h = Hyphae::default();
        let root = h.push(segment(Vec3::ZERO, None, Tissue::Stipe));
        let a = h.push(segment(Vec3::Y, Some(root), Tissue::Stipe));
        let b = h.push(segment(Vec3::Y * 2.0, Some(a), Tissue::Stipe));
        h.push(segment(Vec3::new(1.0, 1.0, 0.0), Some(a), Tissue::Stipe));
        h.push(segment(Vec3::Y * 3.0, Some(b), Tissue::Cap));
        h.push(HyphaSegment { flags: flags::BROKEN, ..segment(Vec3::Y * 4.0, Some(4), Tissue::Cap) });
        let paths = h.to_paths();
        let points: Vec<Vec<f32>> = paths.iter().map(|p| p.points.chunks(4).map(|c| c[1]).collect()).collect();
        assert_eq!(points, vec![vec![0.0, 1.0, 2.0], vec![1.0, 1.0], vec![2.0, 3.0], vec![4.0]]);
        assert_eq!(paths[2].group, PathGroup::Tissue(Tissue::Cap));
        assert_eq!(paths[2].style, PathStyle::of_tissue(Tissue::Cap));
        // every segment of the hyphae is in a path, the broken one as a point
        let segments: usize = paths.iter().map(|p| path_segments(&p.points).len()).sum();
        assert_eq!(segments, h.len() - 1);
    }

    #[test]
    fn tissues_keep_their_numbers_next_to_creators() {
        let mut h = Hyphae::default();
        let root = h.push(segment(Vec3::ZERO, None, Tissue::Gill));
        h.push(segment(Vec3::Y, Some(root), Tissue::Gill));
        let mut paths = h.to_paths();
        let words = list(1, 77);
        paths.extend(read_paths(&words, PathStyle::BODY));
        assert_eq!(group_numbers(&paths), (vec![Tissue::Gill as u32, TISSUE_COUNT], true));
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
        let path = MeshPath { points: Cow::Owned(points), group: PathGroup::Creator(1), style: PathStyle::BODY };
        let kept = retain_points(&[path], |p| p.z <= 0.0);
        assert_eq!(kept.iter().map(|p| p.len()).collect::<Vec<_>>(), [1, 2]);
    }
}
