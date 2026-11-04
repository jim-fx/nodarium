use glam::{vec3, vec4, Vec3, Vec4, Vec4Swizzles};

// 0: node-type, stem: 0
// 1: depth
// 2: id, a hash of the creator, the parent and the index, so it is unique in a list of paths
//    and stays the same when other paths come or go
// 3: id of the parent path, `NO_PARENT` for a path that grows on its own
// 4: where on the parent this path is attached (f32 bits, 0..1 along its length)
// 5: creator, the id of the graph node that made the path
// 6: index among the paths the creator made on the same parent, the first is 0
static PATH_HEADER_SIZE: usize = 7;

/// The parent id of a path that is not attached to another path.
pub const NO_PARENT: i32 = -1;

/// Whether a list item is an encoded path with at least one point, brackets included.
pub fn is_path(item: &[i32]) -> bool {
    item.len() >= 2 + PATH_HEADER_SIZE + 4 + 2 && item[2] == 0
}

/// The depth of the deepest path in a list, 0 when there is no path.
pub fn max_path_depth<T: AsRef<[i32]>>(items: &[T]) -> i32 {
    items
        .iter()
        .map(AsRef::as_ref)
        .filter(|item| is_path(item))
        .map(|item| item[3])
        .max()
        .unwrap_or(0)
}

/// The paths on the last `depth` levels of a list, counted from the deepest path. These are the
/// paths a node with a depth input works on, a depth of 1 is only the deepest level.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeepestPaths {
    pub min_depth: i32,
}

impl DeepestPaths {
    pub fn new<T: AsRef<[i32]>>(items: &[T], depth: i32) -> Self {
        Self {
            min_depth: max_path_depth(items) - depth + 1,
        }
    }

    pub fn contains(&self, item: &[i32]) -> bool {
        is_path(item) && item[3] >= self.min_depth
    }

    pub fn count<T: AsRef<[i32]>>(&self, items: &[T]) -> usize {
        items
            .iter()
            .filter(|item| self.contains(item.as_ref()))
            .count()
    }
}

/// The id of the path number `index` that `creator` grows on `parent_id` (`NO_PARENT` for a path
/// that grows on its own). It only depends on these three, so it is the same in every run and
/// when the paths around it change. It is never negative, so it can't be mistaken for `NO_PARENT`.
pub fn path_id(creator: i32, parent_id: i32, index: i32) -> i32 {
    let mut hash = 0x811c_9dc5_u32;
    for word in [creator, parent_id, index] {
        hash = (hash ^ word as u32).wrapping_mul(0x9e37_79b1);
        hash ^= hash >> 15;
        hash = hash.wrapping_mul(0x85eb_ca6b);
        hash ^= hash >> 13;
    }
    (hash & 0x7fff_ffff) as i32
}

/// A number from 0 to 1 that only depends on the seed, the path and `salt`.
pub fn path_random(seed: i32, id: i32, salt: i32) -> f32 {
    path_id(seed, id, salt) as f32 / i32::MAX as f32
}

/// Where a path sits in the structure of paths: its own id, who made it and the spot on its
/// parent it grows from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PathLink {
    pub id: i32,
    pub parent_id: i32,
    pub parent_alpha: f32,
    pub creator: i32,
    pub index: i32,
}

impl PathLink {
    /// The `index`-th path `creator` grows on its own.
    pub fn root(creator: i32, index: i32) -> Self {
        Self {
            id: path_id(creator, NO_PARENT, index),
            parent_id: NO_PARENT,
            parent_alpha: 0.0,
            creator,
            index,
        }
    }

    /// The `index`-th path `creator` grows on the path `parent_id`, `parent_alpha` along its length.
    pub fn child(creator: i32, parent_id: i32, parent_alpha: f32, index: i32) -> Self {
        Self {
            id: path_id(creator, parent_id, index),
            parent_id,
            parent_alpha,
            creator,
            index,
        }
    }

    pub fn has_parent(&self) -> bool {
        self.parent_id != NO_PARENT
    }
}

/// Reads the link of an encoded path, brackets included.
pub fn path_link(path: &[i32]) -> PathLink {
    PathLink {
        id: path[4],
        parent_id: path[5],
        parent_alpha: f32::from_bits(path[6] as u32),
        creator: path[7],
        index: path[8],
    }
}

