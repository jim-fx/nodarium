//! The nearest edge, found among the middles near the place only.
//!
//! [`Patches::edge_distance`] measures the middles of the rows within `ROW_REACH` of a place, and
//! of every row those in a window around the angle of the place, about 70 of them. Only the few
//! nearest decide the edge. [`Near`] keeps the middles within reach of a small disc and measures
//! only those for the places in the disc, four at a time. The cells of a ring follow each other
//! around it, so a disc serves several of them.
//!
//! The kept middles give the same edge as the full search, bit for bit, where the bounds below
//! show that the full search finds it among them too. Elsewhere, at a few places in ten
//! thousand, the full search runs.

use crate::lanes::F4;
use crate::{Patches, ROW_REACH};
use std::f32::consts::{PI, TAU};

/// The nearest edge is closer than this many spacings in the stretched measure almost everywhere
const REACH: f32 = 1.4;
/// The radius of the disc of places that share the kept middles, in spacings
const MOVE: f32 = 0.5;
/// A middle far away, to fill the last lanes
const FAR: f32 = 1e15;

/// How much of the window of a row around the angle of the place is sure to hold a middle
#[derive(Clone, Copy)]
struct Window {
    /// The window holds the whole row
    whole: bool,
    /// The window holds a middle whose angle from the place has a cos over this
    cos: f32,
}

/// The middles that the places of a small disc may need
pub struct Near {
    center: [f32; 2],
    /// How far a place may be from the center, squared
    moved: f32,
    /// The kept middles four at a time, the last lanes filled with middles far away
    x: Vec<[f32; 4]>,
    y: Vec<[f32; 4]>,
    count: usize,
    id: Vec<u32>,
    row: Vec<usize>,
    /// How far each kept middle is from the axis
    radius: Vec<f32>,
    /// The windows of the rows from `first_row` on
    first_row: usize,
    windows: Vec<Window>,
    /// The kept middles in the frame of the place, their measure squared and the distance to
    /// their edge with the nearest one
    along: Vec<[f32; 4]>,
    across: Vec<[f32; 4]>,
    length: Vec<[f32; 4]>,
    edge: Vec<[f32; 4]>,
    root: f32,
    spacing: f32,
    /// [`REACH`] squared, in metres
    reach: f32,
    /// Middles within `REACH` of the place in the stretched measure are within this in metres
    plain: f32,
}

impl Near {
    pub fn new(patches: &Patches, spacing: f32) -> Near {
        let square = patches.root * patches.root;
        let least = 0.999 * if square < 1.0 { square } else { 1.0 / square };
        let reach = REACH * spacing;
        Near {
            center: [f32::INFINITY; 2],
            moved: 0.0,
            x: Vec::new(),
            y: Vec::new(),
            count: 0,
            id: Vec::new(),
            row: Vec::new(),
            radius: Vec::new(),
            first_row: 0,
            windows: Vec::new(),
            along: Vec::new(),
            across: Vec::new(),
            length: Vec::new(),
            edge: Vec::new(),
            root: patches.root,
            spacing,
            reach: reach * reach,
            plain: reach / least.sqrt(),
        }
    }

    fn push(&mut self, m: [f32; 2], id: u32, row: usize) {
        if self.count.is_multiple_of(4) {
            self.x.push([FAR; 4]);
            self.y.push([FAR; 4]);
        }
        let (lane, i) = (self.count / 4, self.count % 4);
        self.x[lane][i] = m[0];
        self.y[lane][i] = m[1];
        self.id.push(id);
        self.row.push(row);
        self.radius.push((m[0] * m[0] + m[1] * m[1]).sqrt());
        self.count += 1;
    }

