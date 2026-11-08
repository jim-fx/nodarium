//! Gill plates under a cap, one for every stripe of a pattern.
//!
//! The pattern (from `reaction_diffusion`) is a disc of values around the cap axis. Its radius
//! is the distance along the cap underside from the axis, measured along the rays of the cap, so
//! a gill stays on the same spot of the tissue while the cap opens: the rays bend but keep their
//! length. The stripes of every ring of the pattern are found and followed from ring to ring:
//! a stripe that starts between two others is a short gill (lamellula), one that splits off a
//! stripe is a fork, one that runs into another is a join.
//!
//! Every gill is a plate of thin paths, stacked from inside the cap flesh down to the gill edge,
//! perpendicular to the underside. The depth follows `depth_curve` from the stipe to the margin,
//! a gill that starts or ends on its own grows in and out over a few spacings, and the edge
//! waves sideways and up and down with `edge_wave`.
//!
//! `blunt` turns the plates into the false gills of Cantharellus: low folds with a round back,
//! a half ellipse as wide as the thickness that flares into the flesh, never deeper than wide.
//! As a mesh they stay blunt at any mesh resolution, as paths meshed loose they'd melt into
//! fins with slits between them.
//!
//! With the stipe connected, the gills that leave it run on `decurrent` down it (decurrent
//! gills, like Pleurotus and Cantharellus), at the angle they meet it and lower and lower until
//! they are gone. Where they turn from the cap onto the stipe the way they hang turns over
//! about a depth, so their edge curves round the corner.

use glam::{Vec2, Vec3};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{
    concat_arg_vecs, curve, decode_float, decode_points,
    geometry::{create_path, orthogonal_to, set_path_link, wrap_path, wrap_path_mut, PathLink},
    int, split_args, Ctx, Pattern, Point,
};
use myzel_core::Mesh;
use noise::{NoiseFn, OpenSimplex};
use std::f32::consts::{PI, TAU};

nodarium_definition_file!("src/definition.json");

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
const EMBED: f32 = 0.5;
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

/// The words inside the brackets of a framed list `[0, n + 1, ...words, 1, 1]`
fn unframe(arg: &[i32]) -> Option<&[i32]> {
    let n = arg.len();
    let framed = n >= 4 && arg[0] == 0 && arg[1] as usize == n - 3 && arg[n - 2] == 1 && arg[n - 1] == 1;
    framed.then(|| &arg[2..n - 2])
}

/// A list of floats as the host sends a curve: one frame for the value on the node,
/// `concat_args([frame])` from a connected node
fn read_floats(arg: &[i32]) -> Vec<f32> {
    let n = arg.len();
    let connected = n >= 8 && arg[..2] == [0, 1] && arg[n - 2..] == [1, 1] && unframe(&arg[2..n - 2]).is_some();
    let frame = if connected { &arg[2..n - 2] } else { arg };
    unframe(frame).map_or_else(Vec::new, |words| words.iter().map(|&w| decode_float(w)).collect())
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
    (b - a + std::f32::consts::PI).rem_euclid(TAU) - std::f32::consts::PI
}

/// The frame of a cap: its axis and the side its angles count from, as `cap` uses them
#[derive(Clone, Copy)]
struct Frame {
    hinge: Vec3,
    axis: Vec3,
    side: Vec3,
    around: Vec3,
}

impl Frame {
    fn new(hinge: &Point) -> Frame {
        let axis = hinge.tangent.try_normalize().unwrap_or(Vec3::Y);
        let side = orthogonal_to(axis);
        Frame { hinge: hinge.position, axis, side, around: axis.cross(side) }
    }

    fn out(&self, angle: f32) -> Vec3 {
        self.side * angle.cos() + self.around * angle.sin()
    }

