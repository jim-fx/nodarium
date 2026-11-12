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
//! The inner ring starts with stripes at their closest, about 0.7 spacings apart, like gills at the
//! stipe, and a new tier of stripes comes in about where the circumference has doubled, at 2, 4
//! and 8 inner radii (Fischer & Money 2010 measured 2.1, 3.8 and 6.5 stipe radii).
//!
//! The model is an activator-inhibitor pair with linear reactions and a saturating synthesis,
//! the Kondo-Asai kind (Painter, Maini & Othmer 1999). The saturation makes stripes of even
//! width that don't break up into spots. The inhibitor spreads 6 times faster than the
//! activator. How high the activator's synthesis may go decides how new stripes come in: at the
//! full ceiling they start between two others (lamellulae, `forking` 0), a lower one makes them
//! split off a stripe (forks, `forking` 1).
//!
//! `alignment` gives the two species different diffusion along the radius (Shoji et al. 2002):
//! an activator that spreads less along the radius than around the axis winds the stripes into
//! a labyrinth, an inhibitor that does straightens them out. The grid's rings are further apart
//! than its cells around the axis, with diffusion along the radius scaled to match, which only
//! saves cells and doesn't orient anything.

mod lanes;

use lanes::{Cell, F4};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{Ctx, Pattern};
use std::f32::consts::TAU;

nodarium_definition_file!("src/definition.json");

/// Cells per `spacing` around a ring. The model's own stripes are about 6.6 cells apart, the
/// stripes on the growing disc settle at about this many cells apart as gaps widen before a new
/// stripe starts.
const CELLS_PER_SPACING: f32 = 9.4;
/// The stripes on the inner ring are this many spacings apart, at their closest like gills at
/// the stipe, so the first new ones come in where the circumference has about doubled
const INNER_SPACING: f32 = 0.73;
/// The activator's synthesis ceiling, in DECAY, at forking 0 and how far forking 1 lowers it.
/// Over about 2.05 forks come back and the spacing drifts, at 2.33 no stripes start at all.
const U_CEILING: f32 = 2.0;
const FORKING_DROP: f32 = 0.35;
/// How much slower the activator (at alignment 0) or the inhibitor (at alignment 1) spreads
/// along the radius than the other one. Above 1 for either kills the pattern in whole sectors.
const RADIAL_SLOWDOWN: f32 = 0.5;
/// The grid never gets more cells than this, a finer spacing on a large disc is coarsened
const MAX_CELLS: f32 = 1_500_000.0;
/// Rings that settle together, the ones further in are fixed
const BAND: usize = 4;
/// At alignment 1 the rings are this many cells apart and diffusion along the radius is scaled
/// to match, so a step reaches as far with fewer rings.
const MAX_STRETCH: f32 = 3.0;
/// At irregularity 1 the gaps between the stripes on the inner ring are between 0.7 and 1.3
/// spacings, about what they are further out
const GAP_VARIATION: f32 = 0.3;

// The model, in cells and steps: u activates itself and v, v inhibits u. Both decay at `DECAY`
// and are made at a rate that is linear in u and v, clamped to [0, ceiling DECAY], so both
// settle at 1 when evenly spread and between 0 and the ceiling in a stripe.
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
    pub forking: f32,
    pub steps: usize,
    pub seed: i32,
}

struct Ring {
    /// Radius in cells
    radius: f32,
    /// Where the ring's first cell is in the grid. Before it is a copy of the ring's last cell,
    /// after its last one a copy of its first one and one more cell, so the neighbours around
    /// the ring and between two cells of the next ring never wrap, and the cells can be
    /// stepped two at a time.
    start: usize,
    count: usize,
    /// Weights of the neighbours around the ring, out and in
    around: f32,
    out: f32,
    inn: f32,
}

/// Where the cells read their neighbours on the next ring in or out: between cell `at` and the
/// one after it, `t` of the way. `t` is there twice, once for u and once for v.
struct Across {
    at: Vec<u32>,
    t: Vec<Cell>,
}

