//! Cracks between patches on a disc, like the cuticle of a cap that tears as the cap grows.
//!
//! The patches are the cells of a Voronoi diagram. Their middles sit in rows around the axis,
//! `spacing` apart and jittered by up to half of that, so they are about as large everywhere
//! on the disc. `stretch` measures distances along the radius shorter than around the axis, so
//! the patches grow long outward (radial fibrils) or around the axis (rings of scales).
//!
//! The edges wave a little, the place is moved by a smooth noise a tenth of the spacing far
//! before the nearest edge is found, and every edge gets its own width between half and one
//! and a half times `width`, so the cracks look torn rather than drawn.
//!
//! A crack follows the edge between two patches and is `width` wide. It goes over into the
//! patches on either side over `softness` of half the spacing, at least a cell of the pattern.
//! A crack narrower than that fades in as it opens, so a width that follows time lets the
//! cracks show up out of a closed cuticle. Where the width reaches the spacing only small
//! islands of the patches stay, like the scales of an *Agaricus augustus*.

mod lanes;
mod near;

use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{Ctx, Pattern};
use std::f32::consts::TAU;

nodarium_definition_file!("src/definition.json");

/// Cells of the pattern per spacing, and at least this many across the widest crack
const CELLS_PER_SPACING: f32 = 12.0;
const CELLS_PER_WIDTH: f32 = 2.0;
/// A finer cell than this share of the spacing isn't worth it, the crack is a line then
const FINEST_CELL: f32 = 1.0 / 24.0;
/// The pattern never gets more cells than this, a finer one on a large disc is coarsened
const MAX_CELLS: f32 = 1_500_000.0;
/// How far the middle of a patch moves from its place in the rows, as a share of the spacing
const JITTER: f32 = 0.45;
/// The width is read at this many steps from the axis to the edge of the disc
const WIDTH_STEPS: usize = 64;
/// Rows of patches searched on either side of a cell, enough for the jitter and the stretch
const ROW_REACH: f32 = 2.5;
/// How far the noise moves a place, and its wavelength, as shares of the spacing
const WAVE: f32 = 0.1;
const WAVE_LENGTH: f32 = 0.7;
/// Every edge is between 1 - this and 1 + this times the width wide
const WIDTH_SPREAD: f32 = 0.5;

pub struct Params {
    pub radius: f32,
    pub spacing: f32,
    /// The width at even steps from the axis (0) to the edge of the disc (1)
    pub width: Vec<f32>,
    pub stretch: f32,
    pub softness: f32,
    pub seed: i32,
}

impl Params {
    fn width_at(&self, along: f32) -> f32 {
        let w = &self.width;
        if w.len() < 2 {
            return w.first().copied().unwrap_or(0.0).max(0.0);
        }
        let x = along.clamp(0.0, 1.0) * (w.len() - 1) as f32;
        let k = (x as usize).min(w.len() - 2);
        (w[k] + (w[k + 1] - w[k]) * (x - k as f32)).max(0.0)
    }
}

fn hash(seed: i32, row: usize, k: usize, salt: u32) -> f32 {
    let mut h = (seed as u32).wrapping_mul(0x9e37_79b9)
        ^ (row as u32).wrapping_mul(0x8da6_b343)
        ^ (k as u32).wrapping_mul(0xd816_3841)
        ^ salt.wrapping_mul(0xcb1a_b31f);
    h ^= h >> 16;
    h = h.wrapping_mul(0x7feb_352d);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846c_a68b);
    h ^= h >> 16;
    (h >> 8) as f32 / (1u32 << 24) as f32
}

