//! Thin skins on attachments: the veil that spans from the cap margin to the stipe (`membrane`)
//! and the skins that hang from one attachment once it has torn, the ring and the fringe
//! (`skirt`). Both read their attachments the same way and give their skin as thin sheets.

use crate::{add_sheet, Mesh, SheetGrid};
use glam::Vec3;
use nodarium_utils::{
    geometry::{is_path, orthogonal_to, wrap_path},
    split_args, Point,
};
use std::f32::consts::{PI, TAU};

/// The way a skin sags and a torn one falls
pub const DOWN: Vec3 = Vec3::NEG_Y;
/// A gap this many times the usual spacing of a row of points starts a new attachment
const JUMP: f32 = 4.0;
/// A row of points whose ends are this close, in its usual spacing, goes around: a ring
const CLOSING: f32 = 2.5;
/// A ring of points is around the same axis as another ring when its axis passes the centre of
/// the other one closer than this share of its radius
const CONCENTRIC: f32 = 0.5;
/// A torn edge thins to this share of the thickness over the last `EDGE` of its skin
pub const EDGE_THIN: f32 = 0.3;
const EDGE: f32 = 0.25;
/// A torn skin folds about once for every this many lengths of it around
const FOLD_LENGTH: f32 = 1.6;
/// The folds take up this share of the extra length, the skin crumples and stretches for the rest
const FOLD_DEPTH: f32 = 0.6;
/// How uneven the folds are, a turn of their phase in radians
const FOLD_WOBBLE: f32 = 1.5;

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// `v` without its part along [`DOWN`], as a direction
pub fn horizontal(v: Vec3) -> Vec3 {
    (v - DOWN * v.dot(DOWN)).try_normalize().unwrap_or_else(|| orthogonal_to(DOWN))
}

/// What a point of an attachment sits on: the path, how far along it, and its depth
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Source {
    pub id: i32,
    pub alpha: f32,
    pub depth: i32,
}

/// A spot on an attachment, as thick as the path there
#[derive(Clone, Copy, Debug)]
pub struct Spot {
    pub position: Vec3,
    pub radius: f32,
    pub tangent: Vec3,
    pub source: Source,
}

/// One attachment: a path, or points in a row that may go around
pub struct Attachment<'a> {
    pub points: Vec<Vec3>,
    pub radius: Vec<f32>,
    pub sources: Vec<Source>,
    pub closed: bool,
    /// The encoded path, when the attachment is one
    pub path: Option<&'a [i32]>,
    /// Length along the attachment to every point
    along: Vec<f32>,
    /// The direction of the path every point of a row sits on, empty for a path
    tangents: Vec<Vec3>,
}

impl<'a> Attachment<'a> {
    pub fn new(points: Vec<Vec3>, radius: Vec<f32>, sources: Vec<Source>, closed: bool, path: Option<&'a [i32]>) -> Self {
        let mut along = vec![0.0; points.len()];
        for i in 1..points.len() {
            along[i] = along[i - 1] + points[i].distance(points[i - 1]);
        }
        Attachment { points, radius, sources, closed, path, along, tangents: Vec::new() }
    }

    fn from_row(row: &[Point], closed: bool) -> Self {
        let mut attachment = Attachment::new(
            row.iter().map(|p| p.position).collect(),
            row.iter().map(|p| p.scale.abs()).collect(),
            row.iter().map(|p| Source { id: p.parent_id, alpha: p.alpha, depth: p.depth }).collect(),
            closed,
            None,
        );
        attachment.tangents = row.iter().map(|p| p.tangent).collect();
        attachment
    }

    fn from_path(item: &'a [i32]) -> Self {
        let path = wrap_path(item);
        let points: Vec<Vec3> = path.points.chunks_exact(4).map(|p| Vec3::new(p[0], p[1], p[2])).collect();
        let radius = path.points.chunks_exact(4).map(|p| p[3].abs()).collect();
        let n = points.len();
        let mut attachment = Attachment::new(points, radius, Vec::new(), false, Some(item));
        let length = attachment.length();
        attachment.sources = (0..n)
            .map(|i| Source {
                id: path.link.id,
                alpha: if length > 0.0 { attachment.along[i] / length } else { 0.0 },
                depth: path.depth,
            })
            .collect();
        attachment
    }

