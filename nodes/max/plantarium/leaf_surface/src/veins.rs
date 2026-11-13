use glam::{Vec2, Vec3};
use nodarium_utils::geometry::{path_link, wrap_path};

/// Where along the segment from `a` to `b` (0 to 1) the point closest to `p` lies, and how far
/// it is from `p`.
pub fn closest(a: Vec2, b: Vec2, p: Vec2) -> (f32, f32) {
    let ab = b - a;
    let len2 = ab.length_squared();
    let t = if len2 > 0.0 {
        ((p - a).dot(ab) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (t, (a + ab * t - p).length())
}

/// How wide the groove `relief` sinks a vein into is, on each side of it.
pub fn groove(reach: f32, cell: f32) -> f32 {
    (0.15 * reach).max(1.5 * cell)
}

/// A piece of a vein in leaf space, between two points of its path.
pub struct Segment {
    pub a: Vec2,
    pub b: Vec2,
    /// The height above the leaf plane at `a` and `b`.
    pub ha: f32,
    pub hb: f32,
    /// How far the blade reaches around the vein at `a` and `b`, the thickness of the path.
    pub ra: f32,
    pub rb: f32,
}

impl Segment {
    pub fn closest(&self, p: Vec2) -> (f32, f32) {
        closest(self.a, self.b, p)
    }

    pub fn reach(&self, t: f32) -> f32 {
        self.ra + (self.rb - self.ra) * t
    }

    pub fn height(&self, t: f32) -> f32 {
        self.ha + (self.hb - self.ha) * t
    }
}

pub struct Vein {
    pub segments: Vec<Segment>,
    pub min: Vec2,
    pub max: Vec2,
    pub reach: f32,
    /// The vein it branches from, an index into the veins.
    pub parent: Option<usize>,
}

impl Vein {
    pub fn end(&self) -> Vec2 {
        self.segments[self.segments.len() - 1].b
    }

    pub fn length(&self) -> f32 {
        self.segments.iter().map(|s| (s.b - s.a).length()).sum()
    }

    /// The point `s` along the vein by length, its direction and its reach.
    pub fn at(&self, s: f32) -> (Vec2, Vec2, f32) {
        let mut rest = s.max(0.0);
        for (i, seg) in self.segments.iter().enumerate() {
            let len = (seg.b - seg.a).length();
            if rest <= len || i == self.segments.len() - 1 {
                let t = if len > 0.0 { (rest / len).min(1.0) } else { 0.0 };
                let direction = (seg.b - seg.a).try_normalize().unwrap_or(Vec2::Y);
                return (seg.a.lerp(seg.b, t), direction, seg.reach(t));
            }
            rest -= len;
        }
        unreachable!("a vein has segments")
    }

    /// How far along the vein, by length, the point closest to `p` lies, and how far it is.
    pub fn project(&self, p: Vec2) -> (f32, f32) {
        let (mut walked, mut best) = (0.0, (0.0, f32::MAX));
        for seg in &self.segments {
            let len = (seg.b - seg.a).length();
            let (t, d) = seg.closest(p);
            if d < best.1 {
                best = (walked + t * len, d);
            }
            walked += len;
        }
        best
    }

    /// No point of the blade around this vein is closer to `p` than this.
    pub fn lower_bound(&self, p: Vec2) -> f32 {
        let d = (self.min - p).max(p - self.max).max(Vec2::ZERO);
        d.length() - self.reach
    }
}

/// The veins in leaf space: x across the leaf, y along it, the base at the origin. The frame comes
/// from the main veins, the paths whose parent is not among the veins: the base is where they
/// start, on average, and the leaf points the way they go, seen from above. A single midrib or a
/// palmate fan of them both work.
pub fn leaf_space(paths: &[&[i32]]) -> Vec<Vein> {
    let ids: Vec<i32> = paths.iter().map(|p| path_link(p).id).collect();
    let point = |p: &[f32], i: usize| Vec3::new(p[i * 4], p[i * 4 + 1], p[i * 4 + 2]);

    let (mut base, mut direction, mut roots) = (Vec3::ZERO, Vec3::ZERO, 0);
    for root in paths.iter().filter(|p| !ids.contains(&path_link(p).parent_id)) {
        let path = wrap_path(root);
        if path.length == 0 {
            continue;
        }
        base += point(path.points, 0);
        direction += point(path.points, path.length - 1) - point(path.points, 0);
        roots += 1;
    }
    base /= roots.max(1) as f32;
    let along = Vec3::new(direction.x, 0.0, direction.z)
        .try_normalize()
        .unwrap_or(Vec3::NEG_Z);
    let across = along.cross(Vec3::Y);

    let mut kept = Vec::new();
    let mut veins: Vec<Vein> = paths
        .iter()
        .filter_map(|p| {
            let path = wrap_path(p);
            let points: Vec<(Vec2, f32, f32)> = (0..path.length)
                .map(|i| {
                    let d = point(path.points, i) - base;
                    let reach = path.points[i * 4 + 3].max(0.0);
                    (Vec2::new(d.dot(across), d.dot(along)), d.y, reach)
                })
                .collect();
            if points.len() < 2 {
                return None;
            }
            kept.push(path_link(p));
            let segments = points
                .windows(2)
                .map(|w| Segment {
                    a: w[0].0,
                    b: w[1].0,
                    ha: w[0].1,
                    hb: w[1].1,
                    ra: w[0].2,
                    rb: w[1].2,
                })
                .collect();
            Some(Vein {
                segments,
                min: points.iter().fold(Vec2::MAX, |m, p| m.min(p.0)),
                max: points.iter().fold(Vec2::MIN, |m, p| m.max(p.0)),
                reach: points.iter().fold(0.0_f32, |m, p| m.max(p.2)),
                parent: None,
            })
        })
        .collect();
    for (vein, link) in veins.iter_mut().zip(&kept) {
        vein.parent = kept.iter().position(|k| k.id == link.parent_id);
    }
    veins
}