impl Across {
    fn new(len: usize) -> Across {
        Across { at: vec![0; len], t: vec![[0.0; 2]; len] }
    }

    /// Cell `c` reads `ring` at `angle`
    fn set(&mut self, c: usize, ring: &Ring, angle: f32) {
        let count = ring.count;
        let x = angle / TAU * count as f32 - 0.5;
        let x0 = x.floor();
        let j = (x0 as i64).rem_euclid(count as i64) as usize;
        self.at[c] = (ring.start + j) as u32;
        self.t[c] = [x - x0; 2];
    }

    /// u and v at where cell `c` reads its neighbour
    fn read(&self, x: &[Cell], c: usize) -> Cell {
        let (a, t) = (self.at[c] as usize, self.t[c][0]);
        let ([ua, va], [ub, vb]) = (x[a], x[a + 1]);
        [ua + (ub - ua) * t, va + (vb - va) * t]
    }

    /// Where the cells `from..from + 2 * count` read, two at a time
    fn pairs(&self, from: usize, count: usize) -> impl Iterator<Item = (&[u32; 2], &[Cell; 2])> {
        pairs(&self.at, from, count).iter().zip(pairs(&self.t, from, count))
    }
}

/// `x[from..from + 2 * count]` in pairs
fn pairs<T>(x: &[T], from: usize, count: usize) -> &[[T; 2]] {
    x[from..from + 2 * count].as_chunks().0
}

/// u and v at where two cells read their neighbours
#[inline]
#[cfg_attr(target_arch = "wasm32", target_feature(enable = "simd128"))]
fn read_pair(x: &[Cell], (at, t): (&[u32; 2], &[Cell; 2])) -> F4 {
    // every `at` is in the grid, the min only saves the bounds check
    let cells = |at: u32| -> &[Cell; 2] {
        let a = (at as usize).min(x.len() - 2);
        x[a..a + 2].try_into().unwrap()
    };
    let (a, b) = (F4::load(cells(at[0])), F4::load(cells(at[1])));
    let (first, second) = (F4::firsts(a, b), F4::seconds(a, b));
    first.add(second.sub(first).mul(F4::load(t)))
}

struct Grid {
    rings: Vec<Ring>,
    inward: Across,
    outward: Across,
    len: usize,
}

impl Grid {
    /// Rings `gap` cells apart from `inner` to `outer` (in cells), with diffusion along the radius
    /// `stretch²` times as fast as around the axis
    fn new(inner: f32, outer: f32, rings: usize, stretch: f32) -> Grid {
        let gap = if rings > 1 { (outer - inner) / (rings - 1) as f32 } else { 1.0 };
        let radial = stretch * stretch / (gap * gap);
        let mut len = 0;
        let rings: Vec<Ring> = (0..rings)
            .map(|i| {
                let r = inner + gap * i as f32;
                let count = ((TAU * r).round() as usize).max(3);
                let width = TAU * r / count as f32;
                let spread = if r > 0.0 { gap / (2.0 * r) } else { 1.0 };
                let ring = Ring {
                    radius: r,
                    start: len + 1,
                    count,
                    // tiny rings near the axis have narrow cells, kept stable
                    around: (1.0 / (width * width)).min(1.5),
                    out: radial * (1.0 + spread).min(2.0),
                    inn: if i > 0 { radial * (1.0 - spread).max(0.0) } else { 0.0 },
                };
                len += count + 3;
                ring
            })
            .collect();
        let (mut inward, mut outward) = (Across::new(len), Across::new(len));
        for (i, ring) in rings.iter().enumerate() {
            // and the cell after the last, which a ring with an odd count steps with it
            for j in 0..=ring.count {
                let angle = TAU * ((j % ring.count) as f32 + 0.5) / ring.count as f32;
                let c = ring.start + j;
                inward.set(c, &rings[i.saturating_sub(1)], angle);
                outward.set(c, &rings[(i + 1).min(rings.len() - 1)], angle);
            }
        }
        Grid { rings, inward, outward, len }
    }
}

