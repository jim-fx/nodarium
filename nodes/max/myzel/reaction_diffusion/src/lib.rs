//! A stripe pattern from a reaction-diffusion model on a disc that grows outward.
//!
//! The disc is a grid of rings around the axis, from `inner_radius` to `radius`. Every ring has
//! as many cells as fit around it, so a cell is about as wide on every ring. The pattern starts
//! on the inner ring as evenly spaced stripes and grows outward one ring at a time: a new ring
//! starts as a copy of the ring inside it and settles for `steps` steps together with the few
//! rings before it, the rings further in stay as they are. The stripes run outward along the
//! rings. As the circumference grows they drift apart until a gap gets too wide, then a new
//! stripe starts in it or a stripe forks, so the stripes stay about `spacing` apart from the
//! inner ring to the outer one. That's how gills get shorter gills (lamellulae) between them
//! towards the margin, and how stripes fork on a growing domain (Kondo & Miura 2010).
//!
//! The model is an activator-inhibitor pair with linear reactions and a saturating synthesis,
//! the kind of model Kondo & Miura (2010) use for their figures. The saturation makes stripes of
//! even width that don't break up into spots. The inhibitor spreads 6 times faster than the
//! activator. Diffusion along the radius is up to 9 times as fast as around the axis
//! (`alignment`), which keeps the stripes from winding.

use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{Ctx, Pattern};
use std::f32::consts::TAU;

nodarium_definition_file!("src/definition.json");

/// Cells per `spacing` around a ring. The model's own stripes are about 6.6 cells apart, the
/// stripes on the growing disc settle at about this many cells apart as gaps widen before a new
/// stripe starts.
const CELLS_PER_SPACING: f32 = 9.8;
/// The grid never gets more cells than this, a finer spacing on a large disc is coarsened
const MAX_CELLS: f32 = 1_500_000.0;
/// Rings that settle together, the ones further in are fixed
const BAND: usize = 4;
/// At alignment 1 diffusion along the radius is this many times as fast as around the axis,
/// squared. The rings are this many cells apart, so a step costs the same.
const MAX_STRETCH: f32 = 3.0;
/// At irregularity 1 the gaps between the stripes on the inner ring are between 0.7 and 1.3
/// spacings, about what they are further out
const GAP_VARIATION: f32 = 0.3;

// The model, in cells and steps: u activates itself and v, v inhibits u. Both decay at `DECAY`
// and are made at a rate that is linear in u and v, clamped to [0, 2 DECAY], so both settle at
// 1 when evenly spread and between 0 and 2 in a stripe.
const DU: f32 = 1.0 / 30.0;
const DV: f32 = 0.2;
const DECAY: f32 = 0.1;
/// How much u makes more u and less v takes away, and how much u makes v
const U_FROM_U: f32 = 0.2;
const U_FROM_V: f32 = 0.12;
const V_FROM_U: f32 = 0.2;
const V_FROM_V: f32 = 0.1;
const DT: f32 = 0.9 / (4.0 * DV);

fn hash(seed: i32, ring: usize, cell: usize) -> f32 {
    let mut h = (seed as u32).wrapping_mul(0x9e37_79b9)
        ^ (ring as u32).wrapping_mul(0x8da6_b343)
        ^ (cell as u32).wrapping_mul(0xd816_3841);
    h ^= h >> 16;
    h = h.wrapping_mul(0x7feb_352d);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846c_a68b);
    h ^= h >> 16;
    (h >> 8) as f32 / (1u32 << 24) as f32
}

#[derive(Clone, Copy, Debug)]
pub struct Params {
    pub radius: f32,
    pub inner_radius: f32,
    pub spacing: f32,
    pub alignment: f32,
    pub irregularity: f32,
    pub steps: usize,
    pub seed: i32,
}

/// Where a cell reads its neighbour on the next ring in or out: between two cells, `t` of the
/// way from the first to the second
#[derive(Clone, Copy)]
struct Across {
    a: u32,
    b: u32,
    t: f32,
}

struct Grid {
    /// Radius of every ring, in cells
    radius: Vec<f32>,
    start: Vec<usize>,
    count: Vec<usize>,
    ring_of: Vec<u32>,
    left: Vec<u32>,
    right: Vec<u32>,
    inward: Vec<Across>,
    outward: Vec<Across>,
    /// Weights of the neighbours around the ring, out and in
    around: Vec<f32>,
    out: Vec<f32>,
    inn: Vec<f32>,
}

