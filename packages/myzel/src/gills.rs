//! Gills and the partial veil, grown onto the hyphae of a primordium.
//!
//! Gills: an angular prepattern around the axis marks thin radial sheets, one
//! per gill plus shorter lamellulae between the gills that only reach in from
//! the margin. Tips branch off the cap underside where the pattern is positive
//! and grow down inside their sheet. A tip stops when it leaves its sheet, when
//! it comes closer to the stipe than the gap of free gills, or at the lower
//! edge of the gills.
//!
//! Veil: strands grow from the cap margin and from the stipe towards each other
//! and meet halfway. The ones from the stipe carry [`flags::STIPE_SIDE`], so
//! the veil can tear between the two halves.

use std::f32::consts::TAU;

use glam::Vec3;

use crate::growth::{Rng, MAX_SEGMENTS};
use crate::hyphae::{flags, HyphaSegment, Hyphae, Tissue};
use crate::spatial::SpatialHash;

pub const MAX_GILLS: usize = 512;
pub const MAX_LAMELLULAE: usize = 7;

/// Thickness of a gill sheet in segment lengths
const SHEET_THICKNESS: f32 = 1.2;
/// Free gills keep this many segment lengths away from the stipe
const STIPE_GAP: f32 = 2.0;
/// Lamellulae reach in from the margin over this share of the gill width
const LAMELLULA_REACH: f32 = 0.5;
/// Gills round off over this many segment lengths next to the stipe
const INNER_ROUNDING: f32 = 4.0;
/// Distance of the veil strands at the margin, in segment lengths
const VEIL_SPACING: f32 = 1.5;
const WANDER: f32 = 0.2;
/// How far a gill or veil hypha may look for the segment it branches from, in segment lengths
const ATTACH_REACH: f32 = 4.0;

#[derive(Clone, Debug, PartialEq)]
pub struct GillParams {
    pub seed: u32,
    pub gill_count: usize,
    /// Short gills between two neighbouring gills
    pub lamellulae: usize,
    /// How far the gills reach down into the space under the cap, 0 to 1
    pub gill_depth: f32,
    pub veil: bool,
    /// Budget for the added segments
    pub max_segments: usize,
}

impl Default for GillParams {
    fn default() -> Self {
        GillParams { seed: 0, gill_count: 32, lamellulae: 1, gill_depth: 0.7, veil: true, max_segments: 10_000 }
    }
}

fn radial(p: Vec3) -> f32 {
    (p.x * p.x + p.z * p.z).sqrt()
}

fn outwards(angle: f32) -> Vec3 {
    Vec3::new(angle.cos(), 0.0, angle.sin())
}

fn quantile(values: &mut [f32], q: f32) -> f32 {
    values.sort_by(f32::total_cmp);
    values[((values.len() - 1) as f32 * q) as usize]
}

fn is_cap(tissue: Tissue) -> bool {
    matches!(tissue, Tissue::Cap | Tissue::CapCuticle)
}

/// What the gills and the veil need to know about the primordium, all around the Y axis
#[derive(Debug)]
struct Shape {
    unit: f32,
    stipe_radius: f32,
    /// Gills start this far from the axis
    free_radius: f32,
    /// Where the cap comes down lowest, the gills end and the veil starts there
    margin_radius: f32,
    margin_y: f32,
    /// Height of the cap underside per ring of width `unit`
    underside: Vec<Option<f32>>,
}

