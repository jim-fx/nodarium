//! What the paths sense of each other: how much of the space around a spot they fill, blurred
//! to the range they sense at. The segments are splatted into a grid as the share of a cell they
//! fill, like the density of `avoid`, and blurred with a few box passes, which comes close to a
//! Gaussian. The cost depends on the size of the grid and the number of segments, not on how many
//! neighbours a path has, so a stipe packed with thousands of strands costs no more than a loose one.

use glam::{IVec3, Vec3};
use std::f32::consts::PI;

/// The grid never has more nodes than this, a larger body gets larger cells
const MAX_NODES: usize = 1 << 21;
/// Cells per range
const CELLS_PER_RANGE: f32 = 3.0;
/// Box passes per axis, each one cell to either side
const BLUR_PASSES: usize = 3;

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

pub struct Field {
    origin: Vec3,
    h: f32,
    size: IVec3,
    density: Vec<f32>,
    scratch: Vec<f32>,
    /// the width of the blur
    pub sigma: f32,
}

impl Field {
    /// A grid around `paths` for sensing at `range`, with room for every point to move `reach`
    pub fn new(paths: &[Vec<Vec3>], range: f32, reach: f32) -> Option<Field> {
        let (mut lo, mut hi) = (Vec3::MAX, Vec3::MIN);
        for q in paths.iter().flatten().filter(|q| q.is_finite()) {
            lo = lo.min(*q);
            hi = hi.max(*q);
        }
        if !lo.cmple(hi).all() || !(range > 0.0) {
            return None;
        }
        let mut h = range / CELLS_PER_RANGE;
        // an infinite reach or a cell that underflows to 0 would never shrink the grid below MAX_NODES
        if !(h > 0.0) || !h.is_finite() || !reach.is_finite() || !(hi - lo).is_finite() {
            return None;
        }
        let nodes = |h: f32| {
            let pad = 2.0 * (range + reach) + 3.0 * h;
            ((hi - lo + pad) / h).ceil() + 1.0
        };
        while nodes(h).to_array().iter().product::<f32>() > MAX_NODES as f32 {
            h *= 1.2;
        }
        let pad = range + reach + 1.5 * h;
        let size = nodes(h).as_ivec3();
        let total = (size.x * size.y * size.z) as usize;
        Some(Field {
            origin: lo - Vec3::splat(pad),
            h,
            size,
            density: vec![0.0; total],
            scratch: vec![0.0; total],
            sigma: h * (BLUR_PASSES as f32 * 2.0 / 3.0).sqrt(),
        })
    }

    fn index(&self, c: IVec3) -> usize {
        (c.x + self.size.x * (c.y + self.size.y * c.z)) as usize
    }

    fn corners(&self) -> [usize; 8] {
        let (x, y) = (1, self.size.x as usize);
        let z = (self.size.x * self.size.y) as usize;
        [0, x, y, x + y, z, x + z, y + z, x + y + z]
    }

    /// Splats the segments of `paths` with the radii `radius` and blurs them
    pub fn set(&mut self, paths: &[Vec<Vec3>], radius: &[&[f32]]) {
        self.splat(paths, radius);
        self.blur();
    }

    fn splat(&mut self, paths: &[Vec<Vec3>], radius: &[&[f32]]) {
        self.density.iter_mut().for_each(|d| *d = 0.0);
        let h = self.h;
        let corners = self.corners();
        let last = (self.size - 1).as_vec3();
        for (p, r) in paths.iter().zip(radius) {
            for i in 0..p.len().saturating_sub(1) {
                let (a, b) = (p[i], p[i + 1]);
                let length = a.distance(b);
                let t = 0.5 * (r[i] + r[i + 1]);
                if !(length > 0.0) || !length.is_finite() || t <= 0.0 {
                    continue;
                }
                let samples = (length / h).ceil().max(1.0) as usize;
                let share = length * PI * t * t / (h * h * h) / samples as f32;
                let (from, to) = ((a - self.origin) / h, (b - self.origin) / h);
                let advance = (to - from) / samples as f32;
                let mut g = from + advance * 0.5;
                for _ in 0..samples {
                    if g.cmpge(Vec3::ZERO).all() && g.cmplt(last).all() {
                        let c = g.as_ivec3();
                        let first = self.index(c);
                        for (offset, w) in corners.iter().zip(trilinear(g - c.as_vec3())) {
                            self.density[first + offset] += share * w;
                        }
                    }
                    g += advance;
                }
            }
        }
    }

    /// [`BLUR_PASSES`] box passes along every axis, a row along x at a time: along x within the row,
    /// along y and z from the rows before and after it, a row of zeros past the border
    fn blur(&mut self) {
        let [nx, ny, nz] = self.size.to_array().map(|s| s as usize);
        let rows = ny * nz;
        let zero = vec![0.0f32; nx];
        for (n, rows_per_step) in [(nx, 0), (ny, 1), (nz, ny)] {
            for _ in 0..BLUR_PASSES {
                std::mem::swap(&mut self.density, &mut self.scratch);
                let (from, to) = (&self.scratch, &mut self.density);
                for (r, to) in to.chunks_exact_mut(nx).enumerate() {
                    let here = &from[r * nx..(r + 1) * nx];
                    if rows_per_step == 0 {
                        box_along_row(to, here);
                        continue;
                    }
                    let k = r / rows_per_step % n;
                    let row = |r: usize| &from[r * nx..(r + 1) * nx];
                    let before = if k > 0 { row(r - rows_per_step) } else { &zero };
                    let after = if k + 1 < n && r + rows_per_step < rows { row(r + rows_per_step) } else { &zero };
                    box_across_rows(to, before, here, after);
                }
            }
        }
    }