fn across(start: usize, count: usize, angle: f32) -> Across {
    let x = angle / TAU * count as f32 - 0.5;
    let x0 = x.floor();
    let j = (x0 as i64).rem_euclid(count as i64) as usize;
    Across { a: (start + j) as u32, b: (start + (j + 1) % count) as u32, t: x - x0 }
}

impl Grid {
    /// Rings `gap` cells apart from `inner` to `outer` (in cells), with diffusion along the radius
    /// `stretch²` times as fast as around the axis
    fn new(inner: f32, outer: f32, rings: usize, stretch: f32) -> Grid {
        let gap = if rings > 1 { (outer - inner) / (rings - 1) as f32 } else { 1.0 };
        let radius: Vec<f32> = (0..rings).map(|i| inner + gap * i as f32).collect();
        let count: Vec<usize> = radius.iter().map(|r| ((TAU * r).round() as usize).max(3)).collect();
        let mut start = Vec::with_capacity(rings);
        let mut total = 0;
        for &c in &count {
            start.push(total);
            total += c;
        }
        let mut grid = Grid {
            radius,
            start,
            count,
            ring_of: Vec::with_capacity(total),
            left: Vec::with_capacity(total),
            right: Vec::with_capacity(total),
            inward: Vec::with_capacity(total),
            outward: Vec::with_capacity(total),
            around: Vec::with_capacity(total),
            out: Vec::with_capacity(total),
            inn: Vec::with_capacity(total),
        };
        let radial = stretch * stretch / (gap * gap);
        for i in 0..rings {
            let (s, n, r) = (grid.start[i], grid.count[i], grid.radius[i]);
            let width = TAU * r / n as f32;
            // tiny rings near the axis have narrow cells, kept stable
            let around = (1.0 / (width * width)).min(1.5);
            let spread = if r > 0.0 { gap / (2.0 * r) } else { 1.0 };
            for j in 0..n {
                let angle = TAU * (j as f32 + 0.5) / n as f32;
                let me = Across { a: (s + j) as u32, b: (s + j) as u32, t: 0.0 };
                grid.ring_of.push(i as u32);
                grid.left.push((s + (j + n - 1) % n) as u32);
                grid.right.push((s + (j + 1) % n) as u32);
                grid.inward.push(if i > 0 { across(grid.start[i - 1], grid.count[i - 1], angle) } else { me });
                grid.outward.push(if i + 1 < rings { across(grid.start[i + 1], grid.count[i + 1], angle) } else { me });
                grid.around.push(around);
                grid.out.push(radial * (1.0 + spread).min(2.0));
                grid.inn.push(if i > 0 { radial * (1.0 - spread).max(0.0) } else { 0.0 });
            }
        }
        grid
    }

    fn len(&self) -> usize {
        self.ring_of.len()
    }

    fn rings(&self) -> usize {
        self.count.len()
    }
}

fn react(u: f32, v: f32) -> (f32, f32) {
    let make_u = (DECAY + U_FROM_U * (u - 1.0) - U_FROM_V * (v - 1.0)).clamp(0.0, 2.0 * DECAY);
    let make_v = (DECAY + V_FROM_U * (u - 1.0) - V_FROM_V * (v - 1.0)).clamp(0.0, 2.0 * DECAY);
    (make_u - DECAY * u, make_v - DECAY * v)
}

struct Sim {
    grid: Grid,
    u: Vec<f32>,
    v: Vec<f32>,
    next_u: Vec<f32>,
    next_v: Vec<f32>,
}

impl Sim {
    /// One step of the rings `from..=to`. The rings inside stay as they are, the ones outside
    /// don't exist yet: the outermost ring has no neighbour out.
    fn step(&mut self, from: usize, to: usize) {
        let g = &self.grid;
        let (u, v) = (&self.u, &self.v);
        let lerp = |x: &[f32], c: Across| x[c.a as usize] + (x[c.b as usize] - x[c.a as usize]) * c.t;
        let (lo, hi) = (g.start[from], g.start[to] + g.count[to]);
        let outer = g.start[to];
        for c in lo..hi {
            let (uc, vc) = (u[c], v[c]);
            let (l, r) = (g.left[c] as usize, g.right[c] as usize);
            let inward = g.inward[c];
            let (mut lu, mut lv) = (
                g.around[c] * (u[l] + u[r] - 2.0 * uc) + g.inn[c] * (lerp(u, inward) - uc),
                g.around[c] * (v[l] + v[r] - 2.0 * vc) + g.inn[c] * (lerp(v, inward) - vc),
            );
            if c < outer {
                let outward = g.outward[c];
                lu += g.out[c] * (lerp(u, outward) - uc);
                lv += g.out[c] * (lerp(v, outward) - vc);
            }
            let (ru, rv) = react(uc, vc);
            self.next_u[c] = (uc + DT * (DU * lu + ru)).max(0.0);
            self.next_v[c] = (vc + DT * (DV * lv + rv)).max(0.0);
        }
        self.u[lo..hi].copy_from_slice(&self.next_u[lo..hi]);
        self.v[lo..hi].copy_from_slice(&self.next_v[lo..hi]);
    }
}