impl Shape {
    fn measure(hyphae: &Hyphae) -> Option<Shape> {
        let mut lengths: Vec<f32> = hyphae.iter().map(|(a, b, ..)| a.distance(b)).filter(|&l| l > 0.0).collect();
        if lengths.is_empty() {
            return None;
        }
        let unit = quantile(&mut lengths, 0.5);
        let ends = |keep: fn(Tissue) -> bool| -> Vec<Vec3> {
            hyphae.segments.iter().filter(|s| keep(s.tissue)).map(|s| s.end).collect()
        };
        let stipe = ends(|t| t == Tissue::Stipe);
        let cap = ends(is_cap);
        if stipe.is_empty() || cap.is_empty() {
            return None;
        }

        let top = stipe.iter().map(|p| p.y).fold(f32::MIN, f32::max);
        let mut upper: Vec<f32> = stipe.iter().filter(|p| p.y > top * 0.5).map(|&p| radial(p)).collect();
        let stipe_radius = if upper.is_empty() { 0.0 } else { quantile(&mut upper, 0.9) };
        let free_radius = stipe_radius + STIPE_GAP * unit;

        // the lowest cap hypha in every cell of a polar grid, the underside is a low quantile per ring
        let cap_radius = cap.iter().map(|&p| radial(p)).fold(0.0, f32::max);
        let rings = (cap_radius / unit) as usize + 1;
        let sectors = ((TAU * cap_radius / unit).ceil() as usize).max(8);
        let mut lowest = vec![f32::INFINITY; rings * sectors];
        for &p in &cap {
            let ring = ((radial(p) / unit) as usize).min(rings - 1);
            let sector = ((p.z.atan2(p.x).rem_euclid(TAU) / TAU * sectors as f32) as usize).min(sectors - 1);
            let cell = &mut lowest[ring * sectors + sector];
            *cell = cell.min(p.y);
        }
        let rings: Vec<Option<f32>> = lowest
            .chunks(sectors)
            .map(|ring| {
                let mut heights: Vec<f32> = ring.iter().copied().filter(|h| h.is_finite()).collect();
                (heights.len() >= 3).then(|| quantile(&mut heights, 0.1))
            })
            .collect();
        // averaged with the neighbouring rings, so the top edge of the gills is smooth
        let underside: Vec<Option<f32>> = (0..rings.len())
            .map(|i| {
                rings[i]?;
                let near: Vec<f32> = rings[i.saturating_sub(2)..(i + 3).min(rings.len())].iter().flatten().copied().collect();
                Some(near.iter().sum::<f32>() / near.len() as f32)
            })
            .collect();

        let (margin_ring, margin_y) = underside
            .iter()
            .enumerate()
            .filter(|&(ring, _)| (ring as f32 + 0.5) * unit >= free_radius)
            .filter_map(|(ring, y)| y.map(|y| (ring, y)))
            .min_by(|a, b| a.1.total_cmp(&b.1))?;
        Some(Shape {
            unit,
            stipe_radius,
            free_radius,
            margin_radius: (margin_ring as f32 + 0.5) * unit,
            margin_y,
            underside,
        })
    }

    fn underside_at(&self, r: f32) -> Option<f32> {
        self.underside.get((r / self.unit) as usize).copied().flatten()
    }

    /// Lowest point of the gills at distance `r` from the axis
    fn gill_edge(&self, r: f32, depth: f32) -> Option<f32> {
        let top = self.underside_at(r)?;
        let rounding = ((r - self.free_radius) / (INNER_ROUNDING * self.unit)).clamp(0.0, 1.0).sqrt();
        Some(top - (top - self.margin_y).max(0.0) * depth * rounding)
    }
}

/// The angular prepattern: thin sheets, `gill_count · (lamellulae + 1)` around the axis
struct Prepattern {
    gill_count: usize,
    lamellulae: usize,
    free_radius: f32,
    margin_radius: f32,
    half_thickness: f32,
}

impl Prepattern {
    fn sheets(&self) -> usize {
        self.gill_count * (self.lamellulae + 1)
    }

    fn angle(&self, sheet: usize) -> f32 {
        sheet as f32 * TAU / self.sheets() as f32
    }

    fn is_gill(&self, sheet: usize) -> bool {
        sheet.is_multiple_of(self.lamellulae + 1)
    }

    /// Where a sheet starts, lamellulae only reach in from the margin
    fn inner_radius(&self, sheet: usize) -> f32 {
        if self.is_gill(sheet) {
            self.free_radius
        } else {
            self.margin_radius - (self.margin_radius - self.free_radius) * LAMELLULA_REACH
        }
    }

