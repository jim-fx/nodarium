use glam::{Vec2, Vec3};
use std::f32::consts::PI;
use nodarium_utils::{decode_float, encode_float};

use crate::noise::value_noise;

pub const PATTERN_TAG: i32 = 15;

/// A scalar pattern over a ring from `inner` to `radius`, sampled on a square grid of `size` x
/// `size` cells centred on it. Values are in 0..1, 0 inside the inner radius.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pattern {
    pub size: usize,
    /// Half the side of the square the grid spans, centred on the disc
    pub extent: f32,
    pub radius: f32,
    pub inner: f32,
    /// The wavelength the pattern was made with, its stripes are about this far apart
    pub spacing: f32,
    pub values: Vec<f32>,
}

impl Pattern {
    pub fn encode(&self) -> Vec<i32> {
        let mut words = Vec::with_capacity(6 + self.values.len());
        words.extend([PATTERN_TAG, self.size as i32]);
        words.extend([self.extent, self.radius, self.inner, self.spacing].map(encode_float));
        words.extend(self.values.iter().map(|&v| encode_float(v)));
        words
    }

    /// An empty pattern for anything that is not one, so an unconnected input gives no gills
    pub fn decode(words: &[i32]) -> Pattern {
        if words.len() < 6 || words[0] != PATTERN_TAG {
            return Pattern::default();
        }
        let size = words[1].max(0) as usize;
        let values: Vec<f32> = words[6..].iter().map(|&w| decode_float(w)).collect();
        if values.len() != size * size {
            return Pattern::default();
        }
        let [extent, radius, inner, spacing] = [2, 3, 4, 5].map(|i| decode_float(words[i]));
        Pattern { size, extent, radius, inner, spacing, values }
    }

    pub fn is_empty(&self) -> bool {
        self.size < 2 || self.extent <= 0.0
    }

    fn cell(&self) -> f32 {
        2.0 * self.extent / self.size as f32
    }

    /// The value at a point of the plane, bilinear between the cell centres
    pub fn sample(&self, p: Vec2) -> f32 {
        if self.is_empty() {
            return 0.0;
        }
        let g = (p + Vec2::splat(self.extent)) / self.cell() - Vec2::splat(0.5);
        let max = (self.size - 1) as f32;
        let g = g.clamp(Vec2::ZERO, Vec2::splat(max));
        let (x0, y0) = (g.x.floor() as usize, g.y.floor() as usize);
        let (x1, y1) = ((x0 + 1).min(self.size - 1), (y0 + 1).min(self.size - 1));
        let (fx, fy) = (g.x - x0 as f32, g.y - y0 as f32);
        let v = |x: usize, y: usize| self.values[y * self.size + x];
        let a = v(x0, y0) + (v(x1, y0) - v(x0, y0)) * fx;
        let b = v(x0, y1) + (v(x1, y1) - v(x0, y1)) * fx;
        a + (b - a) * fy
    }

