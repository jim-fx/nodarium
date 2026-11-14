use std::collections::HashMap;
use std::ops::Range;

use glam::Vec2;

use crate::polygon::Pieces;
use crate::veins::{Segment, Vein};

/// A corner of the grid, `(i, j)` from its lower left corner.
pub type Corner = (usize, usize);

fn smin(a: f32, b: f32, k: f32) -> f32 {
    if k <= 0.0 {
        return a.min(b);
    }
    let h = (k - (a - b).abs()).max(0.0) / k;
    a.min(b) - h * h * k * 0.25
}

/// Grid points per side of the largest blocks the blade is sampled in, and of the smallest.
const LARGEST: usize = 32;
const SMALLEST: usize = 4;

/// Whether a point holds the blade, a bound of it, or a bound and the blade is needed there.
const EXACT: u8 = 0;
const ROUGH: u8 = 1;
const NEEDED: u8 = 2;

/// No point of the box from `lo` to `hi` is closer to the segment than this.
fn gap(s: &Segment, lo: Vec2, hi: Vec2) -> f32 {
    let (min, max) = (s.a.min(s.b), s.a.max(s.b));
    (min - hi).max(lo - max).max(Vec2::ZERO).length()
}

/// The cuts at the points of the closed line `margin`, each a share of how far the blade reaches
/// there, read at the piece of it closest to a point.
pub struct Cuts<'a> {
    cuts: Vec<f32>,
    shares: &'a [f32],
    /// No cut is deeper anywhere, as `at` blends between the cuts of two points.
    deepest: f32,
    pieces: Pieces<'a>,
}

impl<'a> Cuts<'a> {
    pub fn new(margin: &'a [Vec2], shares: &'a [f32], reaches: &[f32], cell: f32) -> Cuts<'a> {
        let cuts: Vec<f32> = shares.iter().zip(reaches).map(|(s, r)| s * r).collect();
        let deepest = cuts.iter().fold(0.0_f32, |m, c| m.max(c.abs()));
        // as wide as the band of the grid that is cut, so a point there meets few cells
        let pieces = Pieces::new(margin, deepest + 2.0 * cell);
        Cuts {
            cuts,
            shares,
            deepest,
            pieces,
        }
    }

    fn read(&self, values: &[f32], p: Vec2) -> f32 {
        let n = values.len();
        self.pieces
            .closest(p, f32::MAX)
            .map_or(0.0, |(k, t, _)| values[k] + (values[(k + 1) % n] - values[k]) * t)
    }

    /// The cut of the piece closest to `p`, the first of them where several are as close.
    pub fn at(&self, p: Vec2) -> f32 {
        self.read(&self.cuts, p)
    }

    /// The share of the reach cut at the piece closest to `p`.
    pub fn share(&self, p: Vec2) -> f32 {
        self.read(self.shares, p)
    }
}

/// The most cells along a side of the grid, about.
const MOST_CELLS: usize = 1024;

/// The blade sampled on a square grid in leaf space, negative inside. Its zero line is the margin.
pub struct Field<'a> {
    pub nx: usize,
    pub ny: usize,
    pub origin: Vec2,
    pub cell: f32,
    pub values: Vec<f32>,
    /// Whether a point holds the blade or only a bound of it, on its side of the margin: the
    /// blade is only worked out where it is needed.
    state: Vec<u8>,
    /// Points the cut is not added to yet, as it cannot move them across the margin.
    pending: Vec<bool>,
    veins: &'a [Vein],
    smooth: f32,
    /// How far the bounds of the blade may be off by rounding.
    slack: f32,
    /// The lower bounds of the segments of a vein in a block, kept to save allocating them.
    lows: Vec<f32>,
}