    /// The sheet `p` lies in, `None` on the negative part of the pattern between the sheets
    fn sheet_at(&self, p: Vec3) -> Option<usize> {
        let r = radial(p);
        let slot = TAU / self.sheets() as f32;
        let angle = p.z.atan2(p.x).rem_euclid(TAU);
        let nearest = (angle / slot).round();
        let sheet = nearest as usize % self.sheets();
        let inside = (angle - nearest * slot).abs() * r <= self.half_thickness;
        (inside && r >= self.inner_radius(sheet) && r <= self.margin_radius).then_some(sheet)
    }
}

struct Builder<'a> {
    hyphae: Hyphae,
    hash: SpatialHash,
    rng: Rng,
    shape: &'a Shape,
    budget: usize,
    birth: f32,
}

impl Builder<'_> {
    fn add(&mut self, parent: u32, end: Vec3, tissue: Tissue, flags: u32) -> Option<u32> {
        if self.budget == 0 {
            return None;
        }
        self.budget -= 1;
        Some(self.hyphae.push(HyphaSegment {
            end,
            radius: self.hyphae.segments[parent as usize].radius,
            parent: Some(parent),
            birth: self.birth,
            tissue,
            flags,
        }))
    }

    /// The segment of the given tissue whose end is nearest to `p`, within `ATTACH_REACH`
    fn attach(&mut self, p: Vec3, keep: impl Fn(Tissue) -> bool) -> Option<u32> {
        let segments = &self.hyphae.segments;
        let mut best: Option<(u32, f32)> = None;
        self.hash.for_each_near(p, ATTACH_REACH * self.shape.unit, |i, _| {
            let s = &segments[i as usize];
            let d = s.end.distance(p);
            if keep(s.tissue) && best.is_none_or(|(_, b)| d < b) {
                best = Some((i, d));
            }
        });
        best.map(|(i, _)| i)
    }

    fn grow_gills(&mut self, pattern: &Prepattern, depth: f32) {
        let unit = self.shape.unit;
        for sheet in 0..pattern.sheets() {
            let angle = pattern.angle(sheet);
            let normal = Vec3::new(-angle.sin(), 0.0, angle.cos());
            let mut r = pattern.inner_radius(sheet);
            while r <= pattern.margin_radius {
                let seed = self.shape.underside_at(r).map(|y| outwards(angle) * r + Vec3::Y * y);
                r += unit;
                let Some(mut seed) = seed else { continue };
                // where the cap hyphae are sparse, branch off the next one above
                let parent = (0..5).find_map(|k| self.attach(seed + Vec3::Y * (k as f32 * ATTACH_REACH * unit), is_cap));
                let Some(parent) = parent else { continue };
                seed.y = seed.y.max(self.hyphae.segments[parent as usize].end.y - unit);
                let Some(mut segment) = self.add(parent, seed, Tissue::Gill, flags::STANDARD) else { return };
                let (mut at, mut direction) = (seed, Vec3::NEG_Y);
                loop {
                    let mut d = direction + Vec3::NEG_Y + self.rng.unit() * WANDER;
                    // stay in the plane of the sheet and drift back to its middle
                    d -= normal * (d.dot(normal) + at.dot(normal) / unit);
                    direction = d.try_normalize().unwrap_or(Vec3::NEG_Y);
                    let next = at + direction * unit;
                    let above_edge = self.shape.gill_edge(radial(next), depth).is_some_and(|edge| next.y >= edge);
                    if pattern.sheet_at(next) != Some(sheet) || !above_edge {
                        break;
                    }
                    let Some(s) = self.add(segment, next, Tissue::Gill, flags::STANDARD) else { return };
                    (segment, at) = (s, next);
                }
            }
        }
    }

    /// A veil strand from `parent` to `target`
    fn grow_strand(&mut self, mut parent: u32, target: Vec3, side: u32) {
        let unit = self.shape.unit;
        let mut at = self.hyphae.segments[parent as usize].end;
        let mut direction = (target - at).normalize_or_zero();
        let steps = (at.distance(target) / unit * 2.0) as usize + 4;
        for _ in 0..steps {
            let left = at.distance(target);
            if left < unit {
                break;
            }
            let towards = (target - at) / left;
            direction = (direction + towards + self.rng.unit() * WANDER).try_normalize().unwrap_or(towards);
            at += direction * unit;
            let Some(s) = self.add(parent, at, Tissue::Veil, side) else { return };
            parent = s;
        }
    }

    fn grow_veil(&mut self) {
        let shape = self.shape;
        let strands = ((TAU * shape.margin_radius / (VEIL_SPACING * shape.unit)).ceil() as usize).max(3);
        for strand in 0..strands {
            let angle = strand as f32 * TAU / strands as f32;
            let out = outwards(angle);
            let Some(margin) = self.attach(out * shape.margin_radius + Vec3::Y * shape.margin_y, is_cap) else {
                continue;
            };
            let near_stipe = out * shape.stipe_radius + Vec3::Y * shape.margin_y;
            let Some(stipe) = self.attach(near_stipe, |t| matches!(t, Tissue::Stipe | Tissue::Mycelium)) else {
                continue;
            };
            let a = self.hyphae.segments[margin as usize].end;
            let b = self.hyphae.segments[stipe as usize].end;
            let middle = (a + b) / 2.0;
            self.grow_strand(margin, middle, flags::STANDARD);
            self.grow_strand(stipe, middle, flags::STIPE_SIDE);
        }
    }
}

