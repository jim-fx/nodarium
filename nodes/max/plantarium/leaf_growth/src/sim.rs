//! Developmental leaf model after Runions, Tsiantis & Prusinkiewicz (2017),
//! "A common developmental program can produce diverse leaf shapes",
//! New Phytologist, doi:10.1111/nph.14449.
//!
//! The leaf is a planar structure made of three coupled parts:
//! - the margin: an open polygon running from the leaf base to the tip. Its
//!   sample points carry morphogens and each belongs to a convergence point (CP),
//! - the veins: a tree rooted at the leaf base whose tips end at the CPs,
//! - the lamina spanning both, which is triangulated afterwards (see `mesh.rs`).
//!
//! Only the right half (x >= 0) is simulated, the midvein runs along the y axis
//! and the left half is mirrored. Every step runs the feedback loop of the paper:
//! 1. veins grow (basal growth zone + tip growth + isotropic expansion) and carry
//!    the margin points projected onto them along,
//! 2. the margin propagates outwards and is smoothed ("webbing"),
//! 3. new CPs appear where the margin got long enough (Hofmeister's rule),
//! 4. new veins connect them to the vasculature, minimising transport resistance.

use std::collections::VecDeque;

use glam::Vec2;

pub struct Params {
    /// Number of growth steps.
    pub steps: usize,
    /// Arc length along the margin between CPs before a new one is inserted (λ).
    pub vein_spacing: f32,
    /// Length added to every vein tip per step.
    pub tip_growth: f32,
    /// Relative elementary rate of growth (RERG) of veins inside the growth zone.
    pub base_growth: f32,
    /// Extent of the basal growth zone, measured along the veins from the base.
    pub growth_zone: f32,
    /// Growth decreases linearly towards the end of the growth zone.
    pub basipetal: bool,
    /// Isotropic expansion of the whole blade per step.
    pub expansion: f32,
    /// Propagation of the margin along its normal per step.
    pub margin_growth: f32,
    /// Angle between new veins and the vein they attach to, θ = arccos(v/b).
    pub branch_angle: f32,
    /// Maximum angle between a new vein and the margin normal (σmax).
    pub max_vein_angle: f32,
    /// Resistance of the margin to stretching and bending, 0..1.
    pub webbing: f32,
    /// 0 = only stretching resistance (pointed teeth), 1 = only bending (round).
    pub roundness: f32,
    /// Difference between proximal and distal webbing around each tooth, -1..1.
    pub serration: f32,
    /// Strength of the sinus morphogen that suppresses webbing and CPs, 0..1.
    pub lobing: f32,
    /// Number of steps after the first lateral CP before new CPs start to
    /// create sinuses. Later sinuses form between more CPs and stay smaller.
    pub lobe_delay: usize,
    /// Number of steps after that during which new CPs create sinuses, both
    /// ends included.
    pub lobe_steps: usize,
    /// Length of the petiole as a fraction of the primordium.
    pub petiole: f32,
}

/// A vein from where it branches off to its end. At every junction it goes on
/// along the straightest of the veins that leave it, the others start strands
/// of their own.
pub struct Strand {
    pub points: Vec<Vec2>,
    /// Number of CPs supplied through the segment ending in each point, the
    /// first point has the value of the first segment. Used for vein thickness.
    pub flux: Vec<f32>,
    /// The strand this one branches off and the index of the point in it where
    /// this one starts. The midvein has none.
    pub parent: Option<(usize, usize)>,
}

pub struct Leaf {
    /// Right half of the margin from the base (0, 0) to the tip on the +y axis.
    pub margin: Vec<Vec2>,
    /// Veins of the right half, including the midvein, every strand comes
    /// after the one it branches off.
    pub strands: Vec<Strand>,
}

impl Leaf {
    /// Closed, counter clockwise outline of the whole leaf.
    pub fn outline(&self) -> Vec<Vec2> {
        let n = self.margin.len();
        let mut outline = self.margin.clone();
        for p in self.margin[1..n - 1].iter().rev() {
            outline.push(Vec2::new(-p.x, p.y));
        }
        outline
    }
}

