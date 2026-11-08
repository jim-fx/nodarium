//! Spreads a crowded bundle by its density instead of by every contact, after Petrovic, Henne
//! and Anderson, "Volumetric Methods for Simulation and Rendering of Hair" (Pixar 2005): the
//! segments are splatted into a grid as the share of a cell they fill, wherever a cell is fuller
//! than strands can be packed there is pressure, and every point moves down the slope of it.
//! That costs the same for any number of contacts and still has an answer when the strands can't
//! all fit, where the contacts would push against each other forever.

use glam::{IVec3, Vec3};
use std::f32::consts::PI;

/// The grid never has more nodes than this, a larger bundle gets larger cells
const MAX_NODES: usize = 1 << 21;
/// The share of a cell strands fill when they are packed hexagonally, a little less
const PACKING: f32 = 0.9;
/// How far down the slope of the pressure a point moves per step, in cells per unit of slope
const STEP: f32 = 4.0;
/// A point moves at most this many cells in one step
const MAX_STEP: f32 = 0.5;
/// How often a step that doesn't lower the pressure is halved before the bundle counts as spread
const HALVINGS: usize = 4;
/// Room around the bundle for the points to move into, in cells
const PADDING: f32 = 2.0;
/// The samples of a segment are this many cells apart, the splat of a sample reaches a cell
/// to each side, so a long segment leaves no gap
const SAMPLE_SPACING: f32 = 1.0;

fn point(points: &[f32], i: usize) -> Vec3 {
    Vec3::new(points[i * 4], points[i * 4 + 1], points[i * 4 + 2])
}

/// The weights of the eight nodes around a spot `f` of the way into its cell, in the order of
/// `Density::corners`
fn trilinear(f: Vec3) -> [f32; 8] {
    let g = Vec3::ONE - f;
    [
        g.x * g.y * g.z,
        f.x * g.y * g.z,
        g.x * f.y * g.z,
        f.x * f.y * g.z,
        g.x * g.y * f.z,
        f.x * g.y * f.z,
        g.x * f.y * f.z,
        f.x * f.y * f.z,
    ]
}

/// The nodes of a grid of cells of size `h`, a node sits on every corner
pub struct Density {
    origin: Vec3,
    h: f32,
    size: IVec3,
    /// the density no cell may go above
    limit: f32,
    /// the pressure on every node, how much fuller than `limit` it is
    pressure: Vec<f32>,
    scratch: Vec<f32>,
}

impl Density {
    /// A grid around `paths` (points as x, y, z, thickness) for strands that keep `radius` free
    /// between them. `None` when the paths have no thickness or no extent.
    pub fn new(paths: &[Vec<f32>], radius: f32) -> Option<Density> {
        let (mut lo, mut hi) = (Vec3::MAX, Vec3::MIN);
        let (mut thickness, mut count) = (0.0f64, 0usize);
        let mut thickest = 0.0f64;
        for p in paths {
            let mut widest = 0.0f32;
            for i in 0..p.len() / 4 {
                let q = point(p, i);
                if !q.is_finite() {
                    continue;
                }
                lo = lo.min(q);
                hi = hi.max(q);
                thickness += p[i * 4 + 3].abs() as f64;
                widest = widest.max(p[i * 4 + 3].abs());
                count += 1;
            }
            thickest += widest as f64;
        }
        if count == 0 || !(thickness > 0.0) {
            return None;
        }
        let thickness = (thickness / count as f64) as f32;
        let extent = hi - lo;
        let nodes = |h: f32| ((extent / h + 2.0 * PADDING).ceil() + 1.0).to_array().iter().product::<f32>();
        // a cell holds a path where it is thickest, a bundle that thins out gets no finer grid
        let mut h = 2.0 * ((thickest / paths.len() as f64) as f32 + radius);
        if !(h > 0.0) || !h.is_finite() || !extent.is_finite() {
            return None;
        }
        if nodes(h) > MAX_NODES as f32 {
            h = (extent.x * extent.y * extent.z / MAX_NODES as f32).cbrt().max(h);
            while nodes(h) > MAX_NODES as f32 {
                h *= 1.1;
            }
        }
        let size = ((extent / h + 2.0 * PADDING).ceil() + 1.0).as_ivec3();
        let total = (size.x * size.y * size.z) as usize;
        let fill = thickness / (thickness + radius * 0.5);
        Some(Density {
            origin: lo - Vec3::splat(PADDING * h),
            h,
            size,
            limit: PACKING * fill * fill,
            pressure: vec![0.0; total],
            scratch: vec![0.0; total],
        })
    }

    fn index(&self, c: IVec3) -> usize {
        (c.x + self.size.x * (c.y + self.size.y * c.z)) as usize
    }

    /// The offsets of the eight nodes of a cell from its first
    fn corners(&self) -> [usize; 8] {
        let (x, y) = (1, self.size.x as usize);
        let z = (self.size.x * self.size.y) as usize;
        [0, x, y, x + y, z, x + z, y + z, x + y + z]
    }

    /// Whether the nodes from `c - before` to `c + 1 + after` are all in the grid
    fn holds(&self, c: IVec3, before: i32, after: i32) -> bool {
        c.cmpge(IVec3::splat(before)).all() && (c + after + 1).cmplt(self.size).all()
    }

    /// The cell `q` is in and how far into it
    fn cell(&self, q: Vec3) -> (IVec3, Vec3) {
        let g = (q - self.origin) / self.h;
        let base = g.floor();
        (base.as_ivec3(), g - base)
    }