/// Smooth noise between -1 and 1 with features about 1 apart
fn noise(seed: i32, x: f32, y: f32, salt: u32) -> f32 {
    let (x0, y0) = (x.floor(), y.floor());
    let (tx, ty) = (x - x0, y - y0);
    let (sx, sy) = (tx * tx * (3.0 - 2.0 * tx), ty * ty * (3.0 - 2.0 * ty));
    let at = |i: f32, j: f32| hash(seed, (x0 + i) as i32 as usize, (y0 + j) as i32 as usize, salt);
    let top = at(0.0, 0.0) + (at(1.0, 0.0) - at(0.0, 0.0)) * sx;
    let bottom = at(0.0, 1.0) + (at(1.0, 1.0) - at(0.0, 1.0)) * sx;
    2.0 * (top + (bottom - top) * sy) - 1.0
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The middles of the patches, row by row around the axis, every row sorted by angle
struct Patches {
    /// How far apart the rows are along the radius
    row_gap: f32,
    /// How far apart the middles of a row are around the axis
    around: f32,
    root: f32,
    /// The angle, the place and a number of every middle
    rows: Vec<Vec<(f32, [f32; 2], u32)>>,
    /// How near to the axis and how far from it the middles of each row are
    low: Vec<f32>,
    high: Vec<f32>,
}

impl Patches {
    fn new(p: &Params) -> Patches {
        let root = p.stretch.sqrt();
        let (row_gap, around) = (p.spacing * root, p.spacing / root);
        let count = (p.radius / row_gap).ceil() as usize + 3;
        let (mut low, mut high) = (vec![f32::INFINITY; count], vec![0.0f32; count]);
        let rows = (0..count)
            .map(|row| {
                let r = row as f32 * row_gap;
                let n = ((TAU * r / around).round() as usize).max(1);
                let mut middles: Vec<(f32, [f32; 2], u32)> = (0..n)
                    .map(|k| {
                        let jitter = |salt: u32| JITTER * (2.0 * hash(p.seed, row, k, salt) - 1.0);
                        let r = (r + jitter(0) * row_gap).max(0.0);
                        low[row] = low[row].min(r);
                        high[row] = high[row].max(r);
                        let angle = (TAU * (k as f32 + 0.5 + jitter(1)) / n as f32).rem_euclid(TAU);
                        (angle, [r * angle.cos(), r * angle.sin()], (row << 16 ^ k) as u32)
                    })
                    .collect();
                middles.sort_by(|a, b| a.0.total_cmp(&b.0));
                middles
            })
            .collect();
        Patches { row_gap, around, root, rows, low, high }
    }

    /// How far a place is from the nearest edge between two patches, in a measure that is
    /// `stretch` times shorter along the radius than around the axis, and a number between 0
    /// and 1 for that edge
    fn edge_distance(&self, place: [f32; 2], stretch: f32) -> (f32, f32) {
        let radius = (place[0] * place[0] + place[1] * place[1]).sqrt();
        let (cos, sin) = if radius > 0.0 { (place[0] / radius, place[1] / radius) } else { (1.0, 0.0) };
        let angle = sin.atan2(cos).rem_euclid(TAU);
        let root = stretch.sqrt();
        let row = radius / self.row_gap;
        let from = (row - ROW_REACH).floor().max(0.0) as usize;
        let to = ((row + ROW_REACH).ceil() as usize).min(self.rows.len() - 1);
        let mut near: Vec<([f32; 2], u32)> = Vec::with_capacity(48);
        for middles in &self.rows[from..=to] {
            let n = middles.len();
            let mut add = |&(_, m, id): &(f32, [f32; 2], u32)| {
                let d = [m[0] - place[0], m[1] - place[1]];
                // along the radius and around the axis at the place
                let (along, across) = (d[0] * cos + d[1] * sin, d[1] * cos - d[0] * sin);
                near.push(([along / root, across * root], id));
            };
            if n <= 16 {
                middles.iter().for_each(&mut add);
                continue;
            }
            let k = middles.partition_point(|m| m.0 < angle);
            let reach = (ROW_REACH * self.around * n as f32 / (TAU * radius.max(self.around))).ceil() as usize + 2;
            let reach = reach.min(n / 2);
            for i in 0..=2 * reach {
                add(&middles[(k + n + i - reach) % n]);
            }
        }
        let length = |v: [f32; 2]| v[0] * v[0] + v[1] * v[1];
        let Some((first, id)) = near.iter().copied().min_by(|a, b| length(a.0).total_cmp(&length(b.0))) else {
            return (f32::INFINITY, 0.0);
        };
        let (distance, other) = near
            .iter()
            .filter(|m| m.1 != id)
            .map(|&(m, other)| {
                let gap = [m[0] - first[0], m[1] - first[1]];
                ((length(m) - length(first)) / (2.0 * length(gap).sqrt().max(f32::EPSILON)), other)
            })
            .fold((f32::INFINITY, id), |a, b| if b.0 < a.0 { b } else { a });
        (distance, hash(0, id.min(other) as usize, id.max(other) as usize, 7))
    }
}

pub fn cracks(p: &Params) -> Pattern {
    let outer = p.radius.max(0.0);
    let spacing = p.spacing.max(1e-5);
    let p = Params { spacing, stretch: p.stretch.clamp(0.05, 20.0), width: p.width.clone(), ..*p };
    if outer <= 0.0 {
        return Pattern { inner: 0.0, outer, spacing, rings: Vec::new() };
    }
    // narrower cracks than the widest fade in over a cell
    let widest = p.width.iter().copied().fold(0.0, f32::max);
    let mut cell = (spacing / CELLS_PER_SPACING).min(widest / CELLS_PER_WIDTH).max(spacing * FINEST_CELL);
    let area = std::f32::consts::PI * outer * outer;
    if area / (cell * cell) > MAX_CELLS {
        cell = (area / MAX_CELLS).sqrt();
    }
    let rings = ((outer / cell).ceil() as usize + 1).max(2);
    if p.width.iter().all(|&w| w <= 0.0) {
        let rings = (0..rings).map(|i| vec![0.0; ring_cells(outer * i as f32 / (rings - 1) as f32, cell)]).collect();
        return Pattern { inner: 0.0, outer, spacing, rings };
    }
    let patches = Patches::new(&p);
    let mut near = near::Near::new(&patches, spacing);
    let blur = (p.softness.clamp(0.0, 1.0) * 0.5 * spacing).max(cell);
    let wave_length = WAVE_LENGTH * spacing;
    let rings = (0..rings)
        .map(|i| {
            let r = outer * i as f32 / (rings - 1) as f32;
            let width = p.width_at(r / outer);
            let half = 0.5 * width;
            // a crack narrower than its blur fades in as it opens
            let fade = (width / blur).min(1.0);
            let n = ring_cells(r, cell);
            (0..n)
                .map(|j| {
                    if fade <= 0.0 {
                        return 0.0;
                    }
                    let angle = nodarium_utils::ring_angle(j, n);
                    let (x, y) = (r * angle.cos(), r * angle.sin());
                    let wave = |salt: u32| WAVE * spacing * noise(p.seed, x / wave_length, y / wave_length, salt);
                    let (e, edge) = near.edge_distance(&patches, [x + wave(1), y + wave(2)], p.stretch);
                    let half = half * (1.0 + WIDTH_SPREAD * (2.0 * edge - 1.0));
                    fade * (1.0 - smoothstep(half, half + blur, e))
                })
                .collect()
        })
        .collect();
    Pattern { inner: 0.0, outer, spacing, rings }
}

fn ring_cells(radius: f32, cell: f32) -> usize {
    ((TAU * radius / cell).round() as usize).max(3)
}

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let mut ctx = Ctx::default();
    let radius = ctx.float(args[0]);
    let spacing = ctx.float(args[1]);
    let width = ctx.float_along(args[2], (0..=WIDTH_STEPS).map(|i| i as f32 / WIDTH_STEPS as f32));
    let params = Params { radius, spacing, width, stretch: ctx.float(args[3]), softness: ctx.float(args[4]), seed: nodarium_utils::int(args[5]) };
    cracks(&params).encode()
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodarium_utils::encode_float;

    fn params(width: f32) -> Params {
        Params { radius: 0.03, spacing: 0.004, width: vec![width; 2], stretch: 1.0, softness: 0.0, seed: 3 }
    }

    fn share_in_cracks(p: &Pattern, from: f32, to: f32) -> f32 {
        let (mut on, mut all) = (0.0, 0.0);
        for (i, ring) in p.rings.iter().enumerate() {
            let r = p.radius(i);
            if r < from * p.outer || r > to * p.outer {
                continue;
            }
            on += ring.iter().filter(|&&v| v > 0.5).count() as f32;
            all += ring.len() as f32;
        }
        on / all
    }

    /// The length of the runs of patch (value under 0.5) along a ring, in metres
    fn patch_runs(p: &Pattern, ring: usize) -> Vec<f32> {
        let values = &p.rings[ring];
        let n = values.len();
        let cell = TAU * p.radius(ring) / n as f32;
        let Some(start) = (0..n).find(|&j| values[j] > 0.5) else {
            return Vec::new();
        };
        let mut runs = Vec::new();
        let mut run = 0;
        for k in 1..=n {
            if values[(start + k) % n] <= 0.5 {
                run += 1;
            } else if run > 0 {
                runs.push(run as f32 * cell);
                run = 0;
            }
        }
        runs
    }

    #[test]
    fn no_width_no_cracks() {
        let p = cracks(&params(0.0));
        assert!(!p.rings.is_empty());
        assert!(p.rings.iter().flatten().all(|&v| v == 0.0));
    }

    #[test]
    fn the_cracks_take_more_of_the_disc_as_they_widen() {
        let shares: Vec<f32> = [0.0002, 0.0006, 0.0015, 0.003].iter().map(|&w| share_in_cracks(&cracks(&params(w)), 0.0, 1.0)).collect();
        for pair in shares.windows(2) {
            assert!(pair[1] > pair[0] + 0.05, "{shares:?}");
        }
        // narrow cracks between large patches, wide ones leave small islands
        assert!(shares[1] < 0.4, "{shares:?}");
        assert!(shares[3] > 0.7, "{shares:?}");
        assert!(cracks(&params(0.003)).rings.iter().flatten().any(|&v| v < 0.5));
    }

    #[test]
    fn the_patches_are_about_spacing_across_everywhere() {
        let p = cracks(&params(0.0004));
        for ring in [p.rings.len() / 3, p.rings.len() - 1] {
            let runs = patch_runs(&p, ring);
            let mean = runs.iter().sum::<f32>() / runs.len() as f32;
            assert!((0.4..1.2).contains(&(mean / 0.004)), "patches {mean} long around ring {ring}");
        }
    }

    #[test]
    fn the_width_is_read_from_the_axis_to_the_edge() {
        // closed at the axis, open at the margin
        let p = cracks(&Params { width: vec![0.0, 0.0, 0.002], ..params(0.0) });
        let (inner, outer) = (share_in_cracks(&p, 0.0, 0.4), share_in_cracks(&p, 0.8, 1.0));
        assert_eq!(inner, 0.0);
        assert!(outer > 0.3, "{outer} of the outer ring is cracks");
    }

    #[test]
    fn stretch_makes_the_patches_run_outward() {
        let around = |stretch: f32| {
            let p = cracks(&Params { stretch, ..params(0.0004) });
            let runs = patch_runs(&p, p.rings.len() * 2 / 3);
            runs.iter().sum::<f32>() / runs.len() as f32
        };
        // long patches along the radius are narrow around the axis
        assert!(around(3.0) < 0.75 * around(1.0), "{} {}", around(3.0), around(1.0));
        assert!(around(0.33) > 1.3 * around(1.0), "{} {}", around(0.33), around(1.0));
    }

    #[test]
    fn soft_cracks_round_off_the_patches() {
        let sharp = cracks(&params(0.0008));
        let soft = cracks(&Params { softness: 1.0, ..params(0.0008) });
        let between = |p: &Pattern| p.rings.iter().flatten().filter(|&&v| v > 0.05 && v < 0.95).count() as f32;
        assert!(between(&soft) > 3.0 * between(&sharp));
        assert!(soft.rings.iter().flatten().all(|v| (0.0..=1.0).contains(v)));
    }

    #[test]
    fn deterministic_and_the_seed_changes_it() {
        assert_eq!(cracks(&params(0.0006)), cracks(&params(0.0006)));
        assert_ne!(cracks(&params(0.0006)), cracks(&Params { seed: 4, ..params(0.0006) }));
    }

    #[test]
    fn reads_its_inputs() {
        let out = __nodarium_execute_inner(&[
            &[encode_float(0.03)],
            &[encode_float(0.004)],
            &[encode_float(0.0006)],
            &[encode_float(1.0)],
            &[encode_float(0.0)],
            &[3],
        ]);
        let expected = cracks(&Params { width: vec![0.0006; WIDTH_STEPS + 1], ..params(0.0006) });
        assert_eq!(Pattern::decode(&out), Some(expected));
    }

    #[test]
    fn the_defaults_are_the_node_defaults() {
        let def: serde_json::Value = serde_json::from_str(include_str!("definition.json")).unwrap();
        let inputs = def["inputs"].as_object().unwrap();
        assert!(inputs.len() <= 8);
        assert_eq!(inputs["width"]["unit"], "m");
    }
}