#[derive(Clone, Copy)]
struct MarginPoint {
    pos: Vec2,
    /// CP whose vein carries this point.
    owner: usize,
    is_cp: bool,
    /// Sinus morphogen, suppresses webbing, propagation and CP formation.
    sinus: f32,
    /// Petiole morphogen, suppresses lateral growth and CP formation.
    petiole: f32,
}

const ROOT: usize = 0;
/// Upper bounds that keep extreme parameters (e.g. recursively toothed
/// margins) responsive. The margin gets coarser instead of growing further.
const MAX_MARGIN_POINTS: usize = 1500;
const MAX_CPS: usize = 200;

struct Sim<'a> {
    p: &'a Params,
    margin: Vec<MarginPoint>,
    node_pos: Vec<Vec2>,
    node_parent: Vec<Option<usize>>,
    /// Vein tip node of every CP.
    cps: Vec<usize>,
    step: usize,
    /// Step in which the first lateral CP appeared.
    first_cp_step: Option<usize>,
    /// Maximum distance between neighbouring margin points.
    spacing: f32,
}

pub fn simulate(p: &Params) -> Leaf {
    let mut sim = Sim::new(p);
    for step in 0..p.steps {
        sim.step = step;
        sim.grow();
        sim.propagate();
        sim.fair();
        sim.separate();
        sim.resample();
        sim.pattern();
    }
    sim.leaf()
}

fn rotate(v: Vec2, angle: f32) -> Vec2 {
    let (s, c) = angle.sin_cos();
    Vec2::new(v.x * c - v.y * s, v.x * s + v.y * c)
}

impl<'a> Sim<'a> {
    fn new(p: &'a Params) -> Self {
        let mut sim = Sim {
            p,
            margin: Vec::new(),
            node_pos: vec![Vec2::ZERO, Vec2::new(0.0, 1.0)],
            node_parent: vec![None, Some(ROOT)],
            cps: vec![1],
            step: 0,
            first_cp_step: None,
            spacing: p.vein_spacing / 6.0,
        };

        // Primordium of length 1: a narrow petiole followed by a pointed blade.
        let half_width = 0.3;
        let petiole_width = 0.05;
        let petiole_end = 0.4 * p.petiole.clamp(0.0, 1.0);
        let point = |pos: Vec2, petiole: f32| MarginPoint {
            pos,
            owner: 0,
            is_cp: false,
            sinus: 0.0,
            petiole,
        };

        sim.margin.push(point(Vec2::ZERO, 1.0));
        let samples = 24;
        for i in 0..samples {
            let y = i as f32 / samples as f32;
            let x = if y < petiole_end {
                petiole_width
            } else {
                let u = (y - petiole_end) / (1.0 - petiole_end);
                petiole_width * (1.0 - u) + half_width * (std::f32::consts::PI * u).sin().powf(0.8)
            };
            let petiole = if y < petiole_end { 1.0 } else { 0.0 };
            sim.margin.push(point(Vec2::new(x, y), petiole));
        }
        let mut tip = point(Vec2::new(0.0, 1.0), 0.0);
        tip.is_cp = true;
        sim.margin.push(tip);

        sim.resample();
        sim
    }

    fn children(&self) -> Vec<Vec<usize>> {
        let mut children = vec![Vec::new(); self.node_pos.len()];
        for (i, parent) in self.node_parent.iter().enumerate() {
            if let Some(p) = parent {
                children[*p].push(i);
            }
        }
        children
    }

    /// Nodes ordered so that every parent comes before its children.
    fn topological_order(children: &[Vec<usize>]) -> Vec<usize> {
        let mut order = vec![ROOT];
        let mut i = 0;
        while i < order.len() {
            order.extend_from_slice(&children[order[i]]);
            i += 1;
        }
        order
    }

    /// Distance of every vein node from the leaf base, measured along the veins.
    fn base_distances(&self, order: &[usize]) -> Vec<f32> {
        let mut dist = vec![0.0; self.node_pos.len()];
        for &c in &order[1..] {
            let p = self.node_parent[c].unwrap();
            dist[c] = dist[p] + self.node_pos[p].distance(self.node_pos[c]);
        }
        dist
    }