pub fn reaction_diffusion(p: &Params) -> Pattern {
    let outer = p.radius.max(0.0);
    let inner = p.inner_radius.clamp(0.0, outer);
    let alignment = p.alignment.clamp(0.0, 1.0);
    let irregularity = p.irregularity.clamp(0.0, 1.0);
    let stretch = 1.0 + (MAX_STRETCH - 1.0) * alignment;
    let mut spacing = p.spacing.max(1e-5);
    if outer <= 0.0 {
        return Pattern { inner, outer, spacing, rings: Vec::new() };
    }
    // a cell is spacing / CELLS_PER_SPACING wide, or wider on a disc that would need too many
    let area = std::f32::consts::PI * (outer * outer - inner * inner) / stretch;
    let mut cell = spacing / CELLS_PER_SPACING;
    if area / (cell * cell) > MAX_CELLS {
        cell = (area / MAX_CELLS).sqrt();
        spacing = cell * CELLS_PER_SPACING;
    }
    let (inner_cells, outer_cells) = (inner / cell, outer / cell);
    let rings = (((outer_cells - inner_cells) / stretch).ceil() as usize + 1).max(2);
    let grid = Grid::new(inner_cells, outer_cells, rings, stretch);
    let total = grid.len();
    let mut sim = Sim { grid, u: vec![1.0; total], v: vec![1.0; total], next_u: vec![0.0; total], next_v: vec![0.0; total] };

    // the inner ring: stripes with gaps between 1 ± GAP_VARIATION * irregularity times the
    // spacing, like further out where some gaps just got a new stripe and others are about to
    let n = sim.grid.count[0];
    let stripes = (TAU * sim.grid.radius[0] / CELLS_PER_SPACING).round().max(1.0) as usize;
    let gaps: Vec<f32> =
        (0..stripes).map(|k| 1.0 + GAP_VARIATION * irregularity * (2.0 * hash(p.seed, 0, n + k) - 1.0)).collect();
    let total: f32 = gaps.iter().sum();
    let mut edges = Vec::with_capacity(stripes + 1);
    let mut at = TAU * hash(p.seed, 0, n + stripes);
    for &gap in &gaps {
        edges.push(at);
        at += TAU * gap / total;
    }
    edges.push(at);
    for j in 0..n {
        let mut angle = TAU * (j as f32 + 0.5) / n as f32;
        if angle < edges[0] {
            angle += TAU;
        }
        let k = edges.partition_point(|&e| e <= angle).clamp(1, stripes) - 1;
        let phase = TAU * (angle - edges[k]) / (edges[k + 1] - edges[k]);
        let wave = 0.5 + 0.5 * phase.cos() + irregularity * (hash(p.seed, 0, j) - 0.5);
        sim.u[j] = 2.0 * wave.clamp(0.0, 1.0);
    }
    for _ in 0..p.steps {
        sim.step(0, 0);
    }
    // grow outward: every ring starts as the one inside it, with some noise
    for k in 1..rings {
        let s = sim.grid.start[k];
        for j in 0..sim.grid.count[k] {
            let c = s + j;
            let inward = sim.grid.inward[c];
            let lerp = |x: &[f32]| x[inward.a as usize] + (x[inward.b as usize] - x[inward.a as usize]) * inward.t;
            sim.u[c] = (lerp(&sim.u) + irregularity * (hash(p.seed, k, j) - 0.5)).max(0.0);
            sim.v[c] = lerp(&sim.v);
        }
        let from = (k + 1).saturating_sub(BAND);
        for _ in 0..p.steps {
            sim.step(from, k);
        }
    }
    // the last rings settle as long as every other ring did
    let from = rings.saturating_sub(BAND);
    for _ in 0..p.steps * (BAND - 1) {
        sim.step(from, rings - 1);
    }

    let g = &sim.grid;
    let rings = (0..g.rings())
        .map(|i| (g.start[i]..g.start[i] + g.count[i]).map(|c| (0.5 * sim.u[c]).clamp(0.0, 1.0)).collect())
        .collect();
    Pattern { inner, outer, spacing, rings }
}

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let mut ctx = Ctx::default();
    let params = Params {
        radius: ctx.float(args[0]),
        inner_radius: ctx.float(args[1]),
        spacing: ctx.float(args[2]),
        alignment: ctx.float(args[3]),
        irregularity: ctx.float(args[4]),
        steps: ctx.int(args[5]).clamp(1, 1000) as usize,
        seed: ctx.int(args[6]),
    };
    reaction_diffusion(&params).encode()
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodarium_utils::encode_float;

    fn params() -> Params {
        Params {
            radius: 0.03,
            inner_radius: 0.006,
            spacing: 0.0015,
            alignment: 0.7,
            irregularity: 0.5,
            steps: 60,
            seed: 3,
        }
    }

    /// Stripes around ring `i`: where the value rises over 0.5
    fn stripes(p: &Pattern, i: usize) -> usize {
        let r = &p.rings[i];
        (0..r.len()).filter(|&j| r[j] <= 0.5 && r[(j + 1) % r.len()] > 0.5).count()
    }

    fn spacing_at(p: &Pattern, i: usize) -> f32 {
        TAU * p.radius(i) / stripes(p, i).max(1) as f32
    }

    #[test]
    fn stripes_keep_their_spacing_from_the_inner_ring_to_the_outer_one() {
        // regular stripes all fork at the same radius, so before that the gaps on a ring grow
        // together: only their mean keeps the spacing
        for (alignment, irregularity, every_ring) in
            [(0.7, 0.5, true), (0.0, 0.5, true), (1.0, 1.0, true), (0.5, 0.25, false), (0.7, 0.0, false)]
        {
            for spacing in [0.001, 0.002] {
                let p = reaction_diffusion(&Params { alignment, irregularity, spacing, ..params() });
                let rings = p.rings.len();
                let mean = (0..rings).map(|i| spacing_at(&p, i)).sum::<f32>() / rings as f32;
                assert!(
                    (mean / spacing - 1.0).abs() < 0.2,
                    "alignment {alignment} irregularity {irregularity}: stripes {mean} apart, spacing {spacing}"
                );
                if !every_ring {
                    continue;
                }
                for i in 0..rings {
                    let s = spacing_at(&p, i);
                    assert!(
                        (s / spacing - 1.0).abs() < 0.25,
                        "alignment {alignment} irregularity {irregularity}: stripes {s} apart at {}, spacing {spacing}",
                        p.radius(i)
                    );
                }
            }
        }
    }

    #[test]
    fn more_stripes_at_the_margin_than_near_the_axis() {
        let p = reaction_diffusion(&params());
        let (first, last) = (stripes(&p, 0), stripes(&p, p.rings.len() - 1));
        // the circumference grows 5 times, the stripes about as much
        assert!(last as f32 > 3.5 * first as f32, "{first} stripes at the inner ring, {last} at the outer one");
        let count = |i: usize| stripes(&p, i);
        // they never thin out on the way
        for i in 1..p.rings.len() {
            assert!(count(i) + 2 >= count(i - 1), "{} stripes at ring {}, {} at the next", count(i - 1), i - 1, count(i));
        }
    }

    #[test]
    fn the_pattern_covers_the_disc() {
        let p = reaction_diffusion(&params());
        assert_eq!(p.inner, 0.006);
        assert_eq!(p.outer, 0.03);
        assert_eq!(p.spacing, 0.0015);
        assert!(p.rings.iter().flatten().all(|v| (0.0..=1.0).contains(v)));
        // stripes and gaps
        let values: Vec<f32> = p.rings.iter().flatten().copied().collect();
        let on = values.iter().filter(|&&v| v > 0.5).count() as f32 / values.len() as f32;
        assert!((0.3..0.7).contains(&on), "{on} of the disc is stripes");
        assert!(values.iter().any(|&v| v > 0.9) && values.iter().any(|&v| v < 0.1));
    }

    #[test]
    fn deterministic_and_the_seed_changes_it() {
        let a = reaction_diffusion(&params());
        assert_eq!(a, reaction_diffusion(&params()));
        assert_ne!(a, reaction_diffusion(&Params { seed: 4, ..params() }));
    }

    #[test]
    fn alignment_straightens_the_stripes() {
        // how much a stripe moves around the axis from ring to ring, as a share of the spacing
        let wander = |alignment: f32| {
            let p = reaction_diffusion(&Params { alignment, ..params() });
            let mut moved = 0.0;
            let mut samples = 0;
            for i in 0..p.rings.len() - 1 {
                let (r0, r1) = (p.radius(i), p.radius(i + 1));
                let gap = r1 - r0;
                for j in 0..64 {
                    let angle = TAU * j as f32 / 64.0;
                    let (a, b) = (p.ring_value(i, angle), p.ring_value(i + 1, angle));
                    moved += (a - b).abs() / gap;
                    samples += 1;
                }
            }
            moved / samples as f32
        };
        assert!(wander(1.0) < 0.8 * wander(0.0), "aligned {} vs winding {}", wander(1.0), wander(0.0));
    }

    #[test]
    fn a_huge_disc_gets_a_coarser_spacing_instead_of_endless_cells() {
        let p = reaction_diffusion(&Params { radius: 0.2, inner_radius: 0.0, spacing: 0.0002, steps: 1, ..params() });
        assert!(p.spacing > 0.0002);
        let cells: usize = p.rings.iter().map(|r| r.len()).sum();
        assert!(cells as f32 <= MAX_CELLS * 1.05, "{cells} cells");
    }

    #[test]
    fn reads_its_inputs() {
        let p = params();
        let out = __nodarium_execute_inner(&[
            &[encode_float(p.radius)],
            &[encode_float(p.inner_radius)],
            &[encode_float(p.spacing)],
            &[encode_float(p.alignment)],
            &[encode_float(p.irregularity)],
            &[p.steps as i32],
            &[p.seed],
        ]);
        assert_eq!(Pattern::decode(&out), Some(reaction_diffusion(&p)));
    }

    #[test]
    fn the_defaults_are_the_node_defaults() {
        let def: serde_json::Value = serde_json::from_str(include_str!("definition.json")).unwrap();
        let value = |key: &str| def["inputs"][key]["value"].as_f64().unwrap() as f32;
        assert_eq!(value("radius"), 0.03);
        assert_eq!(value("spacing"), 0.001);
        assert_eq!(value("alignment"), 0.85);
        assert_eq!(value("irregularity"), 0.5);
        assert_eq!(value("steps"), 60.0);
    }
}

