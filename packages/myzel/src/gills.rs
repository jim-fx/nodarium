//! Gills under a cap, one for every stripe of a pattern, shared by `lamellae` (thin plates) and
//! `ridges` (blunt folds).
//!
//! The pattern (from `reaction_diffusion`) is a disc of values around the cap axis. Its radius
//! is the distance along the cap underside from the axis, measured along the rays of the cap, so
//! a gill stays on the same spot of the tissue while the cap opens: the rays bend but keep their
//! length. The stripes of every ring of the pattern are found and followed from ring to ring:
//! a stripe that starts between two others is a short gill (lamellula), one that splits off a
//! stripe is a fork, one that runs into another is a join.
//!
//! Every gill is a plate stacked from inside the cap flesh down to the gill edge, perpendicular
//! to the underside. The depth is read along the gill, alpha 0 at the stipe to 1 at the margin,
//! a gill that starts or ends on its own grows in and out over a few spacings, and the edge
//! waves sideways and up and down with `edge_wave`.
//!
//! A [`GillProfile::Fold`] is a false gill of Cantharellus: a low fold with a round back, a half
//! ellipse as wide as the thickness that flares into the flesh, never deeper than wide. As a
//! mesh it stays blunt at any mesh resolution, as paths meshed loose it would melt into fins with
//! slits between them.
//!
//! The gills that leave the stipe run on `decurrent` down it (decurrent gills, like Pleurotus
//! and Cantharellus), at the angle they meet it and lower and lower until they are gone, at the
//! latest where the stipe starts. Where they turn from the cap onto the stipe the way they hang
//! turns over about a depth, so their edge curves round the corner.

use crate::{add_sheet, smooth_normals, Cap, CapFrame, Mesh, SheetGrid};
use glam::{Vec2, Vec3};
use nodarium_utils::{
    geometry::{create_path, set_path_link, wrap_path, wrap_path_mut, PathLink},
    split_args, Ctx, Pattern,
};
use noise::{NoiseFn, OpenSimplex};
use std::f32::consts::{PI, TAU};

/// A stripe continues on the next ring if it is at most this many spacings away there
const LINK_REACH: f32 = 0.4;
/// Gills that start and end on their own and are shorter than this many spacings are left out
const MIN_LENGTH: f32 = 1.0;
/// A gill that starts or ends on its own grows to its full depth over this many spacings
const END_TAPER: f32 = 1.5;
/// Points along a gill, this many spacings apart
const POINT_SPACING: f32 = 0.5;
/// The paths of a plate are this many plate thicknesses apart, so they overlap into one sheet
const STRAND_GAP: f32 = 0.75;
const MAX_STRANDS: usize = 48;
/// The top path of a plate sits this share of the flesh half thickness inside the cap, so the
/// plate grows out of the flesh without a seam
pub(crate) const EMBED: f32 = 0.5;
/// Length of one wave of the gill edge, in spacings
const WAVE_LENGTH: f32 = 3.0;
/// How far the edge sways sideways at edge_wave 1, in spacings
const WAVE_SWAY: f32 = 0.33;
/// How much the depth changes along a wave at edge_wave 1, as a share of the depth
const WAVE_DEPTH: f32 = 0.15;
/// OpenSimplex rarely leaves ±0.5, this brings the waves close to their full size
const NOISE_GAIN: f32 = 2.0;
/// A ray that `cap` adds between others starts at half the flesh thickness and grows to the
/// full thickness over 4 of its start radii. The underside only follows it from there on, or it
/// would step up where the ray starts.
const RAY_RAMP: f32 = 4.0;
/// Where a gill turns from the stipe onto the cap, the way it hangs turns over about this many
/// depths on either side. Under 0.8 the edge folds over itself in a right-angled corner.
const CORNER: f32 = 1.2;
/// The depth is read at this many steps from the stipe to the margin, and in between linearly
const DEPTH_STEPS: usize = 64;

/// The cross-section of a gill
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GillProfile {
    /// A thin plate with a sharp edge, a true gill
    Plate,
    /// A low fold with a round back, never deeper than wide, a false gill
    Fold,
}

/// The inputs of a gill node as the host sends them. The sizes are read for every stipe, the
/// depth also along the gill.
pub struct GillInputs<'a> {
    /// The rays of the caps and their stipes
    pub cap: &'a [i32],
    pub pattern: &'a [i32],
    pub depth: &'a [i32],
    /// The gap between the gills and the stipe, none is 0
    pub free_distance: Option<&'a [i32]>,
    pub decurrent: &'a [i32],
    pub thickness: &'a [i32],
    pub edge_wave: &'a [i32],
    pub seed: i32,
    pub creator: i32,
}

