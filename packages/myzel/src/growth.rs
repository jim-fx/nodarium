//! Neighbour-Sensing growth (Meškauskas, McNulty, Moore 2004; Meškauskas,
//! Fricker, Moore 2004): every tip follows the same local rules each
//! iteration, the shape of the fruit body comes from schedules that change the
//! rules over simulation time.
//!
//! New direction of a tip:
//!
//! ```text
//! normalize(k · old + I_density · density + a(t) · long_range + c(t) · galvano + I_g · gravi + wander · noise)
//! ```
//!
//! Every field vector is capped at length 1, so the impacts compare directly.
//! Distances are measured in segment lengths, which makes the defaults
//! independent of the size of the mushroom.

use glam::Vec3;
use nodarium_utils::curve;

use crate::hyphae::{flags, HyphaSegment, Hyphae, Tissue};
use crate::spatial::SpatialHash;

/// Short-range fields only look this many segment lengths far
const CUTOFF: f32 = 6.0;
/// Closer neighbours count as this close, so one neighbour can't dominate everything
const MIN_DISTANCE: f32 = 0.25;
const DENSITY_IMPACT: f32 = 0.6;
const GRAVITROPISM_IMPACT: f32 = 0.9;
/// The autotropism schedule reads 0 to 1, a full pull outweighs the other fields
const AUTOTROPISM_SCALE: f32 = 2.0;
const TOLERANCE: f32 = 5.0 * std::f32::consts::PI / 180.0;
const WANDER: f32 = 0.15;
const BRANCH_ANGLE: f32 = 0.7;
/// Cells per axis of the coarse grid for long-range autotropism
const COARSE: usize = 8;
/// Upper limits whatever the inputs say, so no input can hang the graph
pub const MAX_ITERATIONS: usize = 400;
pub const MAX_SEGMENTS: usize = 50_000;

/// A curve input `x0, y0, x1, y1, ...` with x and y in [0, 1], read over normalised simulation time
#[derive(Clone, Debug, PartialEq)]
pub struct Schedule(pub Vec<f32>);

impl Schedule {
    pub fn constant(value: f32) -> Schedule {
        Schedule(vec![0.0, value, 1.0, value])
    }

    pub fn at(&self, t: f32) -> f32 {
        if self.0.len() < 4 {
            return self.0.get(1).copied().unwrap_or(0.0);
        }
        curve(&self.0, t.clamp(0.0, 1.0))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GrowthParams {
    pub seed: u32,
    pub iterations: usize,
    pub initial_tips: usize,
    pub knot_radius: f32,
    pub segment_length: f32,
    pub hypha_radius: f32,
    /// k, how much a tip keeps its old direction
    pub persistence: f32,
    pub branch_probability: f32,
    /// Tips only branch where the density is below this
    pub density_threshold: f32,
    /// Old branch points branch again below `density_threshold · secondary_factor`
    pub secondary_factor: f32,
    /// Share of new branches that are leading hyphae
    pub leading_share: f32,
    /// Preferred angle to the vertical, 0 is up, 1 is down (180°)
    pub gravitropic_angle: Schedule,
    pub autotropism: Schedule,
    pub galvanotropism: Schedule,
    /// Segments grown below this height while the tips point up are stipe
    pub waist_height: f32,
    pub max_segments: usize,
}

impl Default for GrowthParams {
    /// The four phases of the Coprinopsis primordium in the paper: spread out as
    /// a disc, gather and grow up, elongate, turn outwards and down
    fn default() -> Self {
        GrowthParams {
            seed: 0,
            iterations: 70,
            initial_tips: 60,
            knot_radius: 0.0015,
            segment_length: 0.0004,
            hypha_radius: 0.00025,
            persistence: 1.0,
            branch_probability: 0.4,
            density_threshold: 0.6,
            secondary_factor: 4.0,
            leading_share: 0.1,
            gravitropic_angle: Schedule(vec![0.0, 0.5, 0.05, 0.5, 0.15, 0.0, 0.5, 0.0, 0.7, 0.75, 1.0, 0.85]),
            autotropism: Schedule(vec![0.0, 0.0, 0.05, 0.0, 0.15, 1.0, 0.45, 0.2, 1.0, 0.1]),
            galvanotropism: Schedule(vec![0.0, 0.1, 0.25, 0.7, 1.0, 0.7]),
            waist_height: 0.008,
            max_segments: 5000,
        }
    }
}

struct Rng(u32);

impl Rng {
    fn new(seed: u32) -> Rng {
        Rng(seed.wrapping_mul(0x9e37_79b9) ^ 0x6a09_e667 | 1)
    }

    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 >> 8) as f32 / (1u32 << 24) as f32
    }

