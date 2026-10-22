use super::{interpolate_along_path, path_link, wrap_path, wrap_path_mut};
use glam::Vec3;
use std::collections::HashMap;

/// Where every path of a list sits in the structure of stems and branches, so a node can move a
/// parent and let the paths that grow on it follow.
///
/// A path whose parent can't be found (it has none, or a node removed it) is treated as a path
/// without a parent and doesn't follow anything.
pub struct PathStructure {
    /// The paths in the order they have to be visited, parents come before their children.
    order: Vec<usize>,
    /// For every path the index of its parent and where it is attached on it.
    parents: Vec<Option<(usize, f32)>>,
}

fn is_path(item: &[i32]) -> bool {
    item.len() > 7 && item[2] == 0
}

fn xyz(points: &[f32], alpha: f32) -> Option<Vec3> {
    if points.len() < 8 {
        return None;
    }
    Some(interpolate_along_path(points, alpha).0.truncate())
}

impl PathStructure {
    pub fn new(paths: &[Vec<i32>]) -> Self {
        let mut by_id: HashMap<i32, usize> = HashMap::new();
        for (index, path) in paths.iter().enumerate() {
            if is_path(path) {
                by_id.insert(path_link(path).id, index);
            }
        }

        let parents = paths
            .iter()
            .enumerate()
            .map(|(index, path)| {
                if !is_path(path) {
                    return None;
                }
                let link = path_link(path);
                if !link.has_parent() {
                    return None;
                }
                by_id
                    .get(&link.parent_id)
                    .filter(|parent| **parent != index)
                    .map(|parent| (*parent, link.parent_alpha))
            })
            .collect();

        // a parent is always a layer above its children
        let mut order: Vec<usize> = (0..paths.len()).collect();
        order.sort_by_key(|index| {
            if is_path(&paths[*index]) {
                paths[*index][3]
            } else {
                i32::MAX
            }
        });

        Self { order, parents }
    }

    pub fn order(&self) -> &[usize] {
        &self.order
    }

    pub fn parent(&self, index: usize) -> Option<usize> {
        self.parents[index].map(|(parent, _)| parent)
    }

    /// How far the spot a path grows from has moved, between the `original` list and the
    /// `current` one. The spot is found by its alpha along the length of the parent, so this is
    /// exact as long as the parent keeps its length.
    pub fn shift(&self, index: usize, original: &[Vec<i32>], current: &[Vec<i32>]) -> Vec3 {
        let Some((parent, alpha)) = self.parents[index] else {
            return Vec3::ZERO;
        };
        let before = xyz(wrap_path(&original[parent]).points, alpha);
        let after = xyz(wrap_path(&current[parent]).points, alpha);
        match (before, after) {
            (Some(before), Some(after)) => after - before,
            _ => Vec3::ZERO,
        }
    }
}

/// Moves a whole path by `offset`.
pub fn translate_path(path: &mut [i32], offset: Vec3) {
    if offset == Vec3::ZERO || !is_path(path) {
        return;
    }
    let points = wrap_path_mut(path).points;
    for point in points.chunks_exact_mut(4) {
        point[0] += offset.x;
        point[1] += offset.y;
        point[2] += offset.z;
    }
}

