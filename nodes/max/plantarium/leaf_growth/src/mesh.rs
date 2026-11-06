//! Triangulation of the leaf blade. The outline, the veins with a ribbon of
//! rows along both sides and a jittered grid inside the blade are triangulated
//! with Bowyer-Watson such that every vein runs along mesh edges, then
//! triangles outside the outline are dropped. The groove of a vein lies within
//! its ribbon, so the mesh follows the veins where they show.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use glam::{DVec2, Vec2};

use crate::veins::VeinLine;

/// Very detailed outlines are simplified further to keep the mesh small.
const MAX_OUTLINE_POINTS: usize = 600;
/// Rounds of splitting vein edges that the Delaunay triangulation missed.
const MAX_RECOVERY_ROUNDS: usize = 8;

pub struct Mesh {
    pub points: Vec<Vec2>,
    /// Counter clockwise triangles.
    pub triangles: Vec<[usize; 3]>,
    /// Relative thickness of the vein running through each point, 0 on the
    /// blade and the margin.
    pub vein: Vec<f32>,
    /// Points that are meant to lie on the flat blade: the outline, the outer
    /// rows of the ribbons and the points between the veins.
    pub flat: Vec<bool>,
}

/// `vein_width` is the width of the thickest vein, thinner ones get narrower.
pub fn triangulate(outline: &[Vec2], lines: &[VeinLine], spacing: f32, vein_width: f32) -> Mesh {
    let eps = 1e-3 * spacing;
    let ends: Vec<Vec2> = lines.iter().filter_map(|l| l.points.last().copied()).collect();
    let on_vein_end = |p: Vec2| ends.iter().any(|e| e.distance_squared(p) < eps * eps);
    let outline = refine_outline(outline, spacing, on_vein_end);
    let outline_grid = SegmentGrid::new(
        (0..outline.len()).map(|i| (outline[i], outline[(i + 1) % outline.len()])),
        spacing,
    );
    let near_outline = |p: Vec2, d: f32| {
        let mut near = false;
        outline_grid.visit(p, d, |i| {
            let (a, b) = outline_grid.segments[i];
            near |= distance_to_segment(p, a, b) < d;
        });
        near
    };
    let mut points = outline.clone();
    let mut vein = vec![0.0; points.len()];
    let mut flat = vec![true; points.len()];

    // The center of every vein, sampled at about the mesh spacing, and every
    // pair of consecutive samples has to become an edge. The points other
    // veins branch off at are samples too, so they start right on it.
    let max_flux = lines
        .iter()
        .flat_map(|l| l.flux.iter())
        .fold(1.0f32, |m, &f| m.max(f));
    let mut constraints: Vec<(usize, usize)> = Vec::new();
    let mut samples: Vec<Sample> = Vec::new();
    // Veins fade out over this length before they reach the margin.
    let taper_length = 4.0 * vein_width.max(0.25);
    for (line_index, line) in lines.iter().enumerate() {
        // the widest ribbon reaches vein_width from its vein
        let picked = pick_samples(line, spacing, 1.5 * vein_width.max(0.1 * spacing));
        let arc = arc_lengths(&line.points);
        let length = arc[arc.len() - 1];
        let mut prev: Option<usize> = None;
        for (k, &i) in picked.iter().enumerate() {
            let p = line.points[i];
            if !inside(p, &outline) && !near_outline(p, eps) {
                prev = None;
                continue;
            }
            let fade = smoothstep((length - arc[i]) / taper_length);
            let weight = (line.flux[i] / max_flux).powf(0.3) * fade;
            if let Some(prev) = prev {
                constraints.push((prev, points.len()));
            }
            prev = Some(points.len());
            points.push(p);
            vein.push(weight);
            flat.push(false);
            let before = line.points[picked[k.saturating_sub(1)]];
            let after = line.points[picked[(k + 1).min(picked.len() - 1)]];
            samples.push(Sample {
                pos: p,
                side: (after - before).normalize_or_zero().perp(),
                // narrower than the mesh can show they would just be creases
                half_width: (0.5 * vein_width).max(0.1 * spacing) * weight,
                weight,
                line: line_index,
            });
        }
    }

    // Every vein lies in a ribbon of its own: flat rows of points at the edge
    // of the groove and as far again outside, level with the samples. The
    // groove is made of long triangles along the vein, and the rest of the
    // blade only meets it at flat points, so it can't dent the groove.
    let ribbon_grid = SegmentGrid::new(
        samples.windows(2).filter(|w| w[0].line == w[1].line).map(|w| (w[0].pos, w[1].pos)),
        spacing,
    );
    // line, half width and weight at both ends of every ribbon segment
    let ribbon_info: Vec<(usize, [f32; 2], [f32; 2])> = samples
        .windows(2)
        .filter(|w| w[0].line == w[1].line)
        .map(|w| (w[0].line, [w[0].half_width, w[1].half_width], [w[0].weight, w[1].weight]))
        .collect();
    // Distance from `p` to the closest ribbon, measured from its outer edge.
    // `part` says which part of the ribbon of a line counts, None skips it.
    let clearance = |p: Vec2, radius: f32, part: &dyn Fn(usize) -> Option<f32>| {
        let mut clear = f32::MAX;
        ribbon_grid.visit(p, radius, |i| {
            let (a, b) = ribbon_grid.segments[i];
            let (l, half_width, _) = ribbon_info[i];
            if let Some(part) = part(l) {
                let t = project(p, a, b);
                let extent = 2.0 * lerp(half_width, t) * part;
                clear = clear.min(p.distance(a.lerp(b, t)) - extent);
            }
        });
        clear
    };
    let mut row_grid = PointGrid::new(spacing);
    for s in &samples {
        if s.half_width <= 1e-4 * spacing {
            continue;
        }
        for offset in [1.0, 2.0] {
            for sign in [-1.0, 1.0] {
                let distance = offset * s.half_width;
                let p = s.pos + s.side * (sign * distance);
                let mut own = f32::MAX;
                ribbon_grid.visit(p, distance, |i| {
                    let (a, b) = ribbon_grid.segments[i];
                    if ribbon_info[i].0 == s.line {
                        own = own.min(distance_to_segment(p, a, b));
                    }
                });
                // Lines come thickest first. The rows of a thinner vein stay
                // out of the ribbons of the thicker ones, the rows of a thicker
                // vein only keep clear of the middle of the thinner ones, so
                // where a vein branches off the thicker one keeps its ribbon.
                // Both grooves add up there, see below.
                let part = |l: usize| match l.cmp(&s.line) {
                    Ordering::Less => Some(1.0),
                    Ordering::Equal => None,
                    Ordering::Greater => Some(0.125),
                };
                if inside(p, &outline)
                    && !near_outline(p, 0.5 * s.half_width)
                    // on the inner side of a sharp bend the rows would overlap
                    && own > 0.9 * distance
                    && clearance(p, vein_width + spacing, &part) > 0.25 * s.half_width
                    && !row_grid.any_within(p, 0.5 * s.half_width)
                {
                    row_grid.insert(p);
                    points.push(p);
                    vein.push(0.0);
                    flat.push(offset > 1.0);
                }
            }
        }
    }

    // Jittered hexagonal grid inside the blade. Points too close to the margin
    // or a ribbon are left out, so they do not cut through its edges.
    let (min, max) = bounds(&outline);
    let row_height = spacing * 0.866;
    let rows = ((max.y - min.y) / row_height).ceil() as i32;
    let cols = ((max.x - min.x) / spacing).ceil() as i32;
    for row in 0..=rows {
        for col in 0..=cols {
            let offset = if row % 2 == 0 { 0.0 } else { 0.5 };
            let jitter = Vec2::new(hash(row, col, 0), hash(row, col, 1)) * 0.15 * spacing;
            let p =
                min + Vec2::new((col as f32 + offset) * spacing, row as f32 * row_height) + jitter;
            if inside(p, &outline)
                && !near_outline(p, 0.6 * spacing)
                && clearance(p, vein_width + spacing, &|_| Some(1.0)) > 0.5 * spacing
            {
                points.push(p);
                vein.push(0.0);
                flat.push(true);
            }
        }
    }

    merge_coincident(&mut points, &mut vein, &mut flat, &mut constraints, eps);

    // Conforming triangulation: a vein edge missing from the Delaunay
    // triangulation is split in the middle until it shows up. Only the new
    // points are added to the triangulation.
    let mut delaunay = Delaunay::new(&points);
    let mut triangles = delaunay.triangles();
    for _ in 0..MAX_RECOVERY_ROUNDS {
        let known = points.len();
        let edges: HashSet<(usize, usize)> = triangles
            .iter()
            .flat_map(|t| [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])])
            .map(|(a, b)| (a.min(b), a.max(b)))
            .collect();
        let mut missing = false;
        let mut recovered = Vec::with_capacity(constraints.len());
        for &(a, b) in &constraints {
            if edges.contains(&(a.min(b), a.max(b))) {
                recovered.push((a, b));
                continue;
            }
            missing = true;
            let m = points.len();
            points.push((points[a] + points[b]) * 0.5);
            vein.push(vein[a].max(vein[b]));
            flat.push(false);
            recovered.extend([(a, m), (m, b)]);
        }
        if !missing {
            break;
        }
        constraints = recovered;
        // Two missing edges can share a midpoint, or a midpoint can land on a
        // point that was already there. The known points keep their index.
        merge_coincident(&mut points, &mut vein, &mut flat, &mut constraints, eps);
        for &p in &points[known..] {
            delaunay.insert(p);
        }
        triangles = delaunay.triangles();
    }

    // Every point inside a groove sinks in as deep as the deepest groove it
    // lies in, so where veins meet their grooves merge.
    for i in outline.len()..points.len() {
        let p = points[i];
        ribbon_grid.visit(p, 0.5 * vein_width + 0.1 * spacing, |k| {
            let (a, b) = ribbon_grid.segments[k];
            let (_, half_width, weight) = ribbon_info[k];
            let t = project(p, a, b);
            let half_width = lerp(half_width, t);
            if half_width > 0.0 {
                let d = p.distance(a.lerp(b, t));
                vein[i] = vein[i].max(lerp(weight, t) * (1.0 - d / half_width));
            }
        });
    }

    let triangles = triangles
        .into_iter()
        .filter(|t| {
            let centroid = (points[t[0]] + points[t[1]] + points[t[2]]) / 3.0;
            inside(centroid, &outline)
        })
        .collect();

    Mesh {
        points,
        triangles,
        vein,
        flat,
    }
}

