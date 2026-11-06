//! Gills and the partial veil, grown onto the hyphae of a primordium.
//!
//! Gills: an angular prepattern around the axis marks thin radial sheets, one
//! per gill plus shorter lamellulae between the gills that only reach in from
//! the margin. The gill chamber is the space between the cap underside and the
//! stipe, measured from the top of the stipe. Tips branch off the cap underside
//! where the pattern is positive, from the horizontal down to just inside the
//! margin, and grow inside their sheet towards the top of the stipe. A tip stops
//! when it leaves its sheet, when it comes closer to the stipe than the gap of
//! free gills, or at the edge of the gills, a share of the way across the chamber.
//!
//! Veil: strands grow from the cap margin and from the stipe, at a height
//! between the margin and the top of the stipe, towards each other and meet
//! halfway. The ones from the stipe carry [`flags::STIPE_SIDE`], so
//! the veil can tear between the two halves.

use std::f32::consts::{FRAC_PI_2, PI, TAU};

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
/// Lamellulae reach in from the margin over this share of the gill length
const LAMELLULA_REACH: f32 = 0.5;
/// Gills round off over this many segment lengths next to the stipe
const INNER_ROUNDING: f32 = 4.0;
/// Gills end this many segment lengths inside the cap margin
const MARGIN_INSET: f32 = 3.0;
/// The cap underside is measured in rings of this angle around the hinge
const ANGLE_STEP: f32 = PI / 90.0;
const SECTORS: usize = 48;
/// Share of the sectors around the axis a ring of the cap has to cover to count, sparse
/// hyphae further out don't make a cap margin
const COVERAGE: f32 = 0.25;
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
    /// Where the veil meets the stipe, 0 level with the cap margin, 1 at the top of the stipe
    pub veil_attachment: f32,
    /// Budget for the added segments
    pub max_segments: usize,
}

impl Default for GillParams {
    fn default() -> Self {
        GillParams {
            seed: 0,
            gill_count: 32,
            lamellulae: 1,
            gill_depth: 0.7,
            veil: true,
            veil_attachment: 0.7,
            max_segments: 10_000,
        }
    }
}

fn radial(p: Vec3) -> f32 {
    (p.x * p.x + p.z * p.z).sqrt()
}

fn outwards(azimuth: f32) -> Vec3 {
    Vec3::new(azimuth.cos(), 0.0, azimuth.sin())
}

fn azimuth(p: Vec3) -> f32 {
    p.z.atan2(p.x).rem_euclid(TAU)
}

/// Angle of `d` to straight up
fn polar_angle(d: Vec3) -> f32 {
    radial(d).atan2(d.y)
}

/// Unit vector at `angle` to straight up, turned to `azimuth` around the axis
fn direction(angle: f32, azimuth: f32) -> Vec3 {
    outwards(azimuth) * angle.sin() + Vec3::Y * angle.cos()
}

fn quantile(values: &mut [f32], q: f32) -> f32 {
    values.sort_by(f32::total_cmp);
    values[((values.len() - 1) as f32 * q) as usize]
}

fn is_cap(tissue: Tissue) -> bool {
    matches!(tissue, Tissue::Cap | Tissue::CapCuticle)
}

/// What the gills and the veil need to know about the primordium. The cap is
/// measured in polar coordinates around the hinge, the top of the stipe on the
/// axis: the gill chamber is the space between the cap underside and the
/// stipe below the horizontal through the hinge.
#[derive(Debug)]
struct Shape {
    unit: f32,
    hinge: Vec3,
    stipe_radius: f32,
    /// Gills keep this far from the axis
    free_radius: f32,
    /// Distance of the cap underside from the hinge per bin of `ANGLE_STEP` of the angle to straight up
    underside: Vec<Option<f32>>,
    /// Where the cap ends, as angle to straight up
    margin_angle: f32,
    /// Stipe radius per height bin of `unit`
    stipe_profile: Vec<Option<f32>>,
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
        let hinge = Vec3::Y * top;
        let mut upper: Vec<f32> = stipe.iter().filter(|p| p.y > top * 0.5).map(|&p| radial(p)).collect();
        let stipe_radius = if upper.is_empty() { 0.0 } else { quantile(&mut upper, 0.9) };
        let free_radius = stipe_radius + STIPE_GAP * unit;

        let mut by_height = vec![Vec::new(); (top.max(0.0) / unit) as usize + 1];
        for &p in &stipe {
            if p.y >= 0.0 {
                let bin = ((p.y / unit) as usize).min(by_height.len() - 1);
                by_height[bin].push(radial(p));
            }
        }
        let stipe_profile = by_height.iter_mut().map(|r| (r.len() >= 3).then(|| quantile(r, 0.9))).collect();