#[cfg(test)]
mod debug {
    use super::*;
    use std::io::Write;

    /// `RD_DEBUG=dir cargo test --release -p myzel_reaction_diffusion debug -- --ignored --nocapture`
    /// prints the spacing on every ring and writes the pattern as a picture
    #[test]
    #[ignore]
    fn dump() {
        let dir = std::env::var("RD_DEBUG").unwrap_or_else(|_| ".".into());
        let get = |k: &str, d: f32| std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d);
        let p = Params {
            radius: get("RADIUS", 0.03),
            inner_radius: get("INNER", 0.006),
            spacing: get("SPACING", 0.0015),
            alignment: get("ALIGNMENT", 0.7),
            irregularity: get("IRREGULARITY", 0.5),
            steps: get("STEPS", 60.0) as usize,
            seed: get("SEED", 3.0) as i32,
        };
        let t = std::time::Instant::now();
        let pattern = reaction_diffusion(&p);
        println!("{:?} in {:?}, {} rings", p, t.elapsed(), pattern.rings.len());
        let mut line = String::new();
        for i in 0..pattern.rings.len() {
            let r = &pattern.rings[i];
            let n = (0..r.len()).filter(|&j| r[j] <= 0.5 && r[(j + 1) % r.len()] > 0.5).count();
            line += &format!("{:.1}:{:.2} ", pattern.radius(i) * 1000.0, TAU * pattern.radius(i) / n.max(1) as f32 / p.spacing);
        }
        println!("{line}");
        let size = 600;
        let mut out = Vec::with_capacity(size * size);
        for y in 0..size {
            for x in 0..size {
                let px = ((x as f32 + 0.5) / size as f32 * 2.0 - 1.0) * pattern.outer;
                let py = ((y as f32 + 0.5) / size as f32 * 2.0 - 1.0) * pattern.outer;
                let v = pattern.sample(py.atan2(px).rem_euclid(TAU), (px * px + py * py).sqrt());
                out.push(v.map_or(128, |v| (v * 255.0) as u8));
            }
        }
        let name = std::env::var("NAME").unwrap_or_else(|_| "pattern".into());
        let words: Vec<String> = pattern.encode().iter().map(|w| w.to_string()).collect();
        std::fs::write(format!("{dir}/{name}.words"), words.join(" ")).unwrap();
        let mut f = std::fs::File::create(format!("{dir}/{name}.pgm")).unwrap();
        write!(f, "P5\n{size} {size}\n255\n").unwrap();
        f.write_all(&out).unwrap();
    }
}