/// The gills of all caps: the plates as a mesh, and every plate as a stack of paths from the top
/// inside the flesh to the gill edge, for the tests
pub struct Gills {
    pub mesh: Mesh,
    pub paths: Vec<Vec<i32>>,
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    if edge1 <= edge0 {
        return if x >= edge1 { 1.0 } else { 0.0 };
    }
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The angle from `a` to `b`, between -PI and PI
fn turn(a: f32, b: f32) -> f32 {
    (b - a + PI).rem_euclid(TAU) - PI
}

/// A point of a ray in the plane through the axis
#[derive(Clone, Copy, Debug)]
struct Section {
    /// Distance along the underside from the axis
    s: f32,
    /// Distance from the axis, negative on the far side
    out: f32,
    /// Height over the hinge along the axis
    up: f32,
    /// The side of the flesh that faces away from the gills, in (out, up)
    normal: Vec2,
    /// Half the flesh thickness
    radius: f32,
}

impl Section {
    fn lerp(&self, other: &Section, t: f32) -> Section {
        Section {
            s: self.s + (other.s - self.s) * t,
            out: self.out + (other.out - self.out) * t,
            up: self.up + (other.up - self.up) * t,
            normal: self.normal.lerp(other.normal, t).try_normalize().unwrap_or(self.normal),
            radius: self.radius + (other.radius - self.radius) * t,
        }
    }
}

struct Ray {
    angle: f32,
    sections: Vec<Section>,
    /// Where the ray has its full thickness
    settled: f32,
    /// From the axis to the tip
    length: f32,
}

impl Ray {
    fn reaches(&self, s: f32) -> bool {
        self.settled <= s && self.sections.last().is_some_and(|b| s <= b.s)
    }

    fn at(&self, s: f32) -> Section {
        let k = self.sections.partition_point(|a| a.s <= s).clamp(1, self.sections.len() - 1);
        let (a, b) = (&self.sections[k - 1], &self.sections[k]);
        let t = if b.s > a.s { ((s - a.s) / (b.s - a.s)).clamp(0.0, 1.0) } else { 0.0 };
        a.lerp(b, t)
    }
}

/// A spot of the cap, in the middle of its flesh
pub(crate) struct Spot {
    pub position: Vec3,
    /// Points from the gills into the flesh
    pub normal: Vec3,
    pub radius: f32,
}

/// The cap underside, from its rays, found by angle around the axis and distance from the axis
pub(crate) struct Underside {
    pub frame: CapFrame,
    /// Sorted by angle
    rays: Vec<Ray>,
    /// Distance along the rays from the axis to the margin, the middle of all rays. The pattern
    /// is stretched to the length of every ray, so the gills of a cap that reaches further on
    /// one side (a radius that changes around the axis) reach the margin all round.
    pub length: f32,
    /// Half the flesh thickness at the margin
    pub margin_radius: f32,
    /// The stretch starts here, the gills next to the stipe stay next to it
    pub anchor: f32,
}

impl Underside {
    pub fn new(frame: CapFrame, paths: &[&[i32]]) -> Option<Underside> {
        // angle, sections, the length of a ray that crosses the axis, where it starts from the axis
        let mut rays: Vec<(f32, Vec<Section>, Option<f32>, f32)> = Vec::new();
        for path in paths {
            let path = wrap_path(path);
            let n = path.length;
            if n < 2 {
                continue;
            }
            let point = |i: usize| Vec3::from_slice(&path.points[i * 4..i * 4 + 3]);
            let angle = frame.angle(point(n - 1));
            let dir = frame.out(angle);
            // distance from the axis (signed by the side of the axis) and height in the plane of the ray
            let plane: Vec<Vec2> = (0..n)
                .map(|i| {
                    let q = point(i) - frame.hinge;
                    let up = q.dot(frame.axis);
                    let across = (q - frame.axis * up).length();
                    Vec2::new(if q.dot(dir) < 0.0 { -across } else { across }, up)
                })
                .collect();
            let mut to_end = vec![0.0f32; n];
            for i in (0..n - 1).rev() {
                to_end[i] = to_end[i + 1] + point(i).distance(point(i + 1));
            }
            // a ray that starts in the stipe crosses the axis, from there on it measures the cap
            let crossing = (1..n).find(|&k| plane[k - 1].x < 0.0 && plane[k].x >= 0.0).map(|k| {
                let t = -plane[k - 1].x / (plane[k].x - plane[k - 1].x);
                to_end[k - 1] + (to_end[k] - to_end[k - 1]) * t
            });
            let sections: Vec<Section> = (0..n)
                .map(|i| {
                    let tangent =
                        (plane[(i + 1).min(n - 1)] - plane[i.saturating_sub(1)]).try_normalize().unwrap_or(Vec2::X);
                    Section {
                        s: -to_end[i],
                        out: plane[i].x,
                        up: plane[i].y,
                        normal: tangent.perp(),
                        radius: path.points[i * 4 + 3].abs(),
                    }
                })
                .collect();
            rays.push((angle, sections, crossing, plane[0].x + to_end[0]));
        }
        if rays.is_empty() {
            return None;
        }
        rays.sort_by(|a, b| a.0.total_cmp(&b.0));
        if rays.iter().all(|r| r.2.is_none()) {
            // no ray crosses the axis: each one is as long as it reaches from the axis. This also
            // keeps `crossing` from being empty, which the median below relies on
            for r in &mut rays {
                r.2 = Some(r.3);
            }
        }
        let crossing: Vec<(f32, f32)> = rays.iter().filter_map(|r| r.2.map(|l| (r.0, l))).collect();
        let mut lengths: Vec<f32> = crossing.iter().map(|c| c.1).collect();
        lengths.sort_by(f32::total_cmp);
        let length = lengths[lengths.len() / 2];
        // a ray that starts out on the cap is as long as the rays around it that cross the axis
        let around = |angle: f32| {
            let k = crossing.partition_point(|c| c.0 <= angle);
            let (a, b) = (crossing[(k + crossing.len() - 1) % crossing.len()], crossing[k % crossing.len()]);
            let (into, span) = ((angle - a.0).rem_euclid(TAU), (b.0 - a.0).rem_euclid(TAU));
            a.1 + (b.1 - a.1) * if span > 0.0 { into / span } else { 0.0 }
        };
        let margin_radius = rays.iter().map(|r| r.1.last().unwrap().radius).sum::<f32>() / rays.len() as f32;
        let rays: Vec<Ray> = rays
            .into_iter()
            .map(|(angle, sections, own, _)| {
                let length = own.unwrap_or_else(|| around(angle));
                let sections: Vec<Section> = sections.into_iter().map(|a: Section| Section { s: a.s + length, ..a }).collect();
                let settled = if own.is_some() { f32::MIN } else { sections[0].s + RAY_RAMP * sections[0].radius };
                Ray { angle, sections, settled, length }
            })
            .collect();
        Some(Underside { frame, rays, length, margin_radius, anchor: 0.0 })
    }