    /// The value at `radius` from the centre and `angle` around it
    pub fn sample_polar(&self, radius: f32, angle: f32) -> f32 {
        self.sample(Vec2::from_angle(angle) * radius)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReactionDiffusionParams {
    /// Outer radius of the ring the pattern grows on
    pub radius: f32,
    /// Inner radius, the pattern starts here and grows outwards
    pub inner: f32,
    /// Distance between neighbouring stripes
    pub spacing: f32,
    /// 0 grows a labyrinth, 1 clean stripes from the centre outwards
    pub alignment: f32,
    /// 0 keeps stripes even, 1 lets their width, direction and forking vary a lot
    pub irregularity: f32,
    pub seed: u32,
    /// Simulation steps, more let the pattern settle further
    pub steps: usize,
}

impl Default for ReactionDiffusionParams {
    fn default() -> Self {
        ReactionDiffusionParams {
            radius: 0.025,
            inner: 0.006,
            spacing: 0.002,
            alignment: 0.8,
            irregularity: 0.4,
            seed: 0,
            steps: 3000,
        }
    }
}

/// Gray-Scott constants in the stripe (labyrinth) regime
const FEED: f32 = 0.03;
const KILL: f32 = 0.058;
const DIFFUSION_U: f32 = 0.07;
const DIFFUSION_V: f32 = 0.035;
/// Stripe wavelength in cells, measured on the ring. Aligned stripes keep further apart.
const WAVELENGTH_CELLS: f32 = 8.0;
const WAVELENGTH_ALIGNED: f32 = 2.5;
/// At full alignment the substrate diffuses this much slower along the radius than around it.
/// Lateral inhibition is then weaker along the radius, so the pattern only breaks up around it
/// and the stripes run outwards.
const RADIAL_DIFFUSION: f32 = 0.4;
/// How far irregular stripes turn off the radius, in radians
const MAX_TURN: f32 = 1.0;
/// How much irregular patches lose their direction, where they do stripes join up
const LOOSE_PATCHES: f32 = 1.5;
/// How much irregularity varies the feed, a richer feed makes wider stripes
const FEED_VARIATION: f32 = 0.15;
/// Radius of a spot of activator that starts a stripe, in wavelengths
const SPOT_RADIUS: f32 = 0.3;
/// Spots per square wavelength where the ring grows: always, with irregularity and without
/// alignment. A spot in a gap starts a new stripe, one next to a stripe makes it fork.
const SPOTS: f32 = 0.4;
const SPOTS_IRREGULAR: f32 = 0.6;
const SPOTS_UNALIGNED: f32 = 0.5;
/// The grid never gets more cells along a side than this, wider rings get coarser cells
pub const MAX_PATTERN_SIZE: usize = 512;
/// Share of the steps over which the ring grows from the inner radius to the outer one
const GROWTH_SHARE: f32 = 0.7;

/// A hash of `i` in [-1, 1]
fn hash(i: u32, seed: u32) -> f32 {
    let mut h = i.wrapping_mul(0x8da6_b343) ^ seed.wrapping_mul(0x9e37_79b9);
    h ^= h >> 16;
    h = h.wrapping_mul(0x7feb_352d);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846c_a68b);
    h ^= h >> 16;
    (h >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0
}

/// Grows a Gray-Scott pattern on a ring that widens from `inner` to `radius`. The substrate
/// diffuses slower along the radius than around it, so stripes line up from the centre outwards.
/// Where the ring widens and they drift apart, new stripes split off or start between them.
/// Without alignment the whole ring starts at once and grows a labyrinth.
pub fn reaction_diffusion(p: &ReactionDiffusionParams) -> Pattern {
    let radius = p.radius.max(0.0);
    let inner = p.inner.clamp(0.0, radius);
    let spacing = p.spacing.max(1e-6);
    if radius <= 0.0 || radius - inner <= 0.0 {
        return Pattern::default();
    }
    let alignment = p.alignment.clamp(0.0, 1.0);
    let irregularity = p.irregularity.clamp(0.0, 1.0);

    let (along, around) = (1.0 - (1.0 - RADIAL_DIFFUSION) * alignment, 1.0);
    let wavelength = WAVELENGTH_CELLS + WAVELENGTH_ALIGNED * alignment;
    // the ring grows a stripe further than the radius, so the stripes run out across the margin
    // instead of turning along it
    let outer = radius + spacing;
    let size = ((2.0 * outer * wavelength / spacing).ceil() as usize).clamp(8, MAX_PATTERN_SIZE);
    let cell = 2.0 * outer / size as f32;
    let n = size;

    let seed = p.seed;
    let noise = |q: Vec2, scale: f32, salt: u32| value_noise(Vec3::new(q.x / scale, q.y / scale, 0.37), seed ^ salt);

    let centre = |i: usize| -> Vec2 { Vec2::new((i % n) as f32 + 0.5, (i / n) as f32 + 0.5) * cell - Vec2::splat(outer) };
    let mut dist = vec![0.0f32; n * n];
    // diffusion tensor (xx, xy, yy) of the substrate and the feed of every cell
    let mut tensor = vec![[0.0f32; 3]; n * n];
    let mut feed = vec![FEED; n * n];
    for i in 0..n * n {
        let q = centre(i);
        dist[i] = q.length();
        let turn = irregularity * MAX_TURN * noise(q, 3.0 * spacing, 0x51ed);
        // in patches the stripes lose their direction and join up
        let loose = 1.0 - irregularity * LOOSE_PATCHES * noise(q, 4.0 * spacing, 0x1d2c).max(0.0);
        let along = 1.0 - (1.0 - along) * loose;
        let dir = Vec2::from_angle(q.y.atan2(q.x) + turn);
        let (c, s) = (dir.x, dir.y);
        tensor[i] = [around + (along - around) * c * c, (along - around) * c * s, around + (along - around) * s * s];
        // a richer feed makes wider stripes, a leaner one thinner, broken ones
        feed[i] = FEED * (1.0 + irregularity * FEED_VARIATION * noise(q, 2.5 * spacing, 0x7a31));
    }
    let max_d = tensor.iter().map(|t| t[0] + t[2] + 2.0 * t[1].abs()).fold(0.0, f32::max) * DIFFUSION_U;
    let dt = (0.45 / max_d).min(1.0);
    let steps = ((p.steps as f32) / dt).ceil() as usize;

    let mut u = vec![1.0f32; n * n];
    let mut v = vec![0.0f32; n * n];
    let mut active = vec![false; n * n];
    let (mut nu, mut nv) = (u.clone(), v.clone());

    // spots of the activator, `rate` of them per square wavelength between two radii. A spot
    // spans a few cells, a single cell would diffuse away.
    let mut seeded = 0u32;
    let spot = SPOT_RADIUS * spacing;
    let mut seed_band = |v: &mut [f32], u: &mut [f32], from: f32, to: f32, rate: f32| {
        let expected = rate * PI * (to * to - from * from) / (spacing * spacing);
        let count = (expected + 0.5 * (hash(seeded, seed ^ 0x2b1f) + 1.0)) as u32;
        for _ in 0..count {
            seeded += 1;
            let angle = PI * hash(seeded, seed ^ 0x3c6e);
            let r = (from * from + (to * to - from * from) * 0.5 * (hash(seeded, seed ^ 0x9a7d) + 1.0)).sqrt();
            let at = Vec2::from_angle(angle) * r;
            let g = (at + Vec2::splat(outer)) / cell;
            let reach = (spot / cell).ceil() as isize;
            for dy in -reach..=reach {
                for dx in -reach..=reach {
                    let (x, y) = (g.x as isize + dx, g.y as isize + dy);
                    if x < 0 || y < 0 || x >= n as isize || y >= n as isize {
                        continue;
                    }
                    let i = y as usize * n + x as usize;
                    if centre(i).distance(at) <= spot {
                        v[i] = 0.25;
                        u[i] = 0.5;
                    }
                }
            }
        }
    };
    let start = (inner + spacing).min(outer);
    let mut front = inner;
    let mut cells: Vec<usize> = Vec::new();
    // neighbours of every active cell, inactive ones are the cell itself so nothing flows out
    let mut around_of: Vec<[u32; 8]> = Vec::new();
    let grow = |front: f32, active: &mut [bool], cells: &mut Vec<usize>, around_of: &mut Vec<[u32; 8]>| {
        for i in 0..n * n {
            if !active[i] && dist[i] >= inner && dist[i] <= front {
                active[i] = true;
                cells.push(i);
            }
        }
        around_of.clear();
        around_of.extend(cells.iter().map(|&i| {
            let (x, y) = ((i % n) as isize, (i / n) as isize);
            [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)].map(|(dx, dy)| {
                let (xx, yy) = (x + dx, y + dy);
                let inside = xx >= 0 && yy >= 0 && xx < n as isize && yy < n as isize;
                let j = if inside { yy as usize * n + xx as usize } else { i };
                if active[j] { j as u32 } else { i as u32 }
            })
        }));
    };

    let grow_steps = (steps as f32 * GROWTH_SHARE * alignment) as usize;
    for step in 0..steps {
        if front < outer {
            let next = if grow_steps == 0 {
                outer
            } else {
                start + (outer - start) * (step as f32 / grow_steps as f32).min(1.0)
            };
            if next - front >= cell || next >= outer {
                // the first band is packed with spots, after that a few land where the ring grows
                let rate = if step == 0 {
                    2.0
                } else {
                    SPOTS + SPOTS_IRREGULAR * irregularity + SPOTS_UNALIGNED * (1.0 - alignment).powi(2)
                };
                seed_band(&mut v, &mut u, front, next, rate);
                grow(next, &mut active, &mut cells, &mut around_of);
                front = next;
            }
        }
        for (k, &i) in cells.iter().enumerate() {
            let a = &around_of[k];
            let t = &tensor[i];
            let lap = |f: &[f32], t: &[f32; 3]| -> f32 {
                let c = f[i];
                let xx = f[a[0] as usize] + f[a[1] as usize] - 2.0 * c;
                let yy = f[a[2] as usize] + f[a[3] as usize] - 2.0 * c;
                let xy = 0.25 * (f[a[4] as usize] - f[a[5] as usize] - f[a[6] as usize] + f[a[7] as usize]);
                t[0] * xx + 2.0 * t[1] * xy + t[2] * yy
            };
            let (uu, vv) = (u[i], v[i]);
            let uvv = uu * vv * vv;
            nu[i] = uu + dt * (DIFFUSION_U * lap(&u, t) - uvv + feed[i] * (1.0 - uu));
            nv[i] = vv + dt * (DIFFUSION_V * lap(&v, &[1.0, 0.0, 1.0]) + uvv - (feed[i] + KILL) * vv);
        }
        for &i in &cells {
            u[i] = nu[i];
            v[i] = nv[i];
        }
    }

    let max = v.iter().cloned().fold(0.0, f32::max);
    let values = (0..n * n).map(|i| if active[i] && max > 0.0 { (v[i] / max).clamp(0.0, 1.0) } else { 0.0 }).collect();
    Pattern { size: n, extent: outer, radius, inner, spacing, values }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::f32::consts::TAU;

    /// Writes the pattern as a greyscale image when `PATTERN_DUMP` names a folder
    pub fn dump(pattern: &Pattern, name: &str) {
        let Ok(dir) = std::env::var("PATTERN_DUMP") else { return };
        let n = pattern.size;
        let mut bytes = format!("P5\n{} {}\n255\n", n, n).into_bytes();
        bytes.extend(pattern.values.iter().map(|v| (v * 255.0) as u8));
        std::fs::write(format!("{}/{}.pgm", dir, name), bytes).unwrap();
    }

    /// Stripes a circle at `r` crosses
    pub fn stripes_at(p: &Pattern, r: f32) -> usize {
        let samples = 4096;
        let above: Vec<bool> = (0..samples).map(|k| p.sample_polar(r, TAU * k as f32 / samples as f32) > 0.5).collect();
        (0..samples).filter(|&k| above[k] && !above[(k + samples - 1) % samples]).count()
    }

    /// The mean distance between stripes over the outer part of the ring, where they run outwards
    pub fn measured_spacing(p: &Pattern) -> f32 {
        let rings: Vec<f32> = (0..8).map(|k| p.inner + (p.radius - p.inner) * (0.4 + 0.075 * k as f32)).collect();
        rings.iter().map(|&r| TAU * r / stripes_at(p, r).max(1) as f32).sum::<f32>() / rings.len() as f32
    }

    #[test]
    fn stripes_keep_the_spacing() {
        for (spacing, alignment) in [(0.002, 0.8), (0.0015, 1.0), (0.003, 0.5)] {
            let p = reaction_diffusion(&ReactionDiffusionParams { spacing, alignment, ..Default::default() });
            let measured = measured_spacing(&p);
            assert!(
                (measured - spacing).abs() < 0.2 * spacing,
                "stripes {} mm apart, asked for {} mm at alignment {}",
                measured * 1000.0,
                spacing * 1000.0,
                alignment
            );
        }
    }

    #[test]
    fn more_stripes_at_the_margin_than_at_the_stipe() {
        let p = reaction_diffusion(&ReactionDiffusionParams::default());
        let near = stripes_at(&p, p.inner + 0.1 * (p.radius - p.inner));
        let margin = stripes_at(&p, 0.95 * p.radius);
        assert!(margin as f32 > 1.8 * near as f32, "{} stripes at the stipe, {} at the margin", near, margin);
    }

    #[test]
    fn alignment_turns_a_labyrinth_into_stripes_from_the_centre() {
        // how much the pattern changes along the radius compared to around it
        let radial_share = |p: &Pattern| {
            let (mut along, mut across) = (0.0, 0.0);
            for k in 0..2000 {
                let angle = TAU * k as f32 / 2000.0;
                let r = p.inner + (p.radius - p.inner) * (0.3 + 0.6 * (k % 7) as f32 / 7.0);
                let h = 0.25 * p.spacing;
                let at = |r: f32, a: f32| p.sample_polar(r, a);
                along += (at(r + h, angle) - at(r - h, angle)).abs();
                across += (at(r, angle + h / r) - at(r, angle - h / r)).abs();
            }
            along / across
        };
        let labyrinth = radial_share(&reaction_diffusion(&ReactionDiffusionParams { alignment: 0.0, ..Default::default() }));
        let stripes = radial_share(&reaction_diffusion(&ReactionDiffusionParams { alignment: 1.0, ..Default::default() }));
        assert!(labyrinth > 0.6, "a labyrinth changes along the radius about as much as around it, {}", labyrinth);
        assert!(stripes < 0.35, "aligned stripes barely change along the radius, {}", stripes);
    }

    #[test]
    fn the_same_inputs_give_the_same_pattern() {
        let params = ReactionDiffusionParams { steps: 600, ..Default::default() };
        let a = reaction_diffusion(&params);
        assert_eq!(a, reaction_diffusion(&params));
        assert_ne!(a, reaction_diffusion(&ReactionDiffusionParams { seed: 1, ..params.clone() }));
        assert_eq!(Pattern::decode(&a.encode()), a);
        assert!(a.values.iter().all(|v| (0.0..=1.0).contains(v)));
        // nothing inside the inner radius
        assert_eq!(a.sample(Vec2::ZERO), 0.0);
    }

    #[test]
    fn anything_else_decodes_as_an_empty_pattern() {
        assert!(Pattern::decode(&[]).is_empty());
        assert!(Pattern::decode(&[11, 3, 0, 0]).is_empty());
        assert!(reaction_diffusion(&ReactionDiffusionParams { radius: 0.0, ..Default::default() }).is_empty());
    }

    #[test]
    fn dump_variants() {
        if std::env::var("PATTERN_DUMP").is_err() {
            return;
        }
        let variants: Vec<(f32, f32, u32)> = std::env::var("VARIANTS")
            .unwrap_or("0,0.4,0;0.5,0.4,0;0.8,0.4,0;1,0.4,0;0.8,0,0;0.8,1,0".into())
            .split(';')
            .map(|v| {
                let f: Vec<f32> = v.split(',').map(|x| x.parse().unwrap()).collect();
                (f[0], f[1], f[2] as u32)
            })
            .collect();
        let steps = std::env::var("STEPS").map(|s| s.parse().unwrap()).unwrap_or(ReactionDiffusionParams::default().steps);
        for (index, (alignment, irregularity, seed)) in variants.into_iter().enumerate() {
            let t = std::time::Instant::now();
            let p = reaction_diffusion(&ReactionDiffusionParams { alignment, irregularity, seed, steps, ..Default::default() });
            println!(
                "a{} i{}: {} cells, {:?}, spacing {:.2} mm (asked {:.2}), stripes inner {} outer {}",
                alignment,
                irregularity,
                p.size,
                t.elapsed(),
                measured_spacing(&p) * 1000.0,
                p.spacing * 1000.0,
                stripes_at(&p, p.inner + 0.15 * (p.radius - p.inner)),
                stripes_at(&p, p.radius * 0.97)
            );
            dump(&p, &format!("{}-rd-a{}-i{}-s{}", index, alignment, irregularity, seed));
        }
    }
}