/// Writes the link of an encoded path, brackets included.
pub fn set_path_link(path: &mut [i32], link: PathLink) {
    path[4] = link.id;
    path[5] = link.parent_id;
    path[6] = link.parent_alpha.to_bits() as i32;
    path[7] = link.creator;
    path[8] = link.index;
}

fn read_link(header: &[i32]) -> PathLink {
    PathLink {
        id: header[2],
        parent_id: header[3],
        parent_alpha: f32::from_bits(header[4] as u32),
        creator: header[5],
        index: header[6],
    }
}

#[derive(Debug)]
pub struct PathDataMut<'a> {
    pub length: usize,
    pub depth: i32,
    pub link: PathLink,
    pub points: &'a mut [f32],
}

impl PathDataMut<'_> {
    pub fn get_length(&self) -> f32 {
        let mut l = 0.0;
        for i in 0..(self.length - 1) {
            let a = vec3(
                self.points[i * 4],
                self.points[i * 4 + 1],
                self.points[i * 4 + 2],
            );
            let b = vec3(
                self.points[(i + 1) * 4],
                self.points[(i + 1) * 4 + 1],
                self.points[(i + 1) * 4 + 2],
            );
            l += (b - a).length();
        }
        l
    }
}

#[derive(Debug)]
pub struct PathData<'a> {
    pub depth: i32,
    pub link: PathLink,
    pub length: usize,
    pub points: &'a [f32],
}

impl PathData<'_> {
    pub fn get_length(&self) -> f32 {
        let mut l = 0.0;
        for i in 0..(self.length - 1) {
            let a = vec3(
                self.points[i * 4],
                self.points[i * 4 + 1],
                self.points[i * 4 + 2],
            );
            let b = vec3(
                self.points[(i + 1) * 4],
                self.points[(i + 1) * 4 + 1],
                self.points[(i + 1) * 4 + 2],
            );
            l += (b - a).length();
        }
        l
    }
    pub fn get_point_at(&self, alpha: f32) -> [f32; 4] {
        get_point_at_path(self.points, alpha)
    }
    pub fn get_direction_at(&self, alpha: f32) -> [f32; 3] {
        get_direction_at_path(self.points, alpha)
    }
    pub fn interpolate_along(&self, alpha: f32) -> (Vec4, Vec3, Vec3) {
        interpolate_along_path(self.points, alpha)
    }
}

/// `amount` paths that grow on their own, made by the graph node `creator`.
pub fn create_multiple_paths(
    amount: usize,
    point_amount: usize,
    depth: i32,
    creator: i32,
) -> Vec<i32> {
    let output_size = amount * (point_amount * 4 + PATH_HEADER_SIZE + 4) + 4;

    let mut path: Vec<i32> = vec![0; output_size];

    path[0] = 0;
    path[1] = 1;
    path[output_size - 2] = 1;
    path[output_size - 1] = 1;

    for i in 0..amount {
        let start_index = 2 + i * (point_amount * 4 + PATH_HEADER_SIZE + 4);
        let end_index = 2 + (i + 1) * (point_amount * 4 + PATH_HEADER_SIZE + 4);

        path[start_index] = 0;
        path[start_index + 1] = (point_amount * 4 + PATH_HEADER_SIZE + 1) as i32;
        path[start_index + 2] = 0;
        path[start_index + 3] = depth;
        set_path_link(
            &mut path[start_index..end_index],
            PathLink::root(creator, i as i32),
        );

        path[end_index - 2] = 1;
        path[end_index - 1] = 1;
    }

    path
}

pub fn wrap_multiple_paths(mut input: &mut [i32]) -> Vec<PathDataMut<'_>> {
    let mut paths = Vec::new();
    let mut end_index = 2;

    // remove starting bracket
    input = &mut input[2..];

    while end_index < input.len() - 1 {
        end_index = input[1] as usize + 3;
        if end_index < input.len() {
            let (path_slice, remaining) = input.split_at_mut(end_index);
            let path_data = wrap_path_mut(path_slice);
            paths.push(path_data);
            input = remaining;
        } else {
            break;
        }
    }

    paths
}

pub fn create_path(point_amount: usize, depth: i32) -> Vec<i32> {
    let output_size = point_amount * 4 + PATH_HEADER_SIZE + 4;

    let mut path: Vec<i32> = vec![0; output_size];

    path[0] = 0;
    path[1] = ((point_amount * 4) + PATH_HEADER_SIZE + 1) as i32;
    path[2] = 0;
    path[3] = depth;
    set_path_link(&mut path, PathLink::root(0, 0));
    path[output_size - 2] = 1;
    path[output_size - 1] = 1;

    path
}