/// Moves every path by what its parent moved from `original` to `current`, so the paths that
/// grow on a modified path keep touching it. Use it after a node changed some paths and left
/// the paths growing on them behind.
pub fn follow_parents(original: &[Vec<i32>], current: &mut [Vec<i32>]) {
    let structure = PathStructure::new(original);
    for index in structure.order().to_vec() {
        let shift = structure.shift(index, original, current);
        translate_path(&mut current[index], shift);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{create_path, set_path_link, PathLink};

    fn path(id: i32, depth: i32, parent: Option<(i32, f32)>, points: &[[f32; 3]]) -> Vec<i32> {
        let mut data = create_path(points.len(), depth);
        let (parent_id, parent_alpha) = parent.unwrap_or((-1, 0.0));
        set_path_link(
            &mut data,
            PathLink {
                id,
                parent_id,
                parent_alpha,
            },
        );
        let path = wrap_path_mut(&mut data);
        for (i, p) in points.iter().enumerate() {
            path.points[i * 4..i * 4 + 4].copy_from_slice(&[p[0], p[1], p[2], 0.1]);
        }
        data
    }

    fn base(path_data: &[i32]) -> Vec3 {
        Vec3::from_slice(&wrap_path(path_data).points[..3])
    }

    fn stem(x: f32) -> Vec<i32> {
        path(0, 1, None, &[[x, 0.0, 0.0], [x, 1.0, 0.0], [x, 2.0, 0.0]])
    }

    #[test]
    fn a_child_follows_the_spot_it_is_attached_to() {
        // the branch starts half way up the stem
        let branch = path(1, 2, Some((0, 0.5)), &[[0.0, 1.0, 0.0], [1.0, 1.0, 0.0]]);
        let original = vec![stem(0.0), branch];

        let mut current = original.clone();
        translate_path(&mut current[0], Vec3::new(0.0, 0.0, 3.0));
        follow_parents(&original, &mut current);

        assert_eq!(base(&current[1]), Vec3::new(0.0, 1.0, 3.0));
        // the stem itself is not moved a second time
        assert_eq!(base(&current[0]), Vec3::new(0.0, 0.0, 3.0));
    }

    #[test]
    fn a_grandchild_follows_through_two_levels() {
        let branch = path(1, 2, Some((0, 0.5)), &[[0.0, 1.0, 0.0], [2.0, 1.0, 0.0]]);
        let twig = path(2, 3, Some((1, 0.5)), &[[1.0, 1.0, 0.0], [1.0, 2.0, 0.0]]);
        let original = vec![stem(0.0), branch, twig];

        let mut current = original.clone();
        translate_path(&mut current[0], Vec3::new(0.0, 0.0, 3.0));
        follow_parents(&original, &mut current);

        assert_eq!(base(&current[2]), Vec3::new(1.0, 1.0, 3.0));
    }

    #[test]
    fn the_order_does_not_depend_on_the_order_of_the_list() {
        let branch = path(1, 2, Some((0, 0.5)), &[[0.0, 1.0, 0.0], [1.0, 1.0, 0.0]]);
        let original = vec![branch, stem(0.0)];

        let mut current = original.clone();
        translate_path(&mut current[1], Vec3::Z);
        follow_parents(&original, &mut current);

        assert_eq!(base(&current[0]), Vec3::new(0.0, 1.0, 1.0));
    }

    #[test]
    fn a_missing_parent_is_ignored() {
        let branch = path(1, 2, Some((42, 0.5)), &[[0.0, 1.0, 0.0], [1.0, 1.0, 0.0]]);
        let original = vec![stem(0.0), branch];

        let mut current = original.clone();
        translate_path(&mut current[0], Vec3::Z);
        follow_parents(&original, &mut current);

        assert_eq!(base(&current[1]), Vec3::new(0.0, 1.0, 0.0));
    }

    #[test]
    fn paths_without_a_parent_stay_where_they_are() {
        let original = vec![stem(0.0), stem(5.0)];
        let mut current = original.clone();
        translate_path(&mut current[0], Vec3::Z);
        follow_parents(&original, &mut current);

        assert_eq!(base(&current[1]), Vec3::new(5.0, 0.0, 0.0));
    }

    #[test]
    fn a_bent_parent_carries_the_child_to_the_new_spot() {
        let branch = path(1, 2, Some((0, 1.0)), &[[0.0, 2.0, 0.0], [1.0, 2.0, 0.0]]);
        let original = vec![stem(0.0), branch];

        // the stem is bent over, same length
        let mut current = original.clone();
        {
            let stem = wrap_path_mut(&mut current[0]);
            stem.points[4..8].copy_from_slice(&[1.0, 0.0, 0.0, 0.1]);
            stem.points[8..12].copy_from_slice(&[2.0, 0.0, 0.0, 0.1]);
        }
        follow_parents(&original, &mut current);

        assert!((base(&current[1]) - Vec3::new(2.0, 0.0, 0.0)).length() < 1e-4);
    }
}