        // the nearest cap hypha in every cell of a polar grid around the hinge, the
        // underside is a low quantile per ring of cells where the cap covers most of the way around
        let bins = (PI / ANGLE_STEP) as usize + 1;
        let mut nearest = vec![f32::INFINITY; bins * SECTORS];
        for &p in cap.iter().filter(|&&p| radial(p) > free_radius) {
            let d = p - hinge;
            let bin = ((polar_angle(d) / ANGLE_STEP) as usize).min(bins - 1);
            let sector = ((azimuth(d) / TAU * SECTORS as f32) as usize).min(SECTORS - 1);
            let cell = &mut nearest[bin * SECTORS + sector];
            *cell = cell.min(d.length());
        }
        let rings: Vec<Option<f32>> = nearest
            .chunks(SECTORS)
            .map(|ring| {
                let mut distances: Vec<f32> = ring.iter().copied().filter(|d| d.is_finite()).collect();
                (distances.len() as f32 >= SECTORS as f32 * COVERAGE).then(|| quantile(&mut distances, 0.1))
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
        let margin_bin = (0..bins).rev().find(|&b| underside[b].is_some())?;
        let margin_angle = (margin_bin as f32 + 0.5) * ANGLE_STEP;
        (margin_angle > FRAC_PI_2).then_some(Shape {
            unit,
            hinge,
            stipe_radius,
            free_radius,
            underside,
            margin_angle,
            stipe_profile,
        })
    }

    fn underside_at(&self, angle: f32) -> Option<f32> {
        self.underside.get((angle / ANGLE_STEP) as usize).copied().flatten()
    }

    fn stipe_radius_at(&self, y: f32) -> f32 {
        let bin = ((y.max(0.0) / self.unit) as usize).min(self.stipe_profile.len() - 1);
        self.stipe_profile[bin].unwrap_or(self.stipe_radius)
    }

    /// Where the gills end inside the cap margin
    fn gill_end(&self) -> f32 {
        let distance = self.underside_at(self.margin_angle).unwrap_or(self.unit);
        self.margin_angle - MARGIN_INSET * self.unit / distance
    }

    /// Distance from the hinge where the gills end at `angle`: a share `depth`
    /// of the way from the cap underside to the stipe, rounded off at the top
    /// of the gill chamber
    fn gill_edge(&self, angle: f32, depth: f32) -> Option<f32> {
        let underside = self.underside_at(angle)?;
        if angle <= FRAC_PI_2 {
            return None;
        }
        let stipe = self.free_radius / angle.sin();
        let rounding = ((angle - FRAC_PI_2) * underside / (INNER_ROUNDING * self.unit)).clamp(0.0, 1.0).sqrt();
        Some(underside - (underside - stipe).max(0.0) * depth * rounding)
    }
}

/// The angular prepattern: thin sheets, `gill_count · (lamellulae + 1)` around the axis
struct Prepattern {
    gill_count: usize,
    lamellulae: usize,
    /// Angles to straight up, seen from the hinge, the gills span
    inner_angle: f32,
    outer_angle: f32,
    half_thickness: f32,
}

impl Prepattern {
    fn sheets(&self) -> usize {
        self.gill_count * (self.lamellulae + 1)
    }

    fn azimuth(&self, sheet: usize) -> f32 {
        sheet as f32 * TAU / self.sheets() as f32
    }

    fn is_gill(&self, sheet: usize) -> bool {
        sheet.is_multiple_of(self.lamellulae + 1)
    }

    /// Where a sheet starts, lamellulae only reach in from the margin
    fn inner_angle(&self, sheet: usize) -> f32 {
        if self.is_gill(sheet) {
            self.inner_angle
        } else {
            self.outer_angle - (self.outer_angle - self.inner_angle) * LAMELLULA_REACH
        }
    }