/// Adds gills under the cap and a closed veil between cap margin and stipe.
/// Hyphae without stipe or cap come back unchanged.
pub fn grow_gills(hyphae: &Hyphae, params: &GillParams) -> Hyphae {
    let Some(shape) = Shape::measure(hyphae) else { return hyphae.clone() };
    let mut builder = Builder {
        hyphae: hyphae.clone(),
        hash: SpatialHash::from_segments(ATTACH_REACH * shape.unit, hyphae.iter().map(|(a, b, ..)| (a, b))),
        rng: Rng::new(params.seed),
        shape: &shape,
        budget: params.max_segments.min(MAX_SEGMENTS),
        birth: hyphae.segments.iter().map(|s| s.birth).fold(0.0, f32::max) + 1.0,
    };
    if params.gill_count > 0 && shape.margin_radius > shape.free_radius {
        let pattern = Prepattern {
            gill_count: params.gill_count.min(MAX_GILLS),
            lamellulae: params.lamellulae.min(MAX_LAMELLULAE),
            free_radius: shape.free_radius,
            margin_radius: shape.margin_radius,
            half_thickness: SHEET_THICKNESS * shape.unit / 2.0,
        };
        builder.grow_gills(&pattern, params.gill_depth.clamp(0.0, 0.95));
    }
    if params.veil {
        builder.birth += 1.0;
        builder.grow_veil();
    }
    builder.hyphae
}

#[cfg(test)]
mod tests {
    use std::sync::OnceLock;

    use super::*;
    use crate::growth::{grow, GrowthParams};