    fn unit(&mut self) -> Vec3 {
        loop {
            let v = Vec3::new(self.next(), self.next(), self.next()) * 2.0 - Vec3::ONE;
            let l = v.length_squared();
            if l > 1e-4 && l <= 1.0 {
                return v / l.sqrt();
            }
        }
    }
}

struct Tip {
    segment: u32,
    direction: Vec3,
    kind: u32,
}

/// Summed segment mass on a coarse grid, the long-range field is read from it
/// instead of from every segment
struct CoarseGrid {
    origin: Vec3,
    cell: f32,
    mass: Vec<f32>,
    field: Vec<Vec3>,
}

impl CoarseGrid {
    fn new(center: Vec3, size: f32) -> CoarseGrid {
        CoarseGrid {
            origin: center - Vec3::splat(size / 2.0),
            cell: size / COARSE as f32,
            mass: vec![0.0; COARSE * COARSE * COARSE],
            field: vec![Vec3::ZERO; COARSE * COARSE * COARSE],
        }
    }

    fn cell_center(&self, i: usize) -> Vec3 {
        let c = Vec3::new((i % COARSE) as f32, (i / COARSE % COARSE) as f32, (i / (COARSE * COARSE)) as f32);
        self.origin + (c + Vec3::splat(0.5)) * self.cell
    }

    fn add(&mut self, p: Vec3) {
        let c = ((p - self.origin) / self.cell).floor().as_ivec3().clamp(glam::IVec3::ZERO, glam::IVec3::splat(COARSE as i32 - 1));
        self.mass[c.x as usize + COARSE * (c.y as usize + COARSE * c.z as usize)] += 1.0;
    }

    /// Pull towards the mass with 1/√d falloff, normalised by the total mass
    fn update(&mut self, unit: f32) {
        let total: f32 = self.mass.iter().sum();
        if total <= 0.0 {
            return;
        }
        let massive: Vec<(Vec3, f32)> =
            (0..self.mass.len()).filter(|&j| self.mass[j] > 0.0).map(|j| (self.cell_center(j), self.mass[j])).collect();
        for i in 0..self.field.len() {
            let c = self.cell_center(i);
            let mut f = Vec3::ZERO;
            for &(q, m) in &massive {
                let offset = q - c;
                let d = offset.length().max(self.cell * 0.5);
                f += offset / d * m / (d / unit).sqrt();
            }
            self.field[i] = f / total;
        }
    }

    /// Trilinear sample of the field
    fn sample(&self, p: Vec3) -> Vec3 {
        let g = ((p - self.origin) / self.cell - Vec3::splat(0.5)).clamp(Vec3::ZERO, Vec3::splat(COARSE as f32 - 1.0));
        let base = g.floor().as_uvec3().min(glam::UVec3::splat(COARSE as u32 - 2));
        let f = g - base.as_vec3();
        let mut v = Vec3::ZERO;
        for corner in 0..8 {
            let o = glam::UVec3::new(corner & 1, corner >> 1 & 1, corner >> 2 & 1);
            let w = Vec3::select(o.cmpeq(glam::UVec3::ONE), f, Vec3::ONE - f);
            let c = base + o;
            v += self.field[(c.x + COARSE as u32 * (c.y + COARSE as u32 * c.z)) as usize] * (w.x * w.y * w.z);
        }
        v
    }
}

