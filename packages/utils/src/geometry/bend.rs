use super::{path_link, PathStructure};
use crate::Point;
use glam::Vec3;

/// Lead-in segments never go past this many, however far a path starts from its root
const MAX_LEAD: usize = 256;

/// How far a point at `distance` from the root of a path lies into `reach`, from 0 at the root
/// to 1 at `reach` and beyond. A `reach` of 0 or less puts every point at 1.
pub fn place_alpha(distance: f32, reach: f32) -> f32 {
    if reach > 0.0 {
        (distance / reach).min(1.0)
    } else {
        1.0
    }
}

/// Where the path at `index` of `paths` grows from, for a bend by place:
/// - the point of `roots` that sits on its parent (the point's path is the path's parent), the
///   one closest to where on the parent it grows if there are several. This is how a node gets
///   the root of paths whose parent is not in the list, like the hinge of a cap for its rays.
/// - else the spot on its parent in the list,
/// - else its own base.
pub fn place_root(
    structure: &PathStructure,
    paths: &[Vec<i32>],
    index: usize,
    roots: &[Point],
) -> Option<Vec3> {
    let link = path_link(&paths[index]);
    let given = roots
        .iter()
        .filter(|p| link.has_parent() && p.parent_id == link.parent_id)
        .min_by(|a, b| {
            let off = |p: &Point| (p.alpha - link.parent_alpha).abs();
            off(a).total_cmp(&off(b))
        });
    given
        .map(|p| p.position)
        .or_else(|| structure.root(index, paths))
}

/// A path prepared to be bent by place instead of by its own alpha, for `apply_force` and
/// `gravity`. Every segment is weighted by how far its middle is from the root of the path (the
/// spot on its parent it grows from), so paths that pass through the same place get the same
/// weight there, whatever their length or start.
///
/// A path can start away from its root, like the rays a cap adds further out. A straight lead-in
/// from the root to the base comes before the path and is bent with it, so the base moves as far
/// as a path that runs from the root straight through the base moves there. Bending only ever
/// turns a segment away from its own rest direction, turns don't carry over from one segment to
/// the next. A path that is the outer part of a longer straight path is therefore bent exactly
/// like that part of the longer one.
#[derive(Debug, Clone, PartialEq)]
pub struct PlaceChain {
    /// The lead-in, then the path, at their original place.
    pub points: Vec<Vec3>,
    /// How many points of the lead-in come before the base of the path.
    pub lead: usize,
    /// For every segment of `points`, how far its middle lies into the reach (`place_alpha`).
    pub alphas: Vec<f32>,
}

impl PlaceChain {
    pub fn new(path: &[Vec3], root: Vec3, reach: f32) -> Self {
        let mut points = Vec::with_capacity(path.len());
        if let Some(&base) = path.first() {
            let distance = base.distance(root);
            let length: f32 = path.windows(2).map(|w| w[0].distance(w[1])).sum();
            // the lead-in gets segments about as long as those of the path
            let step = length / path.len().saturating_sub(1).max(1) as f32;
            // a base that sits on its root, up to rounding, needs no lead-in
            if distance > 1e-3 * step && distance > 0.0 {
                let count = if step > 0.0 {
                    ((distance / step).round() as usize).clamp(1, MAX_LEAD)
                } else {
                    1
                };
                points.extend((0..count).map(|k| root + (base - root) * (k as f32 / count as f32)));
            }
        }
        let lead = points.len();
        points.extend_from_slice(path);
        let alphas = points
            .windows(2)
            .map(|w| place_alpha(((w[0] + w[1]) * 0.5).distance(root), reach))
            .collect();
        Self {
            points,
            lead,
            alphas,
        }
    }