    pub fn length(&self) -> f32 {
        self.along.last().copied().unwrap_or(0.0)
    }

    pub fn centre(&self) -> Vec3 {
        self.points.iter().copied().sum::<Vec3>() / self.points.len().max(1) as f32
    }

    /// Whether this is a ring of points
    pub fn is_ring(&self) -> bool {
        self.path.is_none() && self.closed
    }

    /// The axis of a ring: its centre and the normal of its plane (Newell)
    pub fn axis(&self) -> Axis {
        let m = self.points.len();
        let normal = (0..m).fold(Vec3::ZERO, |sum, i| sum + self.points[i].cross(self.points[(i + 1) % m]));
        (self.centre(), normal.try_normalize().unwrap_or(Vec3::Y))
    }

    /// Point `i` of a row as a spot on the path it sits on
    pub fn spot(&self, i: usize) -> Spot {
        Spot {
            position: self.points[i],
            radius: self.radius[i],
            tangent: self.tangents.get(i).and_then(|t| t.try_normalize()).unwrap_or(Vec3::Y),
            source: self.sources[i],
        }
    }

    /// Whether this is a ring of points whose axis runs about through the centre of `other`
    pub fn around_axis_of(&self, other: &Attachment) -> bool {
        if !self.is_ring() {
            return false;
        }
        let axis = self.axis();
        let radius = self.points.iter().map(|p| p.distance(axis.0)).sum::<f32>() / self.points.len() as f32;
        from_axis(Some(axis), other.centre()) < CONCENTRIC * radius
    }

    /// The segments between neighbouring points, with the one back to the start of a ring
    fn segments(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        let n = self.points.len();
        let closing = (self.closed && n > 2).then_some((n - 1, 0));
        (0..n.saturating_sub(1)).map(|i| (i, i + 1)).chain(closing)
    }

    /// How far `p` is from the attachment, and the closest spot as a share of its length (0 for
    /// a single point)
    pub fn closest(&self, p: Vec3) -> (f32, f32) {
        if self.points.len() == 1 {
            return (self.points[0].distance(p), 0.0);
        }
        let length = self.length();
        self.segments()
            .map(|(i, j)| {
                let (a, b) = (self.points[i], self.points[j]);
                let ab = b - a;
                let t = if ab.length_squared() > 0.0 { ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
                let distance = (a + ab * t).distance(p);
                let s = if j > i { self.along[i] + t * ab.length() } else { self.along[i] };
                (distance, if length > 0.0 { s / length } else { 0.0 })
            })
            .fold((f32::INFINITY, 0.0), |best, c| if c.0 < best.0 { c } else { best })
    }

    /// The spot `u` of the way along the attachment, by length. On a path the spot is on the
    /// path, between points of a row it takes the nearer point as its source.
    pub fn at(&self, u: f32) -> Spot {
        let n = self.points.len();
        let u = u.clamp(0.0, 1.0);
        if n == 1 {
            return self.spot(0);
        }
        let s = u * self.length();
        let i = (0..n - 1).find(|&i| self.along[i + 1] >= s).unwrap_or(n - 2);
        let span = self.along[i + 1] - self.along[i];
        let t = if span > 0.0 { ((s - self.along[i]) / span).clamp(0.0, 1.0) } else { 0.0 };
        let (a, b) = (self.points[i], self.points[i + 1]);
        let source = if self.path.is_some() {
            Source { alpha: u, ..self.sources[i] }
        } else {
            self.sources[if t < 0.5 { i } else { i + 1 }]
        };
        Spot {
            position: a.lerp(b, t),
            radius: self.radius[i] + (self.radius[i + 1] - self.radius[i]) * t,
            tangent: (b - a).try_normalize().unwrap_or(Vec3::Y),
            source,
        }
    }

    /// The points by their angle around the axis through `centre`, in the plane of `e1` and `e2`
    pub fn by_angle(&self, centre: Vec3, e1: Vec3, e2: Vec3) -> Vec<(f32, usize)> {
        let mut angles: Vec<(f32, usize)> = self
            .points
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let v = *p - centre;
                (v.dot(e2).atan2(v.dot(e1)).rem_euclid(TAU), i)
            })
            .collect();
        angles.sort_by(|a, b| a.0.total_cmp(&b.0));
        angles
    }

    /// The ring of points at `angle`, between the two points around it
    pub fn at_angle(&self, sorted: &[(f32, usize)], angle: f32) -> (Vec3, Source) {
        let n = sorted.len();
        if n == 1 {
            return (self.points[sorted[0].1], self.sources[sorted[0].1]);
        }
        let next = sorted.partition_point(|s| s.0 <= angle);
        let (lo, hi) = if next == 0 || next == n { (n - 1, 0) } else { (next - 1, next) };
        let span = (sorted[hi].0 - sorted[lo].0).rem_euclid(TAU);
        let t = if span > 0.0 { ((angle - sorted[lo].0).rem_euclid(TAU) / span).clamp(0.0, 1.0) } else { 0.0 };
        let (a, b) = (sorted[lo].1, sorted[hi].1);
        (self.points[a].lerp(self.points[b], t), self.sources[if t < 0.5 { a } else { b }])
    }
}