    /// Where a distance `s` of the pattern from the axis lies on a ray
    fn on_ray(&self, ray: &Ray, s: f32) -> f32 {
        let span = self.length - self.anchor;
        if span <= 0.0 || s <= self.anchor {
            return s;
        }
        self.anchor + (s - self.anchor) * (ray.length - self.anchor).max(0.0) / span
    }

    /// The underside at an angle and a distance of the pattern from the axis, between the
    /// nearest rays on either side that reach that far in
    pub fn at(&self, angle: f32, s: f32) -> Option<Spot> {
        let n = self.rays.len();
        let first = self.rays.partition_point(|r| r.angle <= angle);
        let reaches = |i: usize| self.rays[i].reaches(self.on_ray(&self.rays[i], s));
        let left = (1..=n).map(|k| (first + n - k) % n).find(|&i| reaches(i))?;
        let right = (0..n).map(|k| (first + k) % n).find(|&i| reaches(i))?;
        let (ray_a, ray_b) = (&self.rays[left], &self.rays[right]);
        let (a, b) = (ray_a.at(self.on_ray(ray_a, s)), ray_b.at(self.on_ray(ray_b, s)));
        let (to_left, to_right) = ((angle - ray_a.angle).rem_euclid(TAU), (ray_b.angle - angle).rem_euclid(TAU));
        let t = if to_left + to_right > 0.0 { to_left / (to_left + to_right) } else { 0.0 };
        let section = if left == right { a } else { a.lerp(&b, t) };
        let f = &self.frame;
        let out = f.out(angle);
        Some(Spot {
            position: f.hinge + f.axis * section.up + out * section.out,
            normal: (f.axis * section.normal.y + out * section.normal.x).try_normalize().unwrap_or(f.axis),
            radius: section.radius,
        })
    }
}

/// The angles of the stripes around a ring of the pattern: the middle of every run of values
/// over 0.5, weighted by how far they are over it
fn stripe_angles(values: &[f32]) -> Vec<f32> {
    let n = values.len();
    let on = |j: usize| values[j % n] > 0.5;
    let Some(gap) = (0..n).find(|&j| !on(j)) else {
        return Vec::new();
    };
    let cell = TAU / n as f32;
    let mut angles = Vec::new();
    let mut j = gap + 1;
    while j <= gap + n {
        if !on(j) {
            j += 1;
            continue;
        }
        let (mut weight, mut sum) = (0.0, 0.0);
        while j <= gap + n && on(j) {
            let w = values[j % n] - 0.5;
            weight += w;
            sum += w * (j as f32 + 0.5) * cell;
            j += 1;
        }
        angles.push((sum / weight).rem_euclid(TAU));
    }
    angles.sort_by(f32::total_cmp);
    angles
}

/// The stripe nearest to an angle, and how far it is around the axis
fn nearest(angles: &[f32], angle: f32) -> Option<(usize, f32)> {
    if angles.is_empty() {
        return None;
    }
    let k = angles.partition_point(|&a| a < angle);
    let candidates = [(k + angles.len() - 1) % angles.len(), k % angles.len()];
    candidates.into_iter().map(|i| (i, turn(angle, angles[i]).abs())).min_by(|a, b| a.1.total_cmp(&b.1))
}

/// A gill along the pattern, as (angle, distance from the axis) from the stipe outward
#[derive(Debug, Clone)]
struct Gill {
    points: Vec<(f32, f32)>,
    /// Starts between other gills instead of at the stipe or on a fork
    free_start: bool,
    /// Ends between other gills instead of at the margin or in a join
    free_end: bool,
}

/// Follows the stripes of the rings from `from` to `to` outward
fn trace_gills(pattern: &Pattern, from: usize, to: usize, reach: f32) -> Vec<Gill> {
    let mut gills: Vec<Gill> = Vec::new();
    let mut active: Vec<(f32, usize)> = Vec::new();
    for i in from..=to {
        let s = pattern.radius(i);
        let angles = stripe_angles(&pattern.rings[i]);
        if i == from {
            for &a in &angles {
                active.push((a, gills.len()));
                gills.push(Gill { points: vec![(a, s)], free_start: false, free_end: false });
            }
            continue;
        }
        let reach_angle = reach / s.max(f32::EPSILON);
        let previous: Vec<f32> = active.iter().map(|a| a.0).collect();
        // every stripe picks the stripe of the ring before that is nearest to it
        let picks: Vec<Option<(usize, f32)>> = angles.iter().map(|&a| nearest(&previous, a)).collect();
        let mut next = Vec::with_capacity(angles.len());
        let mut continued = vec![false; active.len()];
        for (k, &a) in angles.iter().enumerate() {
            match picks[k] {
                Some((p, d)) if d <= reach_angle => {
                    // the nearest of the stripes that picked this one continues its gill, the
                    // others fork off it
                    let best = angles
                        .iter()
                        .enumerate()
                        .filter(|(m, _)| matches!(picks[*m], Some((q, e)) if q == p && e <= reach_angle))
                        .min_by(|x, y| turn(previous[p], *x.1).abs().total_cmp(&turn(previous[p], *y.1).abs()))
                        .map(|(m, _)| m);
                    if best == Some(k) {
                        gills[active[p].1].points.push((a, s));
                        continued[p] = true;
                        next.push((a, active[p].1));
                    } else {
                        let fork = (previous[p], pattern.radius(i - 1));
                        next.push((a, gills.len()));
                        gills.push(Gill { points: vec![fork, (a, s)], free_start: false, free_end: false });
                    }
                }
                _ => {
                    next.push((a, gills.len()));
                    gills.push(Gill { points: vec![(a, s)], free_start: true, free_end: false });
                }
            }
        }
        // a gill that doesn't go on ends here, in the stripe next to it if that is close
        for (p, &(a, g)) in active.iter().enumerate() {
            if continued[p] {
                continue;
            }
            match nearest(&angles, a) {
                Some((k, d)) if d <= reach_angle => gills[g].points.push((angles[k], s)),
                _ => gills[g].free_end = true,
            }
        }
        active = next;
    }
    gills
}

/// Smooths the angles along a gill, the stripes of neighbouring rings jitter by a cell
fn smooth(points: &mut [(f32, f32)]) {
    let n = points.len();
    if n < 3 {
        return;
    }
    // unwrap so the angles don't jump at a full turn
    for i in 1..n {
        points[i].0 = points[i - 1].0 + turn(points[i - 1].0, points[i].0);
    }
    for _ in 0..2 {
        let copy: Vec<f32> = points.iter().map(|p| p.0).collect();
        for i in 1..n - 1 {
            points[i].0 = 0.25 * copy[i - 1] + 0.5 * copy[i] + 0.25 * copy[i + 1];
        }
    }
}

/// Points along a gill about `step` apart, by distance from the axis
fn resample(points: &[(f32, f32)], step: f32) -> Vec<(f32, f32)> {
    let (s0, s1) = (points[0].1, points[points.len() - 1].1);
    let count = (((s1 - s0) / step).ceil() as usize).max(1) + 1;
    let mut k = 0;
    (0..count)
        .map(|i| {
            let s = s0 + (s1 - s0) * i as f32 / (count - 1) as f32;
            while k + 2 < points.len() && points[k + 1].1 < s {
                k += 1;
            }
            let (a, b) = (points[k], points[(k + 1).min(points.len() - 1)]);
            let t = if b.1 > a.1 { ((s - a.1) / (b.1 - a.1)).clamp(0.0, 1.0) } else { 0.0 };
            (a.0 + (b.0 - a.0) * t, s)
        })
        .collect()
}

/// The depth along a gill, read at even steps from the stipe (0) to the margin (1)
pub(crate) struct DepthAlong(pub Vec<f32>);

impl DepthAlong {
    pub fn read(ctx: &mut Ctx, arg: &[i32]) -> DepthAlong {
        DepthAlong(ctx.float_along(arg, (0..=DEPTH_STEPS).map(|i| i as f32 / DEPTH_STEPS as f32)))
    }