    /// Keeps the middles that the places within `MOVE` of a point a bit ahead of `place` may need
    fn keep(&mut self, patches: &Patches, place: [f32; 2], cos: f32, sin: f32) {
        let ahead = 0.8 * MOVE * self.spacing;
        let center = [place[0] - sin * ahead, place[1] + cos * ahead];
        let moved = MOVE * self.spacing;
        let far = (self.plain + moved) * 1.001;
        self.center = center;
        self.moved = 0.999 * moved * moved;
        self.x.clear();
        self.y.clear();
        self.count = 0;
        self.id.clear();
        self.row.clear();
        self.radius.clear();
        self.windows.clear();

        let radius = (center[0] * center[0] + center[1] * center[1]).sqrt();
        let angle = center[1].atan2(center[0]).rem_euclid(TAU);
        let gap = patches.row_gap;
        let first = ((radius - far) / gap - 1.0).floor();
        let first = if first > 0.0 { first as usize } else { 0 };
        let last = (((radius + far) / gap + 1.0).ceil() as usize).min(patches.rows.len() - 1);
        self.first_row = first;
        // a place within `moved` of the center is no farther than this from the axis
        let widest = (radius + moved) * 1.001;
        let wide = if widest > patches.around { widest } else { patches.around };
        for r in first..=last {
            let middles = &patches.rows[r];
            let n = middles.len();
            let (low, high) = (patches.low[r], patches.high[r]);
            if low - radius > far || radius - high > far {
                self.windows.push(Window { whole: false, cos: 2.0 });
                continue;
            }
            // a middle within `far` of the center is within `turn` of its angle, as
            // |m - c| ≥ 2 sqrt(|m| |c|) sin(Δ / 2), and asin(z) is under z asin(0.5) / 0.5
            // up to 0.5 and under z π / 2 up to 1
            let z = far / (2.0 * (low * radius).sqrt());
            let turn = if z < 0.5 { 2.0944 * z } else { PI * z } * 1.001 + 1e-4;
            let around = n as f32 / TAU;
            let (start, count) = if turn < PI {
                let start = ((angle - turn) * around).floor() as i64 - 1;
                let end = ((angle + turn) * around).floor() as i64 + 2;
                (start.rem_euclid(n as i64) as usize, ((end - start) as usize).min(n))
            } else {
                (0, n)
            };
            for i in start..start + count {
                let (_, m, id) = middles[if i < n { i } else { i - n }];
                let d = [m[0] - center[0], m[1] - center[1]];
                if d[0] * d[0] + d[1] * d[1] <= far * far {
                    self.push(m, id, r);
                }
            }
            // the place is no farther out than `widest`, so the window of the row reaches this
            // far at least
            let reach = (ROW_REACH * patches.around * n as f32 / (TAU * wide)).ceil() as usize + 2;
            let reach = reach.min(n / 2);
            let whole = n <= 16 || 2 * reach + 1 >= n;
            // the first middle not before the place is less than 1.9 slots from it, so a middle
            // within reach - 1.9 slots of the place is in the window. 1 - x² / 2 + x⁴ / 24 is
            // never under cos x, so the test errs on the side of the full search.
            let within = (reach as f32 - 3.0) * TAU / n as f32 - 1e-4;
            let square = within * within;
            let cos = if within > 0.0 { 1.0 - square / 2.0 + square * square / 24.0 } else { 2.0 };
            self.windows.push(Window { whole, cos });
        }
        let lanes = self.x.len();
        for v in [&mut self.along, &mut self.across, &mut self.length, &mut self.edge] {
            v.resize(lanes, [0.0; 4]);
        }
    }

    /// Whether the full search looks at kept middle `i` from the place
    fn looked_at(&self, i: usize, place: [f32; 2], radius: f32, rows: (usize, usize)) -> bool {
        let r = self.row[i];
        if r < rows.0 || r > rows.1 {
            return false;
        }
        let window = self.windows[r - self.first_row];
        if window.whole {
            return true;
        }
        let dot = self.x[i / 4][i % 4] * place[0] + self.y[i / 4][i % 4] * place[1];
        let scale = self.radius[i] * radius;
        dot - window.cos * scale > 1e-5 * scale
    }

    /// The same as [`Patches::edge_distance`], bit for bit
    pub fn edge_distance(&mut self, patches: &Patches, place: [f32; 2], stretch: f32) -> (f32, f32) {
        self.kept_only(patches, place).unwrap_or_else(|| patches.edge_distance(place, stretch))
    }

    /// The edge from the kept middles, `None` where they may not give the same as the full search
    fn kept_only(&mut self, patches: &Patches, place: [f32; 2]) -> Option<(f32, f32)> {
        let radius = (place[0] * place[0] + place[1] * place[1]).sqrt();
        let (cos, sin) = if radius > 0.0 { (place[0] / radius, place[1] / radius) } else { (1.0, 0.0) };
        let moved = [place[0] - self.center[0], place[1] - self.center[1]];
        if !(moved[0] * moved[0] + moved[1] * moved[1] <= self.moved) {
            self.keep(patches, place, cos, sin);
        }
        self.search(patches, place, radius, cos, sin)
    }

