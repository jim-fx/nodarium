use std::cell::OnceCell;

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

/// Sorts items into `count` buckets, where `each` calls its argument with every bucket and item
/// that goes into it, items in order. Returns where every bucket starts in the items, and one more
/// for the end, and the items.
pub fn buckets(count: usize, each: impl Fn(&mut dyn FnMut(usize, u32))) -> (Vec<u32>, Vec<u32>) {
    let mut starts = vec![0u32; count + 1];
    each(&mut |c, _| starts[c + 1] += 1);
    for c in 1..starts.len() {
        starts[c] += starts[c - 1];
    }
    let mut fill = starts.clone();
    let mut items = vec![0u32; starts[count] as usize];
    each(&mut |c, k| {
        items[fill[c] as usize] = k;
        fill[c] += 1;
    });
    (starts, items)
}

/// The pieces of a closed line sorted into a grid of square cells, so that a point only meets the
/// pieces near it.
pub struct Pieces<'a> {
    points: &'a [Vec2],
    origin: Vec2,
    cell: f32,
    nx: i32,
    ny: i32,
    starts: Vec<u32>,
    pieces: Vec<u32>,
    /// How many rings of cells around every cell hold no piece.
    empty: Vec<u16>,
}

impl<'a> Pieces<'a> {
    pub fn new(points: &'a [Vec2], cell: f32) -> Pieces<'a> {
        let n = points.len();
        let min = points.iter().fold(Vec2::MAX, |m, &p| m.min(p));
        let max = points.iter().fold(Vec2::MIN, |m, &p| m.max(p));
        let cell = cell.max((max - min).max_element() / 256.0).max(f32::MIN_POSITIVE);
        let (nx, ny) = if n > 0 {
            let size = ((max - min) / cell).floor() + 1.0;
            (size.x as i32, size.y as i32)
        } else {
            (0, 0)
        };
        let key = |p: Vec2| {
            let k = ((p - min) / cell).floor();
            ((k.x as i32).clamp(0, nx - 1), (k.y as i32).clamp(0, ny - 1))
        };
        let each = |visit: &mut dyn FnMut(usize, u32)| {
            for k in 0..n {
                let (a, b) = (points[k], points[(k + 1) % n]);
                let ((x0, y0), (x1, y1)) = (key(a.min(b)), key(a.max(b)));
                for y in y0..=y1 {
                    for x in x0..=x1 {
                        visit((y * nx + x) as usize, k as u32);
                    }
                }
            }
        };
        let cells = (nx * ny) as usize;
        let (starts, pieces) = buckets(cells, each);

        let mut empty = vec![u16::MAX; cells];
        let mut queue: Vec<usize> = (0..cells).filter(|&c| starts[c] < starts[c + 1]).collect();
        for &c in &queue {
            empty[c] = 0;
        }
        let mut at = 0;
        while at < queue.len() {
            let c = queue[at];
            at += 1;
            let (x, y) = ((c as i32) % nx, (c as i32) / nx);
            for (dx, dy) in [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)] {
                let (x, y) = (x + dx, y + dy);
                if x < 0 || y < 0 || x >= nx || y >= ny {
                    continue;
                }
                let next = (y * nx + x) as usize;
                if empty[next] == u16::MAX {
                    empty[next] = empty[c] + 1;
                    queue.push(next);
                }
            }
        }
        Pieces {
            points,
            origin: min,
            cell,
            nx,
            ny,
            starts,
            pieces,
            empty,
        }
    }