/// The parts of an input: paths and points, also those in lists inside it
fn collect<'a>(list: &'a [i32], paths: &mut Vec<&'a [i32]>, points: &mut Vec<Point>) {
    if list.len() < 4 {
        return;
    }
    for item in split_args(list) {
        if let Some(point) = Point::decode(item) {
            points.push(point);
        } else if item.len() > 4 && item[0] == 0 && item[1] == 1 {
            // a list inside the list, a path never has a count of 1
            collect(item, paths, points);
        } else if is_path(item) {
            paths.push(item);
        }
    }
}

fn median(mut values: Vec<f32>) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(f32::total_cmp);
    values[values.len() / 2]
}

/// The attachments of an input: every path, and the points split into rows where they jump
pub fn read_attachments(arg: &[i32]) -> Vec<Attachment<'_>> {
    let (mut paths, mut points) = (Vec::new(), Vec::new());
    collect(arg, &mut paths, &mut points);
    let mut attachments: Vec<Attachment> =
        paths.into_iter().filter(|p| wrap_path(p).length > 0).map(Attachment::from_path).collect();

    let gaps: Vec<f32> = points.windows(2).map(|w| w[0].position.distance(w[1].position)).collect();
    let limit = JUMP * median(gaps.clone());
    let mut start = 0;
    for end in 1..=points.len() {
        if end < points.len() && gaps[end - 1] <= limit {
            continue;
        }
        let row = &points[start..end];
        let spacing = median(row.windows(2).map(|w| w[0].position.distance(w[1].position)).collect());
        let ends = row[0].position.distance(row[row.len() - 1].position);
        let closed = row.len() >= 3 && ends <= CLOSING * spacing;
        attachments.push(Attachment::from_row(row, closed));
        start = end;
    }
    attachments
}

/// The centre and direction of the axis a skin goes around
pub type Axis = (Vec3, Vec3);

/// How far `p` is from the axis
pub fn from_axis(axis: Option<Axis>, p: Vec3) -> f32 {
    axis.map_or(0.0, |(centre, direction)| {
        let q = p - centre;
        (q - direction * q.dot(direction)).length()
    })
}

/// The circle around a spot on a path, across its tangent, a little inside its surface so a skin
/// attached there grows out of it
pub struct Collar {
    pub centre: Vec3,
    pub tangent: Vec3,
    /// Angle 0 and a quarter turn around the path
    pub e1: Vec3,
    pub e2: Vec3,
    reach: f32,
}

impl Collar {
    pub fn new(spot: &Spot, inset: f32) -> Self {
        let e1 = orthogonal_to(spot.tangent);
        Collar {
            centre: spot.position,
            tangent: spot.tangent,
            e1,
            e2: spot.tangent.cross(e1),
            reach: (spot.radius - inset).max(0.0),
        }
    }