    /// The sheet `d`, seen from the hinge, lies in, `None` on the negative part of the pattern between the sheets
    fn sheet_at(&self, d: Vec3) -> Option<usize> {
        let slot = TAU / self.sheets() as f32;
        let a = azimuth(d);
        let nearest = (a / slot).round();
        let sheet = nearest as usize % self.sheets();
        let inside = (a - nearest * slot).abs() * radial(d) <= self.half_thickness;
        let angle = polar_angle(d);
        (inside && angle >= self.inner_angle(sheet) && angle <= self.outer_angle).then_some(sheet)
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
        let (unit, hinge) = (self.shape.unit, self.shape.hinge);
        for sheet in 0..pattern.sheets() {
            let around = pattern.azimuth(sheet);
            let normal = Vec3::new(-around.sin(), 0.0, around.cos());
            let mut angle = pattern.inner_angle(sheet);
            while angle <= pattern.outer_angle {
                let underside = self.shape.underside_at(angle);
                let ray = direction(angle, around);
                angle += unit / underside.unwrap_or(unit).max(unit);
                let Some(underside) = underside else { continue };
                let seed = hinge + ray * underside;
                if radial(seed) < self.shape.free_radius {
                    continue;
                }
                // where the cap hyphae are sparse, branch off the next one further out
                let parent = (0..5).find_map(|k| self.attach(seed + ray * (k as f32 * ATTACH_REACH * unit), is_cap));
                let Some(parent) = parent else { continue };
                let Some(mut segment) = self.add(parent, seed, Tissue::Gill, flags::STANDARD) else { return };
                let mut at = seed;
                let mut heading = -ray;
                loop {
                    let inwards = (hinge - at).normalize_or_zero();
                    let mut d = heading + inwards + self.rng.unit() * WANDER;
                    // stay in the plane of the sheet and drift back to its middle
                    d -= normal * (d.dot(normal) + at.dot(normal) / unit);
                    heading = d.try_normalize().unwrap_or(inwards);
                    let next = at + heading * unit;
                    let from_hinge = next - hinge;
                    let inside = self
                        .shape
                        .gill_edge(polar_angle(from_hinge), depth)
                        .is_some_and(|edge| from_hinge.length() >= edge);
                    let free = radial(next) >= self.shape.free_radius;
                    if pattern.sheet_at(from_hinge) != Some(sheet) || !inside || !free {
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

    fn grow_veil(&mut self, attachment: f32) {
        let shape = self.shape;
        let Some(distance) = shape.underside_at(shape.margin_angle) else { return };
        let ring = distance * shape.margin_angle.sin();
        let strands = ((TAU * ring / (VEIL_SPACING * shape.unit)).ceil() as usize).max(3);
        for strand in 0..strands {
            let around = strand as f32 * TAU / strands as f32;
            let ray = direction(shape.margin_angle, around);
            let edge = shape.hinge + ray * distance;
            let margin = (0..5).find_map(|k| self.attach(edge + ray * (k as f32 * ATTACH_REACH * shape.unit), is_cap));
            let Some(margin) = margin else { continue };
            let a = self.hyphae.segments[margin as usize].end;
            let height = a.y.max(0.0) + (shape.hinge.y - a.y.max(0.0)).max(0.0) * attachment;
            let near_stipe = outwards(around) * shape.stipe_radius_at(height) + Vec3::Y * height;
            let Some(stipe) = self.attach(near_stipe, |t| matches!(t, Tissue::Stipe | Tissue::Mycelium)) else {
                continue;
            };
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
    if params.gill_count > 0 && shape.gill_end() > FRAC_PI_2 {
        let pattern = Prepattern {
            gill_count: params.gill_count.min(MAX_GILLS),
            lamellulae: params.lamellulae.min(MAX_LAMELLULAE),
            inner_angle: FRAC_PI_2,
            outer_angle: shape.gill_end(),
            half_thickness: SHEET_THICKNESS * shape.unit / 2.0,
        };
        builder.grow_gills(&pattern, params.gill_depth.clamp(0.0, 0.95));
    }
    if params.veil {
        builder.birth += 1.0;
        builder.grow_veil(params.veil_attachment.clamp(0.0, 1.0));
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

    /// Sheets around the axis: groups of gill hyphae, between the given angles
    /// to straight up seen from the hinge, separated by an empty angle
    fn count_sheets(h: &Hyphae, shape: &Shape, from: f32, to: f32) -> usize {
        let mut around: Vec<(f32, f32)> = h
            .segments
            .iter()
            .filter(|s| s.tissue == Tissue::Gill && (from..to).contains(&polar_angle(s.end - shape.hinge)))
            .map(|s| (azimuth(s.end), radial(s.end)))
            .collect();
        around.sort_by(|a, b| a.0.total_cmp(&b.0));
        let Some(&first) = around.first() else { return 0 };
        let gap = |a: (f32, f32), b: f32| (b - a.0) * a.1 > SHEET_THICKNESS * shape.unit;
        let wraps = !gap(*around.last().unwrap(), first.0 + TAU);
        around.windows(2).filter(|w| gap(w[0], w[1].0)).count() + usize::from(!wraps)
    }

    #[test]
    fn the_gill_count_matches_the_input() {
        let params = GillParams { gill_count: 24, lamellulae: 1, veil: false, ..GillParams::default() };
        let (h, shape) = with_gills(&params);
        let width = shape.gill_end() - FRAC_PI_2;
        let inner = FRAC_PI_2 + width * 0.15;
        let outer = FRAC_PI_2 + width * 0.75;
        assert_eq!(count_sheets(&h, &shape, inner, inner + width * 0.25), 24);
        assert_eq!(count_sheets(&h, &shape, outer, outer + width * 0.2), 48);
    }

    #[test]
    fn gills_stay_under_the_cap() {
        let (h, shape) = with_gills(&GillParams::default());
        for s in h.segments.iter().filter(|s| s.tissue == Tissue::Gill) {
            let d = s.end - shape.hinge;
            let angle = polar_angle(d);
            assert!(angle < shape.margin_angle, "gill at {} reaches past the cap margin", s.end);
            let underside = shape.underside_at(angle).expect("the cap covers every gill");
            assert!(d.length() <= underside + shape.unit, "gill at {} grows into the cap flesh", s.end);
        }
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
        let (h, shape) = with_gills(&GillParams::default());
        for s in h.segments.iter().filter(|s| s.tissue == Tissue::Gill) {
            let parent = &h.segments[s.parent.unwrap() as usize];
            assert!(matches!(parent.tissue, Tissue::Gill | Tissue::Cap | Tissue::CapCuticle));
            // the first segment branches off the cap onto the sheet, the rest grow from the cap towards the stipe
            if parent.tissue == Tissue::Gill {
                assert!(s.end.distance(shape.hinge) < parent.end.distance(shape.hinge), "gills grow inwards");
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
