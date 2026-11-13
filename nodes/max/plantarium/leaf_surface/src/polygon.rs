use glam::Vec2;

use crate::veins::closest;

/// Twice the signed area of a closed line, positive when it runs counter-clockwise.
pub fn area2(points: &[Vec2]) -> f32 {
    let n = points.len();
    (0..n).map(|i| points[i].perp_dot(points[(i + 1) % n])).sum()
}

pub fn nearest(points: &[Vec2], p: Vec2) -> usize {
    (0..points.len())
        .min_by(|&a, &b| points[a].distance_squared(p).total_cmp(&points[b].distance_squared(p)))
        .unwrap_or(0)
}

/// Whether the segments `a`-`b` and `c`-`d` cross in their insides.
pub fn cross(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> bool {
    let side = |p: Vec2, q: Vec2, r: Vec2| (q - p).perp_dot(r - p);
    let (d1, d2) = (side(c, d, a), side(c, d, b));
    let (d3, d4) = (side(a, b, c), side(a, b, d));
    d1 * d2 < 0.0 && d3 * d4 < 0.0
}

/// A closed line with its edges sorted into horizontal bands, so that a point only meets the
/// edges near it.
pub struct Polygon<'a> {
    pub points: &'a [Vec2],
    bands: Vec<Vec<u32>>,
    bottom: f32,
    height: f32,
}

impl<'a> Polygon<'a> {
    pub fn new(points: &'a [Vec2]) -> Polygon<'a> {
        let n = points.len();
        let bottom = points.iter().fold(f32::MAX, |m, p| m.min(p.y));
        let top = points.iter().fold(f32::MIN, |m, p| m.max(p.y));
        let count = (n / 4).clamp(1, 256);
        let height = ((top - bottom) / count as f32).max(f32::MIN_POSITIVE);
        let mut bands = vec![Vec::new(); count];
        for i in 0..n {
            let (a, b) = (points[i], points[(i + 1) % n]);
            let (lo, hi) = (a.y.min(b.y), a.y.max(b.y));
            for band in &mut bands[Self::band_of(lo, bottom, height, count)..=Self::band_of(hi, bottom, height, count)] {
                band.push(i as u32);
            }
        }
        Polygon {
            points,
            bands,
            bottom,
            height,
        }
    }

    fn band_of(y: f32, bottom: f32, height: f32, count: usize) -> usize {
        (((y - bottom) / height).max(0.0) as usize).min(count - 1)
    }

    fn edge(&self, i: u32) -> (Vec2, Vec2) {
        let i = i as usize;
        (self.points[i], self.points[(i + 1) % self.points.len()])
    }

    pub fn contains(&self, p: Vec2) -> bool {
        if self.points.len() < 3 || p.y < self.bottom || p.y > self.bottom + self.height * self.bands.len() as f32 {
            return false;
        }
        let band = &self.bands[Self::band_of(p.y, self.bottom, self.height, self.bands.len())];
        let mut inside = false;
        for &i in band {
            let (a, b) = self.edge(i);
            if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x) {
                inside = !inside;
            }
        }
        inside
    }

    pub fn distance(&self, p: Vec2) -> f32 {
        self.distance_within(p, f32::MAX)
    }

    /// The distance from `p` to the line, or `limit` when the line is farther away.
    pub fn distance_within(&self, p: Vec2, limit: f32) -> f32 {
        let count = self.bands.len();
        let at = ((p.y - self.bottom) / self.height).floor() as isize;
        let mut best = limit;
        for k in 0..=count as isize + at.unsigned_abs() as isize {
            let gap = (k - 1).max(0) as f32 * self.height;
            if gap > best {
                break;
            }
            for band in [at - k, at + k] {
                if band < 0 || band >= count as isize || (k == 0 && band != at - k) {
                    continue;
                }
                for &i in &self.bands[band as usize] {
                    let (a, b) = self.edge(i);
                    best = best.min(closest(a, b, p).1);
                }
            }
        }
        best
    }
}