    /// Integral of the relative elementary rate of growth between two distances
    /// from the base (Eqn 2). RERG is piecewise linear, so this is analytic.
    fn growth_integral(&self, a: f32, b: f32) -> f32 {
        let zone = self.p.growth_zone.max(1e-4);
        let b = b.min(zone);
        if b <= a {
            return 0.0;
        }
        if self.p.basipetal {
            self.p.base_growth * ((b - a) - (b * b - a * a) / (2.0 * zone))
        } else {
            self.p.base_growth * (b - a)
        }
    }

    /// Closest point to `q` on the vein path from a CP down to the base, as the
    /// segment's child node and the distance along the segment.
    fn project(&self, cp: usize, q: Vec2) -> (usize, f32) {
        let mut c = self.cps[cp];
        let mut best = (c, 0.0, f32::MAX);
        while let Some(p) = self.node_parent[c] {
            let a = self.node_pos[p];
            let ab = self.node_pos[c] - a;
            let len = ab.length();
            let x = if len > 0.0 {
                ((q - a).dot(ab) / len).clamp(0.0, len)
            } else {
                0.0
            };
            let d = q.distance_squared(a + ab * (x / len.max(1e-12)));
            if d < best.2 {
                best = (c, x, d);
            }
            c = p;
        }
        (best.0, best.1)
    }

    /// Veins elongate and the margin follows them: each margin point is moved by
    /// the same vector as its projection onto the vein of its CP (Fig. 4f).
    /// New vein length is added at the tips, so only the parts of the margin in
    /// front of a tip are pushed out by it, while the flanks lag behind.
    fn grow(&mut self) {
        let children = self.children();
        let order = Self::topological_order(&children);
        let dist = self.base_distances(&order);
        let scale = 1.0 + self.p.expansion;

        let segment = |c: usize| {
            let p = self.node_parent[c].unwrap();
            let d = self.node_pos[c] - self.node_pos[p];
            (p, d.length(), d.try_normalize().unwrap_or(Vec2::Y))
        };
        // New distance from the parent node of a point at distance x along the
        // segment ending in c.
        let grown = |c: usize, x: f32| {
            let (p, len, _) = segment(c);
            let mut x_new = x * scale + self.growth_integral(dist[p], dist[p] + x);
            if children[c].is_empty() && x >= len * 0.9999 {
                x_new += self.p.tip_growth;
            }
            x_new
        };

        let mut new_pos = self.node_pos.clone();
        for &c in &order[1..] {
            let (p, len, dir) = segment(c);
            new_pos[c] = new_pos[p] + dir * grown(c, len);
        }

        let margin: Vec<Vec2> = self
            .margin
            .iter()
            .map(|m| {
                let (c, x) = self.project(m.owner, m.pos);
                let (p, _, dir) = segment(c);
                let old_anchor = self.node_pos[p] + dir * x;
                let new_anchor = new_pos[p] + dir * grown(c, x);
                new_anchor + (m.pos - old_anchor) * scale
            })
            .collect();
        for (m, pos) in self.margin.iter_mut().zip(margin) {
            m.pos = pos;
        }
        self.node_pos = new_pos;
    }

    fn normals(&self) -> Vec<Vec2> {
        let n = self.margin.len();
        (0..n)
            .map(|i| {
                let prev = self.margin[i.saturating_sub(1)].pos;
                let next = self.margin[(i + 1).min(n - 1)].pos;
                let d = next - prev;
                Vec2::new(d.y, -d.x).normalize_or_zero()
            })
            .collect()
    }

    fn is_fixed(&self, i: usize) -> bool {
        i == 0 || i == self.margin.len() - 1 || self.margin[i].is_cp
    }