    /// The direction out of the path at `angle`
    pub fn out(&self, angle: f32) -> Vec3 {
        self.e1 * angle.cos() + self.e2 * angle.sin()
    }

    pub fn at(&self, angle: f32) -> Vec3 {
        self.centre + self.out(angle) * self.reach
    }
}

/// One strand's share of a skin: its points from where it is attached to its other end, and at
/// every point how much longer the circle around the axis it would lie on spread out is than the
/// one it hangs on, as a share of the latter. `None` where the strand has no skin.
pub type Column = Option<Vec<(Vec3, f32)>>;

/// A skin, strand by strand
pub struct Skin<'s> {
    pub columns: &'s [Column],
    /// Where every strand is attached
    pub attached: &'s [Vec3],
    /// The axis of a skin that goes around, a torn one folds around it
    pub axis: Option<Axis>,
    /// A torn skin thins towards its edge and, around an axis, folds where it has more skin
    /// than the circle it hangs on: where every strand sits around it (0..1) and a smooth noise
    /// there that makes the folds uneven
    pub torn: Option<(&'s [f32], &'s [f32])>,
}

/// Adds a skin to `mesh` as thin closed sheets: one for every run of neighbouring strands that
/// have a column. Around an axis a run of every strand closes on itself. A run that ends is half
/// a strand spacing wider on either side, as wide as its strands cover, so a single strand still
/// gives a narrow flap.
///
/// A torn skin that hangs closer to the axis than it would lie spread out has more skin than the
/// circle it hangs on, and a thin sheet bends rather than squeezes: it folds (Sharon et al.
/// 2002), out and in across the sheet, see [`fold`].
pub fn add_skin(mesh: &mut Mesh, skin: &Skin, thickness: f32) {
    let piece = skin.columns;
    let n = piece.len();
    let closed = skin.axis.is_some();
    let radius = 0.5 * thickness;
    let torn_edge = |u: f32| radius * (1.0 - EDGE_THIN * smoothstep(1.0 - EDGE, 1.0, u));
    // the spacing of the strands where they are attached, across the skin
    let spacing = |k: usize| {
        let (prev, next, steps) = if closed {
            ((k + n - 1) % n, (k + 1) % n, 2.0)
        } else {
            let (prev, next) = (k.saturating_sub(1), (k + 1).min(n - 1));
            (prev, next, ((next - prev) as f32).max(1.0))
        };
        (skin.attached[next] - skin.attached[prev]) / steps
    };
    for (mut run, wrap) in runs(piece, closed) {
        let rows = piece[run[0]].as_ref().map_or(0, Vec::len);
        if rows < 2 {
            continue;
        }
        let column = |k: usize| piece[k].as_ref().unwrap();
        let mut grid: Vec<Vec<Vec3>> = run.iter().map(|&k| column(k).iter().map(|q| q.0).collect()).collect();
        let mut excess: Vec<Vec<f32>> = run.iter().map(|&k| column(k).iter().map(|q| q.1).collect()).collect();
        if !wrap {
            // half a spacing more on either side, across the skin
            let m = grid.len();
            let side = |c: usize, towards: usize, k: usize, sign: f32| -> Vec<Vec3> {
                (0..rows)
                    .map(|i| {
                        let across = if m > 1 { grid[c][i] - grid[towards][i] } else { spacing(k) * sign };
                        grid[c][i] + 0.5 * across
                    })
                    .collect()
            };
            // not past the ends of a skin that doesn't go around
            let (first, last) = (run[0], run[run.len() - 1]);
            let before = (closed || first > 0).then(|| side(0, 1.min(m - 1), first, -1.0));
            let after = (closed || last + 1 < n).then(|| side(m - 1, m.saturating_sub(2), last, 1.0));
            if let Some(before) = before {
                grid.insert(0, before);
                excess.insert(0, excess[0].clone());
                run.insert(0, first);
            }
            if let Some(after) = after {
                grid.push(after);
                excess.push(excess[excess.len() - 1].clone());
                run.push(last);
            }
            if grid.len() < 2 {
                continue;
            }
        }
        if let (Some(axis), Some((place, wobble))) = (skin.axis, skin.torn) {
            let places: Vec<(f32, f32)> = run.iter().map(|&k| (place[k], wobble[k])).collect();
            fold(&mut grid, &excess, &places, wrap, axis);
        }
        let half: Vec<f32> =
            (0..rows).map(|i| if skin.torn.is_some() { torn_edge(i as f32 / (rows - 1) as f32) } else { radius }).collect();
        let normals = sheet_normals(&grid, wrap);
        let sheet = SheetGrid { columns: grid.len(), rows, wrap };
        add_sheet(mesh, sheet, |c, i| grid[c][i] + normals[c][i] * half[i], |c, i| grid[c][i] - normals[c][i] * half[i], None);
    }
}