    fn angle(&self, p: Vec3) -> f32 {
        let q = p - self.hinge;
        q.dot(self.around).atan2(q.dot(self.side)).rem_euclid(TAU)
    }
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
struct Spot {
    position: Vec3,
    /// Points from the gills into the flesh
    normal: Vec3,
    radius: f32,
}

/// The cap underside, from its rays, found by angle around the axis and distance from the axis
struct Underside {
    frame: Frame,
    /// Sorted by angle
    rays: Vec<Ray>,
    /// Distance along the rays from the axis to the margin, the middle of all rays. The pattern
    /// is stretched to the length of every ray, so the gills of a cap that reaches further on
    /// one side (cap_lopsided) reach the margin all round.
    length: f32,
    /// Half the flesh thickness at the margin
    margin_radius: f32,
    /// The stretch starts here, the gills next to the stipe stay next to it
    anchor: f32,
}

impl Underside {
    fn new(frame: Frame, paths: &[&[i32]]) -> Option<Underside> {
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
    fn at(&self, angle: f32, s: f32) -> Option<Spot> {
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

pub struct Params {
    pub depth: f32,
    pub depth_curve: Vec<f32>,
    pub free_distance: f32,
    pub decurrent: f32,
    pub thickness: f32,
    pub blunt: f32,
    pub edge_wave: f32,
    pub seed: i32,
    pub creator: i32,
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
/// A blunt fold is this share of its width wider where it comes out of the flesh, so it grows
/// out of it in a curve instead of a crease
const FLARE: f32 = 0.6;
/// The flare is gone this share of the way from the flesh to the fold's back
const FLARE_REACH: f32 = 0.4;

/// Half the thickness of a gill `f` of the way from its top to its edge, as a share of half
/// the thickness. A plate thins towards its edge, a blunt fold is a half ellipse out of the
/// flesh, as wide as the thickness and as deep as the depth.
fn half_share(f: f32, surface: f32, blunt: f32) -> f32 {
    let z = if surface < 1.0 { ((f - surface) / (1.0 - surface)).clamp(0.0, 1.0) } else { 0.0 };
    let plate = 1.0 - EDGE_THINNING * f;
    let fold = (1.0 - z * z).max(0.0).sqrt() + FLARE * (1.0 - z / FLARE_REACH).max(0.0).powi(2);
    plate + (fold - plate) * blunt
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
/// top inside the flesh, the others share out the part outside it, a blunt fold's rows close
/// in on its round back.
fn row(k: usize, surface: f32, blunt: f32) -> f32 {
    if k == 0 {
        return 0.0;
    }
    let t = (k - 1) as f32 / (PLATE_ROWS - 2) as f32;
    let round = (0.5 * PI * (k - 1) as f32 / (PLATE_ROWS - 1) as f32).sin();
    surface + (1.0 - surface) * (t + (round - t) * blunt)
}

/// Adds one gill as a closed plate: two faces, the top, the edge and both ends. A thin plate
/// gets a rounded edge, a blunt fold its round back. `place` moves a vertex of slice `i` where
/// it may be.
fn add_plate(mesh: &mut Mesh, plate: &[Slice], thickness: f32, blunt: f32, place: impl Fn(Vec3, usize) -> Vec3) {
    let m = plate.len();
    let base = mesh.vertex_count() as u32;
    let mut push = |p: Vec3, i: usize| {
        let p = place(p, i);
        mesh.positions.extend([p.x, p.y, p.z]);
        mesh.normals.extend([0.0; 3]);
        mesh.groups.push(0);
    };
    let along = |i: usize| (plate[(i + 1).min(m - 1)].top - plate[i.saturating_sub(1)].top).try_normalize().unwrap_or(Vec3::X);
    // both faces row by row, then the gill edge between them
    for side in [1.0f32, -1.0] {
        for (i, slice) in plate.iter().enumerate() {
            let across = slice.normal.cross(along(i)).try_normalize().unwrap_or(Vec3::Z);
            for k in 0..PLATE_ROWS {
                let f = row(k, slice.surface, blunt);
                let half = 0.5 * thickness * half_share(f, slice.surface, blunt) * (0.5 + 0.5 * slice.taper);
                push(slice.at(f) + across * (side * half), i);
            }
        }
    }
    for (i, slice) in plate.iter().enumerate() {
        let rim = (1.0 - blunt) * 0.5 * thickness * (1.0 - EDGE_THINNING) * (0.5 + 0.5 * slice.taper);
        push(slice.at(1.0) - slice.normal * rim, i);
    }
    let a = |i: usize, k: usize| base + (i * PLATE_ROWS + k) as u32;
    let b = |i: usize, k: usize| base + ((m + i) * PLATE_ROWS + k) as u32;
    let edge = |i: usize| base + (2 * m * PLATE_ROWS + i) as u32;
    let faces = &mut mesh.faces;
    let mut quad = |p: u32, q: u32, r: u32, t: u32| faces.extend([p, q, r, p, r, t]);
    for i in 0..m - 1 {
        for k in 0..PLATE_ROWS - 1 {
            quad(a(i, k), a(i + 1, k), a(i + 1, k + 1), a(i, k + 1));
            quad(b(i, k), b(i, k + 1), b(i + 1, k + 1), b(i + 1, k));
        }
        // the top, inside the flesh
        quad(a(i + 1, 0), a(i, 0), b(i, 0), b(i + 1, 0));
        let last = PLATE_ROWS - 1;
        quad(a(i, last), a(i + 1, last), edge(i + 1), edge(i));
        quad(edge(i), edge(i + 1), b(i + 1, last), b(i, last));
    }
    for k in 0..PLATE_ROWS - 1 {
        quad(a(0, k), a(0, k + 1), b(0, k + 1), b(0, k));
        quad(a(m - 1, k), b(m - 1, k), b(m - 1, k + 1), a(m - 1, k + 1));
    }
    let last = PLATE_ROWS - 1;
    mesh.faces.extend([a(0, last), edge(0), b(0, last)]);
    mesh.faces.extend([a(m - 1, last), b(m - 1, last), edge(m - 1)]);
}

/// Vertex normals from the faces around every vertex, weighted by their area
fn smooth_normals(mesh: &mut Mesh) {
    let p = |i: u32| Vec3::from_slice(&mesh.positions[i as usize * 3..i as usize * 3 + 3]);
    let mut normals = vec![Vec3::ZERO; mesh.vertex_count()];
    for f in mesh.faces.chunks_exact(3) {
        let n = (p(f[1]) - p(f[0])).cross(p(f[2]) - p(f[0]));
        for &v in f {
            normals[v as usize] += n;
        }
    }
    mesh.normals = normals.iter().flat_map(|n| n.try_normalize().unwrap_or(Vec3::Y).to_array()).collect();
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

    /// The length of the stipe
    fn length(&self) -> f32 {
        *self.along.last().unwrap()
    }
}

/// A gill that runs on from the cap underside `length` down the stipe, at the angle it meets the
/// stipe, lower and lower until it is gone. The slices go down from where the gill meets the stipe.
fn run_down(stipe: &Stipe, frame: &Frame, first: &Slice, angle: f32, length: f32, step: f32) -> Vec<Slice> {
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

/// The gills of one cap, as stacks of paths and as plates
fn cap_gills(underside: &mut Underside, hinge: &Point, stipe: Option<&Stipe>, pattern: &Pattern, p: &Params, plates: &mut Mesh) -> Vec<Vec<i32>> {
    let spacing = pattern.spacing.max(1e-5);
    let free = hinge.scale.abs() + p.free_distance.max(0.0);
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
    let blunt = p.blunt.clamp(0.0, 1.0);
    let depth = p.depth.max(0.0);
    let embed = EMBED * underside.rays.iter().flat_map(|r| r.sections.iter()).map(|a| a.radius).fold(0.0, f32::max);
    let strands = (((embed + depth) / (STRAND_GAP * thickness)).ceil() as usize + 1).clamp(2, MAX_STRANDS);
    let wave = p.edge_wave.clamp(0.0, 1.0);
    let decurrent = p.decurrent.max(0.0);
    let frame = underside.frame;
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
                let d = depth * curve(&p.depth_curve, along).max(0.0) * taper * deeper;
                // a blunt fold is never deeper than it is wide
                let d = d + (d.min(thickness) - d) * blunt;
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
        let down = match stipe {
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
        add_plate(plates, &plate, thickness, blunt, |q, i| keep_free(q, angles[i]));
        for j in 0..strands {
            let f = j as f32 / (strands - 1) as f32;
            let link = PathLink::child(p.creator, hinge.parent_id, 1.0, paths.len() as i32);
            let mut data = create_path(plate.len(), hinge.depth + 1);
            set_path_link(&mut data, link);
            let path = wrap_path_mut(&mut data);
            for (m, slice) in plate.iter().enumerate() {
                let q = keep_free(slice.at(f), angles[m]);
                let r = 0.5 * thickness * half_share(f, slice.surface, blunt) * (0.5 + 0.5 * slice.taper);
                path.points[m * 4..m * 4 + 4].copy_from_slice(&[q.x, q.y, q.z, r]);
            }
            paths.push(data);
        }
    }
    paths
}

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> (Vec<i32>, Vec<i32>) {
    let mut ctx = Ctx::default();
    let mut plates = Mesh::default();
    // args follow the key order of the inputs in definition.json, so a new input there shifts these
    let Some(pattern) = Pattern::decode(args[2]) else {
        return (concat_arg_vecs(Vec::new()), plates.encode());
    };
    let rays = split_args(args[0]);
    let stipes = split_args(args[3]);
    let hinges = decode_points(args[1]);
    let mut output = Vec::new();
    for (rank, hinge) in hinges.iter().enumerate() {
        // the rays of this cap grow on its stipe
        let own: Vec<&[i32]> = rays.iter().copied().filter(|r| r.len() > 9 && wrap_path(r).link.parent_id == hinge.parent_id).collect();
        let stipe_path = stipes.iter().copied().find(|s| s.len() > 9 && wrap_path(s).link.id == hinge.parent_id);
        // the sizes are read for every stipe, so they can follow the size of the mushroom
        match stipe_path {
            Some(path) => ctx.set_path(path, rank, hinges.len()),
            None => ctx.seed_path(hinge.parent_id),
        }
        ctx.alpha = 1.0;
        let params = Params {
            depth: ctx.float(args[4]),
            depth_curve: read_floats(args[5]),
            free_distance: ctx.float(args[6]),
            decurrent: ctx.float(args[7]),
            thickness: ctx.float(args[8]),
            blunt: ctx.float(args[9]),
            edge_wave: ctx.float(args[10]),
            seed: int(args[11]),
            creator: int(args[12]),
        };
        let stipe = stipe_path.and_then(Stipe::new);
        if let Some(mut underside) = Underside::new(Frame::new(hinge), &own) {
            output.extend(cap_gills(&mut underside, hinge, stipe.as_ref(), &pattern, &params, &mut plates));
        }
    }
    smooth_normals(&mut plates);
    (concat_arg_vecs(output), plates.encode())
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodarium_utils::{encode_float, encode_points, geometry::path_link};

    const CREATOR: i32 = 9;
    const STIPE_ID: i32 = 1234;
    const STIPE_RADIUS: f32 = 0.005;
    const RADIUS: f32 = 0.025;
    const HINGE: Vec3 = Vec3::new(0.0, 0.05, 0.0);

    /// The middle of the flesh of an open cap in the plane of a ray: it starts inside the stipe
    /// on the far side of the axis, rises over the axis and falls along a straight slope to the
    /// margin, `RADIUS` out and 6 mm below the hinge
    fn midline(x: f32) -> f32 {
        let top = 0.004;
        if x <= 0.0 {
            top + x * 2.0
        } else {
            top - (top + 0.006) * x / RADIUS
        }
    }

    /// Half the flesh thickness, from 4 mm over the axis to 1 mm at the margin
    fn flesh(x: f32) -> f32 {
        0.004 - 0.003 * (x / RADIUS).clamp(0.0, 1.0)
    }

    /// 12 rays that start in the stipe and 12 that start 12 mm out between them, like `cap`
    /// makes them. `bend` turns the outer part of every ray down, like a closing cap.
    fn cap(bend: f32) -> Vec<i32> {
        let mut rays = Vec::new();
        for k in 0..24 {
            let primary = k % 2 == 0;
            let angle = TAU * k as f32 / 24.0 + 0.01;
            let out = Vec3::new(angle.cos(), 0.0, angle.sin());
            let start = if primary { -0.0015 } else { 0.012 };
            let n = 48;
            let mut data = create_path(n, 2);
            set_path_link(&mut data, PathLink::child(7, STIPE_ID, 1.0, k));
            let path = wrap_path_mut(&mut data);
            let mut previous = Vec2::new(start, midline(start));
            let mut position = previous;
            for i in 0..n {
                let x = start + (RADIUS - start) * i as f32 / (n - 1) as f32;
                let target = Vec2::new(x, midline(x));
                if i > 0 {
                    // the segment keeps its length and turns down by `bend` towards the margin
                    let step = target - previous;
                    let turn = bend * (x / RADIUS).max(0.0);
                    position += Vec2::from_angle(-turn).rotate(step);
                }
                previous = target;
                let p = HINGE + out * position.x + Vec3::Y * position.y;
                // an added ray starts at half the flesh and grows to all of it, like in `cap`
                let ramp = if primary { 1.0 } else { 0.5 + 0.5 * smoothstep(0.0, 2.0 * flesh(start), x - start) };
                path.points[i * 4..i * 4 + 4].copy_from_slice(&[p.x, p.y, p.z, flesh(x) * ramp]);
            }
            rays.push(data);
        }
        concat_arg_vecs(rays)
    }

    fn hinge() -> Vec<i32> {
        encode_points(&[Point {
            position: HINGE,
            direction: Vec3::X,
            tangent: Vec3::Y,
            scale: STIPE_RADIUS,
            parent_id: STIPE_ID,
            alpha: 1.0,
            depth: 1,
            index: 0,
        }])
    }

    /// `inner` stripes from 4 mm out, every gap gets a new stripe from `split` on
    fn pattern(inner: usize, split: f32) -> Pattern {
        let (from, to) = (0.004, 0.03);
        let rings = 105;
        let rings = (0..rings)
            .map(|i| {
                let r = from + (to - from) * i as f32 / (rings - 1) as f32;
                let n = (TAU * r / 0.0001) as usize;
                let stripes = if r < split { inner } else { inner * 2 };
                (0..n).map(|j| 0.5 + 0.5 * (stripes as f32 * nodarium_utils::ring_angle(j, n)).cos()).collect()
            })
            .collect();
        Pattern { inner: from, outer: to, spacing: TAU * 0.01 / inner as f32, rings }
    }

    fn params() -> Params {
        Params {
            depth: 0.006,
            depth_curve: vec![0.0, 0.3, 0.2, 0.85, 0.45, 1.0, 0.8, 0.65, 1.0, 0.1],
            free_distance: 0.001,
            decurrent: 0.0,
            thickness: 0.0004,
            blunt: 0.0,
            edge_wave: 0.3,
            seed: 2,
            creator: CREATOR,
        }
    }

    /// The stipe under the cap: upright from the ground to the hinge
    fn stipe() -> Vec<i32> {
        let n = 21;
        let mut data = create_path(n, 1);
        set_path_link(&mut data, PathLink { id: STIPE_ID, ..PathLink::root(3, 0) });
        let path = wrap_path_mut(&mut data);
        for i in 0..n {
            let y = HINGE.y * i as f32 / (n - 1) as f32;
            path.points[i * 4..i * 4 + 4].copy_from_slice(&[HINGE.x, y, HINGE.z, STIPE_RADIUS]);
        }
        concat_arg_vecs(vec![data])
    }

    fn floats(values: &[f32]) -> Vec<i32> {
        let mut words = vec![0, values.len() as i32 + 1];
        words.extend(values.iter().map(|&v| encode_float(v)));
        words.extend([1, 1]);
        words
    }

    fn run_with(cap: &[i32], pattern: &Pattern, p: &Params, curve: Vec<i32>) -> Vec<Vec<i32>> {
        let (out, _) = execute_both(cap, pattern, p, curve);
        split_args(&out).iter().map(|s| s.to_vec()).collect()
    }

    fn plates(cap: &[i32], pattern: &Pattern, p: &Params) -> Mesh {
        Mesh::decode(&execute_both(cap, pattern, p, floats(&p.depth_curve)).1)
    }

    fn execute_both(cap: &[i32], pattern: &Pattern, p: &Params, curve: Vec<i32>) -> (Vec<i32>, Vec<i32>) {
        execute_on(cap, &stipe(), pattern, p, curve)
    }

    fn execute_on(cap: &[i32], stipe: &[i32], pattern: &Pattern, p: &Params, curve: Vec<i32>) -> (Vec<i32>, Vec<i32>) {
        __nodarium_execute_inner(&[
            cap,
            &hinge(),
            &pattern.encode(),
            stipe,
            &[encode_float(p.depth)],
            &curve,
            &[encode_float(p.free_distance)],
            &[encode_float(p.decurrent)],
            &[encode_float(p.thickness)],
            &[encode_float(p.blunt)],
            &[encode_float(p.edge_wave)],
            &[p.seed],
            &[p.creator],
        ])
    }

    fn run(cap: &[i32], pattern: &Pattern, p: &Params) -> Vec<Vec<i32>> {
        run_with(cap, pattern, p, floats(&p.depth_curve))
    }

    fn points(path: &[i32]) -> Vec<Vec3> {
        wrap_path(path).points.chunks_exact(4).map(|p| Vec3::new(p[0], p[1], p[2])).collect()
    }

    fn from_axis(p: Vec3) -> f32 {
        Vec2::new(p.x - HINGE.x, p.z - HINGE.z).length()
    }

    #[test]
    fn gills_stay_under_the_cap_and_inside_the_margin() {
        let gills = run(&cap(0.0), &pattern(24, 0.014), &params());
        assert!(!gills.is_empty());
        for gill in &gills {
            for p in points(gill) {
                let x = from_axis(p);
                assert!(x <= RADIUS, "gill {x} from the axis, the margin is at {RADIUS}");
                // below the top of the flesh, and the deepest strands below its underside
                let top = HINGE.y + midline(x) + flesh(x);
                assert!(p.y < top, "gill at {} over a cap top at {top}", p.y);
            }
            let last = *points(gill).last().unwrap();
            let x = from_axis(last);
            assert!(last.y < HINGE.y + midline(x) + flesh(x));
        }
        // the gill edges hang below the underside
        let lowest = gills.iter().flat_map(|g| points(g)).map(|p| p.y - (HINGE.y + midline(from_axis(p)) - flesh(from_axis(p)))).fold(0.0, f32::min);
        assert!(lowest < -0.004, "the gills reach {lowest} below the underside");
    }

    #[test]
    fn the_underside_has_no_step_where_rays_are_added() {
        let rays = cap(0.0);
        let hinge = decode_points(&hinge())[0];
        let underside = Underside::new(Frame::new(&hinge), &split_args(&rays)).unwrap();
        for angle in [0.01, 0.2, TAU * 1.5 / 24.0] {
            let height = |s: f32| {
                let spot = underside.at(angle, s).unwrap();
                (spot.position - spot.normal * spot.radius).y
            };
            let mut s = 0.008;
            while s < 0.022 {
                let step = (height(s + 0.0005) - height(s)).abs();
                assert!(step < 0.0003, "the underside steps {step} at {s} on angle {angle}");
                s += 0.0005;
            }
        }
    }

    #[test]
    fn no_gill_is_closer_to_the_axis_than_the_free_distance() {
        for free_distance in [0.0, 0.001, 0.004] {
            let gills = run(&cap(0.0), &pattern(24, 0.014), &Params { free_distance, ..params() });
            let closest = gills.iter().flat_map(|g| points(g)).map(from_axis).fold(f32::MAX, f32::min);
            assert!(closest >= STIPE_RADIUS + free_distance - 1e-5, "a gill is {closest} from the axis");
            // and they start close to it
            assert!(closest < STIPE_RADIUS + free_distance + 0.001, "the gills start {closest} from the axis");
        }
    }

    /// The gills that cross a circle `x` from the axis, by the angle of their top strand
    fn plates_at(gills: &[Vec<i32>], x: f32) -> usize {
        let mut angles: Vec<f32> = gills
            .iter()
            .filter_map(|g| {
                let pts = points(g);
                let k = pts.windows(2).position(|w| (from_axis(w[0]) - x) * (from_axis(w[1]) - x) <= 0.0)?;
                let p = pts[k];
                Some(p.z.atan2(p.x).rem_euclid(TAU))
            })
            .collect();
        angles.sort_by(f32::total_cmp);
        angles.dedup_by(|a, b| (*a - *b).abs() < 0.02);
        let wraps = angles.len() > 1 && angles[0] + TAU - angles[angles.len() - 1] < 0.02;
        angles.len() - wraps as usize
    }

    #[test]
    fn every_stripe_is_a_plate_and_new_stripes_start_between_them() {
        let gills = run(&cap(0.0), &pattern(24, 0.014), &Params { edge_wave: 0.0, ..params() });
        assert_eq!(plates_at(&gills, 0.01), 24);
        assert_eq!(plates_at(&gills, 0.02), 48);
        // a plate is a stack of paths, all children of the stipe made by this node
        for (k, gill) in gills.iter().enumerate() {
            let link = path_link(gill);
            assert_eq!(link.parent_id, STIPE_ID);
            assert_eq!(gill[7], CREATOR);
            assert_eq!(link, PathLink::child(CREATOR, STIPE_ID, 1.0, k as i32));
        }
        // the new ones start between two others, shallow, and grow to the full depth
        let short: Vec<&Vec<i32>> = gills.iter().filter(|g| from_axis(points(g)[0]) > 0.012).collect();
        assert!(!short.is_empty());
    }

    #[test]
    fn depth_follows_the_input() {
        let reach = |depth: f32| {
            let gills = run(&cap(0.0), &pattern(24, 0.03), &Params { depth, edge_wave: 0.0, ..params() });
            gills
                .iter()
                .flat_map(|g| points(g))
                .map(|p| HINGE.y + midline(from_axis(p)) - flesh(from_axis(p)) - p.y)
                .fold(0.0, f32::max)
        };
        let (a, b) = (reach(0.004), reach(0.008));
        // measured vertically under a sloped cap, so a bit less than the depth
        assert!((a - 0.004).abs() < 0.0008, "depth 4 mm reaches {a}");
        assert!((b - 0.008).abs() < 0.0012, "depth 8 mm reaches {b}");
        assert!(reach(0.0) < 0.0005);
    }

    #[test]
    fn the_depth_curve_shapes_the_gills_from_the_stipe_to_the_margin() {
        let deepest_near = |x: f32, curve: &[f32]| {
            let gills = run(&cap(0.0), &pattern(24, 0.03), &Params { depth_curve: curve.to_vec(), edge_wave: 0.0, ..params() });
            gills
                .iter()
                .flat_map(|g| points(g))
                .filter(|p| (from_axis(*p) - x).abs() < 0.001)
                .map(|p| HINGE.y + midline(from_axis(p)) - flesh(from_axis(p)) - p.y)
                .fold(0.0, f32::max)
        };
        let rising = [0.0, 0.2, 1.0, 1.0];
        assert!(deepest_near(0.008, &rising) < 0.5 * deepest_near(0.022, &rising));
        let falling = [0.0, 1.0, 1.0, 0.2];
        assert!(deepest_near(0.008, &falling) > 2.0 * deepest_near(0.022, &falling));
    }

    #[test]
    fn edge_wave_moves_the_edge_and_leaves_the_top() {
        let straight = run(&cap(0.0), &pattern(24, 0.03), &Params { edge_wave: 0.0, ..params() });
        let wavy = run(&cap(0.0), &pattern(24, 0.03), &Params { edge_wave: 1.0, ..params() });
        assert_eq!(straight.len(), wavy.len());
        // the edge of the first gill: the last strand of its plate
        let strands = straight.iter().filter(|g| from_axis(points(g)[0]) < 0.008).count() / 24;
        let sideways = |gills: &[Vec<i32>]| {
            let edge = points(&gills[strands - 1]);
            let angles: Vec<f32> = edge.iter().map(|p| (p.z - HINGE.z).atan2(p.x - HINGE.x)).collect();
            let mean = angles.iter().sum::<f32>() / angles.len() as f32;
            angles.iter().map(|a| (a - mean).abs() * 0.015).fold(0.0, f32::max)
        };
        assert!(sideways(&straight) < 1e-4, "a straight edge moves {}", sideways(&straight));
        assert!(sideways(&wavy) > 2e-4, "a wavy edge moves {}", sideways(&wavy));
        // the top strand stays where it is
        assert_eq!(points(&straight[0]), points(&wavy[0]));
    }

    #[test]
    fn gills_follow_the_cap_as_it_closes() {
        let open = run(&cap(0.0), &pattern(24, 0.014), &params());
        let closing = run(&cap(0.8), &pattern(24, 0.014), &params());
        // the same gills, the pattern sits on the same spots of the tissue
        assert_eq!(open.len(), closing.len());
        for (a, b) in open.iter().zip(&closing) {
            assert_eq!(path_link(a), path_link(b));
            assert_eq!(points(a).len(), points(b).len());
        }
        // the margin comes down, so do the outer gills
        let low = |gills: &[Vec<i32>]| gills.iter().flat_map(|g| points(g)).map(|p| p.y).fold(f32::MAX, f32::min);
        assert!(low(&closing) < low(&open) - 0.003);
    }

    #[test]
    fn plates_are_thin_closed_sheets_under_the_cap() {
        for p in [params(), Params { edge_wave: 1.0, free_distance: 0.003, ..params() }] {
            let mesh = plates(&cap(0.0), &pattern(24, 0.014), &p);
            assert!(mesh.face_count() > 0);
            assert!(mesh.groups.iter().all(|&g| g == 0));
            let vertex = |v: usize| Vec3::from_slice(&mesh.positions[v * 3..v * 3 + 3]);
            for v in 0..mesh.vertex_count() {
                let q = vertex(v);
                let x = from_axis(q);
                assert!(x <= RADIUS + 1e-4, "plate {x} from the axis");
                assert!(x >= STIPE_RADIUS + p.free_distance - 1e-5, "plate {x} from the axis, free distance {}", p.free_distance);
                assert!(q.y < HINGE.y + midline(x) + flesh(x));
                let n = Vec3::from_slice(&mesh.normals[v * 3..v * 3 + 3]);
                assert!((n.length() - 1.0).abs() < 1e-4);
            }
            // closed: every edge belongs to two faces, walked once each way
            let mut edges = std::collections::HashMap::new();
            for f in mesh.faces.chunks_exact(3) {
                for (a, b) in [(f[0], f[1]), (f[1], f[2]), (f[2], f[0])] {
                    *edges.entry((a, b)).or_insert(0) += 1;
                }
            }
            for (&(a, b), &n) in &edges {
                assert_eq!(n, 1, "edge {a}-{b} is used {n} times the same way");
                assert!(edges.contains_key(&(b, a)), "edge {a}-{b} is open");
            }
            // the faces look out of the plate: away from the middle of their gill
            let outward = mesh
                .faces
                .chunks_exact(3)
                .filter(|f| {
                    let (a, b, c) = (vertex(f[0] as usize), vertex(f[1] as usize), vertex(f[2] as usize));
                    let n = (b - a).cross(c - a);
                    let mid = (a + b + c) / 3.0;
                    let around = Vec3::new(-(mid.z - HINGE.z), 0.0, mid.x - HINGE.x).normalize();
                    n.dot(around).abs() < 0.5 * n.length() || n.dot(around) != 0.0
                })
                .count();
            assert_eq!(outward, mesh.face_count());
        }
    }

    #[test]
    fn the_edge_waves_and_the_waves_die_away_towards_the_cap() {
        let slice =
            Slice { top: Vec3::ZERO, normal: Vec3::Y, sway: Vec3::X * 0.001, reach: 0.004, surface: 0.25, taper: 1.0, damping: 8.0 };
        let side = |f: f32| slice.at(f).x;
        assert_eq!(side(0.0), 0.0);
        assert!((side(1.0) - 0.001).abs() < 1e-9);
        // within about a wavelength / TAU of the edge, a quarter of the way up only a little is left
        assert!(side(0.75) < 0.2 * 0.001, "{} left a quarter up", side(0.75));
        assert!((1..=10).all(|k| side(k as f32 / 10.0) >= side((k - 1) as f32 / 10.0)));
    }

    /// 12 stripes from 4 mm out, every one splits in two from 12 mm on, the halves move apart
    /// to evenly spaced stripes by 20 mm
    fn forking_pattern() -> Pattern {
        let (from, to, rings) = (0.004, 0.03, 131);
        let width = 0.0006;
        let rings = (0..rings)
            .map(|i| {
                let r = from + (to - from) * i as f32 / (rings - 1) as f32;
                let apart = ((r - 0.012) / 0.008).clamp(0.0, 1.0) * TAU / 48.0;
                let n = (TAU * r / 0.0001) as usize;
                (0..n)
                    .map(|j| {
                        let angle = nodarium_utils::ring_angle(j, n);
                        (0..12)
                            .flat_map(|k| [TAU * k as f32 / 12.0 - apart, TAU * k as f32 / 12.0 + apart])
                            .map(|c| (-(r * turn(c, angle)).powi(2) / (2.0 * width * width)).exp())
                            .fold(0.0, f32::max)
                    })
                    .collect()
            })
            .collect();
        Pattern { inner: from, outer: to, spacing: TAU * 0.008 / 12.0, rings }
    }

    #[test]
    fn forks_open_away_from_the_stipe() {
        // Moore (1987): forked gills always point the fork away from the stipe
        let pattern = forking_pattern();
        let gills = trace_gills(&pattern, 0, pattern.rings.len() - 1, LINK_REACH * pattern.spacing);
        let forks: Vec<&Gill> = gills.iter().filter(|g| !g.free_start && g.points[0].1 > pattern.inner + 1e-6).collect();
        assert_eq!(forks.len(), 12, "{} forks", forks.len());
        assert!(gills.iter().all(|g| !g.free_start), "the halves split off, none starts on its own");
        for fork in forks {
            let (at, r) = fork.points[0];
            // the branch goes outward from the fork point
            assert!(fork.points[1].1 > r);
            // and the stripe it splits off runs through the fork point and on outward
            let parent = gills
                .iter()
                .find(|g| !std::ptr::eq(*g, fork) && g.points.iter().any(|&(a, s)| (s - r).abs() < 1e-6 && turn(a, at).abs() < 1e-4))
                .expect("the stripe the fork splits off");
            assert!(parent.points[0].1 < r && parent.points.last().unwrap().1 > r);
        }
    }

    #[test]
    fn a_plate_is_as_thick_as_the_input_at_its_top_and_thinner_at_its_edge() {
        // a straight plate along x, hanging down along -y
        let plate: Vec<Slice> = (0..5)
            .map(|i| Slice {
                top: Vec3::new(i as f32 * 0.001, 0.0, 0.0),
                normal: Vec3::Y,
                sway: Vec3::ZERO,
                reach: 0.004,
                surface: 0.25,
                taper: 1.0,
                damping: 8.0,
            })
            .collect();
        let mut mesh = Mesh::default();
        add_plate(&mut mesh, &plate, 0.0004, 0.0, |q, _| q);
        let vertex = |v: usize| Vec3::from_slice(&mesh.positions[v * 3..v * 3 + 3]);
        let (m, rows) = (plate.len(), PLATE_ROWS);
        assert_eq!(mesh.vertex_count(), m * (2 * rows + 1));
        let thick = |k: usize| vertex(2 * rows + k).distance(vertex((m + 2) * rows + k));
        assert!((thick(0) - 0.0004).abs() < 1e-6, "the top is {} thick", thick(0));
        assert!((thick(rows - 1) - 0.0004 * (1.0 - EDGE_THINNING)).abs() < 1e-6, "the edge is {} thick", thick(rows - 1));
        // the edge hangs reach below the top, and its rim a little further
        assert!((vertex(2 * rows + rows - 1).y + 0.004).abs() < 1e-6);
        assert!(vertex(2 * m * rows + 2).y < -0.004);
    }

    /// The underside of the test cap, `x` from the axis
    fn underside(x: f32) -> f32 {
        HINGE.y + midline(x) - flesh(x)
    }

    #[test]
    fn a_blunt_fold_is_at_least_as_wide_as_it_is_deep() {
        // a straight fold along x out of flesh above y = 0, its top 1 mm up in the flesh
        let (width, depth) = (0.0015, 0.0012);
        let plate: Vec<Slice> = (0..5)
            .map(|i| Slice {
                top: Vec3::new(i as f32 * 0.001, 0.001, 0.0),
                normal: Vec3::Y,
                sway: Vec3::ZERO,
                reach: 0.001 + depth,
                surface: 0.001 / (0.001 + depth),
                taper: 1.0,
                damping: 8.0,
            })
            .collect();
        let mut mesh = Mesh::default();
        add_plate(&mut mesh, &plate, width, 1.0, |q, _| q);
        // the cross-section of the middle slice: its width at every height out of the flesh
        let section: Vec<Vec3> = (0..mesh.vertex_count())
            .map(|v| Vec3::from_slice(&mesh.positions[v * 3..v * 3 + 3]))
            .filter(|q| (q.x - 0.002).abs() < 1e-6)
            .collect();
        let deepest = section.iter().map(|q| -q.y).fold(f32::MIN, f32::max);
        assert!((deepest - depth).abs() < 1e-6, "the fold reaches {deepest} out of the flesh");
        let wide_at = |h: f32| {
            let near: Vec<&Vec3> = section.iter().filter(|q| -q.y <= h + 1e-6 && q.y <= 1e-6).collect();
            near.iter().map(|q| q.z).fold(f32::MIN, f32::max) - near.iter().map(|q| q.z).fold(f32::MAX, f32::min)
        };
        assert!(wide_at(0.0) >= width, "{} wide where it leaves the flesh", wide_at(0.0));
        assert!(wide_at(0.0) >= deepest);
        // a round back, not a sharp edge: still most of its width half way out
        let half_way = 2.0 * half_share(plate[2].surface + 0.5 * (1.0 - plate[2].surface), plate[2].surface, 1.0);
        assert!(half_way > 0.8, "{half_way} of the width half way out");
    }

    #[test]
    fn blunt_folds_are_never_deeper_than_wide() {
        let p = Params { blunt: 1.0, thickness: 0.0015, edge_wave: 0.0, ..params() };
        let gills = run(&cap(0.0), &pattern(24, 0.03), &p);
        assert!(!gills.is_empty());
        let deepest = gills.iter().flat_map(|g| points(g)).map(|q| underside(from_axis(q)) - q.y).fold(0.0, f32::max);
        // measured straight down under a cap that slopes by up to 22°, a bit more than across it
        let slope = 1.0 / (1.0f32 + (0.01 / RADIUS).powi(2)).sqrt();
        assert!(deepest * slope <= 1.05 * p.thickness, "folds reach {deepest} below the cap, {} wide", p.thickness);
        assert!(deepest > 0.5 * p.thickness, "folds reach only {deepest} below the cap");
        let mesh = plates(&cap(0.0), &pattern(24, 0.03), &p);
        let lowest = (0..mesh.vertex_count())
            .map(|v| Vec3::from_slice(&mesh.positions[v * 3..v * 3 + 3]))
            .map(|q| underside(from_axis(q)) - q.y)
            .fold(0.0, f32::max);
        assert!(lowest * slope <= 1.05 * p.thickness, "the fold mesh reaches {lowest} below the cap");
    }

    fn decurrent(length: f32) -> Params {
        Params { decurrent: length, free_distance: 0.0, depth_curve: vec![0.0, 1.0, 1.0, 1.0], edge_wave: 0.0, ..params() }
    }

    #[test]
    fn decurrent_gills_run_down_the_stipe_by_the_given_length() {
        // the lowest point next to the stipe, the gills under the cap hang lower further out
        let lowest = |length: f32| {
            let mesh = plates(&cap(0.0), &pattern(24, 0.014), &decurrent(length));
            let vertex = |v: usize| Vec3::from_slice(&mesh.positions[v * 3..v * 3 + 3]);
            (0..mesh.vertex_count()).map(vertex).filter(|q| from_axis(*q) < 0.008).map(|q| q.y).fold(f32::MAX, f32::min)
        };
        let (none, short, long) = (lowest(0.0), lowest(0.01), lowest(0.02));
        assert!(short < none - 0.002, "gills of 1 cm reach {short}, without {none}");
        assert!(((short - long) - 0.01).abs() < 0.0005, "1 cm more reaches {} further down", short - long);
        // they start where the underside meets the stipe
        let junction = underside(STIPE_RADIUS);
        assert!((short - (junction - 0.01)).abs() < 0.001, "1 cm down from {junction} ends at {short}");
    }

    #[test]
    fn decurrent_gills_stay_on_the_stipe_and_get_lower() {
        let p = decurrent(0.02);
        let junction = underside(STIPE_RADIUS);
        // the tops of the gills lie in the stipe, just under its surface
        let gills = run(&cap(0.0), &pattern(24, 0.014), &p);
        let embed = EMBED * flesh(0.0);
        let strands = (((embed + p.depth) / (STRAND_GAP * p.thickness)).ceil() as usize + 1).clamp(2, MAX_STRANDS);
        let mut decurrent = 0;
        for gill in gills.iter().step_by(strands) {
            // a gill that runs down the stipe starts at its lower end, on the stipe up to the cap
            let tops = points(gill);
            let on_stipe: Vec<&Vec3> = tops.iter().take_while(|q| from_axis(**q) <= STIPE_RADIUS + 1e-5).collect();
            if on_stipe.len() < 2 {
                continue;
            }
            decurrent += 1;
            for q in &on_stipe {
                assert!(from_axis(**q) > 0.5 * STIPE_RADIUS - 1e-5, "a gill top {} from the axis", from_axis(**q));
            }
            let (low, high) = (on_stipe[0].y, on_stipe[on_stipe.len() - 1].y);
            assert!((low - (junction - p.decurrent)).abs() < 0.001, "a gill reaches down to {low}");
            assert!(high > junction - 0.002, "a gill leaves the stipe at {high}");
        }
        assert_eq!(decurrent, 24, "every gill that leaves the stipe runs down it");
        // below where the gills under the cap hang, only the ones on the stipe are left
        let mesh = plates(&cap(0.0), &pattern(24, 0.014), &p);
        let on_stipe: Vec<Vec3> = (0..mesh.vertex_count())
            .map(|v| Vec3::from_slice(&mesh.positions[v * 3..v * 3 + 3]))
            .filter(|q| q.y < junction - 0.012 && from_axis(*q) < 0.015)
            .collect();
        assert!(on_stipe.len() > 100);
        for q in &on_stipe {
            let x = from_axis(*q);
            assert!(x > 0.5 * STIPE_RADIUS - 1e-4, "{x} from the axis, inside the stipe");
            assert!(x < STIPE_RADIUS + p.depth + p.thickness, "{x} from the axis, off the stipe");
        }
        // lower and lower: how far they stand off the stipe, from the top down
        let stand_off = |y: f32| on_stipe.iter().filter(|q| (q.y - y).abs() < 0.0005).map(|q| from_axis(*q)).fold(0.0, f32::max) - STIPE_RADIUS;
        let (high, middle, low) = (stand_off(junction - 0.013), stand_off(junction - 0.016), stand_off(junction - 0.019));
        assert!(high > middle && middle > low, "{high}, {middle}, {low} off the stipe");
        assert!(high < 0.6 * p.depth && low < 0.1 * p.depth);
    }

    #[test]
    fn the_edge_of_a_decurrent_gill_turns_the_corner_without_folding() {
        let p = Params { depth: 0.004, ..decurrent(0.015) };
        let gills = run(&cap(0.0), &pattern(24, 0.014), &p);
        let (top, edge) = (points(&gills[0]), points(&gills[gills.len() / 24 - 1]));
        assert!(top.iter().any(|q| q.y < underside(STIPE_RADIUS) - 0.005), "the first gill runs down the stipe");
        for k in 1..top.len() {
            let (a, b) = (top[k] - top[k - 1], edge[k] - edge[k - 1]);
            assert!(a.dot(b) > 0.0, "the edge runs back at {k}: {a} on top, {b} at the edge");
        }
    }

    #[test]
    fn gills_without_decurrent_ignore_the_stipe() {
        let p = params();
        let curve = floats(&p.depth_curve);
        let with = execute_on(&cap(0.0), &stipe(), &pattern(24, 0.014), &p, curve.clone());
        let without = execute_on(&cap(0.0), &[], &pattern(24, 0.014), &p, curve.clone());
        assert_eq!(with, without);
        // and decurrent gills without a stipe stay under the cap
        let alone = execute_on(&cap(0.0), &[], &pattern(24, 0.014), &decurrent(0.02), floats(&[0.0, 1.0, 1.0, 1.0]));
        let lowest = Mesh::decode(&alone.1).positions.chunks_exact(3).map(|q| q[1]).fold(f32::MAX, f32::min);
        assert!(lowest > underside(RADIUS) - 0.01);
    }

    #[test]
    fn gills_reach_the_margin_of_a_lopsided_cap_all_round() {
        // every ray stretched from the axis, 1.4 times as long on one side, 0.6 times on the other
        let stretch = |angle: f32| 1.0 + 0.4 * angle.cos();
        let rays: Vec<Vec<i32>> = split_args(&cap(0.0))
            .iter()
            .map(|r| {
                let mut ray = r.to_vec();
                let path = wrap_path_mut(&mut ray);
                let n = path.points.len() / 4;
                let tip = Vec3::from_slice(&path.points[(n - 1) * 4..(n - 1) * 4 + 3]);
                let k = stretch((tip.z - HINGE.z).atan2(tip.x - HINGE.x));
                for i in 0..n {
                    let (x, z) = (path.points[i * 4] - HINGE.x, path.points[i * 4 + 2] - HINGE.z);
                    if x * tip.x + z * tip.z > 0.0 {
                        path.points[i * 4] = HINGE.x + x * k;
                        path.points[i * 4 + 2] = HINGE.z + z * k;
                    }
                }
                ray
            })
            .collect();
        let gills = run(&concat_arg_vecs(rays), &pattern(24, 0.014), &Params { edge_wave: 0.0, ..params() });
        let reach = |angle: f32| {
            gills
                .iter()
                .flat_map(|g| points(g))
                .filter(|q| turn((q.z - HINGE.z).atan2(q.x - HINGE.x), angle).abs() < 0.2)
                .map(from_axis)
                .fold(0.0, f32::max)
        };
        assert!(reach(0.0) > 0.85 * 1.4 * RADIUS, "the long side's gills reach {}", reach(0.0));
        assert!(reach(PI) > 0.85 * 0.6 * RADIUS && reach(PI) < 0.6 * RADIUS + 1e-3, "the short side's gills reach {}", reach(PI));
        let closest = gills.iter().flat_map(|g| points(g)).map(from_axis).fold(f32::MAX, f32::min);
        assert!(closest >= STIPE_RADIUS + 0.001 - 1e-5);
    }

    #[test]
    fn the_same_inputs_give_the_same_gills() {
        let p = pattern(24, 0.014);
        assert_eq!(run(&cap(0.0), &p, &params()), run(&cap(0.0), &p, &params()));
        assert_eq!(plates(&cap(0.0), &p, &params()), plates(&cap(0.0), &p, &params()));
        assert_ne!(run(&cap(0.0), &p, &params()), run(&cap(0.0), &p, &Params { seed: 3, ..params() }));
    }

    #[test]
    fn reads_a_connected_curve_node() {
        let p = params();
        let curve = floats(&p.depth_curve);
        let connected = nodarium_utils::concat_args(vec![&curve]);
        assert_eq!(read_floats(&connected), p.depth_curve);
        assert_eq!(run_with(&cap(0.0), &pattern(24, 0.014), &p, connected), run(&cap(0.0), &pattern(24, 0.014), &p));
    }

    #[test]
    fn nothing_without_a_pattern_or_a_cap() {
        let (empty, plates) = __nodarium_execute_inner(&[
            &cap(0.0),
            &hinge(),
            &[],
            &stipe(),
            &[encode_float(0.006)],
            &floats(&[0.0, 1.0, 1.0, 1.0]),
            &[encode_float(0.001)],
            &[encode_float(0.0)],
            &[encode_float(0.0004)],
            &[encode_float(0.0)],
            &[encode_float(0.3)],
            &[2],
            &[CREATOR],
        ]);
        assert!(split_args(&empty).is_empty());
        assert_eq!(Mesh::decode(&plates).face_count(), 0);
        // rays of another stipe don't make a cap for this hinge
        let other: Vec<i32> = {
            let mut rays = cap(0.0);
            let mut paths: Vec<Vec<i32>> = split_args(&rays).iter().map(|s| s.to_vec()).collect();
            for p in paths.iter_mut() {
                p[5] = STIPE_ID + 1;
            }
            rays = concat_arg_vecs(paths);
            rays
        };
        assert!(run(&other, &pattern(24, 0.014), &params()).is_empty());
    }

    #[test]
    fn stripe_angles_find_the_middle_of_every_stripe() {
        let n = 360;
        let values: Vec<f32> = (0..n).map(|j| 0.5 + 0.5 * (8.0 * nodarium_utils::ring_angle(j, n)).cos()).collect();
        let angles = stripe_angles(&values);
        assert_eq!(angles.len(), 8);
        for k in 0..8 {
            let expected = TAU * k as f32 / 8.0;
            let closest = angles.iter().map(|a| turn(*a, expected).abs()).fold(f32::MAX, f32::min);
            assert!(closest < 0.01, "no stripe at {expected}: {angles:?}");
        }
        assert!(stripe_angles(&[1.0; 10]).is_empty());
        assert!(stripe_angles(&[0.0; 10]).is_empty());
    }
}

#[cfg(test)]
mod debug {
    use super::*;
    use std::io::Write;

    /// `PATTERN=file.words OUT=file.ppm cargo test --release -p myzel_lamellae debug -- --ignored`
    /// draws the traced gills over a pattern written by the reaction_diffusion debug test:
    /// gills that run on in blue, ones that start between others in red, forks in green
    #[test]
    #[ignore]
    fn trace() {
        let words: Vec<i32> =
            std::fs::read_to_string(std::env::var("PATTERN").unwrap()).unwrap().split(' ').map(|w| w.parse().unwrap()).collect();
        let pattern = Pattern::decode(&words).unwrap();
        let gills = trace_gills(&pattern, 0, pattern.rings.len() - 1, LINK_REACH * pattern.spacing);
        let (free_starts, forks) = (
            gills.iter().filter(|g| g.free_start).count(),
            gills.iter().filter(|g| !g.free_start && g.points[0].1 > pattern.inner + 1e-6).count(),
        );
        println!("{} gills, {} start between others, {} fork, {} end between others", gills.len(), free_starts, forks, gills.iter().filter(|g| g.free_end).count());
        let size = 900usize;
        let scale = size as f32 / (2.0 * pattern.outer);
        let mut img = vec![[0u8; 3]; size * size];
        for y in 0..size {
            for x in 0..size {
                let (px, py) = ((x as f32 + 0.5) / scale - pattern.outer, (y as f32 + 0.5) / scale - pattern.outer);
                if let Some(v) = pattern.sample(py.atan2(px).rem_euclid(TAU), (px * px + py * py).sqrt()) {
                    let g = (140.0 + 100.0 * v) as u8;
                    img[y * size + x] = [g, g, g];
                }
            }
        }
        for gill in &gills {
            let color = if gill.free_start { [220, 30, 30] } else if gill.points[0].1 > pattern.inner + 1e-6 { [20, 160, 40] } else { [30, 60, 200] };
            for w in gill.points.windows(2) {
                for k in 0..20 {
                    let t = k as f32 / 20.0;
                    let (a, s) = (w[0].0 + turn(w[0].0, w[1].0) * t, w[0].1 + (w[1].1 - w[0].1) * t);
                    let (x, y) = (((s * a.cos() + pattern.outer) * scale) as usize, ((s * a.sin() + pattern.outer) * scale) as usize);
                    if x < size && y < size {
                        img[y * size + x] = color;
                    }
                }
            }
        }
        let mut f = std::fs::File::create(std::env::var("OUT").unwrap()).unwrap();
        write!(f, "P6\n{size} {size}\n255\n").unwrap();
        f.write_all(&img.concat()).unwrap();
    }
}
