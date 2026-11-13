use std::collections::HashMap;

use glam::Vec2;

use crate::veins::{closest, Vein};

/// A corner of the grid, `(i, j)` from its lower left corner.
pub type Corner = (usize, usize);

fn smin(a: f32, b: f32, k: f32) -> f32 {
    if k <= 0.0 {
        return a.min(b);
    }
    let h = (k - (a - b).abs()).max(0.0) / k;
    a.min(b) - h * h * k * 0.25
}

/// About the signed distance from `p` to the margin, negative on the blade: every vein is a round
/// cone as wide as its reach, joined to the others with a fillet of `smooth`.
fn blade(veins: &[Vein], p: Vec2, smooth: f32) -> f32 {
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

/// The blade sampled on a square grid in leaf space, negative inside. Its zero line is the margin.
pub struct Field {
    pub nx: usize,
    pub ny: usize,
    pub origin: Vec2,
    pub cell: f32,
    pub values: Vec<f32>,
}

impl Field {
    /// The blade around `veins`, `resolution` cells across the longer side of the leaf, without
    /// holes.
    pub fn blade(veins: &[Vein], smooth: f32, resolution: usize) -> Option<Field> {
        let reach = veins.iter().fold(0.0_f32, |m, v| m.max(v.reach));
        let min = veins.iter().fold(Vec2::MAX, |m, v| m.min(v.min));
        let max = veins.iter().fold(Vec2::MIN, |m, v| m.max(v.max));
        let extent = (max - min).max_element() + 2.0 * reach;
        if !(extent > 0.0) {
            return None;
        }
        let cell = extent / resolution as f32;
        let pad = reach + smooth + 2.0 * cell;
        let origin = min - Vec2::splat(pad);
        let size = max - min + Vec2::splat(2.0 * pad);
        let nx = ((size.x / cell).ceil() as usize).clamp(2, 1024);
        let ny = ((size.y / cell).ceil() as usize).clamp(2, 1024);

        let mut field = Field {
            nx,
            ny,
            origin,
            cell,
            values: Vec::with_capacity((nx + 1) * (ny + 1)),
        };
        for j in 0..=ny {
            for i in 0..=nx {
                let p = field.point((i, j));
                field.values.push(blade(veins, p, smooth));
            }
        }
        field.fill_holes();
        Some(field)
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

    pub fn edge_count(&self) -> usize {
        self.horizontal_edges() + (self.nx + 1) * self.ny
    }

    /// The corners and edges of a cell, counter-clockwise from its lower left corner. Edge k goes
    /// from corner k to corner k + 1.
    pub fn cell(&self, i: usize, j: usize) -> ([Corner; 4], [usize; 4]) {
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
    pub fn crossing(&self, edge: usize) -> Vec2 {
        let (a, b) = self.edge_ends(edge);
        let (fa, fb) = (self.values[self.index(a)], self.values[self.index(b)]);
        let t = if fa != fb { fa / (fa - fb) } else { 0.5 };
        self.point(a).lerp(self.point(b), t.clamp(0.0, 1.0))
    }

    /// Whether the middle of a cell whose corners are inside and outside in turn is inside.
    pub fn center_inside(&self, corners: &[Corner; 4]) -> bool {
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
        for (value, outside) in self.values.iter_mut().zip(outside) {
            if !outside && *value >= 0.0 {
                *value = inside;
            }
        }
    }

    /// Moves the margin in by `cuts`, given at every point of the closed line `margin`. A grid
    /// point takes the cut of the closest piece of the margin.
    pub fn cut(&mut self, margin: &[Vec2], cuts: &[f32]) {
        let deepest = cuts.iter().fold(0.0_f32, |m, c| m.max(c.abs()));
        if deepest == 0.0 || margin.len() < 2 {
            return;
        }
        let band = deepest + 2.0 * self.cell;
        let n = margin.len();
        for j in 0..=self.ny {
            for i in 0..=self.nx {
                let index = self.index((i, j));
                if self.values[index].abs() >= band {
                    continue;
                }
                let p = self.point((i, j));
                let mut best = (f32::MAX, 0.0);
                for k in 0..n {
                    let (t, d) = closest(margin[k], margin[(k + 1) % n], p);
                    if d < best.0 {
                        best = (d, cuts[k] + (cuts[(k + 1) % n] - cuts[k]) * t);
                    }
                }
                self.values[index] += best.1;
            }
        }
        self.fill_holes();
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