    fn primordium() -> &'static Hyphae {
        static PRIMORDIUM: OnceLock<Hyphae> = OnceLock::new();
        PRIMORDIUM.get_or_init(|| grow(&GrowthParams::default()))
    }

    fn with_gills(params: &GillParams) -> (Hyphae, Shape) {
        (grow_gills(primordium(), params), Shape::measure(primordium()).unwrap())
    }

    /// Sheets around the axis: groups of gill hyphae separated by an empty angle
    fn count_sheets(h: &Hyphae, from: f32, to: f32) -> usize {
        let mut angles: Vec<f32> = h
            .segments
            .iter()
            .filter(|s| s.tissue == Tissue::Gill && (from..to).contains(&radial(s.end)))
            .map(|s| s.end.z.atan2(s.end.x).rem_euclid(TAU))
            .collect();
        angles.sort_by(f32::total_cmp);
        let Some(&first) = angles.first() else { return 0 };
        let gap = |a: f32, b: f32| (b - a) * from > SHEET_THICKNESS * 0.0004;
        let wraps = !gap(*angles.last().unwrap(), first + TAU);
        angles.windows(2).filter(|w| gap(w[0], w[1])).count() + usize::from(!wraps)
    }

    #[test]
    fn the_gill_count_matches_the_input() {
        let params = GillParams { gill_count: 24, lamellulae: 1, veil: false, ..GillParams::default() };
        let (h, shape) = with_gills(&params);
        let width = shape.margin_radius - shape.free_radius;
        let inner = shape.free_radius + width * 0.15;
        let outer = shape.free_radius + width * 0.75;
        assert_eq!(count_sheets(&h, inner, inner + width * 0.25), 24);
        assert_eq!(count_sheets(&h, outer, outer + width * 0.2), 48);
    }

    #[test]
    fn gills_are_free_of_the_stipe() {
        let (h, shape) = with_gills(&GillParams::default());
        assert!(shape.stipe_radius > 0.0);
        let mut gills = 0;
        for (i, s) in h.segments.iter().enumerate() {
            if s.tissue == Tissue::Gill {
                gills += 1;
                assert!(radial(s.end) > shape.stipe_radius, "gill segment {} touches the stipe", i);
            }
        }
        assert!(gills > 500);
    }

    #[test]
    fn gills_hang_under_the_cap() {
        let (h, _) = with_gills(&GillParams::default());
        for s in h.segments.iter().filter(|s| s.tissue == Tissue::Gill) {
            let parent = &h.segments[s.parent.unwrap() as usize];
            assert!(matches!(parent.tissue, Tissue::Gill | Tissue::Cap | Tissue::CapCuticle));
            // the first segment branches off the cap onto the sheet, the rest grow down
            if parent.tissue == Tissue::Gill {
                assert!(s.end.y < parent.end.y, "gills grow down");
            }
        }
    }

    #[test]
    fn the_veil_connects_cap_margin_and_stipe() {
        let (h, shape) = with_gills(&GillParams::default());
        let side = |i: usize| h.segments[i].flags & flags::STIPE_SIDE != 0;
        let mut has_children = vec![false; h.len()];
        for s in &h.segments {
            if let Some(p) = s.parent {
                has_children[p as usize] = true;
            }
        }
        let mut ends = [Vec::new(), Vec::new()];
        for (i, s) in h.segments.iter().enumerate().filter(|(_, s)| s.tissue == Tissue::Veil) {
            // a veil hypha starts on the side it remembers
            let mut root = i;
            while h.segments[root].tissue == Tissue::Veil {
                assert_eq!(side(root), side(i));
                root = h.segments[root].parent.unwrap() as usize;
            }
            let anchor = h.segments[root].tissue;
            if side(i) {
                assert!(matches!(anchor, Tissue::Stipe | Tissue::Mycelium));
            } else {
                assert!(is_cap(anchor));
            }
            if !has_children[i] {
                ends[side(i) as usize].push(s.end);
            }
        }
        let [cap_side, stipe_side] = ends;
        assert!(cap_side.len() > 50);
        assert_eq!(cap_side.len(), stipe_side.len());
        for p in cap_side {
            let meets = stipe_side.iter().any(|q| q.distance(p) < 2.0 * shape.unit);
            assert!(meets, "veil strand ending at {} doesn't meet the stipe side", p);
        }
    }

    #[test]
    fn the_input_hyphae_stay_as_they_are() {
        let h = grow_gills(primordium(), &GillParams::default());
        assert_eq!(&h.segments[..primordium().len()], &primordium().segments[..]);
        assert_eq!(h, grow_gills(primordium(), &GillParams::default()));
    }

    #[test]
    fn the_budget_holds() {
        let h = grow_gills(primordium(), &GillParams { max_segments: 100, ..GillParams::default() });
        assert_eq!(h.len(), primordium().len() + 100);
        let none = GillParams { gill_count: 0, veil: false, ..GillParams::default() };
        assert_eq!(&grow_gills(primordium(), &none), primordium());
    }

    #[test]
    fn hyphae_without_a_cap_come_back_unchanged() {
        let mut h = Hyphae::default();
        let root = h.push(HyphaSegment {
            end: Vec3::ZERO,
            radius: 0.001,
            parent: None,
            birth: 0.0,
            tissue: Tissue::Stipe,
            flags: 0,
        });
        h.push(HyphaSegment { end: Vec3::Y * 0.01, parent: Some(root), ..h.segments[0] });
        assert_eq!(grow_gills(&h, &GillParams::default()), h);
        assert_eq!(grow_gills(&Hyphae::default(), &GillParams::default()), Hyphae::default());
    }
}