    /// Growth in width: the margin moves along its outward normal.
    fn propagate(&mut self) {
        let normals = self.normals();
        let lobing = self.p.lobing;
        for i in 0..self.margin.len() {
            if self.is_fixed(i) {
                continue;
            }
            let m = &mut self.margin[i];
            let rate = (1.0 - m.petiole) * (1.0 - lobing * m.sinus);
            m.pos += normals[i] * self.p.margin_growth * rate;
            m.pos.x = m.pos.x.max(0.0);
        }
    }

    /// Indices of the base and all CPs along the margin.
    fn markers(&self) -> Vec<usize> {
        (0..self.margin.len())
            .filter(|&i| i == 0 || self.margin[i].is_cp)
            .collect()
    }

    fn arc_lengths(&self) -> Vec<f32> {
        let mut s = vec![0.0; self.margin.len()];
        for i in 1..self.margin.len() {
            s[i] = s[i - 1] + self.margin[i].pos.distance(self.margin[i - 1].pos);
        }
        s
    }

    /// Webbing: the margin resists stretching and bending, implemented as
    /// geometric fairing. Every step the margin is replaced by the minimiser of
    ///   Σ |x_i - p_i|² + α Σ |x_i+1 - x_i|² + β Σ |x_i-1 - 2x_i + x_i+1|²
    /// with CPs, base and tip held in place. The smoothing length is tied to λ,
    /// so the result does not depend on the margin resolution. Points only move
    /// along the normal, so morphogens and CP segments do not slide.
    fn fair(&mut self) {
        let n = self.margin.len();
        let lobing = self.p.lobing;
        let roundness = self.p.roundness.clamp(0.0, 1.0);

        // Points closer to the CP above them form the proximal flank of that
        // tooth, the others the distal flank of the tooth below.
        let s = self.arc_lengths();
        let mut weight = vec![0.0; n];
        for w in self.markers().windows(2) {
            let (a, b) = (w[0], w[1]);
            for k in a + 1..b {
                let proximal = s[b] - s[k] < s[k] - s[a];
                let skew = if proximal {
                    1.0 + self.p.serration
                } else {
                    1.0 - self.p.serration
                };
                let m = &self.margin[k];
                weight[k] = (skew * (1.0 - lobing * m.sinus) * (1.0 - m.petiole)).max(0.0);
            }
        }
        for i in 0..n {
            if self.margin[i].is_cp {
                let prev = weight[i.saturating_sub(1)];
                let next = weight[(i + 1).min(n - 1)];
                weight[i] = 0.5 * (prev + next);
            }
        }

        let spacing = s[n - 1] / (n - 1) as f32;
        let r = self.p.webbing * self.p.vein_spacing / spacing;
        let alpha = (1.0 - roundness) * r * r;
        let beta = roundness * r * r * r * r;

        let pos: Vec<Vec2> = self.margin.iter().map(|m| m.pos).collect();
        let mut system = BandSystem::new(&pos);
        for i in 0..n - 1 {
            let w = 0.5 * (weight[i] + weight[i + 1]);
            system.add_term(&[(i, 1.0), (i + 1, -1.0)], alpha * w);
        }
        for i in 1..n - 1 {
            system.add_term(&[(i - 1, 1.0), (i, -2.0), (i + 1, 1.0)], beta * weight[i]);
        }
        for i in 0..n {
            if self.is_fixed(i) || weight[i] <= 0.0 {
                system.fix(i, pos[i]);
            }
        }
        let faired = system.solve();

        let normals = self.normals();
        for (i, m) in self.margin.iter_mut().enumerate() {
            m.pos += normals[i] * (faired[i] - pos[i]).dot(normals[i]);
            m.pos.x = m.pos.x.max(0.0);
        }
    }