/// What a tip senses from the hyphae around it
struct Sensed {
    density: f32,
    away: Vec3,
    parallel: Vec3,
}

struct Simulation<'a> {
    params: &'a GrowthParams,
    hyphae: Hyphae,
    hash: SpatialHash,
    coarse: CoarseGrid,
    rng: Rng,
    tips: Vec<Tip>,
    /// Segments a tip branched from, with the iteration, until they branched a second time
    branch_points: Vec<(u32, usize)>,
    stipe_started: bool,
}

impl Simulation<'_> {
    fn add(&mut self, segment: HyphaSegment) -> u32 {
        let start = segment.parent.map_or(segment.end, |p| self.hyphae.segments[p as usize].end);
        let index = self.hyphae.push(segment);
        self.hash.insert(start, segment.end);
        self.coarse.add((start + segment.end) / 2.0);
        index
    }

    fn sense(&mut self, at: Vec3, own: u32) -> Sensed {
        let unit = self.params.segment_length;
        // a tip doesn't sense the last few segments of its own hypha
        let mut lineage = [own; 3];
        for i in 1..3 {
            lineage[i] = self.hyphae.segments[lineage[i - 1] as usize].parent.unwrap_or(lineage[i - 1]);
        }
        let mut sensed = Sensed { density: 0.0, away: Vec3::ZERO, parallel: Vec3::ZERO };
        let mut near = Vec::new();
        self.hash.for_each_near(at, CUTOFF * unit, |i, d| near.push((i, d)));
        for (i, d) in near {
            if lineage.contains(&i) {
                continue;
            }
            let end = self.hyphae.segments[i as usize].end;
            let start = self.hyphae.start(i as usize);
            let d_units = (d / unit).max(MIN_DISTANCE);
            let falloff = 1.0 / (d_units * d_units);
            sensed.density += falloff;
            let closest = closest_point(at, start, end);
            sensed.away += (at - closest).try_normalize().unwrap_or(Vec3::ZERO) * falloff;
            // parallel to the segment, pointing to the end nearer the tip
            let axis = (end - start).try_normalize().unwrap_or(Vec3::ZERO);
            let towards = if at.distance_squared(end) <= at.distance_squared(start) { axis } else { -axis };
            sensed.parallel += towards / d_units;
        }
        sensed
    }

    fn gravitropism(&self, direction: Vec3, at: Vec3, angle: f32) -> Vec3 {
        let current = direction.y.clamp(-1.0, 1.0).acos();
        if (current - angle).abs() <= TOLERANCE {
            return Vec3::ZERO;
        }
        // turn in the vertical plane of the tip, upright tips turn away from the axis
        let horizontal = Vec3::new(direction.x, 0.0, direction.z);
        let radial = Vec3::new(at.x, 0.0, at.z);
        let outwards = if horizontal.length() > 0.2 { horizontal } else { radial + horizontal };
        let outwards = outwards.try_normalize().unwrap_or(Vec3::X);
        Vec3::Y * angle.cos() + outwards * angle.sin()
    }

    fn tissue(&self, end: Vec3, angle: f32) -> Tissue {
        if !self.stipe_started {
            Tissue::Mycelium
        } else if end.y < self.params.waist_height && angle <= std::f32::consts::FRAC_PI_2 {
            Tissue::Stipe
        } else {
            Tissue::Cap
        }
    }

    fn branch_direction(&mut self, direction: Vec3) -> Vec3 {
        let side = direction.cross(self.rng.unit()).try_normalize().unwrap_or(Vec3::X);
        (direction * BRANCH_ANGLE.cos() + side * BRANCH_ANGLE.sin()).normalize()
    }

    fn new_kind(&mut self) -> u32 {
        if self.rng.next() < self.params.leading_share {
            flags::LEADING
        } else {
            flags::STANDARD
        }
    }

    fn step(&mut self, iteration: usize) {
        let p = self.params;
        let t = iteration as f32 / p.iterations.max(1) as f32;
        let angle = p.gravitropic_angle.at(t) * std::f32::consts::PI;
        let autotropism = p.autotropism.at(t) * AUTOTROPISM_SCALE;
        let galvanotropism = p.galvanotropism.at(t);
        if angle < std::f32::consts::FRAC_PI_4 {
            self.stipe_started = true;
        }
        if autotropism > 0.0 {
            self.coarse.update(p.segment_length);
        }

        let tips = std::mem::take(&mut self.tips);
        let mut next_tips = Vec::with_capacity(tips.len() * 2);
        for tip in tips {
            if self.hyphae.len() >= p.max_segments {
                next_tips.push(tip);
                continue;
            }
            let at = self.hyphae.segments[tip.segment as usize].end;
            let sensed = self.sense(at, tip.segment);

            let mut direction = tip.direction * p.persistence
                + sensed.away.clamp_length_max(1.0) * DENSITY_IMPACT
                + self.coarse.sample(at).normalize_or_zero() * autotropism
                + self.gravitropism(tip.direction, at, angle) * GRAVITROPISM_IMPACT
                + self.rng.unit() * WANDER;
            if tip.kind != flags::LEADING {
                direction += sensed.parallel.clamp_length_max(1.0) * galvanotropism;
            }
            let mut direction = direction.try_normalize().unwrap_or(tip.direction);
            // tips don't grow into the ground
            if at.y + direction.y * p.segment_length < 0.0 {
                direction = Vec3::new(direction.x, 0.0, direction.z).try_normalize().unwrap_or(Vec3::X);
            }

            let end = at + direction * p.segment_length;
            self.hyphae.segments[tip.segment as usize].flags &= !flags::ACTIVE_TIP;
            let segment = self.add(HyphaSegment {
                end,
                radius: p.hypha_radius,
                parent: Some(tip.segment),
                birth: iteration as f32,
                tissue: self.tissue(end, angle),
                flags: tip.kind | flags::ACTIVE_TIP,
            });
            next_tips.push(Tip { segment, direction, kind: tip.kind });

            if sensed.density < p.density_threshold && self.rng.next() < p.branch_probability {
                let kind = self.new_kind();
                let direction = self.branch_direction(direction);
                next_tips.push(Tip { segment, direction, kind });
                self.branch_points.push((segment, iteration));
            }
        }

        // old branch points branch again once the hyphae around them have moved on
        let mut i = 0;
        while i < self.branch_points.len() {
            let (segment, born) = self.branch_points[i];
            if iteration < born + 4 || self.hyphae.len() + next_tips.len() >= p.max_segments {
                i += 1;
                continue;
            }
            let at = self.hyphae.segments[segment as usize].end;
            let sensed = self.sense(at, segment);
            if sensed.density < p.density_threshold * p.secondary_factor
                && self.rng.next() < p.branch_probability * 0.5
            {
                let parent = self.hyphae.start(segment as usize);
                let axis = (at - parent).try_normalize().unwrap_or(Vec3::Y);
                let direction = self.branch_direction(axis);
                next_tips.push(Tip { segment, direction, kind: flags::SECONDARY });
                self.branch_points.swap_remove(i);
            } else {
                i += 1;
            }
        }
        self.tips = next_tips;
    }

    /// Marks the cap segments that nothing covers from outside as cuticle
    fn mark_cuticle(&mut self) {
        let cap: Vec<usize> = (0..self.hyphae.len()).filter(|&i| self.hyphae.segments[i].tissue == Tissue::Cap).collect();
        if cap.is_empty() {
            return;
        }
        let center = cap.iter().map(|&i| self.hyphae.segments[i].end).sum::<Vec3>() / cap.len() as f32;
        let unit = self.params.segment_length;
        let mut cuticle = Vec::new();
        for &i in &cap {
            let end = self.hyphae.segments[i].end;
            let outwards = (end - center).try_normalize().unwrap_or(Vec3::Y);
            let mut covered = false;
            let segments = &self.hyphae.segments;
            self.hash.for_each_near(end, 3.0 * unit, |j, _| {
                if segments[j as usize].tissue == Tissue::Cap && (segments[j as usize].end - end).dot(outwards) > unit {
                    covered = true;
                }
            });
            if !covered {
                cuticle.push(i);
            }
        }
        for i in cuticle {
            self.hyphae.segments[i].tissue = Tissue::CapCuticle;
        }
    }
}