struct Sim {
    grid: Grid,
    /// Diffusion along the radius of u and v, as a share of the grid's
    radial: Cell,
    /// The synthesis ceilings of u and v
    ceiling: Cell,
    /// u and v of every cell, and the next step's
    uv: Vec<Cell>,
    next: Vec<Cell>,
}

impl Sim {
    /// One step of ring `i`. With `INWARD` the ring reads the ring inside it, with `OUTWARD` the
    /// one outside it. Steps the cells two at a time.
    #[cfg_attr(target_arch = "wasm32", target_feature(enable = "simd128"))]
    fn step_ring<const INWARD: bool, const OUTWARD: bool>(&mut self, i: usize) {
        let g = &self.grid;
        let ring = &g.rings[i];
        let (s, n) = (ring.start, ring.count);
        let both = |c: Cell| F4::pair(c, c);
        let two = |x: f32| both([x; 2]);
        let (around, out, inn) = (two(ring.around), two(ring.out), two(ring.inn));
        let (radial, ceiling, rate) = (both(self.radial), both(self.ceiling), both([DU, DV]));
        let (from_u, from_v) = (both([U_FROM_U, V_FROM_U]), both([U_FROM_V, V_FROM_V]));
        let (zero, one, decay, dt) = (two(0.0), two(1.0), two(DECAY), two(DT));
        let uv = &self.uv[..];
        // a ring with an odd count steps the cell after its last one too, then pads it
        let count = n.div_ceil(2);
        let cells = pairs(uv, s, count).iter().zip(pairs(uv, s - 1, count)).zip(pairs(uv, s + 1, count));
        let next = self.next[s..s + 2 * count].as_chunks_mut().0;
        let across = g.inward.pairs(s, count).zip(g.outward.pairs(s, count));
        for (((here, left), right), (next, (inward, outward))) in cells.zip(next.iter_mut().zip(across)) {
            let (here, left, right) = (F4::load(here), F4::load(left), F4::load(right));
            let mut across = zero;
            if INWARD {
                across = inn.mul(read_pair(uv, inward).sub(here));
            }
            if OUTWARD {
                across = across.add(out.mul(read_pair(uv, outward).sub(here)));
            }
            let laplace = around.mul(left.add(right).sub(here.add(here))).add(radial.mul(across));
            let less = here.sub(one);
            let make = decay.add(from_u.mul(less.uu())).sub(from_v.mul(less.vv()));
            let react = make.at_least(zero).at_most(ceiling).sub(decay.mul(here));
            here.add(dt.mul(rate.mul(laplace).add(react))).positive().store(next);
        }
        self.next[s - 1] = self.next[s + n - 1];
        self.next[s + n] = self.next[s];
    }

    /// `steps` steps of the rings `from..=to`. The rings inside stay as they are, the ones
    /// outside don't exist yet: the outermost ring has no neighbour out.
    fn settle(&mut self, from: usize, to: usize, steps: usize) {
        for _ in 0..steps {
            for i in from..=to {
                match (i > 0, i < to) {
                    (false, false) => self.step_ring::<false, false>(i),
                    (false, true) => self.step_ring::<false, true>(i),
                    (true, false) => self.step_ring::<true, false>(i),
                    (true, true) => self.step_ring::<true, true>(i),
                }
            }
            std::mem::swap(&mut self.uv, &mut self.next);
        }
        // both buffers agree on the rings that are only read from now on
        let rings = &self.grid.rings;
        let (lo, hi) = (rings[from].start - 1, rings[to].start + rings[to].count + 2);
        self.next[lo..hi].copy_from_slice(&self.uv[lo..hi]);
    }

    /// Copies the first and the last cell of ring `i` around it
    fn pad(&mut self, i: usize) {
        let ring = &self.grid.rings[i];
        let (s, n) = (ring.start, ring.count);
        self.uv[s - 1] = self.uv[s + n - 1];
        self.uv[s + n] = self.uv[s];
    }
}