    /// Splats the segments of `paths` into `cells` as the share of a cell they fill and turns it
    /// into pressure. Gives the total pressure.
    fn splat(&self, paths: &[Vec<f32>], cells: &mut [f32]) -> f32 {
        cells.iter_mut().for_each(|c| *c = 0.0);
        let h = self.h;
        let corners = self.corners();
        let cell_volume = h * h * h;
        // the last spot whose cell still has all eight nodes in the grid
        let last = (self.size - 1).as_vec3();
        // segment by segment across all paths, a bundle of paths that run side by side then
        // splats into a few layers of the grid at a time instead of all of it for every path
        let longest = paths.iter().map(|p| p.len() / 4).max().unwrap_or(0);
        for i in 0..longest.saturating_sub(1) {
            for p in paths {
                if i + 1 >= p.len() / 4 {
                    continue;
                }
                let (a, b) = (point(p, i), point(p, i + 1));
                let length = a.distance(b);
                let t = 0.5 * (p[i * 4 + 3].abs() + p[i * 4 + 7].abs());
                if !(length > 0.0) || !length.is_finite() || t <= 0.0 {
                    continue;
                }
                let samples = (length / (SAMPLE_SPACING * h)).ceil().max(1.0) as usize;
                let share = length * PI * t * t / cell_volume / samples as f32;
                let (from, to) = ((a - self.origin) / h, (b - self.origin) / h);
                let advance = (to - from) / samples as f32;
                let mut g = from + advance * 0.5;
                for _ in 0..samples {
                    // a point pushed out of the grid takes no room any more
                    if g.cmpge(Vec3::ZERO).all() && g.cmplt(last).all() {
                        let c = g.as_ivec3();
                        let first = self.index(c);
                        for (offset, w) in corners.iter().zip(trilinear(g - c.as_vec3())) {
                            cells[first + offset] += share * w;
                        }
                    }
                    g += advance;
                }
            }
        }
        let mut total = 0.0f64;
        for c in cells.iter_mut() {
            *c = (*c - self.limit).max(0.0);
            total += *c as f64;
        }
        total as f32
    }

    /// Splats `paths` as the current state, gives its total pressure
    pub fn set(&mut self, paths: &[Vec<f32>]) -> f32 {
        let mut pressure = std::mem::take(&mut self.pressure);
        let total = self.splat(paths, &mut pressure);
        self.pressure = pressure;
        total
    }

    /// The total pressure `paths` would have, they become the current state if `keep` says so
    pub fn try_set(&mut self, paths: &[Vec<f32>], keep: impl FnOnce(f32) -> bool) -> Option<f32> {
        let mut scratch = std::mem::take(&mut self.scratch);
        let total = self.splat(paths, &mut scratch);
        let kept = keep(total);
        if kept {
            std::mem::swap(&mut self.pressure, &mut scratch);
        }
        self.scratch = scratch;
        kept.then_some(total)
    }

    /// How far a point at `q` moves down the slope of the pressure
    pub fn push(&self, q: Vec3, strength: f32) -> Vec3 {
        let (c, f) = self.cell(q);
        // the slope on a node reads the nodes to each side of it
        if !q.is_finite() || !self.holds(c, 1, 1) {
            return Vec3::ZERO;
        }
        let first = self.index(c);
        let (x, y, z) = (1, self.size.x as usize, (self.size.x * self.size.y) as usize);
        let p = &self.pressure;
        let mut slope = Vec3::ZERO;
        for (offset, w) in self.corners().iter().zip(trilinear(f)) {
            let n = first + offset;
            slope += Vec3::new(p[n + x] - p[n - x], p[n + y] - p[n - y], p[n + z] - p[n - z]) * w;
        }
        (slope * (-0.5 * STEP * strength)).clamp_length_max(MAX_STEP) * self.h
    }
}

/// Moves the points of `paths` down the pressure for up to `iterations` steps. `step` turns the
/// pushes of a path into its new points, scaled by its last argument. A step that doesn't lower
/// the total pressure is halved, and when that doesn't help either the bundle is as spread as it
/// gets, so the total pressure only ever falls. Gives the total pressure before and after every
/// step.
pub fn spread(
    paths: &mut Vec<Vec<f32>>,
    radius: f32,
    strength: f32,
    iterations: usize,
    mut step: impl FnMut(&[f32], &[Vec3], f32) -> Vec<f32>,
) -> Vec<f32> {
    let Some(mut grid) = Density::new(paths, radius) else {
        return Vec::new();
    };
    let mut history = vec![grid.set(paths)];
    for _ in 0..iterations {
        let current = *history.last().unwrap();
        if current <= 0.0 {
            break;
        }
        let mut pushes: Vec<Vec<Vec3>> = paths.iter().map(|p| vec![Vec3::ZERO; p.len() / 4]).collect();
        let longest = paths.iter().map(|p| p.len() / 4).max().unwrap_or(0);
        for i in 0..longest {
            for (p, push) in paths.iter().zip(&mut pushes) {
                if i < push.len() {
                    push[i] = grid.push(point(p, i), strength);
                }
            }
        }
        let mut scale = 1.0;
        let mut accepted = false;
        for _ in 0..=HALVINGS {
            let candidate: Vec<Vec<f32>> = paths.iter().zip(&pushes).map(|(p, push)| step(p, push, scale)).collect();
            if let Some(total) = grid.try_set(&candidate, |total| total < current) {
                *paths = candidate;
                history.push(total);
                accepted = true;
                break;
            }
            scale *= 0.5;
        }
        if !accepted {
            break;
        }
    }
    history
}