    pub fn at(&self, along: f32) -> f32 {
        let x = along.clamp(0.0, 1.0) * (self.0.len() - 1) as f32;
        let k = (x as usize).min(self.0.len() - 2);
        let t = x - k as f32;
        (self.0[k] + (self.0[k + 1] - self.0[k]) * t).max(0.0)
    }
}

/// The sizes of the gills of one cap
struct Params {
    depth: DepthAlong,
    free_distance: f32,
    decurrent: f32,
    thickness: f32,
    edge_wave: f32,
    profile: GillProfile,
    seed: i32,
    creator: i32,
}

/// A gill plate at one point along the gill
#[derive(Clone, Copy)]
struct Slice {
    /// Inside the flesh, where the plate starts
    top: Vec3,
    /// Points from the gill edge into the flesh
    normal: Vec3,
    /// How far the gill edge sways sideways
    sway: Vec3,
    /// From the top to the gill edge
    reach: f32,
    /// Share of the reach inside the flesh, the gill comes out of it there
    surface: f32,
    /// 0 where a gill that starts or ends on its own begins, 1 at its full depth
    taper: f32,
    /// How fast the sway dies away from the gill edge, per share of the reach
    damping: f32,
}

impl Slice {
    /// The plate `f` of the way from the top to the gill edge. The edge waves like the rim of a
    /// long leaf: the waves die away within about a wavelength / TAU of the free edge (Liang &
    /// Mahadevan 2009), and the top stays where it is.
    fn at(&self, f: f32) -> Vec3 {
        let (rim, top) = ((-(1.0 - f) * self.damping).exp(), (-self.damping).exp());
        let sway = if top < 1.0 { (rim - top) / (1.0 - top) } else { f };
        self.top - self.normal * (f * self.reach) + self.sway * sway
    }
}

/// Rows of a plate from the top to the gill edge
const PLATE_ROWS: usize = 8;
/// A plate is this share of its thickness thinner at the gill edge than at the top
const EDGE_THINNING: f32 = 0.5;
/// A fold is this share of its width wider where it comes out of the flesh, so it grows out of
/// it in a curve instead of a crease
const FLARE: f32 = 0.6;
/// The flare is gone this share of the way from the flesh to the fold's back
const FLARE_REACH: f32 = 0.4;

/// Half the thickness of a gill `f` of the way from its top to its edge, as a share of half
/// the thickness. A plate thins towards its edge, a fold is a half ellipse out of the flesh, as
/// wide as the thickness and as deep as the depth.
fn half_share(f: f32, surface: f32, profile: GillProfile) -> f32 {
    match profile {
        GillProfile::Plate => 1.0 - EDGE_THINNING * f,
        GillProfile::Fold => {
            let z = if surface < 1.0 { ((f - surface) / (1.0 - surface)).clamp(0.0, 1.0) } else { 0.0 };
            (1.0 - z * z).max(0.0).sqrt() + FLARE * (1.0 - z / FLARE_REACH).max(0.0).powi(2)
        }
    }
}

/// The share of the reach inside the flesh. A gill of no depth keeps a sliver of its rows
/// outside it, so its faces don't collapse.
fn surface(inside: f32, reach: f32) -> f32 {
    if reach > 0.0 {
        (inside / reach).min(0.95)
    } else {
        0.95
    }
}

/// Where row `k` of a plate lies, from the top (0) to the gill edge (1). The first row is the
/// top inside the flesh, the others share out the part outside it, a fold's rows close in on
/// its round back.
fn row(k: usize, surface: f32, profile: GillProfile) -> f32 {
    if k == 0 {
        return 0.0;
    }
    let t = match profile {
        GillProfile::Plate => (k - 1) as f32 / (PLATE_ROWS - 2) as f32,
        GillProfile::Fold => (0.5 * PI * (k - 1) as f32 / (PLATE_ROWS - 1) as f32).sin(),
    };
    surface + (1.0 - surface) * t
}

/// Adds one gill as a closed plate: two faces, the top, the edge and both ends. A plate gets a
/// rounded edge, a fold its round back. `place` moves a vertex of slice `i` where it may be.
fn add_plate(mesh: &mut Mesh, plate: &[Slice], thickness: f32, profile: GillProfile, place: impl Fn(Vec3, usize) -> Vec3) {
    let m = plate.len();
    let along = |i: usize| (plate[(i + 1).min(m - 1)].top - plate[i.saturating_sub(1)].top).try_normalize().unwrap_or(Vec3::X);
    // both faces row by row from the top to the gill edge, then the gill edge between them
    let face = |side: f32, i: usize, k: usize| {
        let slice = &plate[i];
        let across = slice.normal.cross(along(i)).try_normalize().unwrap_or(Vec3::Z);
        let f = row(k, slice.surface, profile);
        let half = 0.5 * thickness * half_share(f, slice.surface, profile) * (0.5 + 0.5 * slice.taper);
        place(slice.at(f) + across * (side * half), i)
    };
    let edge = |i: usize| {
        let slice = &plate[i];
        let rim = match profile {
            GillProfile::Plate => 0.5 * thickness * (1.0 - EDGE_THINNING) * (0.5 + 0.5 * slice.taper),
            GillProfile::Fold => 0.0,
        };
        place(slice.at(1.0) - slice.normal * rim, i)
    };
    let grid = SheetGrid { columns: m, rows: PLATE_ROWS, wrap: false };
    add_sheet(mesh, grid, |i, k| face(1.0, i, k), |i, k| face(-1.0, i, k), Some(&edge));
}

/// The stipe a cap sits on, as its path: the middle line and the radius along it
struct Stipe {
    points: Vec<Vec3>,
    radius: Vec<f32>,
    /// Distance along the stipe from its base
    along: Vec<f32>,
}

impl Stipe {
    fn new(path: &[i32]) -> Option<Stipe> {
        let path = wrap_path(path);
        if path.length < 2 {
            return None;
        }
        let points: Vec<Vec3> = path.points.chunks_exact(4).map(|p| Vec3::new(p[0], p[1], p[2])).collect();
        let radius = path.points.chunks_exact(4).map(|p| p[3].abs()).collect();
        let mut along = vec![0.0f32; points.len()];
        for i in 1..points.len() {
            along[i] = along[i - 1] + points[i].distance(points[i - 1]);
        }
        Some(Stipe { points, radius, along })
    }