    /// The piece closest to `p` of those closer than `limit`, the first of them where several are
    /// as close: its index, where along it (0 to 1) the closest point lies and how far it is.
    pub fn closest(&self, p: Vec2, limit: f32) -> Option<(usize, f32, f32)> {
        let n = self.points.len();
        if n == 0 {
            return None;
        }
        // covers the rounding of where a cell begins
        let slack = 1e-4 * self.cell;
        let k = ((p - self.origin) / self.cell).floor().clamp(Vec2::splat(-1e6), Vec2::splat(1e6));
        let (cx, cy) = (k.x as i32, k.y as i32);
        let mut first = 0;
        if cx >= 0 && cy >= 0 && cx < self.nx && cy < self.ny {
            first = self.empty[(cy * self.nx + cx) as usize] as i32;
            if (first - 1) as f32 * self.cell - slack >= limit {
                return None;
            }
        }
        let rings = (cx.max(self.nx - 1 - cx)).max(cy.max(self.ny - 1 - cy)).max(0);
        let mut best = (limit, u32::MAX, 0.0);
        for r in first..=rings {
            // a piece in none of the rings so far is at least this far from `p`
            let lo = self.origin + Vec2::new((cx - r + 1) as f32, (cy - r + 1) as f32) * self.cell;
            let hi = self.origin + Vec2::new((cx + r) as f32, (cy + r) as f32) * self.cell;
            if r > 0 && best.0 < (p - lo).min(hi - p).min_element() - slack {
                break;
            }
            for y in (cy - r).max(0)..=(cy + r).min(self.ny - 1) {
                let whole = y == cy - r || y == cy + r;
                let mut x = (cx - r).max(0);
                while x <= (cx + r).min(self.nx - 1) {
                    let c = (y * self.nx + x) as usize;
                    let (start, end) = (self.starts[c] as usize, self.starts[c + 1] as usize);
                    let corner = self.origin + Vec2::new(x as f32, y as f32) * self.cell;
                    let away = (corner - p).max(p - corner - self.cell).max(Vec2::ZERO).length();
                    let pieces = if start < end && away <= best.0 + slack { start..end } else { 0..0 };
                    for &k in &self.pieces[pieces] {
                        let i = k as usize;
                        let (t, d) = closest(self.points[i], self.points[(i + 1) % n], p);
                        if d < best.0 || (d == best.0 && k < best.1) {
                            best = (d, k, t);
                        }
                    }
                    x = if whole || x >= cx + r { x + 1 } else { cx + r };
                }
            }
        }
        (best.1 != u32::MAX).then_some((best.1 as usize, best.2, best.0))
    }
}

/// The edges of a closed line sorted into horizontal bands, so that a point only meets the edges
/// level with it.
pub struct Bands {
    starts: Vec<u32>,
    edges: Vec<u32>,
    bottom: f32,
    height: f32,
}

impl Bands {
    pub fn new(points: &[Vec2]) -> Bands {
        let n = points.len();
        let bottom = points.iter().fold(f32::MAX, |m, p| m.min(p.y));
        let top = points.iter().fold(f32::MIN, |m, p| m.max(p.y));
        let count = (n / 4).clamp(1, 256);
        let height = ((top - bottom) / count as f32).max(f32::MIN_POSITIVE);
        let range = |i: usize| {
            let (a, b) = (points[i], points[(i + 1) % n]);
            let (lo, hi) = (a.y.min(b.y), a.y.max(b.y));
            Self::band_of(lo, bottom, height, count)..=Self::band_of(hi, bottom, height, count)
        };
        let (starts, edges) = buckets(count, |visit| {
            for i in 0..n {
                for band in range(i) {
                    visit(band, i as u32);
                }
            }
        });
        Bands {
            starts,
            edges,
            bottom,
            height,
        }
    }

    fn band_of(y: f32, bottom: f32, height: f32, count: usize) -> usize {
        (((y - bottom) / height).max(0.0) as usize).min(count - 1)
    }

    /// Whether `p` lies inside `points`, the line the bands were made of.
    pub fn contains(&self, points: &[Vec2], p: Vec2) -> bool {
        let count = self.starts.len() - 1;
        if points.len() < 3 || p.y < self.bottom || p.y > self.bottom + self.height * count as f32 {
            return false;
        }
        let band = Self::band_of(p.y, self.bottom, self.height, count);
        let mut inside = false;
        for &i in &self.edges[self.starts[band] as usize..self.starts[band + 1] as usize] {
            let i = i as usize;
            let (a, b) = (points[i], points[(i + 1) % points.len()]);
            if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x) {
                inside = !inside;
            }
        }
        inside
    }
}

/// The distance from `p` to the closed line through `points`.
pub fn distance(points: &[Vec2], p: Vec2) -> f32 {
    let n = points.len();
    (0..n).map(|i| closest(points[i], points[(i + 1) % n], p).1).fold(f32::MAX, f32::min)
}

/// A closed line that finds the points inside it and how far points are from it.
pub struct Polygon<'a> {
    pub points: &'a [Vec2],
    bands: Bands,
    pieces: OnceCell<Pieces<'a>>,
}

impl<'a> Polygon<'a> {
    pub fn new(points: &'a [Vec2]) -> Polygon<'a> {
        Polygon {
            points,
            bands: Bands::new(points),
            pieces: OnceCell::new(),
        }
    }

    pub fn contains(&self, p: Vec2) -> bool {
        self.bands.contains(self.points, p)
    }

    /// The distance from `p` to the line, or `limit` when the line is farther away.
    pub fn distance_within(&self, p: Vec2, limit: f32) -> f32 {
        let pieces = self.pieces.get_or_init(|| {
            let n = self.points.len();
            let length: f32 = (0..n).map(|i| self.points[i].distance(self.points[(i + 1) % n])).sum();
            Pieces::new(self.points, 2.0 * length / n.max(1) as f32)
        });
        pieces.closest(p, limit).map_or(limit, |(_, _, d)| d)
    }
}