pub fn wrap_path(input: &[i32]) -> PathData<'_> {
    assert!(
        input.len() > PATH_HEADER_SIZE,
        "Geometry vector does not contain enough data for a header."
    );

    let rest = &input[2..];

    let header = &rest[..PATH_HEADER_SIZE];

    let points_slice = &rest[PATH_HEADER_SIZE..rest.len() - 2];

    assert!(
        points_slice.len().is_multiple_of(4),
        "Points slice does not match the expected size. {}",
        points_slice.len()
    );

    let length = points_slice.len() / 4;

    let points: &[f32] = unsafe {
        std::slice::from_raw_parts(points_slice.as_ptr() as *const f32, points_slice.len())
    };

    PathData {
        depth: header[1],
        link: read_link(header),
        length,
        points,
    }
}

pub fn wrap_path_mut<'a>(geometry: &'a mut [i32]) -> PathDataMut<'a> {
    assert!(
        geometry.len() > PATH_HEADER_SIZE,
        "Geometry vector does not contain enough data for a header."
    );

    let (_opening_brackets, rest) = geometry.split_at_mut(2);

    let (header, rest) = rest.split_at_mut(PATH_HEADER_SIZE);

    // split after points, excluding last two that encode closing bracket
    let (points_slice, _closing_bracket) = rest.split_at_mut(rest.len() - 2);

    assert!(
        points_slice.len() % 4 == 0,
        "Points slice does not match the expected size. {}",
        points_slice.len()
    );

    let length = points_slice.len() / 4;

    let points: &'a mut [f32] = unsafe {
        std::slice::from_raw_parts_mut(points_slice.as_mut_ptr() as *mut f32, points_slice.len())
    };

    PathDataMut {
        depth: header[1],
        link: read_link(header),
        length,
        points,
    }
}

pub fn get_point_at_path(path: &[f32], alpha: f32) -> [f32; 4] {
    let a = alpha.min(0.999999).max(0.000001);

    let num_points = path.len() / 4;
    let segment_length = 1.0 / (num_points - 1) as f32;
    let mut target_index = (a / segment_length).floor() as usize;
    target_index = target_index.min(num_points - 2);

    let start_index = target_index * 4;
    let end_index = (target_index + 1) * 4;

    let t = (a - target_index as f32 * segment_length) / segment_length;

    let x1 = path[start_index];
    let y1 = path[start_index + 1];
    let z1 = path[start_index + 2];
    let w1 = path[start_index + 3];

    let x2 = path[end_index];
    let y2 = path[end_index + 1];
    let z2 = path[end_index + 2];
    let w2 = path[end_index + 3];

    let x = x1 + t * (x2 - x1);
    let y = y1 + t * (y2 - y1);
    let z = z1 + t * (z2 - z1);
    let w = w1 + t * (w2 - w1);

    [x, y, z, w]
}

pub fn get_direction_at_path(path: &[f32], alpha: f32) -> [f32; 3] {
    let num_points = path.len() / 4;

    let a = alpha.min(0.999999).max(0.000001);

    let segment_length = 1.0 / (num_points - 1) as f32;
    let target_index = (a / segment_length).floor() as usize;

    let start_index = target_index * 4;
    let end_index = (target_index + 1) * 4;

    let x1 = path[start_index];
    let y1 = path[start_index + 1];
    let z1 = path[start_index + 2];
    let x2 = path[end_index];
    let y2 = path[end_index + 1];
    let z2 = path[end_index + 2];

    let dx = x2 - x1;
    let dy = y2 - y1;
    let dz = z2 - z1;

    let norm = (dx * dx + dy * dy + dz * dz).sqrt();

    if norm == 0.0 {
        return [0.0, 1.0, 0.0];
    }

    [dx / norm, dy / norm, dz / norm]
}

/// The direction of a segment and a vector orthogonal to it. A vertical segment has no
/// orthogonal in the horizontal plane, it gets one along the z axis instead.
fn segment_frame(start: Vec3, end: Vec3) -> (Vec3, Vec3) {
    let tangent = (end - start).try_normalize().unwrap_or(Vec3::Y);
    (orthogonal_to(tangent), tangent)
}