/// Folds a torn skin that goes around: about one fold for every `FOLD_LENGTH` lengths of it
/// around it, a little uneven, deep enough to take up `FOLD_DEPTH` of the extra length of every
/// circle. `places` is where the strand of every column sits around the axis and its wobble.
fn fold(grid: &mut [Vec<Vec3>], excess: &[Vec<f32>], places: &[(f32, f32)], wrap: bool, axis: Axis) {
    let (m, rows) = (grid.len(), grid[0].len());
    let length = |c: usize| grid[c].windows(2).map(|w| w[0].distance(w[1])).sum::<f32>();
    let part = (0..m).map(length).sum::<f32>() / m as f32;
    let around = (0..m).map(|c| from_axis(Some(axis), grid[c][rows - 1])).sum::<f32>() / m as f32 * TAU;
    if part <= 0.0 || around <= 0.0 {
        return;
    }
    let folds = (around / (FOLD_LENGTH * part)).round().max(2.0);
    let normals = sheet_normals(grid, wrap);
    for c in 0..m {
        let (place, wobble) = places[c];
        let wave = (TAU * folds * place + FOLD_WOBBLE * wobble).sin();
        for i in 0..rows {
            // a wave of `folds` waves around a circle is longer by (π depth / wavelength)²
            let wavelength = TAU * from_axis(Some(axis), grid[c][i]) / folds;
            let depth = FOLD_DEPTH * wavelength / PI * excess[c][i].sqrt();
            grid[c][i] += normals[c][i] * (depth * wave);
        }
    }
}

/// The runs of neighbouring strands that have a column, and whether a run goes all the way round
fn runs(piece: &[Column], closed: bool) -> Vec<(Vec<usize>, bool)> {
    let n = piece.len();
    let has = |k: usize| piece[k].is_some();
    if n == 0 {
        return Vec::new();
    }
    if (0..n).all(has) {
        return vec![((0..n).collect(), closed)];
    }
    // around a ring, start after a gap so no run is split at the seam
    let start = if closed { (0..n).find(|&k| !has(k)).map_or(0, |k| k + 1) } else { 0 };
    let mut out = Vec::new();
    let mut run = Vec::new();
    for step in 0..n {
        let k = (start + step) % n;
        if has(k) {
            run.push(k);
        } else if !run.is_empty() {
            out.push((std::mem::take(&mut run), false));
        }
    }
    if !run.is_empty() {
        out.push((run, false));
    }
    out
}

/// The normal of a sheet at every point of its grid, the way `column × row` points
fn sheet_normals(grid: &[Vec<Vec3>], wrap: bool) -> Vec<Vec<Vec3>> {
    let (m, rows) = (grid.len(), grid[0].len());
    let around = |c: usize| {
        if wrap {
            ((c + m - 1) % m, (c + 1) % m)
        } else {
            (c.saturating_sub(1), (c + 1).min(m - 1))
        }
    };
    (0..m)
        .map(|c| {
            let (prev, next) = around(c);
            let mut last = Vec3::ZERO;
            (0..rows)
                .map(|i| {
                    let across = grid[next][i] - grid[prev][i];
                    let down = grid[c][(i + 1).min(rows - 1)] - grid[c][i.saturating_sub(1)];
                    let normal = across.cross(down).try_normalize().unwrap_or(if last == Vec3::ZERO { orthogonal_to(down) } else { last });
                    last = normal;
                    normal
                })
                .collect()
        })
        .collect()
}