    /// How far along the stipe the point of its middle line nearest to `p` is
    fn nearest(&self, p: Vec3) -> f32 {
        let mut best = (f32::MAX, 0.0);
        for k in 1..self.points.len() {
            let (a, b) = (self.points[k - 1], self.points[k]);
            let ab = b - a;
            let t = if ab.length_squared() > 0.0 { ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
            let d = p.distance_squared(a + ab * t);
            if d < best.0 {
                best = (d, self.along[k - 1] + (self.along[k] - self.along[k - 1]) * t);
            }
        }
        best.1
    }

    /// The middle, the direction up the stipe and the radius, `s` along it
    fn at(&self, s: f32) -> (Vec3, Vec3, f32) {
        let n = self.points.len();
        let k = self.along.partition_point(|&a| a <= s).clamp(1, n - 1);
        let span = self.along[k] - self.along[k - 1];
        let t = if span > 0.0 { ((s - self.along[k - 1]) / span).clamp(0.0, 1.0) } else { 0.0 };
        let up = (self.points[k] - self.points[k - 1]).try_normalize().unwrap_or(Vec3::Y);
        (self.points[k - 1].lerp(self.points[k], t), up, self.radius[k - 1] + (self.radius[k] - self.radius[k - 1]) * t)
    }

    fn length(&self) -> f32 {
        *self.along.last().unwrap()
    }
}

/// A gill that runs on from the cap underside `length` down the stipe, at the angle it meets the
/// stipe, lower and lower until it is gone. The slices go down from where the gill meets the
/// stipe and stop where the stipe starts.
fn run_down(stipe: &Stipe, frame: &CapFrame, first: &Slice, angle: f32, length: f32, step: f32) -> Vec<Slice> {
    let embed = first.reach * first.surface;
    let depth = first.reach - embed;
    // the length counts from where the gill comes out of the cap underside
    let start = stipe.nearest(first.top - first.normal * embed).min(stipe.length());
    let count = ((length / step.max(1e-5)).ceil() as usize).max(1);
    (1..=count)
        .map_while(|k| {
            let down = length * k as f32 / count as f32;
            let s = start - down;
            if s < 0.0 {
                return None;
            }
            let (middle, up, radius) = stipe.at(s);
            let out = frame.out(angle);
            let out = (out - up * up.dot(out)).try_normalize()?;
            let inside = embed.min(0.5 * radius);
            let fade = smoothstep(0.0, 1.0, 1.0 - down / length);
            let reach = depth * fade + inside;
            Some(Slice {
                top: middle + out * (radius - inside),
                normal: -out,
                sway: Vec3::ZERO,
                reach,
                surface: surface(inside, reach),
                taper: first.taper * fade,
                damping: first.damping,
            })
        })
        .collect()
}

/// Turns the gill round the corner from the stipe onto the cap underside at slice `corner`:
/// the direction the gill hangs in turns over about CORNER depths on both sides, so the edge
/// curves round the corner instead of folding over itself
fn round_corner(plate: &mut [Slice], corner: usize) {
    let n = plate.len();
    if corner == 0 || corner >= n {
        return;
    }
    let mut along = vec![0.0f32; n];
    for i in 1..n {
        along[i] = along[i - 1] + plate[i].top.distance(plate[i - 1].top);
    }
    let width = CORNER * plate[corner].reach.max(plate[corner - 1].reach).max(1e-6);
    let normals: Vec<Vec3> = plate.iter().map(|s| s.normal).collect();
    for i in 0..n {
        let near = (-((along[i] - along[corner]) / (2.0 * width)).powi(2)).exp();
        if near < 1e-3 {
            continue;
        }
        let mut sum = Vec3::ZERO;
        for j in 0..n {
            sum += normals[j] * (-0.5 * ((along[j] - along[i]) / width).powi(2)).exp();
        }
        let smooth = sum.try_normalize().unwrap_or(normals[i]);
        plate[i].normal = normals[i].lerp(smooth, near).try_normalize().unwrap_or(normals[i]);
    }
}

/// The gills of one cap, added to `plates`, and as stacks of paths
fn cap_gills(underside: &mut Underside, stipe: &[i32], pattern: &Pattern, p: &Params, plates: &mut Mesh) -> Vec<Vec<i32>> {
    let spacing = pattern.spacing.max(1e-5);
    let frame = underside.frame;
    let free = frame.stipe_radius + p.free_distance.max(0.0);
    // gills run from the free distance to the margin, inside the pattern
    let (lo, hi) = (free.max(pattern.inner), (underside.length - underside.margin_radius).min(pattern.outer));
    if pattern.rings.len() < 2 || hi <= lo {
        return Vec::new();
    }
    underside.anchor = lo;
    let from = (0..pattern.rings.len()).find(|&i| pattern.radius(i) >= lo);
    let to = (0..pattern.rings.len()).rev().find(|&i| pattern.radius(i) <= hi);
    let (Some(from), Some(to)) = (from, to) else {
        return Vec::new();
    };
    if to <= from {
        return Vec::new();
    }
    let mut gills = trace_gills(pattern, from, to, LINK_REACH * spacing);
    gills.retain(|g| {
        let length = g.points[g.points.len() - 1].1 - g.points[0].1;
        length >= if g.free_start && g.free_end { MIN_LENGTH * spacing } else { 0.5 * spacing }
    });

    let thickness = p.thickness.max(1e-6);
    let deepest = p.depth.0.iter().fold(0.0f32, |a, &b| a.max(b));
    let embed = EMBED * underside.rays.iter().flat_map(|r| r.sections.iter()).map(|a| a.radius).fold(0.0, f32::max);
    let strands = (((embed + deepest) / (STRAND_GAP * thickness)).ceil() as usize + 1).clamp(2, MAX_STRANDS);
    let wave = p.edge_wave.clamp(0.0, 1.0);
    let decurrent = p.decurrent.max(0.0);
    let stipe_path = Stipe::new(stipe);
    let underside = &*underside;

    let mut paths = Vec::new();
    for (g, gill) in gills.iter_mut().enumerate() {
        smooth(&mut gill.points);
        let points = resample(&gill.points, POINT_SPACING * spacing);
        let (start, end) = (points[0].1, points[points.len() - 1].1);
        let sway = OpenSimplex::new(p.seed.wrapping_add(g as i32 * 2) as u32);
        let lift = OpenSimplex::new(p.seed.wrapping_add(g as i32 * 2 + 1) as u32);
        // the plate at every point: where its top is, which way is down and how deep it is
        let mut plate: Vec<Slice> = points
            .iter()
            .map_while(|&(angle, s)| {
                let spot = underside.at(angle, s)?;
                let along = ((s - lo) / (hi - lo)).clamp(0.0, 1.0);
                let mut taper = 1.0;
                if gill.free_start {
                    taper *= smoothstep(0.0, END_TAPER * spacing, s - start);
                }
                if gill.free_end {
                    taper *= smoothstep(0.0, END_TAPER * spacing, end - s);
                }
                let x = (s / (WAVE_LENGTH * spacing)) as f64;
                let side = wave * WAVE_SWAY * spacing * NOISE_GAIN * sway.get([x, 0.0]) as f32;
                let deeper = 1.0 + wave * WAVE_DEPTH * NOISE_GAIN * lift.get([x, 0.0]) as f32;
                let d = p.depth.at(along) * taper * deeper;
                let d = match p.profile {
                    GillProfile::Plate => d,
                    GillProfile::Fold => d.min(thickness),
                };
                // the middle of the flesh, the underside is a flesh half thickness further down
                let top = spot.position - spot.normal * ((1.0 - EMBED) * spot.radius);
                let sideways = frame.out(angle + 0.25 * TAU);
                let inside = EMBED * spot.radius;
                let reach = d + inside;
                let damping = reach * TAU / (WAVE_LENGTH * spacing);
                let surface = surface(inside, reach);
                Some(Slice { top, normal: spot.normal, sway: sideways * side, reach, surface, taper, damping })
            })
            .collect();
        if plate.len() < 2 {
            continue;
        }
        let mut angles: Vec<f32> = points.iter().take(plate.len()).map(|a| a.0).collect();
        // a gill that leaves the stipe runs on down it
        let down = match &stipe_path {
            Some(stipe) if decurrent > 0.0 && !gill.free_start && start - lo < spacing => {
                run_down(stipe, &frame, &plate[0], points[0].0, decurrent, POINT_SPACING * spacing)
            }
            _ => Vec::new(),
        };
        let corner = down.len();
        if corner > 0 {
            angles.splice(0..0, std::iter::repeat_n(points[0].0, corner));
            plate.splice(0..0, down.into_iter().rev());
            round_corner(&mut plate, corner);
        }
        // keep the free distance to the stipe, a gill that runs down it is on it
        let keep_free = |q: Vec3, angle: f32| {
            if corner > 0 {
                return q;
            }
            let rel = q - frame.hinge;
            let flat = rel - frame.axis * rel.dot(frame.axis);
            let from_axis = flat.length();
            if from_axis < free {
                q + flat.try_normalize().unwrap_or_else(|| frame.out(angle)) * (free - from_axis)
            } else {
                q
            }
        };
        add_plate(plates, &plate, thickness, p.profile, |q, i| keep_free(q, angles[i]));
        for j in 0..strands {
            let f = j as f32 / (strands - 1) as f32;
            let link = PathLink::child(p.creator, frame.stipe_id, frame.alpha, paths.len() as i32);
            let mut data = create_path(plate.len(), stipe[3] + 1);
            set_path_link(&mut data, link);
            let path = wrap_path_mut(&mut data);
            for (m, slice) in plate.iter().enumerate() {
                let q = keep_free(slice.at(f), angles[m]);
                let r = 0.5 * thickness * half_share(f, slice.surface, p.profile) * (0.5 + 0.5 * slice.taper);
                path.points[m * 4..m * 4 + 4].copy_from_slice(&[q.x, q.y, q.z, r]);
            }
            paths.push(data);
        }
    }
    paths
}

/// The gills under every cap of `inputs.cap`, see the module docs
pub fn gills(inputs: &GillInputs, profile: GillProfile) -> Gills {
    gills_by(inputs, profile, |ctx| DepthAlong::read(ctx, inputs.depth))
}

fn gills_by(inputs: &GillInputs, profile: GillProfile, mut read_depth: impl FnMut(&mut Ctx) -> DepthAlong) -> Gills {
    let mut mesh = Mesh::default();
    let mut paths = Vec::new();
    let Some(pattern) = Pattern::decode(inputs.pattern) else {
        return Gills { mesh, paths };
    };
    let mut ctx = Ctx::default();
    let items = split_args(inputs.cap);
    let caps = Cap::all(&items);
    for (rank, cap) in caps.iter().enumerate() {
        let stipe = items[cap.stipe];
        ctx.set_path(stipe, rank, caps.len());
        ctx.alpha = cap.frame.alpha;
        let params = Params {
            depth: read_depth(&mut ctx),
            free_distance: inputs.free_distance.map_or(0.0, |arg| ctx.float(arg)),
            decurrent: ctx.float(inputs.decurrent),
            thickness: ctx.float(inputs.thickness),
            edge_wave: ctx.float(inputs.edge_wave),
            profile,
            seed: inputs.seed,
            creator: inputs.creator,
        };
        let mut rays: Vec<usize> = cap.rays.iter().map(|r| r.path).collect();
        rays.sort_unstable();
        let rays: Vec<&[i32]> = rays.into_iter().map(|i| items[i]).collect();
        if let Some(mut underside) = Underside::new(cap.frame, &rays) {
            paths.extend(cap_gills(&mut underside, stipe, &pattern, &params, &mut mesh));
        }
    }
    smooth_normals(&mut mesh);
    Gills { mesh, paths }
}

#[cfg(test)]
mod tests;