    /// The blurred share of space the paths fill at `q` and its slope
    pub fn sample(&self, q: Vec3) -> (f32, Vec3) {
        let g = (q - self.origin) / self.h;
        let base = g.floor();
        let c = base.as_ivec3();
        if !q.is_finite() || !c.cmpge(IVec3::ONE).all() || !(c + 2).cmplt(self.size).all() {
            return (0.0, Vec3::ZERO);
        }
        let first = self.index(c);
        let (x, y, z) = (1, self.size.x as usize, (self.size.x * self.size.y) as usize);
        let d = &self.density;
        let (mut value, mut slope) = (0.0, Vec3::ZERO);
        for (offset, w) in self.corners().iter().zip(trilinear(g - base)) {
            let n = first + offset;
            value += d[n] * w;
            slope += Vec3::new(d[n + x] - d[n - x], d[n + y] - d[n - y], d[n + z] - d[n - z]) * w;
        }
        (value, slope * (0.5 / self.h))
    }
}

/// One box pass along a row, a zero past either end
#[cfg_attr(target_arch = "wasm32", target_feature(enable = "simd128"))]
fn box_along_row(to: &mut [f32], from: &[f32]) {
    let n = from.len();
    for k in 0..n {
        let before = if k > 0 { from[k - 1] } else { 0.0 };
        let after = if k + 1 < n { from[k + 1] } else { 0.0 };
        to[k] = (before + from[k] + after) / 3.0;
    }
}

/// One box pass across rows, `to[x] = (before[x] + here[x] + after[x]) / 3`, four samples at a time on
/// wasm, every lane as the scalar code does it
#[cfg_attr(target_arch = "wasm32", target_feature(enable = "simd128"))]
fn box_across_rows(to: &mut [f32], before: &[f32], here: &[f32], after: &[f32]) {
    let n = to.len().min(before.len()).min(here.len()).min(after.len());
    #[cfg(not(target_arch = "wasm32"))]
    let x = 0;
    #[cfg(target_arch = "wasm32")]
    let x = {
        let mut x = 0;
        use core::arch::wasm32::*;
        let third = f32x4_splat(3.0);
        while x + 4 <= n {
            // SAFETY: four samples of each, 16 bytes, v128 needs no alignment
            unsafe {
                let (b, h, a) = (
                    v128_load(before[x..x + 4].as_ptr().cast()),
                    v128_load(here[x..x + 4].as_ptr().cast()),
                    v128_load(after[x..x + 4].as_ptr().cast()),
                );
                v128_store(to[x..x + 4].as_mut_ptr().cast(), f32x4_div(f32x4_add(f32x4_add(b, h), a), third));
            }
            x += 4;
        }
        x
    };
    for x in x..n {
        to[x] = (before[x] + here[x] + after[x]) / 3.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The blur as it was before it went row by row: every sample finds its place on the axis by division
    fn blur_by_sample(field: &mut Field) {
        let size = field.size.to_array().map(|s| s as usize);
        let strides = [1, size[0], size[0] * size[1]];
        for axis in 0..3 {
            let (n, stride) = (size[axis], strides[axis]);
            for _ in 0..BLUR_PASSES {
                std::mem::swap(&mut field.density, &mut field.scratch);
                let (from, to) = (&field.scratch, &mut field.density);
                for (i, out) in to.iter_mut().enumerate() {
                    let k = i / stride % n;
                    let before = if k > 0 { from[i - stride] } else { 0.0 };
                    let after = if k + 1 < n { from[i + stride] } else { 0.0 };
                    *out = (before + from[i] + after) / 3.0;
                }
            }
        }
    }

    #[test]
    fn the_blur_by_rows_changes_no_bit() {
        let paths: Vec<Vec<Vec3>> = (0..7)
            .map(|p| {
                (0..12).map(|i| Vec3::new((p as f32 * 1.3).sin(), i as f32 * 0.21, (p as f32 * 0.7).cos() * i as f32 * 0.05)).collect()
            })
            .collect();
        let radius: Vec<Vec<f32>> = paths.iter().map(|p| (0..p.len()).map(|i| 0.02 + 0.01 * (i % 3) as f32).collect()).collect();
        let radius: Vec<&[f32]> = radius.iter().map(|r| r.as_slice()).collect();
        let bits = |v: &[f32]| v.iter().map(|v| v.to_bits()).collect::<Vec<_>>();
        for range in [0.05, 0.13, 0.4] {
            let mut field = Field::new(&paths, range, 0.1).unwrap();
            field.set(&paths, &radius);
            let blurred = field.density.clone();
            field.splat(&paths, &radius);
            blur_by_sample(&mut field);
            assert!(blurred.iter().any(|&d| d > 0.0));
            assert_eq!(bits(&blurred), bits(&field.density), "range {}", range);
        }
    }
}