    /// The margin must not run into itself: points that come closer than a
    /// minimum distance to a part of the margin that is not their neighbour are
    /// pushed apart. This keeps deep sinuses between growing lobes open.
    fn separate(&mut self) {
        let n = self.margin.len();
        // Tied to λ rather than the current spacing: a coarser margin must not
        // push harder, that would crumple it further.
        let min_dist = self.p.vein_spacing / 12.0;

        let pos: Vec<Vec2> = self.margin.iter().map(|m| m.pos).collect();
        let grid = Grid::new(&pos, min_dist);
        for i in 0..n {
            if self.is_fixed(i) {
                continue;
            }
            let mut push = Vec2::ZERO;
            grid.visit(pos[i], |j| {
                if i.abs_diff(j) <= 2 {
                    return;
                }
                let d = pos[i] - pos[j];
                let dist = d.length();
                if dist < min_dist && dist > 0.0 {
                    push += d / dist * (min_dist - dist) * 0.5;
                }
            });
            let m = &mut self.margin[i];
            m.pos += push.clamp_length_max(0.5 * min_dist);
            m.pos.x = m.pos.x.max(0.0);
        }
    }

    /// Keeps the spacing of the margin sample points between bounds.
    fn resample(&mut self) {
        let n = self.margin.len();
        let length = self.arc_lengths()[n - 1];
        self.spacing = (self.p.vein_spacing / 6.0).max(length / (0.6 * MAX_MARGIN_POINTS as f32));
        let h_max = self.spacing;
        let h_min = h_max * 0.3;

        let mut split = Vec::with_capacity(n * 2);
        for i in 0..n - 1 {
            let a = self.margin[i];
            let b = self.margin[i + 1];
            split.push(a);
            let pieces = (a.pos.distance(b.pos) / h_max).ceil() as usize;
            for j in 1..pieces {
                let t = j as f32 / pieces as f32;
                split.push(MarginPoint {
                    pos: a.pos.lerp(b.pos, t),
                    owner: if t < 0.5 { a.owner } else { b.owner },
                    is_cp: false,
                    sinus: a.sinus + (b.sinus - a.sinus) * t,
                    petiole: a.petiole + (b.petiole - a.petiole) * t,
                });
            }
        }
        split.push(self.margin[n - 1]);

        let last = split.len() - 1;
        let mut merged: Vec<MarginPoint> = Vec::with_capacity(split.len());
        for (i, m) in split.into_iter().enumerate() {
            let fixed = i == 0 || i == last || m.is_cp;
            if let Some(prev) = merged.last() {
                if prev.pos.distance(m.pos) < h_min {
                    if !fixed {
                        continue;
                    }
                    let prev_fixed = merged.len() == 1 || prev.is_cp;
                    if !prev_fixed {
                        merged.pop();
                    }
                }
            }
            merged.push(m);
        }
        self.margin = merged;
    }

    /// Hofmeister's rule on the margin: within every interval of the margin that
    /// is competent to form CPs, a new CP appears in its middle once that point
    /// is at least λ away from the neighbouring CPs (or the base) (Fig. 3c,d).
    fn pattern(&mut self) {
        let s = self.arc_lengths();
        let lambda = self.p.vein_spacing;
        let markers = self.markers();
        let competent =
            |m: &MarginPoint| m.petiole < 0.5 && (self.p.lobing <= 0.0 || m.sinus < 0.5);

        let mut new_cps = Vec::new();
        for w in markers.windows(2) {
            if self.cps.len() + new_cps.len() >= MAX_CPS {
                break;
            }
            let (a, b) = (w[0], w[1]);
            let mut best = None;
            let mut best_dist = lambda;
            let mut k = a + 1;
            while k < b {
                if !competent(&self.margin[k]) {
                    k += 1;
                    continue;
                }
                let start = k;
                while k + 1 < b && competent(&self.margin[k + 1]) {
                    k += 1;
                }
                // Extend the interval to the neighbouring CPs, which are competent.
                let from = if start == a + 1 { s[a] } else { s[start] };
                let to = if k + 1 == b { s[b] } else { s[k] };
                let middle = 0.5 * (from + to);
                let c = (start..=k)
                    .min_by(|&i, &j| (s[i] - middle).abs().total_cmp(&(s[j] - middle).abs()))
                    .unwrap();
                let d = (s[c] - s[a]).min(s[b] - s[c]);
                if d >= best_dist {
                    best_dist = d;
                    best = Some((c, a, b));
                }
                k += 1;
            }
            if let Some(cp) = best {
                new_cps.push(cp);
            }
        }

        let normals = self.normals();
        for (k, a, b) in new_cps {
            self.add_cp(k, a, b, &s, -normals[k]);
        }
    }

