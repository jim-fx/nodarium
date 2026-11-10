use glam::Vec3;
use nodarium_utils::{decode_float, encode_float, split_args, Point};

pub const HINGE_TAG: i32 = 16;

/// Distances from the axis of the open cap that a hinge keeps, from the top (alpha 0) to the
/// margin (alpha 1)
pub const REST_SAMPLES: usize = 65;

/// Words before the open cap: tag, stipe, creator, depth, position, direction, tangent, scale,
/// length and radius
const HEAD: usize = 16;

/// Where a cap sits on its stipe and what it looked like open, see [`crate::grow_cap`]. The
/// `cap` node puts one into its list for every stipe, the nodes that pose a cap find their rays
/// by it.
#[derive(Clone, Debug, PartialEq)]
pub struct CapHinge {
    /// The stipe, the rays are its children
    pub stipe: i32,
    /// The node that made the rays
    pub creator: i32,
    /// Depth of the stipe
    pub depth: i32,
    /// The stipe top
    pub position: Vec3,
    /// The side the angles around the axis count from
    pub direction: Vec3,
    /// The axis, the last segment of the stipe
    pub tangent: Vec3,
    /// Radius of the stipe top
    pub scale: f32,
    /// Distance along a ray of the open cap from where it crosses the axis to its tip
    pub length: f32,
    /// How far the open cap reaches from the axis
    pub radius: f32,
    /// Distance of a ray from the axis on the open cap as a share of `radius`, at
    /// [`REST_SAMPLES`] places from alpha 0 to 1
    pub rest: Vec<f32>,
}

impl CapHinge {
    /// The hinge as one framed item, ready for `concat_arg_vecs`
    pub fn encode(&self) -> Vec<i32> {
        let mut words = vec![0, (HEAD + self.rest.len()) as i32 + 1, HINGE_TAG, self.stipe, self.creator, self.depth];
        let floats = [
            self.position.to_array(),
            self.direction.to_array(),
            self.tangent.to_array(),
        ];
        words.extend(floats.iter().flatten().map(|&f| encode_float(f)));
        words.extend([self.scale, self.length, self.radius].map(encode_float));
        words.extend(self.rest.iter().map(|&f| encode_float(f)));
        words.extend([1, 1]);
        words
    }

    /// Reads one framed item, `None` for any other item
    pub fn decode(item: &[i32]) -> Option<CapHinge> {
        let n = item.len();
        // one group of plain values: its count points at its own closing bracket
        if n < HEAD + 4 || item[0] != 0 || item[1] as usize != n - 3 || item[2] != HINGE_TAG {
            return None;
        }
        let float = |i: usize| decode_float(item[i]);
        let vec3 = |i: usize| Vec3::new(float(i), float(i + 1), float(i + 2));
        Some(CapHinge {
            stipe: item[3],
            creator: item[4],
            depth: item[5],
            position: vec3(6),
            direction: vec3(9),
            tangent: vec3(12),
            scale: float(15),
            length: float(16),
            radius: float(17),
            rest: item[18..n - 2].iter().map(|&w| decode_float(w)).collect(),
        })
    }

    /// The hinge as the `hinge` output of the `cap` node gives it
    pub fn point(&self) -> Point {
        Point {
            position: self.position,
            direction: self.direction,
            tangent: self.tangent,
            scale: self.scale,
            parent_id: self.stipe,
            alpha: 1.0,
            depth: self.depth,
            index: 0,
        }
    }

    /// How far along the cap a place `to_end` from the tip of a ray is, 0 at the axis to 1 at
    /// the margin
    pub fn alpha(&self, to_end: f32) -> f32 {
        if self.length > 0.0 {
            (1.0 - to_end / self.length).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    /// How far from the axis the place `alpha` along a ray was on the open cap
    pub fn rest_distance(&self, alpha: f32) -> f32 {
        let n = self.rest.len();
        if n < 2 {
            return alpha * self.radius;
        }
        let x = alpha.clamp(0.0, 1.0) * (n - 1) as f32;
        let i = (x as usize).min(n - 2);
        self.radius * (self.rest[i] + (self.rest[i + 1] - self.rest[i]) * (x - i as f32))
    }
}

/// A list inside a list starts with a group, its count then points at index 2
fn is_list(item: &[i32]) -> bool {
    item.len() > 4 && item[0] == 0 && item[1] == 1
}

fn collect_hinges<'a>(items: impl IntoIterator<Item = &'a [i32]>, out: &mut Vec<CapHinge>) {
    for item in items {
        if let Some(hinge) = CapHinge::decode(item) {
            // a list joined twice brings its hinges twice
            out.retain(|h| h.stipe != hinge.stipe || h.creator != hinge.creator);
            out.push(hinge);
        } else if is_list(item) {
            collect_hinges(split_args(item), out);
        }
    }
}

/// Every hinge of a list, also those in lists inside it, one for every cap
pub fn read_hinges(list: &[i32]) -> Vec<CapHinge> {
    let mut out = Vec::new();
    if list.len() >= 4 && list[0] == 0 {
        collect_hinges(split_args(list), &mut out);
    }
    out
}

/// The hinges among the items of a list
pub fn hinges_of(items: &[&[i32]]) -> Vec<CapHinge> {
    let mut out = Vec::new();
    collect_hinges(items.iter().copied(), &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodarium_utils::{concat_arg_vecs, wrap_arg};

    fn hinge(stipe: i32) -> CapHinge {
        CapHinge {
            stipe,
            creator: 7,
            depth: 1,
            position: Vec3::new(0.0, 0.05, 0.0),
            direction: Vec3::X,
            tangent: Vec3::Y,
            scale: 0.005,
            length: 0.03,
            radius: 0.025,
            rest: (0..REST_SAMPLES).map(|k| k as f32 / (REST_SAMPLES - 1) as f32).collect(),
        }
    }

    #[test]
    fn items_round_trip() {
        let h = hinge(-12);
        assert_eq!(CapHinge::decode(&h.encode()), Some(h.clone()));
        let none = CapHinge { rest: Vec::new(), ..h };
        assert_eq!(CapHinge::decode(&none.encode()), Some(none));
        assert_eq!(CapHinge::decode(&wrap_arg(&[13, 1, 0, 5])), None);
        assert_eq!(CapHinge::decode(&hinge(3).point().encode()), None);
    }

    #[test]
    fn hinges_are_found_in_lists_inside_lists_once_per_cap() {
        let inner = concat_arg_vecs(vec![hinge(1).encode(), hinge(2).encode()]);
        let outer = concat_arg_vecs(vec![inner.clone(), hinge(3).encode(), inner]);
        let found = read_hinges(&outer);
        assert_eq!(found.iter().map(|h| h.stipe).collect::<Vec<_>>(), vec![3, 1, 2]);
        assert!(read_hinges(&[0, 1, 1, 1]).is_empty());
    }

    #[test]
    fn rest_distance_follows_the_open_cap() {
        let h = hinge(1);
        assert!((h.rest_distance(0.5) - 0.0125).abs() < 1e-7);
        assert_eq!(h.rest_distance(2.0), h.radius);
        assert_eq!(h.rest_distance(-1.0), 0.0);
    }
}