    /// Builds the chain from `start` outward, every segment keeps its length and points where
    /// `direction(segment, rest, middle)` says: `rest` is its original direction, `middle` where
    /// its middle would be if it didn't turn. Returns the points of the path, without the lead-in.
    pub fn bend(
        &self,
        start: Vec3,
        mut direction: impl FnMut(usize, Vec3, Vec3) -> Vec3,
    ) -> Vec<Vec3> {
        let mut current = Vec::with_capacity(self.points.len());
        current.push(start);
        for (k, w) in self.points.windows(2).enumerate() {
            let segment = w[1] - w[0];
            let length = segment.length();
            let rest = segment.try_normalize().unwrap_or(Vec3::Y);
            let middle = current[k] + rest * length * 0.5;
            let turned = direction(k, rest, middle);
            current.push(current[k] + turned * length);
        }
        current.split_off(self.lead)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::geometry::{create_path, set_path_link, wrap_path_mut, PathLink};

    fn path(id: i32, parent: Option<(i32, f32)>, points: &[Vec3]) -> Vec<i32> {
        let mut data = create_path(points.len(), if parent.is_some() { 2 } else { 1 });
        let (parent_id, parent_alpha) = parent.unwrap_or((-1, 0.0));
        set_path_link(
            &mut data,
            PathLink {
                id,
                parent_id,
                parent_alpha,
                ..PathLink::root(0, 0)
            },
        );
        let path = wrap_path_mut(&mut data);
        for (i, p) in points.iter().enumerate() {
            path.points[i * 4..i * 4 + 4].copy_from_slice(&[p.x, p.y, p.z, 0.1]);
        }
        data
    }

    fn point(parent_id: i32, alpha: f32, position: Vec3) -> Point {
        Point {
            position,
            direction: Vec3::X,
            tangent: Vec3::Y,
            scale: 0.1,
            parent_id,
            alpha,
            depth: 1,
            index: 0,
        }
    }

    #[test]
    fn the_root_is_a_given_point_on_the_parent_then_the_parent_then_the_base() {
        let stem = path(0, None, &[Vec3::ZERO, Vec3::Y, Vec3::Y * 2.0]);
        let on_stem = path(
            1,
            Some((0, 1.0)),
            &[Vec3::new(1.0, 2.0, 0.0), Vec3::new(2.0, 2.0, 0.0)],
        );
        // a ray whose stipe (id 7) is not in the list
        let ray = path(
            2,
            Some((7, 1.0)),
            &[Vec3::new(1.0, 5.0, 0.0), Vec3::new(2.0, 5.0, 0.0)],
        );
        let paths = vec![stem, on_stem, ray];
        let structure = PathStructure::new(&paths);

        let no_points = place_root(&structure, &paths, 2, &[]);
        assert_eq!(no_points, Some(Vec3::new(1.0, 5.0, 0.0)));
        assert!(
            place_root(&structure, &paths, 1, &[])
                .unwrap()
                .distance(Vec3::Y * 2.0)
                < 1e-5
        );

        let hinges = [
            point(7, 0.2, Vec3::new(0.0, 1.0, 0.0)),
            point(7, 1.0, Vec3::new(0.0, 5.0, 0.0)),
            point(3, 1.0, Vec3::new(9.0, 9.0, 9.0)),
        ];
        assert_eq!(
            place_root(&structure, &paths, 2, &hinges),
            Some(Vec3::new(0.0, 5.0, 0.0))
        );
        // a stem without a parent ignores the points
        assert_eq!(place_root(&structure, &paths, 0, &hinges), Some(Vec3::ZERO));
    }

    #[test]
    fn a_path_from_its_root_gets_no_lead_in() {
        let path = [Vec3::ZERO, Vec3::Y, Vec3::Y * 2.0];
        let chain = PlaceChain::new(&path, Vec3::ZERO, 4.0);
        assert_eq!(chain.lead, 0);
        assert_eq!(chain.points, path.to_vec());
        assert_eq!(chain.alphas, vec![0.125, 0.375]);
    }

    #[test]
    fn the_lead_in_runs_straight_from_the_root_in_steps_like_the_path() {
        let path = [Vec3::X * 4.0, Vec3::X * 5.0, Vec3::X * 6.0];
        let chain = PlaceChain::new(&path, Vec3::ZERO, 0.0);
        assert_eq!(chain.lead, 4);
        assert_eq!(
            chain.points[..4],
            [Vec3::ZERO, Vec3::X, Vec3::X * 2.0, Vec3::X * 3.0]
        );
        // without a reach every segment gets the full weight
        assert!(chain.alphas.iter().all(|a| *a == 1.0));
    }

    #[test]
    fn the_weight_stops_growing_at_the_reach() {
        let path: Vec<Vec3> = (0..5).map(|i| Vec3::Y * i as f32).collect();
        let chain = PlaceChain::new(&path, Vec3::ZERO, 2.0);
        assert_eq!(chain.alphas, vec![0.25, 0.75, 1.0, 1.0]);
    }

    #[test]
    fn bend_keeps_the_lengths_and_drops_the_lead_in() {
        let path = [Vec3::X * 2.0, Vec3::X * 3.0, Vec3::X * 5.0];
        let chain = PlaceChain::new(&path, Vec3::ZERO, 0.0);
        // every segment points up
        let bent = chain.bend(Vec3::ZERO, |_, _, _| Vec3::Y);
        assert_eq!(bent, vec![Vec3::Y * 2.0, Vec3::Y * 3.0, Vec3::Y * 5.0]);
    }

    #[test]
    fn unchanged_directions_give_the_path_back() {
        let path = [
            Vec3::new(1.0, 2.0, 0.0),
            Vec3::new(1.5, 3.0, 0.2),
            Vec3::new(2.0, 3.5, 1.0),
        ];
        let chain = PlaceChain::new(&path, Vec3::ZERO, 1.0);
        let bent = chain.bend(Vec3::ZERO, |_, rest, _| rest);
        for (a, b) in bent.iter().zip(&path) {
            assert!(a.distance(*b) < 1e-5, "{:?} {:?}", a, b);
        }
    }
}