    fn add_cp(&mut self, k: usize, a: usize, b: usize, s: &[f32], inward: Vec2) {
        let q = self.margin[k].pos;
        let attach = self.attachment(q, inward);
        let tip = self.node_pos.len();
        self.node_pos.push(q);
        self.node_parent.push(Some(attach));
        let id = self.cps.len();
        self.cps.push(tip);

        let m = &mut self.margin[k];
        m.is_cp = true;
        m.owner = id;
        m.sinus = 0.0;

        // The margin points closer to the new CP than to its neighbours become
        // its margin segment, so segment boundaries lie between the CPs.
        for j in a + 1..b {
            let neighbour = if j < k { s[j] - s[a] } else { s[b] - s[j] };
            let m = &mut self.margin[j];
            if (s[j] - s[k]).abs() < neighbour && m.petiole < 0.5 {
                m.owner = id;
            }
        }

        // Early in development each new CP lays down sinus morphogen between
        // itself and its neighbours, keeping the surrounding of the CPs free.
        // When the first CPs appear depends on the other parameters, so the
        // lobing phase starts with them rather than with the first step.
        let first = *self.first_cp_step.get_or_insert(self.step);
        let start = first + self.p.lobe_delay;
        if self.p.lobing > 0.0 && (start..=start + self.p.lobe_steps).contains(&self.step) {
            let rho = 0.25 * self.p.vein_spacing;
            for j in a + 1..b {
                if j == k {
                    continue;
                }
                let neighbour = if j < k { s[j] - s[a] } else { s[b] - s[j] };
                let free = (s[j] - s[k]).abs() < rho || neighbour < rho;
                self.margin[j].sinus = if free { 0.0 } else { 1.0 };
            }
        }
    }

    /// Node a new vein from `q` attaches to. The attachment point P minimises the
    /// transport resistance b|q - P| + v|P - B| (Fig. 5e), which makes veins meet
    /// at θ = arccos(v/b), and the vein direction is clamped to ±σmax around the
    /// margin normal.
    fn attachment(&mut self, q: Vec2, inward: Vec2) -> usize {
        let children = self.children();
        let order = Self::topological_order(&children);
        let dist = self.base_distances(&order);

        let v = self.p.branch_angle.cos().clamp(0.0, 0.999);
        let cot = v / (1.0 - v * v).sqrt();

        let mut best = (ROOT, 0.0, f32::MAX);
        for &c in &order[1..] {
            let p = self.node_parent[c].unwrap();
            let a = self.node_pos[p];
            let ab = self.node_pos[c] - a;
            let len = ab.length();
            if len <= 0.0 {
                continue;
            }
            let u = ab / len;
            let along = (q - a).dot(u);
            let across = (q - a).perp_dot(u).abs();
            // Do not attach right at a vein tip, that would end its growth.
            let max = if children[c].is_empty() {
                len * 0.9
            } else {
                len
            };
            for x in [(along - across * cot).clamp(0.0, max), 0.0, max] {
                let cost = q.distance(a + u * x) + v * (dist[p] + x);
                if cost < best.2 {
                    best = (c, x, cost);
                }
            }
        }

        let (mut c, mut x, _) = best;
        let p = self.node_parent[c].unwrap_or(ROOT);
        let target =
            self.node_pos[p] + (self.node_pos[c] - self.node_pos[p]).normalize_or_zero() * x;
        let dir = (target - q).normalize_or_zero();
        let angle = inward.perp_dot(dir).atan2(inward.dot(dir));
        let sigma = self.p.max_vein_angle;
        if dir != Vec2::ZERO && angle.abs() > sigma {
            let ray = rotate(inward, sigma.copysign(angle));
            if let Some(hit) = self.ray_cast(q, ray, &order, &children) {
                (c, x) = hit;
            }
        }
        self.split(c, x, children[c].is_empty())
    }