#[cfg(test)]
mod debug {
    use super::*;
    use std::io::Write;

    /// `CRACKS_DEBUG=dir cargo test --release -p myzel_cracks debug -- --ignored` writes the
    /// pattern as a picture, cracks dark
    #[test]
    #[ignore]
    fn dump() {
        let dir = std::env::var("CRACKS_DEBUG").unwrap_or_else(|_| ".".into());
        let get = |k: &str, d: f32| std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d);
        let (inner, outer) = (get("WIDTH_INNER", 0.0006), get("WIDTH", 0.0006));
        let p = Params {
            radius: get("RADIUS", 0.03),
            spacing: get("SPACING", 0.003),
            width: (0..=WIDTH_STEPS).map(|i| inner + (outer - inner) * i as f32 / WIDTH_STEPS as f32).collect(),
            stretch: get("STRETCH", 1.0),
            softness: get("SOFTNESS", 0.2),
            seed: get("SEED", 3.0) as i32,
        };
        let t = std::time::Instant::now();
        let pattern = cracks(&p);
        println!("{} rings in {:?}", pattern.rings.len(), t.elapsed());
        let size = 600;
        let mut out = Vec::with_capacity(size * size);
        for y in 0..size {
            for x in 0..size {
                let px = ((x as f32 + 0.5) / size as f32 * 2.0 - 1.0) * pattern.outer;
                let py = ((y as f32 + 0.5) / size as f32 * 2.0 - 1.0) * pattern.outer;
                let v = pattern.sample(py.atan2(px).rem_euclid(TAU), (px * px + py * py).sqrt());
                out.push(v.map_or(128, |v| (255.0 - v * 255.0) as u8));
            }
        }
        let name = std::env::var("NAME").unwrap_or_else(|_| "cracks".into());
        let mut f = std::fs::File::create(format!("{dir}/{name}.pgm")).unwrap();
        write!(f, "P5\n{size} {size}\n255\n").unwrap();
        f.write_all(&out).unwrap();
    }
}