fn closest_point(p: Vec3, a: Vec3, b: Vec3) -> Vec3 {
    let axis = b - a;
    let length_sq = axis.length_squared();
    if length_sq <= 0.0 {
        return a;
    }
    a + axis * ((p - a).dot(axis) / length_sq).clamp(0.0, 1.0)
}

/// Grows hyphae from a knot of tips at the origin
pub fn grow(params: &GrowthParams) -> Hyphae {
    let iterations = params.iterations.min(MAX_ITERATIONS);
    let max_segments = params.max_segments.min(MAX_SEGMENTS);
    let unit = params.segment_length.max(1e-6);
    let params = &GrowthParams { iterations, max_segments, segment_length: unit, ..params.clone() };

    let reach = iterations as f32 * unit;
    let mut sim = Simulation {
        params,
        hyphae: Hyphae::default(),
        hash: SpatialHash::new(CUTOFF * unit),
        coarse: CoarseGrid::new(Vec3::new(0.0, reach * 0.4, 0.0), reach * 1.2),
        rng: Rng::new(params.seed),
        tips: Vec::new(),
        branch_points: Vec::new(),
        stipe_started: false,
    };

    for _ in 0..params.initial_tips.min(max_segments) {
        let offset = sim.rng.unit() * sim.rng.next().cbrt() * params.knot_radius;
        let at = Vec3::new(offset.x, offset.y.abs(), offset.z);
        let segment = sim.add(HyphaSegment {
            end: at,
            radius: params.hypha_radius,
            parent: None,
            birth: 0.0,
            tissue: Tissue::Mycelium,
            flags: flags::ACTIVE_TIP,
        });
        let direction = sim.rng.unit();
        sim.tips.push(Tip { segment, direction, kind: flags::STANDARD });
    }

    for iteration in 1..=iterations {
        if sim.hyphae.len() >= max_segments {
            break;
        }
        sim.step(iteration);
    }
    sim.mark_cuticle();
    sim.hyphae
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> GrowthParams {
        GrowthParams { max_segments: 3000, ..GrowthParams::default() }
    }

    fn tip_directions(h: &Hyphae) -> Vec<(Vec3, Vec3)> {
        (0..h.len())
            .filter(|&i| h.segments[i].flags & flags::ACTIVE_TIP != 0 && h.segments[i].parent.is_some())
            .map(|i| (h.segments[i].end, (h.segments[i].end - h.start(i)).normalize()))
            .collect()
    }

    #[test]
    fn same_seed_gives_the_same_hyphae() {
        assert_eq!(grow(&params()).encode(), grow(&params()).encode());
        assert_ne!(grow(&params()).encode(), grow(&GrowthParams { seed: 1, ..params() }).encode());
    }

    #[test]
    fn parents_come_first_and_the_budget_holds() {
        let h = grow(&GrowthParams { max_segments: 800, iterations: 300, ..params() });
        assert!(h.len() <= 800, "{} segments", h.len());
        // decode checks the parent order
        assert_eq!(Hyphae::decode(&h.encode()), h);
    }

    #[test]
    fn segments_keep_their_length() {
        let p = params();
        let h = grow(&p);
        for i in 0..h.len() {
            if h.segments[i].parent.is_some() {
                assert!((h.start(i).distance(h.segments[i].end) - p.segment_length).abs() < 1e-6);
            }
        }
    }

    #[test]
    fn galvanotropism_makes_neighbours_parallel() {
        let mean_angle = |galvano: f32| {
            let h = grow(&GrowthParams {
                galvanotropism: Schedule::constant(galvano),
                gravitropic_angle: Schedule::constant(0.5),
                autotropism: Schedule::constant(0.0),
                iterations: 40,
                // no branches, a fresh branch always sits at the branch angle to its sibling
                density_threshold: 0.0,
                initial_tips: 80,
                ..params()
            });
            let tips = tip_directions(&h);
            let unit = params().segment_length;
            let (mut sum, mut count) = (0.0, 0);
            for (i, &(p, d)) in tips.iter().enumerate() {
                for &(q, e) in &tips[i + 1..] {
                    if p.distance(q) < CUTOFF * unit {
                        sum += d.dot(e).clamp(-1.0, 1.0).acos();
                        count += 1;
                    }
                }
            }
            sum / count.max(1) as f32
        };
        let (without, with) = (mean_angle(0.0), mean_angle(1.0));
        assert!(with < without * 0.8, "mean angle between neighbours {} with, {} without", with, without);
    }

    #[test]
    fn tips_first_spread_then_point_up() {
        let mean_tip_y = |iterations: usize| {
            let h = grow(&GrowthParams {
                gravitropic_angle: Schedule(vec![0.0, 0.5, 0.5, 0.5, 0.5001, 0.0, 1.0, 0.0]),
                iterations,
                ..params()
            });
            let tips = tip_directions(&h);
            tips.iter().map(|(_, d)| d.y).sum::<f32>() / tips.len() as f32
        };
        // the schedule is read over each run's own iterations, so look at the state halfway and at the end
        let spread = grow(&GrowthParams {
            gravitropic_angle: Schedule::constant(0.5),
            iterations: 20,
            ..params()
        });
        let flat = tip_directions(&spread).iter().map(|(_, d)| d.y.abs()).sum::<f32>()
            / tip_directions(&spread).len() as f32;
        assert!(flat < 0.35, "tips should grow sideways, mean |y| {}", flat);
        assert!(mean_tip_y(40) > 0.8, "tips should point up at the end, mean y {}", mean_tip_y(40));
    }

    #[test]
    fn branches_only_below_the_density_threshold() {
        let small = GrowthParams { initial_tips: 10, iterations: 20, ..params() };
        let none = grow(&GrowthParams { density_threshold: 0.0, ..small.clone() });
        let some = grow(&GrowthParams { density_threshold: 1e9, ..small.clone() });
        // without branching every tip stays one hypha
        assert_eq!(none.len(), small.initial_tips * (small.iterations + 1));
        assert!(some.len() > none.len());
    }

    #[test]
    fn tissues_follow_the_schedule() {
        let h = grow(&GrowthParams::default());
        let count = |t: Tissue| h.segments.iter().filter(|s| s.tissue == t).count();
        assert!(count(Tissue::Mycelium) > 0);
        assert!(count(Tissue::Stipe) > 0);
        assert!(count(Tissue::Cap) + count(Tissue::CapCuticle) > 0);
        assert!(count(Tissue::CapCuticle) > 0);
        for s in &h.segments {
            if s.tissue == Tissue::Stipe {
                assert!(s.end.y < GrowthParams::default().waist_height);
            }
        }
    }

    #[test]
    fn huge_inputs_are_capped() {
        // each limit alone has to stop the growth
        let h = grow(&GrowthParams { iterations: usize::MAX, max_segments: 2000, ..params() });
        assert!(h.len() <= 2000);
        let h = grow(&GrowthParams { iterations: 30, max_segments: usize::MAX, ..params() });
        assert!(h.len() <= MAX_SEGMENTS);
    }
}