    /// First vein segment hit by a ray, as child node and distance along it.
    fn ray_cast(
        &self,
        origin: Vec2,
        dir: Vec2,
        order: &[usize],
        children: &[Vec<usize>],
    ) -> Option<(usize, f32)> {
        let mut best: Option<(usize, f32, f32)> = None;
        for &c in &order[1..] {
            let a = self.node_pos[self.node_parent[c].unwrap()];
            let ab = self.node_pos[c] - a;
            let denom = dir.perp_dot(ab);
            if denom.abs() < 1e-9 {
                continue;
            }
            let ao = a - origin;
            let t = ao.perp_dot(ab) / denom;
            let u = ao.perp_dot(dir) / denom;
            let max = if children[c].is_empty() { 0.9 } else { 1.0 };
            if t > 1e-5 && (0.0..=max).contains(&u) && best.map_or(true, |b| t < b.2) {
                best = Some((c, u * ab.length(), t));
            }
        }
        best.map(|(c, x, _)| (c, x))
    }

    /// Returns a node at distance `x` along the segment ending in `c`, inserting
    /// one if needed.
    fn split(&mut self, c: usize, x: f32, terminal: bool) -> usize {
        let Some(p) = self.node_parent[c] else {
            return ROOT;
        };
        let a = self.node_pos[p];
        let ab = self.node_pos[c] - a;
        let len = ab.length();
        let eps = 1e-4 * self.p.vein_spacing;
        if x <= eps {
            return p;
        }
        if x >= len - eps && !terminal {
            return c;
        }
        let m = self.node_pos.len();
        self.node_pos.push(a + ab * (x / len));
        self.node_parent.push(Some(p));
        self.node_parent[c] = Some(m);
        m
    }

    fn leaf(&self) -> Leaf {
        let children = self.children();
        let order = Self::topological_order(&children);
        let mut flux = vec![0.0f32; self.node_pos.len()];
        for &c in order.iter().rev() {
            if children[c].is_empty() {
                flux[c] = 1.0;
            }
            if let Some(p) = self.node_parent[c] {
                flux[p] += flux[c];
            }
        }

        // Strands in breadth first order, so every one comes after its parent.
        // A queued strand starts at a node, goes on to one of its children and
        // knows the strand and point it branches off at.
        let pos = &self.node_pos;
        let first = children[ROOT].iter().copied().max_by(|&a, &b| flux[a].total_cmp(&flux[b]));
        let mut queue = VecDeque::new();
        if let Some(c) = first {
            queue.push_back((ROOT, c, None));
        }
        let mut strands = Vec::new();
        while let Some((start, mut next, parent)) = queue.pop_front() {
            let mut nodes = vec![start];
            loop {
                let from = *nodes.last().unwrap();
                nodes.push(next);
                let dir = (pos[next] - pos[from]).normalize_or_zero();
                let straightest = children[next].iter().copied().max_by(|&a, &b| {
                    let turn = |c: usize| dir.dot((pos[c] - pos[next]).normalize_or_zero());
                    turn(a).total_cmp(&turn(b))
                });
                match straightest {
                    Some(c) => next = c,
                    None => break,
                }
            }
            let id = strands.len();
            // The first node of a branch belongs to its parent, which queued
            // the other veins leaving it.
            let own = if parent.is_some() { 1 } else { 0 };
            for (k, &node) in nodes.iter().enumerate().skip(own) {
                for &c in &children[node] {
                    if nodes.get(k + 1) != Some(&c) {
                        queue.push_back((node, c, Some((id, k))));
                    }
                }
            }
            let mut strand_flux: Vec<f32> = nodes.iter().map(|&n| flux[n]).collect();
            strand_flux[0] = strand_flux[1];
            strands.push(Strand {
                points: nodes.iter().map(|&n| pos[n]).collect(),
                flux: strand_flux,
                parent,
            });
        }
        Leaf {
            margin: self.margin.iter().map(|m| m.pos).collect(),
            strands,
        }
    }
}

/// Symmetric pentadiagonal system (I + Σ w c cᵀ) x = p for the margin fairing.
struct BandSystem {
    /// Row i stores the coefficients of columns i-2..=i+2.
    rows: Vec<[f32; 5]>,
    rhs: Vec<Vec2>,
}