/// Merges points closer than `eps` into the first of them, so points that are
/// already apart keep their index, and drops constraints that collapse or
/// repeat. Bowyer-Watson breaks down on coincident points, which a crumpled
/// margin produces where it is pressed onto the midrib and meets its mirror
/// image.
fn merge_coincident(
    points: &mut Vec<Vec2>,
    vein: &mut Vec<f32>,
    flat: &mut Vec<bool>,
    constraints: &mut Vec<(usize, usize)>,
    eps: f32,
) {
    let cell = |p: Vec2| ((p.x / eps).floor() as i32, (p.y / eps).floor() as i32);
    let mut grid: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
    let mut map = Vec::with_capacity(points.len());
    let mut merged: Vec<Vec2> = Vec::with_capacity(points.len());
    let mut merged_vein = Vec::with_capacity(points.len());
    let mut merged_flat = Vec::with_capacity(points.len());
    for ((&p, &w), &f) in points.iter().zip(vein.iter()).zip(flat.iter()) {
        let (x, y) = cell(p);
        let existing = (x - 1..=x + 1)
            .flat_map(|cx| (y - 1..=y + 1).map(move |cy| (cx, cy)))
            .filter_map(|c| grid.get(&c))
            .flatten()
            .copied()
            .find(|&k| merged[k].distance_squared(p) < eps * eps);
        map.push(existing.unwrap_or_else(|| {
            grid.entry((x, y)).or_default().push(merged.len());
            merged.push(p);
            merged_vein.push(w);
            merged_flat.push(f);
            merged.len() - 1
        }));
    }
    *points = merged;
    *vein = merged_vein;
    *flat = merged_flat;
    *constraints = constraints
        .iter()
        .map(|&(a, b)| (map[a].min(map[b]), map[a].max(map[b])))
        .filter(|(a, b)| a != b)
        .collect();
    constraints.sort_unstable();
    constraints.dedup();
}