impl<'a> Field<'a> {
    /// The blade around `veins`, `resolution` cells across the longer side of the leaf, without
    /// holes.
    pub fn blade(veins: &'a [Vein], smooth: f32, resolution: usize) -> Option<Field<'a>> {
        let mut field = Self::grid(veins, smooth, resolution)?;
        field.sample(0.0, ROUGH);
        field.fill_holes();
        field.settle(None);
        Some(field)
    }

    /// The grid around `veins`, every point of it still to be worked out.
    fn grid(veins: &'a [Vein], smooth: f32, resolution: usize) -> Option<Field<'a>> {
        let reach = veins.iter().fold(0.0_f32, |m, v| m.max(v.reach));
        let min = veins.iter().fold(Vec2::MAX, |m, v| m.min(v.min));
        let max = veins.iter().fold(Vec2::MIN, |m, v| m.max(v.max));
        let extent = (max - min).max_element() + 2.0 * reach;
        if !(extent > 0.0) {
            return None;
        }
        // the padding grows with `smooth`, not with the leaf, so a small leaf with a wide fillet
        // gets larger cells instead of a grid that does not reach around it
        let fit = ((max - min).max_element() + 2.0 * (reach + smooth)) / (MOST_CELLS - 4) as f32;
        let cell = (extent / resolution as f32).max(fit);
        let pad = reach + smooth + 2.0 * cell;
        let origin = min - Vec2::splat(pad);
        let size = max - min + Vec2::splat(2.0 * pad);
        let nx = ((size.x / cell).ceil() as usize).max(2);
        let ny = ((size.y / cell).ceil() as usize).max(2);

        let count = (nx + 1) * (ny + 1);
        Some(Field {
            nx,
            ny,
            origin,
            cell,
            values: vec![f32::MAX; count],
            state: vec![ROUGH; count],
            pending: vec![false; count],
            veins,
            smooth,
            slack: 1e-4 * extent,
            lows: vec![],
        })
    }

    /// Works out the blade at the points of `level` or above, in blocks that are split until they
    /// are small. The blade at a point is about its signed distance to the margin, negative on the
    /// blade: every vein is a round cone as wide as its reach, joined to the veins before it with a
    /// fillet of `smooth`, passed over where it lies a fillet beyond the blade so far.
    ///
    /// The blade only grows with the distance to every vein, so bounds of those over a block bound
    /// the blade in it: a block that lies `far` or more from the margin by them keeps the bound, as
    /// only its side of the margin counts. A block also drops the segments that cannot be the
    /// closest of their vein in it, and the veins that lie a fillet beyond the closer ones before
    /// them, which leave the blade as it is, so its smaller blocks and its points only meet the
    /// ones left and get the same blade, to the bit.
    fn sample(&mut self, far: f32, level: u8) {
        let mut segments: Vec<&'a Segment> = self.veins.iter().flat_map(|v| &v.segments).collect();
        let mut near = Vec::new();
        let mut start = 0;
        for (v, vein) in self.veins.iter().enumerate() {
            near.push((v, start, start + vein.segments.len(), f32::MIN));
            start += vein.segments.len();
        }
        let all = 0..near.len();
        for j in (0..=self.ny).step_by(LARGEST) {
            for i in (0..=self.nx).step_by(LARGEST) {
                self.block((i, j), LARGEST, all.clone(), &mut near, &mut segments, far, level);
            }
        }
    }

    fn block(
        &mut self,
        (i0, j0): Corner,
        size: usize,
        parent: Range<usize>,
        near: &mut Vec<(usize, usize, usize, f32)>,
        segments: &mut Vec<&'a Segment>,
        far: f32,
        level: u8,
    ) {
        if i0 > self.nx || j0 > self.ny {
            return;
        }
        let (i1, j1) = ((i0 + size - 1).min(self.nx), (j0 + size - 1).min(self.ny));
        if !(j0..=j1).any(|j| self.state[self.index((i0, j))..=self.index((i1, j))].iter().any(|&r| r >= level)) {
            return;
        }
        let (lo, hi) = (self.point((i0, j0)), self.point((i1, j1)));
        let (center, radius) = ((lo + hi) * 0.5, (hi - lo).length() * 0.5);
        let (smooth, slack) = (self.smooth, self.slack);
        let (first, from) = (near.len(), segments.len());
        let (mut lowest, mut highest, mut closest) = (f32::MAX, f32::MAX, f32::MAX);
        let mut lows = std::mem::take(&mut self.lows);
        for k in parent {
            let (v, a, b, _) = near[k];
            let vein = &self.veins[v];
            let passed = |lower: f32| lower >= closest + smooth + slack;
            if passed((vein.min - hi).max(lo - vein.max).max(Vec2::ZERO).length() - vein.reach - slack) {
                continue;
            }
            lows.clear();
            lows.extend(segments[a..b].iter().map(|s| gap(s, lo, hi) - s.ra.max(s.rb) - slack));
            let lower = lows.iter().copied().fold(f32::MAX, f32::min);
            if passed(lower) {
                continue;
            }
            let upper = segments[a..b]
                .iter()
                .map(|s| s.closest(center).1 + radius - s.ra.min(s.rb))
                .fold(f32::MAX, f32::min)
                + slack;
            let start = segments.len();
            for (k, &l) in (a..b).zip(lows.iter()) {
                if l <= upper {
                    segments.push(segments[k]);
                }
            }
            near.push((v, start, segments.len(), lower));
            lowest = smin(lowest, lower, smooth);
            highest = smin(highest, upper, smooth);
            closest = closest.min(upper);
        }
        self.lows = lows;

        let bound = if lowest - slack >= far {
            Some(lowest - slack)
        } else if highest + slack <= -far {
            Some(highest + slack)
        } else {
            None
        };
        if let Some(bound) = bound {
            for j in j0..=j1 {
                for i in i0..=i1 {
                    let index = self.index((i, j));
                    if self.state[index] >= level {
                        self.values[index] = bound;
                    }
                }
            }
        } else if size <= SMALLEST {
            for j in j0..=j1 {
                for i in i0..=i1 {
                    let index = self.index((i, j));
                    if self.state[index] < level {
                        continue;
                    }
                    let p = self.point((i, j));
                    let mut f = f32::MAX;
                    for &(v, a, b, lower) in &near[first..] {
                        // either leaves the blade as it is
                        if lower >= f + smooth || self.veins[v].lower_bound(p) >= f + smooth {
                            continue;
                        }
                        let mut d = f32::MAX;
                        for s in &segments[a..b] {
                            let (t, dist) = s.closest(p);
                            d = d.min(dist - s.reach(t));
                        }
                        f = smin(f, d, smooth);
                    }
                    self.values[index] = f;
                    self.state[index] = EXACT;
                }
            }
        } else {
            let half = size / 2;
            let own = first..near.len();
            for (di, dj) in [(0, 0), (half, 0), (0, half), (half, half)] {
                self.block((i0 + di, j0 + dj), half, own.clone(), near, segments, far, level);
            }
        }
        near.truncate(first);
        segments.truncate(from);
    }

    /// How far the blade reaches around the veins at every point of `points`: the reach of the
    /// closest point of every vein, by the blade, blended over a fillet and a cell between the
    /// veins so it does not jump in a sinus.
    pub fn reaches(&self, points: &[Vec2]) -> Vec<f32> {
        let blend = self.smooth + self.cell;
        let mut near: Vec<(f32, f32)> = Vec::new();
        // the closest vein of the point before, likely the closest of this one, met first so the
        // other veins are passed over sooner
        let mut previous = 0;
        points
            .iter()
            .map(|&p| {
                near.clear();
                let mut lowest = f32::MAX;
                let (n, start) = (self.veins.len(), previous);
                for v in (0..n).map(|k| (start + k) % n) {
                    let vein = &self.veins[v];
                    if vein.lower_bound(p) >= lowest + blend {
                        continue;
                    }
                    let (mut excess, mut reach) = (f32::MAX, 0.0);
                    for s in &vein.segments {
                        let (t, d) = s.closest(p);
                        let r = s.reach(t);
                        if d - r < excess {
                            (excess, reach) = (d - r, r);
                        }
                    }
                    if excess < lowest {
                        (lowest, previous) = (excess, v);
                    }
                    near.push((excess, reach));
                }
                let (mut sum, mut weights) = (0.0, 0.0);
                for &(excess, reach) in &near {
                    let w = (1.0 - (excess - lowest) / blend).max(0.0).powi(2);
                    sum += w * reach;
                    weights += w;
                }
                if weights > 0.0 {
                    sum / weights
                } else {
                    0.0
                }
            })
            .collect()
    }

    pub fn index(&self, (i, j): Corner) -> usize {
        j * (self.nx + 1) + i
    }

    pub fn point(&self, (i, j): Corner) -> Vec2 {
        self.origin + Vec2::new(i as f32, j as f32) * self.cell
    }

    pub fn inside(&self, corner: Corner) -> bool {
        self.values[self.index(corner)] < 0.0
    }

    fn horizontal_edges(&self) -> usize {
        self.nx * (self.ny + 1)
    }

    /// The corners and edges of a cell, counter-clockwise from its lower left corner. Edge k goes
    /// from corner k to corner k + 1.
    fn cell(&self, i: usize, j: usize) -> ([Corner; 4], [usize; 4]) {
        let h = self.horizontal_edges();
        (
            [(i, j), (i + 1, j), (i + 1, j + 1), (i, j + 1)],
            [
                j * self.nx + i,
                h + j * (self.nx + 1) + i + 1,
                (j + 1) * self.nx + i,
                h + j * (self.nx + 1) + i,
            ],
        )
    }

    fn edge_ends(&self, edge: usize) -> (Corner, Corner) {
        let h = self.horizontal_edges();
        if edge < h {
            let (j, i) = (edge / self.nx, edge % self.nx);
            ((i, j), (i + 1, j))
        } else {
            let e = edge - h;
            let (j, i) = (e / (self.nx + 1), e % (self.nx + 1));
            ((i, j), (i, j + 1))
        }
    }

    /// Where the margin crosses an edge.
    fn crossing(&self, edge: usize) -> Vec2 {
        let (a, b) = self.edge_ends(edge);
        let (fa, fb) = (self.values[self.index(a)], self.values[self.index(b)]);
        let t = if fa != fb { fa / (fa - fb) } else { 0.5 };
        self.point(a).lerp(self.point(b), t.clamp(0.0, 1.0))
    }

    /// Whether the middle of a cell whose corners are inside and outside in turn is inside.
    fn center_inside(&self, corners: &[Corner; 4]) -> bool {
        corners.iter().map(|&c| self.values[self.index(c)]).sum::<f32>() < 0.0
    }

    /// A leaf has no holes: the gaps the veins close in on every side become blade.
    fn fill_holes(&mut self) {
        let mut outside = vec![false; self.values.len()];
        let mut stack: Vec<Corner> = Vec::new();
        for i in 0..=self.nx {
            stack.push((i, 0));
            stack.push((i, self.ny));
        }
        for j in 0..=self.ny {
            stack.push((0, j));
            stack.push((self.nx, j));
        }
        while let Some((i, j)) = stack.pop() {
            let index = self.index((i, j));
            if outside[index] || self.values[index] < 0.0 {
                continue;
            }
            outside[index] = true;
            if i > 0 {
                stack.push((i - 1, j));
            }
            if i < self.nx {
                stack.push((i + 1, j));
            }
            if j > 0 {
                stack.push((i, j - 1));
            }
            if j < self.ny {
                stack.push((i, j + 1));
            }
        }
        let inside = -0.5 * self.cell;
        for (k, outside) in outside.into_iter().enumerate() {
            if !outside && self.values[k] >= 0.0 {
                self.values[k] = inside;
                self.state[k] = EXACT;
                self.pending[k] = false;
            }
        }
    }

    /// Works out the blade at the points next to the margin that hold a bound, or still miss their
    /// cut, where the contour reads them.
    fn settle(&mut self, cuts: Option<&Cuts>) {
        let row = self.nx + 1;
        let mut any = false;
        for index in 0..self.values.len() {
            if self.state[index] == EXACT && !self.pending[index] {
                continue;
            }
            let (i, j) = (index % row, index / row);
            let inside = self.values[index] < 0.0;
            let differs = |other: usize| (self.values[other] < 0.0) != inside;
            if (i > 0 && differs(index - 1))
                || (i < self.nx && differs(index + 1))
                || (j > 0 && differs(index - row))
                || (j < self.ny && differs(index + row))
            {
                if self.state[index] == ROUGH {
                    self.state[index] = NEEDED;
                }
                any = true;
            } else if self.state[index] == EXACT {
                self.pending[index] = false;
            }
        }
        if !any {
            return;
        }
        self.sample(f32::INFINITY, NEEDED);
        let Some(cuts) = cuts else {
            return;
        };
        let band = cuts.deepest + 2.0 * self.cell;
        for index in 0..self.values.len() {
            if self.state[index] == EXACT
                && std::mem::replace(&mut self.pending[index], false)
                && self.values[index].abs() < band
            {
                self.values[index] += cuts.at(self.point((index % row, index / row)));
            }
        }
    }

    /// Moves the margin in by `cuts`. A grid point closer to the margin than the deepest cut and
    /// two cells takes the cut of the closest piece of the margin. Only where the cut can turn a
    /// point to the other side of the margin, and next to the margin, is it worked out: elsewhere
    /// the point keeps its side, and its bound or blade.
    pub fn cut(&mut self, cuts: &Cuts) {
        if cuts.deepest == 0.0 || cuts.cuts.len() < 2 {
            return;
        }
        let band = cuts.deepest + 2.0 * self.cell;
        let turns = cuts.deepest + self.slack;
        self.sample(turns, ROUGH);
        for j in 0..=self.ny {
            for i in 0..=self.nx {
                let index = self.index((i, j));
                let value = self.values[index];
                if self.state[index] != EXACT || value.abs() > turns {
                    self.pending[index] = true;
                } else if value.abs() < band {
                    self.values[index] += cuts.at(self.point((i, j)));
                }
            }
        }
        self.fill_holes();
        self.settle(Some(cuts));
    }

    /// The pieces of the margin in every cell, as pairs of the edges they cross.
    fn contour_segments(&self) -> Vec<[usize; 2]> {
        let mut segments = Vec::new();
        for j in 0..self.ny {
            for i in 0..self.nx {
                let (corners, edges) = self.cell(i, j);
                let inside = corners.map(|c| self.inside(c));
                let crossed: Vec<usize> = (0..4)
                    .filter(|&k| inside[k] != inside[(k + 1) % 4])
                    .map(|k| edges[k])
                    .collect();
                match crossed.len() {
                    2 => segments.push([crossed[0], crossed[1]]),
                    4 if inside[0] == self.center_inside(&corners) => {
                        segments.push([edges[0], edges[1]]);
                        segments.push([edges[2], edges[3]]);
                    }
                    4 => {
                        segments.push([edges[3], edges[0]]);
                        segments.push([edges[1], edges[2]]);
                    }
                    _ => {}
                }
            }
        }
        segments
    }

    /// The closed lines where the field crosses zero.
    pub fn contour_loops(&self) -> Vec<Vec<Vec2>> {
        let segments = self.contour_segments();
        let mut by_edge: HashMap<usize, Vec<usize>> = HashMap::new();
        for (s, seg) in segments.iter().enumerate() {
            for &e in seg {
                by_edge.entry(e).or_default().push(s);
            }
        }
        let mut used = vec![false; segments.len()];
        let mut loops = Vec::new();
        for start in 0..segments.len() {
            if used[start] {
                continue;
            }
            used[start] = true;
            let mut edges = vec![segments[start][0]];
            let mut edge = segments[start][1];
            while edge != edges[0] {
                edges.push(edge);
                let Some(s) = by_edge[&edge].iter().copied().find(|&s| !used[s]) else {
                    break;
                };
                used[s] = true;
                edge = if segments[s][0] == edge {
                    segments[s][1]
                } else {
                    segments[s][0]
                };
            }
            loops.push(edges.into_iter().map(|e| self.crossing(e)).collect());
        }
        loops
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::outline::Margin;
    use crate::tests::{beech, pinnate, ring, teeth};
    use crate::veins::leaf_space;
    use nodarium_utils::{geometry::is_path, split_args, Ctx};

    fn blade_at(veins: &[Vein], p: Vec2, smooth: f32) -> f32 {
        let mut f = f32::MAX;
        for vein in veins {
            if vein.lower_bound(p) >= f + smooth {
                continue;
            }
            let mut d = f32::MAX;
            for s in &vein.segments {
                let (t, dist) = s.closest(p);
                d = d.min(dist - s.reach(t));
            }
            f = smin(f, d, smooth);
        }
        f
    }

    fn cut_at(margin: &[Vec2], cuts: &[f32], p: Vec2) -> f32 {
        let n = margin.len();
        let mut best = (f32::MAX, 0.0);
        for k in 0..n {
            let (t, d) = crate::veins::closest(margin[k], margin[(k + 1) % n], p);
            if d < best.0 {
                best = (d, cuts[k] + (cuts[(k + 1) % n] - cuts[k]) * t);
            }
        }
        best.1
    }

    /// The blade at every point of the grid, one after another, and the margin cut by `cuts` at
    /// every point near it.
    fn dense<'a>(veins: &'a [Vein], smooth: f32, resolution: usize, cuts: Option<(&[Vec2], &[f32])>) -> Field<'a> {
        let mut field = Field::grid(veins, smooth, resolution).unwrap();
        for k in 0..field.values.len() {
            let p = field.point((k % (field.nx + 1), k / (field.nx + 1)));
            field.values[k] = blade_at(veins, p, smooth);
            field.state[k] = EXACT;
        }
        field.fill_holes();
        if let Some((margin, cuts)) = cuts {
            let band = cuts.iter().fold(0.0_f32, |m, c| m.max(c.abs())) + 2.0 * field.cell;
            for k in 0..field.values.len() {
                if field.values[k].abs() < band {
                    let p = field.point((k % (field.nx + 1), k / (field.nx + 1)));
                    field.values[k] += cut_at(margin, cuts, p);
                }
            }
            field.fill_holes();
        }
        field
    }

    #[test]
    fn sampling_by_blocks_finds_the_margin_of_every_point() {
        let leaves = [
            (beech(), 0.1, 96),
            (beech(), 0.0, 64),
            (pinnate(4, |t| 0.01 + 0.04 * t * t).0, 0.01, 128),
            (pinnate(24, |t| 0.03 * (1.0 - 0.5 * t)).0, 0.05, 256),
            (ring(), 0.0, 64),
        ];
        for (data, smooth, resolution) in leaves {
            let paths: Vec<&[i32]> = split_args(&data).into_iter().filter(|p| is_path(p)).collect();
            let veins = leaf_space(&paths);
            let mut field = Field::blade(&veins, smooth, resolution).unwrap();
            let margin = Margin::of(&field);
            let mut ctx = Ctx::default();
            let along = ctx.float_along(&teeth(0.3), margin.alphas.iter().copied());
            let reference = dense(&veins, smooth, resolution, None);
            assert!(field.contour_loops() == reference.contour_loops(), "the margin moved at {resolution}");

            let reaches = field.reaches(&margin.points);
            let scaled: Vec<f32> = along.iter().zip(&reaches).map(|(s, r)| s * r).collect();
            let cuts = Cuts::new(&margin.points, &along, &reaches, field.cell);
            field.cut(&cuts);
            let reference = dense(&veins, smooth, resolution, Some((&margin.points, &scaled)));
            assert!(field.contour_loops() == reference.contour_loops(), "the cut margin moved at {resolution}");
            for k in 0..field.values.len() {
                assert_eq!(field.values[k] < 0.0, reference.values[k] < 0.0, "a point changed sides");
            }
        }
    }
}