impl BandSystem {
    fn new(pos: &[Vec2]) -> Self {
        BandSystem {
            rows: vec![[0.0, 0.0, 1.0, 0.0, 0.0]; pos.len()],
            rhs: pos.to_vec(),
        }
    }

    /// Adds the energy term w (Σ c_i x_i)².
    fn add_term(&mut self, coefficients: &[(usize, f32)], w: f32) {
        if w <= 0.0 {
            return;
        }
        for &(i, ci) in coefficients {
            for &(j, cj) in coefficients {
                self.rows[i][j + 2 - i] += w * ci * cj;
            }
        }
    }

    /// Pins x_i to `value`, keeping the system symmetric.
    fn fix(&mut self, i: usize, value: Vec2) {
        let n = self.rows.len();
        for j in i.saturating_sub(2)..(i + 3).min(n) {
            if j != i {
                self.rhs[j] -= value * self.rows[j][i + 2 - j];
                self.rows[j][i + 2 - j] = 0.0;
            }
        }
        self.rows[i] = [0.0, 0.0, 1.0, 0.0, 0.0];
        self.rhs[i] = value;
    }

    /// Gaussian elimination without pivoting, fine as the matrix is SPD.
    fn solve(mut self) -> Vec<Vec2> {
        let n = self.rows.len();
        for k in 0..n {
            let pivot = self.rows[k][2];
            for i in k + 1..(k + 3).min(n) {
                let f = self.rows[i][k + 2 - i] / pivot;
                if f == 0.0 {
                    continue;
                }
                for j in k..(k + 3).min(n) {
                    self.rows[i][j + 2 - i] -= f * self.rows[k][j + 2 - k];
                }
                let r = self.rhs[k] * f;
                self.rhs[i] -= r;
            }
        }
        let mut x = vec![Vec2::ZERO; n];
        for k in (0..n).rev() {
            let mut v = self.rhs[k];
            for j in k + 1..(k + 3).min(n) {
                v -= x[j] * self.rows[k][j + 2 - k];
            }
            x[k] = v / self.rows[k][2];
        }
        x
    }
}

/// Spatial hash for finding the points within one cell size of a position.
struct Grid {
    cell: f32,
    mask: usize,
    /// Start of every bucket in `items`, counting sort style.
    start: Vec<usize>,
    items: Vec<usize>,
}

impl Grid {
    fn new(points: &[Vec2], cell: f32) -> Self {
        let buckets = (2 * points.len()).next_power_of_two();
        let mut grid = Grid {
            cell,
            mask: buckets - 1,
            start: vec![0; buckets + 1],
            items: vec![0; points.len()],
        };
        let keys: Vec<usize> = points
            .iter()
            .map(|p| {
                let (x, y) = grid.coords(*p);
                grid.bucket(x, y)
            })
            .collect();
        for &k in &keys {
            grid.start[k + 1] += 1;
        }
        for k in 0..buckets {
            grid.start[k + 1] += grid.start[k];
        }
        let mut fill = grid.start.clone();
        for (i, &k) in keys.iter().enumerate() {
            grid.items[fill[k]] = i;
            fill[k] += 1;
        }
        grid
    }

    fn coords(&self, p: Vec2) -> (i32, i32) {
        (
            (p.x / self.cell).floor() as i32,
            (p.y / self.cell).floor() as i32,
        )
    }

    fn bucket(&self, x: i32, y: i32) -> usize {
        (x.wrapping_mul(73_856_093) ^ y.wrapping_mul(19_349_663)) as usize & self.mask
    }

    /// Calls `f` for all points in the cells around `p`, and possibly others.
    fn visit(&self, p: Vec2, mut f: impl FnMut(usize)) {
        let (x, y) = self.coords(p);
        for cy in y - 1..=y + 1 {
            for cx in x - 1..=x + 1 {
                let k = self.bucket(cx, cy);
                for &i in &self.items[self.start[k]..self.start[k + 1]] {
                    f(i);
                }
            }
        }
    }
}