    #[cfg_attr(target_arch = "wasm32", target_feature(enable = "simd128"))]
    fn search(&mut self, patches: &Patches, place: [f32; 2], radius: f32, cos: f32, sin: f32) -> Option<(f32, f32)> {
        let (px, py) = (F4::splat(place[0]), F4::splat(place[1]));
        let (c, s, root) = (F4::splat(cos), F4::splat(sin), F4::splat(self.root));
        let mut least = F4::splat(f32::INFINITY);
        let measured = self.along.iter_mut().zip(&mut self.across).zip(&mut self.length);
        for ((x, y), ((along, across), length)) in self.x.iter().zip(&self.y).zip(measured) {
            let (dx, dy) = (F4::load(x).sub(px), F4::load(y).sub(py));
            // along the radius and around the axis at the place
            let a = dx.mul(c).add(dy.mul(s)).div(root);
            let b = dy.mul(c).sub(dx.mul(s)).mul(root);
            let l = a.mul(a).add(b.mul(b));
            a.store(along);
            b.store(across);
            l.store(length);
            least = least.at_most(l);
        }
        let near = least.least();
        if !(near <= self.reach) {
            return None;
        }
        let first = only(&self.length, near)?;
        let id = self.id[first];

        let (fx, fy) = (F4::splat(self.along[first / 4][first % 4]), F4::splat(self.across[first / 4][first % 4]));
        let (n, two, tiny) = (F4::splat(near), F4::splat(2.0), F4::splat(f32::EPSILON));
        let lengths = self.length.iter().zip(&mut self.edge);
        for ((along, across), (length, edge)) in self.along.iter().zip(&self.across).zip(lengths) {
            let (gx, gy) = (F4::load(along).sub(fx), F4::load(across).sub(fy));
            // `if span > EPSILON { span } else { EPSILON }`
            let span = tiny.at_least(gx.mul(gx).add(gy.mul(gy)).sqrt());
            F4::load(length).sub(n).div(two.mul(span)).store(edge);
        }
        self.edge[first / 4][first % 4] = f32::INFINITY;
        let distance = self.edge.iter().fold(F4::splat(f32::INFINITY), |least, e| least.at_most(F4::load(e))).least();
        // a middle is no nearer to the edge than (|m| - |first|) / 2, so every middle that may
        // be nearer to it than `distance` is kept
        let farthest = (near.sqrt() + 2.002 * distance + 1e-6) / 0.999;
        if !(farthest * farthest * 1.0001 <= self.reach) {
            return None;
        }
        let other = only(&self.edge, distance)?;
        let row = radius / patches.row_gap;
        let from = (row - ROW_REACH).floor();
        let to = (row + ROW_REACH).ceil() as usize;
        let rows = (if from > 0.0 { from as usize } else { 0 }, if to < patches.rows.len() - 1 { to } else { patches.rows.len() - 1 });
        if !self.looked_at(first, place, radius, rows) || !self.looked_at(other, place, radius, rows) {
            return None;
        }
        let other = self.id[other];
        Some((distance, crate::hash(0, id.min(other) as usize, id.max(other) as usize, 7)))
    }
}

/// The index of the only value equal to `least`, `None` if there are more
#[cfg_attr(target_arch = "wasm32", target_feature(enable = "simd128"))]
fn only(values: &[[f32; 4]], least: f32) -> Option<usize> {
    let target = F4::splat(least);
    let mut found = None;
    for (l, lane) in values.iter().enumerate() {
        let bits = F4::load(lane).equal(target);
        if bits == 0 {
            continue;
        }
        if found.is_some() || bits.count_ones() > 1 {
            return None;
        }
        found = Some(l * 4 + bits.trailing_zeros() as usize);
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Params;

    /// Places ring by ring around the axis like the cells of the pattern, every `skip`th ring,
    /// moved by a wave as the cells are
    fn places(radius: f32, cell: f32, skip: usize) -> impl Iterator<Item = [f32; 2]> {
        let rings = (radius / cell) as usize + 1;
        (0..rings).step_by(skip).flat_map(move |i| {
            let r = radius * i as f32 / (rings - 1) as f32;
            let n = ((TAU * r / cell).round() as usize).max(3);
            (0..n).map(move |j| {
                let angle = TAU * (j as f32 + 0.5) / n as f32;
                let wave = 0.3 * cell * (7.0 * angle + r / cell).sin();
                [(r + wave) * angle.cos(), (r - wave) * angle.sin()]
            })
        })
    }

    #[test]
    fn the_kept_middles_give_the_edge_of_the_full_search() {
        let cases = [(0.004, 1.0, 3), (0.0013, 1.0, 0), (0.003, 1.8, 0), (0.002, 3.5, 7), (0.004, 0.3, 1), (0.003, 20.0, 2), (0.01, 0.05, 4)];
        for (spacing, stretch, seed) in cases {
            let p = Params { radius: 0.03, spacing, width: vec![0.001; 2], stretch, softness: 0.0, seed };
            let patches = Patches::new(&p);
            let mut near = Near::new(&patches, spacing);
            let (mut all, mut kept) = (0, 0);
            for place in places(0.031, spacing / 12.0, 5) {
                let full = patches.edge_distance(place, stretch);
                let found = near.kept_only(&patches, place);
                if let Some(found) = found {
                    assert_eq!(found.0.to_bits(), full.0.to_bits(), "{place:?} spacing {spacing} stretch {stretch}");
                    assert_eq!(found.1.to_bits(), full.1.to_bits(), "{place:?} spacing {spacing} stretch {stretch}");
                    kept += 1;
                }
                all += 1;
            }
            // the full search runs at a few places only, for a stretch the node offers
            if (0.25..=4.0).contains(&stretch) {
                assert!(kept as f32 > 0.99 * all as f32, "{kept} of {all} with spacing {spacing} stretch {stretch}");
            }
        }
    }
}
