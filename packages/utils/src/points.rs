//! Points on paths, where nodes like `branch_from_points` and `instance_from_points` place what
//! they make.
//!
//! An encoded point is
//! `[0, 16, 4, parent_id, depth, index, x, y, z, dx, dy, dz, tx, ty, tz, scale, alpha, 1, 1]`,
//! a list of points is a framed list of those. The type word 4 sets it apart from paths (0),
//! geometry (1), instances (2) and forces (3).

use crate::tree::split_args;
use crate::{decode_float, encode_float};
use glam::Vec3;

/// The type word of an encoded point.
pub const POINT_TYPE: i32 = 4;

// type, parent_id, depth, index, position (3), direction (3), tangent (3), scale, alpha
const POINT_WORDS: usize = 15;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub position: Vec3,
    /// Points away from the path, orthogonal to `tangent`. A branch grows this way and an
    /// instance sticks out this way, before their own angle tilts them along the path.
    pub direction: Vec3,
    /// The direction of the path at the point.
    pub tangent: Vec3,
    /// The thickness of the path at the point.
    pub scale: f32,
    /// The id of the path the point sits on.
    pub parent_id: i32,
    /// Where on the path the point sits, 0 at the base to 1 at the tip, along its length.
    pub alpha: f32,
    /// The depth of the path the point sits on.
    pub depth: i32,
    /// The number of the point among the points its node made on the same path, the first is 0.
    /// What grows on the point uses it as its own index, so its id stays the same when other
    /// points come or go.
    pub index: i32,
}

impl Point {
    /// The point as one framed item, ready for `concat_arg_vecs`.
    pub fn encode(&self) -> Vec<i32> {
        let mut data = vec![
            0,
            POINT_WORDS as i32 + 1,
            POINT_TYPE,
            self.parent_id,
            self.depth,
            self.index,
        ];
        data.extend(
            [
                self.position.x,
                self.position.y,
                self.position.z,
                self.direction.x,
                self.direction.y,
                self.direction.z,
                self.tangent.x,
                self.tangent.y,
                self.tangent.z,
                self.scale,
                self.alpha,
            ]
            .map(encode_float),
        );
        data.extend([1, 1]);
        data
    }

    /// Reads one framed item, `None` if it is not a point.
    pub fn decode(item: &[i32]) -> Option<Self> {
        if item.len() != POINT_WORDS + 4 || item[2] != POINT_TYPE {
            return None;
        }
        let value = |i: usize| decode_float(item[6 + i]);
        let vec3 = |i: usize| Vec3::new(value(i), value(i + 1), value(i + 2));
        Some(Self {
            parent_id: item[3],
            depth: item[4],
            index: item[5],
            position: vec3(0),
            direction: vec3(3),
            tangent: vec3(6),
            scale: value(9),
            alpha: value(10),
        })
    }
}

/// A list of points as one framed list.
pub fn encode_points(points: &[Point]) -> Vec<i32> {
    crate::concat_arg_vecs(points.iter().map(Point::encode).collect())
}

/// Reads a framed list of points, items that are not points are skipped.
pub fn decode_points(list: &[i32]) -> Vec<Point> {
    if list.len() < 4 {
        return Vec::new();
    }
    split_args(list)
        .into_iter()
        .filter_map(Point::decode)
        .collect()
}

/// The points grouped by the path they sit on, in the order the paths first appear. A node that
/// works on the points of a path can start that path once and go through its points.
pub fn group_points_by_parent(points: &[Point]) -> Vec<(i32, Vec<Point>)> {
    let mut groups: Vec<(i32, Vec<Point>)> = Vec::new();
    for point in points {
        match groups.iter_mut().find(|(id, _)| *id == point.parent_id) {
            Some((_, group)) => group.push(*point),
            None => groups.push((point.parent_id, vec![*point])),
        }
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(parent_id: i32, index: i32) -> Point {
        Point {
            position: Vec3::new(1.0, 2.0, 3.0),
            direction: Vec3::X,
            tangent: Vec3::Y,
            scale: 0.25,
            parent_id,
            alpha: 0.5,
            depth: 2,
            index,
        }
    }

    #[test]
    fn a_point_survives_a_round_trip() {
        let p = point(7, 3);
        assert_eq!(Point::decode(&p.encode()), Some(p));
    }

    #[test]
    fn a_list_of_points_survives_a_round_trip() {
        let points = vec![point(7, 0), point(7, 1), point(9, 0)];
        assert_eq!(decode_points(&encode_points(&points)), points);
    }

    #[test]
    fn an_empty_list_has_no_points() {
        assert!(decode_points(&encode_points(&[])).is_empty());
        assert!(decode_points(&[]).is_empty());
    }

    #[test]
    fn a_point_with_zeros_in_it_is_read_back() {
        // a value of 0 looks like an opening bracket, the framing must not trip over it
        let p = Point {
            position: Vec3::ZERO,
            direction: Vec3::ZERO,
            tangent: Vec3::ZERO,
            scale: 0.0,
            parent_id: 0,
            alpha: 0.0,
            depth: 0,
            index: 0,
        };
        assert_eq!(decode_points(&encode_points(&[p, p])), vec![p, p]);
    }

    #[test]
    fn other_items_are_not_points() {
        let force = crate::Force {
            kind: crate::ForceKind::Point,
            position: Vec3::ZERO,
            strength: 1.0,
            radius: 1.0,
            falloff: 0.0,
        };
        assert_eq!(Point::decode(&force.encode()), None);
    }

    #[test]
    fn points_are_grouped_by_their_path_in_order() {
        let groups = group_points_by_parent(&[point(9, 0), point(7, 0), point(9, 1)]);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].0, 9);
        assert_eq!(
            groups[0].1.iter().map(|p| p.index).collect::<Vec<_>>(),
            [0, 1]
        );
        assert_eq!(groups[1].0, 7);
    }
}