/// A unit vector orthogonal to `tangent`, in the horizontal plane where there is one. A vertical
/// tangent has none there, it gets one along the z axis instead. This is the side a branch or an
/// instance points to before it is turned around the path.
pub fn orthogonal_to(tangent: Vec3) -> Vec3 {
    tangent
        .cross(Vec3::Y)
        .try_normalize()
        .unwrap_or_else(|| tangent.cross(Vec3::X).normalize())
}

/// The position (with thickness) at `alpha` along the length of the path, a vector orthogonal
/// to the path there and the direction of the path.
pub fn interpolate_along_path(points_data: &[f32], _alpha: f32) -> (Vec4, Vec3, Vec3) {
    let alpha = _alpha.min(0.999999).max(0.000001);
    assert!(
        points_data.len().is_multiple_of(4),
        "The points data must be a multiple of 4."
    );

    let num_points = points_data.len() / 4;
    assert!(
        num_points > 1,
        "There must be at least two points to define a path."
    );

    let mut segment_lengths = Vec::with_capacity(num_points - 1);
    let mut total_length = 0.0;
    for i in 0..num_points - 1 {
        let start_index = i * 4;
        let end_index = (i + 1) * 4;
        let start_point = vec3(
            points_data[start_index],
            points_data[start_index + 1],
            points_data[start_index + 2],
        );
        let end_point = vec3(
            points_data[end_index],
            points_data[end_index + 1],
            points_data[end_index + 2],
        );
        let length = (end_point - start_point).length();
        segment_lengths.push(length);
        total_length += length;
    }

    let target_length = alpha * total_length;
    let mut accumulated_length = 0.0;

    for (i, &length) in segment_lengths.iter().enumerate() {
        if accumulated_length + length >= target_length {
            let segment_alpha = if length > 0.0 {
                (target_length - accumulated_length) / length
            } else {
                0.0
            };
            let start_index = i * 4;
            let end_index = (i + 1) * 4;
            let start_point = vec4(
                points_data[start_index],
                points_data[start_index + 1],
                points_data[start_index + 2],
                points_data[start_index + 3],
            );
            let end_point = vec4(
                points_data[end_index],
                points_data[end_index + 1],
                points_data[end_index + 2],
                points_data[end_index + 3],
            );
            let position = start_point + (end_point - start_point) * segment_alpha;
            let (orthogonal, tangent) = segment_frame(start_point.xyz(), end_point.xyz());

            return (position, orthogonal, tangent);
        }
        accumulated_length += length;
    }

    // As a fallback for numerical precision issues, use the last point and a default orthogonal vector.
    let last_start_index = (num_points - 2) * 4;
    let last_end_index = (num_points - 1) * 4;
    let last_start_point = vec4(
        points_data[last_start_index],
        points_data[last_start_index + 1],
        points_data[last_start_index + 2],
        points_data[last_start_index + 3],
    );
    let last_end_point = vec4(
        points_data[last_end_index],
        points_data[last_end_index + 1],
        points_data[last_end_index + 2],
        points_data[last_end_index + 3],
    );
    let (last_orthogonal, last_tangent) =
        segment_frame(last_start_point.xyz(), last_end_point.xyz());
    (last_end_point, last_orthogonal, last_tangent)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_random_is_in_unit_range_and_repeatable() {
        for seed in [0, 1, -7, i32::MAX] {
            for id in [0, 3, 12_345, i32::MIN] {
                for salt in 0..4 {
                    let value = path_random(seed, id, salt);
                    assert!((0.0..=1.0).contains(&value), "{value} for {seed} {id} {salt}");
                    assert_eq!(value, path_random(seed, id, salt));
                }
            }
        }
        assert_ne!(path_random(1, 2, 0), path_random(1, 2, 1));
    }

    fn assert_frame(path: &[f32], alpha: f32) {
        let (position, orthogonal, tangent) = interpolate_along_path(path, alpha);

        assert!(position.is_finite(), "position {:?}", position);
        assert!(orthogonal.is_normalized(), "orthogonal {:?}", orthogonal);
        assert!(tangent.is_normalized(), "tangent {:?}", tangent);
        assert!(orthogonal.dot(tangent).abs() < 1e-6);
    }

    #[test]
    fn test_interpolate_along_vertical_path() {
        let path = [0.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.5, 0.0, 2.0, 0.0, 0.0];

        for alpha in [0.0, 0.3, 0.5, 1.0] {
            assert_frame(&path, alpha);
        }
    }

    #[test]
    fn test_interpolate_along_collapsed_path() {
        let path = [1.0, 2.0, 3.0, 1.0, 1.0, 2.0, 3.0, 0.0];

        assert_frame(&path, 0.5);
    }

    #[test]
    fn multiple_paths_get_their_own_id_and_no_parent() {
        let mut data = create_multiple_paths(3, 4, 1, 42);
        let paths = wrap_multiple_paths(&mut data);

        assert_eq!(paths.len(), 3);
        for (index, path) in paths.iter().enumerate() {
            assert_eq!(path.link, PathLink::root(42, index as i32));
            assert_eq!(path.link.creator, 42);
            assert_eq!(path.link.index, index as i32);
            assert_eq!(path.length, 4);
            assert!(!path.link.has_parent());
        }
        let mut ids: Vec<i32> = paths.iter().map(|p| p.link.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 3);
    }

    #[test]
    fn the_link_survives_a_round_trip_through_the_header() {
        let mut data = create_path(3, 2);
        let link = PathLink {
            id: 7,
            parent_id: 3,
            parent_alpha: 0.25,
            creator: 11,
            index: 5,
        };
        set_path_link(&mut data, link);

        assert_eq!(path_link(&data), link);
        assert_eq!(wrap_path(&data).link, link);
        assert_eq!(wrap_path_mut(&mut data).link, link);
        assert_eq!(wrap_path(&data).depth, 2);
        assert_eq!(wrap_path(&data).length, 3);
    }

    #[test]
    fn an_id_only_depends_on_creator_parent_and_index() {
        assert_eq!(path_id(4, 9, 2), path_id(4, 9, 2));
        assert_ne!(path_id(4, 9, 2), path_id(5, 9, 2));
        assert_ne!(path_id(4, 9, 2), path_id(4, 8, 2));
        assert_ne!(path_id(4, 9, 2), path_id(4, 9, 3));
        // the same index under a different parent is another path
        assert_ne!(path_id(4, NO_PARENT, 0), path_id(4, path_id(4, NO_PARENT, 0), 0));
    }

    #[test]
    fn an_id_is_never_negative_and_rarely_collides() {
        let mut ids = std::collections::HashSet::new();
        for creator in 0..20 {
            for parent in -1..30 {
                for index in 0..50 {
                    let id = path_id(creator, parent, index);
                    assert!(id >= 0);
                    ids.insert(id);
                }
            }
        }
        assert_eq!(ids.len(), 20 * 31 * 50);
    }

    #[test]
    fn is_path_accepts_a_path_with_one_point_and_nothing_shorter() {
        let one_point = create_path(1, 0);
        assert!(is_path(&one_point));
        assert_eq!(wrap_path(&one_point).length, 1);

        assert!(!is_path(&create_path(0, 0)));
        assert!(!is_path(&one_point[..one_point.len() - 1]));
        assert!(!is_path(&[0, 1, 1, 1]));
        assert!(!is_path(&[5]));

        let mut geometry = create_path(3, 0);
        geometry[2] = 1;
        assert!(!is_path(&geometry));
    }

    #[test]
    fn deepest_paths_keep_the_last_levels() {
        let items = vec![
            create_path(3, 0),
            create_path(3, 1),
            create_path(3, 2),
            vec![7],
            create_path(0, 5),
        ];
        assert_eq!(max_path_depth(&items), 2);
        assert_eq!(max_path_depth::<Vec<i32>>(&[]), 0);

        let deepest = DeepestPaths::new(&items, 1);
        assert_eq!(deepest.min_depth, 2);
        assert!(deepest.contains(&items[2]));
        assert!(!deepest.contains(&items[1]));
        assert_eq!(deepest.count(&items), 1);

        let slices: Vec<&[i32]> = items.iter().map(|item| item.as_slice()).collect();
        assert_eq!(DeepestPaths::new(&slices, 2).count(&slices), 2);
        assert_eq!(DeepestPaths::new(&slices, 9).count(&slices), 3);
    }

    #[test]
    fn a_child_is_linked_to_its_parent() {
        let parent = PathLink::root(3, 1);
        let child = PathLink::child(8, parent.id, 0.5, 2);

        assert_eq!(child.parent_id, parent.id);
        assert_eq!(child.parent_alpha, 0.5);
        assert_eq!(child.creator, 8);
        assert_eq!(child.index, 2);
        assert!(child.has_parent());
        assert_eq!(child.id, path_id(8, parent.id, 2));
    }
}