pub fn reaction_diffusion(p: &Params) -> Pattern {
    let outer = p.radius.max(0.0);
    let inner = p.inner_radius.clamp(0.0, outer);
    let alignment = p.alignment.clamp(0.0, 1.0);
    let irregularity = p.irregularity.clamp(0.0, 1.0);
    let stretch = 1.0 + (MAX_STRETCH - 1.0) * alignment;
    let mut spacing = p.spacing.max(1e-5);
    // a disc without area has no pattern
    if outer <= 0.0 || inner >= outer {
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
    let len = grid.len;
    let mut sim = Sim {
        grid,
        radial: [
            1.0 - RADIAL_SLOWDOWN * (1.0 - 2.0 * alignment).max(0.0),
            1.0 - RADIAL_SLOWDOWN * (2.0 * alignment - 1.0).max(0.0),
        ],
        ceiling: [(U_CEILING - FORKING_DROP * p.forking.clamp(0.0, 1.0)) * DECAY, 2.0 * DECAY],
        uv: vec![[1.0; 2]; len],
        next: vec![[1.0; 2]; len],
    };

    // the inner ring: stripes at their closest, with gaps between 1 ± GAP_VARIATION *
    // irregularity times that, so they don't all fork at the same radius
    let (s, n) = (sim.grid.rings[0].start, sim.grid.rings[0].count);
    let stripes = (TAU * sim.grid.rings[0].radius / (INNER_SPACING * CELLS_PER_SPACING)).round().max(1.0) as usize;
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
        sim.uv[s + j][0] = 2.0 * wave.clamp(0.0, 1.0);
    }
    sim.pad(0);
    sim.settle(0, 0, p.steps);
    // grow outward: every ring starts as the one inside it, with some noise
    for k in 1..rings {
        let s = sim.grid.rings[k].start;
        for j in 0..sim.grid.rings[k].count {
            let [u, v] = sim.grid.inward.read(&sim.uv, s + j);
            sim.uv[s + j] = [(u + irregularity * (hash(p.seed, k, j) - 0.5)).max(0.0), v];
        }
        sim.pad(k);
        sim.settle((k + 1).saturating_sub(BAND), k, p.steps);
    }
    // the last rings settle as long as every other ring did
    sim.settle(rings.saturating_sub(BAND), rings - 1, p.steps * (BAND - 1));

    let rings = sim
        .grid
        .rings
        .iter()
        .map(|r| sim.uv[r.start..r.start + r.count].iter().map(|c| (0.5 * c[0]).clamp(0.0, 1.0)).collect())
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
        forking: ctx.float(args[5]),
        steps: ctx.int(args[6]).clamp(1, 1000) as usize,
        seed: ctx.int(args[7]),
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
            forking: 0.0,
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

    /// The angles of the stripes around ring `i`: the middle of every run of values over 0.5
    fn centres(p: &Pattern, i: usize) -> Vec<f32> {
        let values = &p.rings[i];
        let n = values.len();
        let on = |j: usize| values[j % n] > 0.5;
        let Some(gap) = (0..n).find(|&j| !on(j)) else {
            return Vec::new();
        };
        let mut angles = Vec::new();
        let mut j = gap + 1;
        while j <= gap + n {
            let (mut weight, mut sum) = (0.0, 0.0);
            while j <= gap + n && on(j) {
                let w = values[j % n] - 0.5;
                weight += w;
                sum += w * (j as f32 + 0.5) * TAU / n as f32;
                j += 1;
            }
            if weight > 0.0 {
                angles.push((sum / weight).rem_euclid(TAU));
            }
            j += 1;
        }
        angles
    }

    fn turn(a: f32, b: f32) -> f32 {
        (b - a + std::f32::consts::PI).rem_euclid(TAU) - std::f32::consts::PI
    }

    /// Every stripe of a ring linked to the nearest stripe of the ring inside it, if that is
    /// within 0.4 spacings like `lamellae` follows them: (ring, stripe angle, angle it came from)
    fn links(p: &Pattern) -> Vec<(usize, f32, Option<f32>)> {
        let mut out = Vec::new();
        for i in 1..p.rings.len() {
            let (previous, next) = (centres(p, i - 1), centres(p, i));
            let reach = 0.4 * p.spacing / p.radius(i);
            for &a in &next {
                let from = previous.iter().map(|&b| (b, turn(b, a).abs())).min_by(|x, y| x.1.total_cmp(&y.1));
                out.push((i, a, from.filter(|f| f.1 <= reach).map(|f| f.0)));
            }
        }
        out
    }

    /// New stripes: (forks off a stripe of the ring inside, starts between two)
    fn births(p: &Pattern) -> (usize, usize) {
        let links = links(p);
        let starts = links.iter().filter(|l| l.2.is_none()).count();
        let mut forks = 0;
        for i in 1..p.rings.len() {
            let mut from: Vec<f32> = links.iter().filter(|l| l.0 == i).filter_map(|l| l.2).collect();
            from.sort_by(f32::total_cmp);
            let all = from.len();
            from.dedup();
            forks += all - from.len();
        }
        (forks, starts)
    }

    /// How far every linked stripe leans from the radius, in degrees
    fn tilts(p: &Pattern) -> Vec<f32> {
        let gap = p.radius(1) - p.radius(0);
        let mut tilts: Vec<f32> = links(p)
            .iter()
            .filter_map(|&(i, a, from)| from.map(|b| (p.radius(i) * turn(b, a)).abs().atan2(gap).to_degrees()))
            .collect();
        tilts.sort_by(f32::total_cmp);
        tilts
    }

    fn percentile(sorted: &[f32], q: f32) -> f32 {
        sorted[((sorted.len() - 1) as f32 * q).round() as usize]
    }

    #[test]
    #[ignore]
    fn measure() {
        for alignment in [0.0, 0.5, 0.85, 1.0] {
            let p = reaction_diffusion(&Params { alignment, ..params() });
            let rings = p.rings.len();
            let means: Vec<f32> = (0..rings).map(|i| spacing_at(&p, i) / p.spacing).collect();
            let mean = means.iter().sum::<f32>() / rings as f32;
            let t = tilts(&p);
            println!(
                "alignment {alignment}: mean {mean:.3} ring {:.2}..{:.2} (outer half {:.2}..{:.2}) tilt p50 {:.1} p90 {:.1} births {:?}",
                means.iter().cloned().fold(f32::MAX, f32::min),
                means.iter().cloned().fold(0.0, f32::max),
                means[rings / 2..].iter().cloned().fold(f32::MAX, f32::min),
                means[rings / 2..].iter().cloned().fold(0.0, f32::max),
                percentile(&t, 0.5),
                percentile(&t, 0.9),
                births(&p)
            );
        }
        for forking in [0.0, 0.5, 1.0] {
            let b: Vec<(usize, usize)> =
                [1, 2, 3, 5].iter().map(|&seed| births(&reaction_diffusion(&Params { forking, seed, ..params() }))).collect();
            println!("forking {forking}: (forks, starts) {b:?}");
        }
        let p = reaction_diffusion(&Params { irregularity: 0.0, inner_radius: 0.006, radius: 0.03, ..params() });
        let n0 = stripes(&p, 0) as f32;
        let first = |k: f32| (0..p.rings.len()).find(|&i| stripes(&p, i) as f32 > k * n0).map(|i| p.radius(i) / p.inner);
        println!("tiers: N0 {n0}, 1.5x at {:?}, 3x at {:?} inner radii", first(1.5), first(3.0));
    }

    #[test]
    fn stripes_keep_their_spacing_from_the_inner_ring_to_the_outer_one() {
        // the spacing on a ring is its circumference over its stripes. A gap doubles before a
        // new stripe halves it, so a single ring is somewhere between 0.6 and 1.5 spacings
        // (Iapichino et al. 2021 measure gills the same way), the mean over all rings is the
        // spacing. Regular stripes get their new ones all at the same radius, so their rings
        // swing further.
        for (alignment, irregularity, forking, every_ring) in [
            (0.85, 0.5, 0.0, true),
            (0.0, 0.5, 0.0, true),
            (1.0, 1.0, 0.0, true),
            (0.85, 0.5, 1.0, true),
            (0.5, 0.25, 0.0, false),
            (0.85, 0.0, 0.0, false),
        ] {
            for spacing in [0.001, 0.002] {
                let p = reaction_diffusion(&Params { alignment, irregularity, forking, spacing, ..params() });
                let rings = p.rings.len();
                let mean = (0..rings).map(|i| spacing_at(&p, i)).sum::<f32>() / rings as f32;
                let case = format!("alignment {alignment} irregularity {irregularity} forking {forking}");
                assert!((mean / spacing - 1.0).abs() < 0.2, "{case}: stripes {mean} apart, spacing {spacing}");
                if !every_ring {
                    continue;
                }
                for i in 0..rings {
                    let s = spacing_at(&p, i) / spacing;
                    assert!((0.6..1.5).contains(&s), "{case}: stripes {s} spacings apart at {}", p.radius(i));
                }
            }
        }
    }

    #[test]
    fn new_stripes_come_in_tiers_where_the_circumference_doubles() {
        // Fischer & Money (2010): the first lamellulae start at 2.1, the next at 3.8 stipe radii
        let p = reaction_diffusion(&Params { irregularity: 0.0, inner_radius: 0.006, radius: 0.03, ..params() });
        let first = stripes(&p, 0) as f32;
        let at = |times: f32| {
            let ring = (0..p.rings.len()).find(|&i| stripes(&p, i) as f32 > times * first).unwrap();
            p.radius(ring) / p.inner
        };
        assert!((1.7..2.4).contains(&at(1.5)), "the first tier comes in at {} inner radii", at(1.5));
        assert!((3.3..4.6).contains(&at(3.0)), "the second tier comes in at {} inner radii", at(3.0));
        // at their closest on the inner ring
        assert!((spacing_at(&p, 0) / p.spacing - INNER_SPACING).abs() < 0.05);
    }

    #[test]
    fn forking_makes_new_stripes_split_off_instead_of_starting_between() {
        for seed in [1, 2, 3, 5] {
            let (forks, starts) = births(&reaction_diffusion(&Params { forking: 0.0, seed, ..params() }));
            assert!(starts >= 3 * forks, "forking 0, seed {seed}: {forks} forks, {starts} start between");
            let (forks, starts) = births(&reaction_diffusion(&Params { forking: 1.0, seed, ..params() }));
            assert!(forks >= 5 * starts, "forking 1, seed {seed}: {forks} forks, {starts} start between");
        }
    }

    #[test]
    fn a_disc_without_area_has_no_pattern() {
        for (radius, inner_radius) in [(0.005, 0.05), (0.01, 0.01), (0.0, 0.0)] {
            let p = reaction_diffusion(&Params { radius, inner_radius, ..params() });
            assert!(p.rings.is_empty(), "radius {radius}, inner {inner_radius}");
            assert!(p.rings.iter().flatten().all(|v| v.is_finite()));
        }
    }

    #[test]
    fn more_stripes_at_the_margin_than_near_the_axis() {
        let p = reaction_diffusion(&params());
        let (first, last) = (stripes(&p, 0), stripes(&p, p.rings.len() - 1));
        // the circumference grows 5 times, the stripes start at their closest on the inner ring
        assert!(last as f32 > 3.0 * first as f32, "{first} stripes at the inner ring, {last} at the outer one");
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
        let tilt = |alignment: f32| percentile(&tilts(&reaction_diffusion(&Params { alignment, ..params() })), 0.9);
        let (labyrinth, straight) = (tilt(0.0), tilt(1.0));
        assert!(straight < 6.0, "at alignment 1 a tenth of the stripes lean more than {straight} degrees");
        assert!(labyrinth > 3.0 * straight, "a labyrinth leans {labyrinth} degrees, straight stripes {straight}");
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
            &[encode_float(p.forking)],
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
        assert_eq!(value("forking"), 0.0);
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
            forking: get("FORKING", 0.0),
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