struct Sample {
    pos: Vec2,
    /// Unit vector across the vein.
    side: Vec2,
    half_width: f32,
    /// Relative depth of the groove.
    weight: f32,
    line: usize,
}

fn lerp(v: [f32; 2], t: f32) -> f32 {
    v[0] + (v[1] - v[0]) * t
}

fn smoothstep(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

fn arc_lengths(points: &[Vec2]) -> Vec<f32> {
    let mut arc = Vec::with_capacity(points.len());
    arc.push(0.0);
    for w in points.windows(2) {
        arc.push(arc[arc.len() - 1] + w[0].distance(w[1]));
    }
    arc
}

/// Indices of the points of `line` that become mesh points: about `spacing`
/// apart, with both ends and the points other lines branch off at. Two more
/// sit `inset` from the ends, so the ribbon starts right next to the vein the
/// line branches off and ends right before the margin.
fn pick_samples(line: &VeinLine, spacing: f32, inset: f32) -> Vec<usize> {
    let n = line.points.len();
    let mut stops: Vec<usize> = line.anchors.iter().copied().filter(|&i| i < n).collect();
    stops.extend([0, n - 1]);
    stops.sort_unstable();
    stops.dedup();
    let mut picked = vec![0];
    for w in stops.windows(2) {
        let (from, to) = (w[0], w[1]);
        let mut arc = vec![0.0f32];
        for i in from + 1..=to {
            arc.push(arc.last().unwrap() + line.points[i - 1].distance(line.points[i]));
        }
        let length = *arc.last().unwrap();
        let pieces = (length / spacing).ceil().max(1.0) as usize;
        let mut k = 0;
        for j in 1..pieces {
            let target = length * j as f32 / pieces as f32;
            while k + 1 < arc.len() && arc[k + 1] <= target {
                k += 1;
            }
            let nearest = if k + 1 < arc.len() && arc[k + 1] - target < target - arc[k] {
                k + 1
            } else {
                k
            };
            if from + nearest > *picked.last().unwrap() && from + nearest < to {
                picked.push(from + nearest);
            }
        }
        picked.push(to);
    }

    let arc = arc_lengths(&line.points);
    let length = arc[n - 1];
    if length > 4.0 * inset {
        for target in [inset, length - inset] {
            let i = arc.partition_point(|&a| a < target).min(n - 1);
            let far = picked.iter().all(|&j| (arc[j] - arc[i]).abs() > 0.5 * inset);
            if far {
                picked.push(i);
            }
        }
        picked.sort_unstable();
    }
    picked
}

fn cell_of(p: Vec2, cell: f32) -> (i32, i32) {
    ((p.x / cell).floor() as i32, (p.y / cell).floor() as i32)
}

/// Segments sorted into square cells, to find the ones close to a point.
struct SegmentGrid {
    cell: f32,
    segments: Vec<(Vec2, Vec2)>,
    cells: HashMap<(i32, i32), Vec<usize>>,
}

impl SegmentGrid {
    fn new(segments: impl Iterator<Item = (Vec2, Vec2)>, cell: f32) -> Self {
        let segments: Vec<(Vec2, Vec2)> = segments.collect();
        let mut cells: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
        for (i, &(a, b)) in segments.iter().enumerate() {
            let (lo, hi) = (cell_of(a.min(b), cell), cell_of(a.max(b), cell));
            for x in lo.0..=hi.0 {
                for y in lo.1..=hi.1 {
                    cells.entry((x, y)).or_default().push(i);
                }
            }
        }
        SegmentGrid {
            cell,
            segments,
            cells,
        }
    }

    /// Calls `f` with every segment that may come within `radius` of `p`,
    /// some of them more than once.
    fn visit(&self, p: Vec2, radius: f32, mut f: impl FnMut(usize)) {
        let (lo, hi) = (
            cell_of(p - Vec2::splat(radius), self.cell),
            cell_of(p + Vec2::splat(radius), self.cell),
        );
        for x in lo.0..=hi.0 {
            for y in lo.1..=hi.1 {
                for &i in self.cells.get(&(x, y)).into_iter().flatten() {
                    f(i);
                }
            }
        }
    }
}

/// Points sorted into square cells.
struct PointGrid {
    cell: f32,
    cells: HashMap<(i32, i32), Vec<Vec2>>,
}

impl PointGrid {
    fn new(cell: f32) -> Self {
        PointGrid {
            cell,
            cells: HashMap::new(),
        }
    }

    fn insert(&mut self, p: Vec2) {
        self.cells.entry(cell_of(p, self.cell)).or_default().push(p);
    }

    fn any_within(&self, p: Vec2, radius: f32) -> bool {
        let (lo, hi) = (
            cell_of(p - Vec2::splat(radius), self.cell),
            cell_of(p + Vec2::splat(radius), self.cell),
        );
        (lo.0..=hi.0).any(|x| {
            (lo.1..=hi.1).any(|y| {
                self.cells
                    .get(&(x, y))
                    .is_some_and(|c| c.iter().any(|q| q.distance_squared(p) < radius * radius))
            })
        })
    }
}

/// Where the point of segment `ab` closest to `p` lies, from 0 at `a` to 1 at `b`.
fn project(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let len2 = ab.length_squared();
    if len2 > 0.0 {
        ((p - a).dot(ab) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// Simplifies the outline where it is nearly straight and splits edges longer
/// than the mesh spacing, so the triangulation follows the outline. Both are
/// limited for extremely long, crumpled margins.
/// Points for which `pinned` is true are always kept.
fn refine_outline(outline: &[Vec2], spacing: f32, pinned: impl Fn(Vec2) -> bool) -> Vec<Vec2> {
    let n = outline.len();
    // The tip is the point farthest from the base.
    let tip = (0..n)
        .max_by(|&a, &b| outline[a].y.total_cmp(&outline[b].y))
        .unwrap_or(0);
    // A leaf narrower than the tolerance would collapse onto its midrib.
    let half_width = outline.iter().fold(0.0f32, |m, p| m.max(p.x.abs()));
    let mut tolerance = 0.05 * spacing.min(half_width);
    let kept: Vec<Vec2> = loop {
        let mut keep = vec![false; n];
        keep[0] = true;
        keep[tip] = true;
        simplify(outline, 0, tip, tolerance, &mut keep);
        simplify_closing(outline, tip, tolerance, &mut keep);
        if keep.iter().filter(|&&k| k).count() <= MAX_OUTLINE_POINTS {
            break (0..n)
                .filter(|&i| keep[i] || pinned(outline[i]))
                .map(|i| outline[i])
                .collect();
        }
        tolerance *= 2.0;
    };
    let perimeter: f32 = (0..kept.len())
        .map(|i| kept[i].distance(kept[(i + 1) % kept.len()]))
        .sum();
    let max_edge = spacing.max(perimeter / (2 * MAX_OUTLINE_POINTS) as f32);
    let mut refined = Vec::with_capacity(kept.len() * 2);
    for i in 0..kept.len() {
        let a = kept[i];
        let b = kept[(i + 1) % kept.len()];
        let pieces = (a.distance(b) / max_edge).ceil().max(1.0) as usize;
        for j in 0..pieces {
            refined.push(a.lerp(b, j as f32 / pieces as f32));
        }
    }
    refined
}

/// Ramer-Douglas-Peucker between two kept indices.
fn simplify(points: &[Vec2], from: usize, to: usize, tolerance: f32, keep: &mut [bool]) {
    if to <= from + 1 {
        return;
    }
    let (mut worst, mut worst_dist) = (from, 0.0);
    for i in from + 1..to {
        let d = distance_to_segment(points[i], points[from], points[to]);
        if d > worst_dist {
            worst = i;
            worst_dist = d;
        }
    }
    if worst_dist > tolerance {
        keep[worst] = true;
        simplify(points, from, worst, tolerance, keep);
        simplify(points, worst, to, tolerance, keep);
    }
}

/// Simplifies the part of the closed outline from `tip` back to index 0.
fn simplify_closing(points: &[Vec2], tip: usize, tolerance: f32, keep: &mut [bool]) {
    let mut closed = points[tip..].to_vec();
    closed.push(points[0]);
    let mut closed_keep = vec![false; closed.len()];
    simplify(&closed, 0, closed.len() - 1, tolerance, &mut closed_keep);
    for (i, k) in closed_keep.into_iter().enumerate() {
        if k && tip + i < points.len() {
            keep[tip + i] = true;
        }
    }
}

fn hash(a: i32, b: i32, c: i32) -> f32 {
    let mut h = (a as u32).wrapping_mul(0x9e37_79b1)
        ^ (b as u32).wrapping_mul(0x85eb_ca77)
        ^ (c as u32).wrapping_mul(0xc2b2_ae3d);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    (h & 0xffff) as f32 / 65535.0 * 2.0 - 1.0
}

fn bounds(points: &[Vec2]) -> (Vec2, Vec2) {
    points.iter().fold(
        (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN)),
        |(min, max), p| (min.min(*p), max.max(*p)),
    )
}

fn distance_to_segment(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    p.distance(a.lerp(b, project(p, a, b)))
}

/// Point in polygon test with the nonzero winding rule, so that parts of the
/// margin that grew over each other are filled rather than cut out.
fn inside(p: Vec2, polygon: &[Vec2]) -> bool {
    let mut winding = 0;
    for i in 0..polygon.len() {
        let a = polygon[i];
        let b = polygon[(i + 1) % polygon.len()];
        let side = (b - a).perp_dot(p - a);
        if a.y <= p.y && b.y > p.y && side > 0.0 {
            winding += 1;
        } else if a.y > p.y && b.y <= p.y && side < 0.0 {
            winding -= 1;
        }
    }
    winding != 0
}

struct Triangle {
    v: [usize; 3],
    center: DVec2,
    radius2: f64,
}

impl Triangle {
    fn new(v: [usize; 3], points: &[DVec2]) -> Self {
        let (a, b, c) = (points[v[0]], points[v[1]], points[v[2]]);
        let d = 2.0 * (a.x * (b.y - c.y) + b.x * (c.y - a.y) + c.x * (a.y - b.y));
        if d.abs() < 1e-12 {
            // Degenerate triangles are replaced by the next insertion.
            return Triangle {
                v,
                center: a,
                radius2: f64::INFINITY,
            };
        }
        let (a2, b2, c2) = (a.length_squared(), b.length_squared(), c.length_squared());
        let center = DVec2::new(
            (a2 * (b.y - c.y) + b2 * (c.y - a.y) + c2 * (a.y - b.y)) / d,
            (a2 * (c.x - b.x) + b2 * (a.x - c.x) + c2 * (b.x - a.x)) / d,
        );
        Triangle {
            v,
            center,
            radius2: center.distance_squared(a),
        }
    }
}

/// Bowyer-Watson Delaunay triangulation that takes more points later on. The
/// corners of the super triangle come first, so point i of the caller is
/// point i + 3 here.
struct Delaunay {
    points: Vec<DVec2>,
    triangles: Vec<Triangle>,
    /// Edges of the cavity of the last insertion.
    cavity: Vec<(usize, usize)>,
}

impl Delaunay {
    fn new(points: &[Vec2]) -> Self {
        let (min, max) = bounds(points);
        let center = ((min + max) * 0.5).as_dvec2();
        let size = (max - min).max_element().max(1e-3) as f64 * 20.0;
        let corners = vec![
            center + DVec2::new(-size, -size),
            center + DVec2::new(size, -size),
            center + DVec2::new(0.0, size),
        ];
        let mut delaunay = Delaunay {
            triangles: vec![Triangle::new([0, 1, 2], &corners)],
            points: corners,
            cavity: Vec::new(),
        };
        for &p in points {
            delaunay.insert(p);
        }
        delaunay
    }

    fn insert(&mut self, p: Vec2) {
        let i = self.points.len();
        let p = p.as_dvec2();
        self.points.push(p);
        let cavity = &mut self.cavity;
        cavity.clear();
        self.triangles.retain(|t| {
            if t.center.distance_squared(p) < t.radius2 {
                cavity.extend([(t.v[0], t.v[1]), (t.v[1], t.v[2]), (t.v[2], t.v[0])]);
                false
            } else {
                true
            }
        });
        // The cavity boundary consists of the edges not shared by two removed
        // triangles; shared edges appear once in each direction.
        for &(a, b) in &self.cavity {
            if !self.cavity.contains(&(b, a)) {
                self.triangles.push(Triangle::new([a, b, i], &self.points));
            }
        }
    }

    /// Counter clockwise triangles between the points, without the corners
    /// of the super triangle.
    fn triangles(&self) -> Vec<[usize; 3]> {
        self.triangles
            .iter()
            .filter(|t| t.v.iter().all(|&v| v >= 3))
            .map(|t| t.v.map(|v| v - 3))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A straight vein along the y axis, sampled every unit.
    fn midvein(length: f32) -> Vec<VeinLine> {
        let n = length as usize;
        vec![VeinLine {
            points: (0..=n).map(|i| Vec2::new(0.0, length * i as f32 / n as f32)).collect(),
            flux: vec![1.0; n + 1],
            anchors: Vec::new(),
        }]
    }

    /// Area covered by the triangles, clockwise ones count negative.
    fn area(mesh: &Mesh) -> f32 {
        mesh.triangles
            .iter()
            .map(|t| {
                let [a, b, c] = t.map(|i| mesh.points[i]);
                0.5 * (b - a).perp_dot(c - a)
            })
            .sum()
    }

    #[test]
    fn thin_leaves_keep_their_blade() {
        // Narrower than the mesh spacing, at the lowest resolution.
        let outline: Vec<Vec2> = [(0.0, 0.0), (0.4, 50.0), (0.0, 100.0), (-0.4, 50.0)]
            .map(|(x, y)| Vec2::new(x, y))
            .to_vec();
        let mesh = triangulate(&outline, &midvein(100.0), 25.0, 0.0);

        assert!((area(&mesh) - 40.0).abs() < 1e-2, "area {}", area(&mesh));
    }

    #[test]
    fn the_groove_only_touches_its_ribbon() {
        // The blade around the vein must not reach into its groove, or its
        // triangles dent the walls.
        let outline: Vec<Vec2> = [(0.0, 0.0), (30.0, 50.0), (0.0, 100.0), (-30.0, 50.0)]
            .map(|(x, y)| Vec2::new(x, y))
            .to_vec();
        let (vein_width, spacing) = (2.0, 5.0);
        let mesh = triangulate(&outline, &midvein(100.0), spacing, vein_width);

        let deep = |i: usize| mesh.vein[i] > 0.0;
        for t in mesh.triangles.iter().filter(|t| t.iter().any(|&i| deep(i))) {
            for &i in t {
                let p = mesh.points[i];
                assert!(p.x.abs() <= vein_width + 1e-3, "{p} is not in the ribbon");
            }
        }
        // the grooves fade out before the margin, the midvein reaches the tip
        assert!(mesh.points.iter().zip(&mesh.vein).any(|(p, &w)| w > 0.5 && p.y > 80.0));
    }
}
