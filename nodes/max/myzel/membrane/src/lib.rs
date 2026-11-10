use glam::Vec3;
use myzel_core::{add_sheet, smooth_normals, Mesh, SheetGrid};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{
    concat_arg_vecs, curve, decode_float,
    geometry::{create_path, is_path, orthogonal_to, path_id, path_random, set_path_link, wrap_path, wrap_path_mut, PathLink},
    int, split_args, Ctx, PathInfo, Point,
};
use noise::{NoiseFn, OpenSimplex};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

nodarium_definition_file!("src/definition.json");

/// The way a torn part falls and the skin sags
const DOWN: Vec3 = Vec3::NEG_Y;
/// A gap this many times the usual spacing of a row of points starts a new attachment
const JUMP: f32 = 4.0;
/// A row of points whose ends are this close, in its usual spacing, goes around: a ring
const CLOSING: f32 = 2.5;
/// By this much of `tear` every strand has torn, over the rest of it the parts fall
const TEAR_SPREAD: f32 = 0.5;
/// How far the tear line wanders from `tear_position`, as a share of the span
const RAGGED: f32 = 0.15;
/// A torn part shrinks by this share of its length as it falls, so the parts come apart
const RECOIL: f32 = 0.3;
/// Torn parts shorter than this share of the span are left out
const MIN_PART: f32 = 0.03;
/// A torn edge thins to this share of the thickness over the last `EDGE` of its part
const EDGE_THIN: f32 = 0.3;
const EDGE: f32 = 0.25;
/// Radius of the circle the noise of the tear line is read on, it sets how many lobes it has
const RAGGED_SCALE: f64 = 4.0;
/// The same for where the tear starts, fewer lobes start it in fewer places
const ONSET_SCALE: f64 = 0.6;
/// The skin takes this share of the jitter of single strands into its torn edge, the rest of
/// its raggedness comes from the smooth tear line
const SKIN_JITTER: f32 = 0.35;
/// How far the fall of the skin is smoothed between neighbouring strands, in strands either way
const SKIN_SMOOTH: i32 = 3;
/// A torn skin folds about once for every this many part lengths around
const FOLD_LENGTH: f32 = 1.6;
/// The folds take up this share of the extra length, the skin crumples and stretches for the rest
const FOLD_DEPTH: f32 = 0.6;
/// How uneven the folds are, a turn of their phase in radians
const FOLD_WOBBLE: f32 = 1.5;
/// Radius of the circle the noise that makes folds uneven is read on
const FOLD_SCALE: f64 = 1.5;
const MAX_STRANDS: i32 = 4096;
/// Strands across the skin. Neighbours touch when there are about as many as the longer
/// attachment is long in thicknesses.
const STRANDS: i32 = 256;
const MIN_POINTS: usize = 3;

/// The words inside the brackets of a framed list `[0, n + 1, ...words, 1, 1]`
fn unframe(arg: &[i32]) -> Option<&[i32]> {
    let n = arg.len();
    let framed = n >= 4 && arg[0] == 0 && arg[1] as usize == n - 3 && arg[n - 2] == 1 && arg[n - 1] == 1;
    framed.then(|| &arg[2..n - 2])
}

/// A list of floats as the host sends a shape or curve. The value on the node arrives as one
/// frame, a connected node sends `concat_args([frame])` = `[0, 1, ...frame, 1, 1]`. Not read with
/// `split_args`: a point at 0 would read as the start of a bracket.
fn read_floats(arg: &[i32]) -> Vec<f32> {
    let n = arg.len();
    let connected = n >= 8 && arg[..2] == [0, 1] && arg[n - 2..] == [1, 1] && unframe(&arg[2..n - 2]).is_some();
    let frame = if connected { &arg[2..n - 2] } else { arg };
    unframe(frame).map_or_else(Vec::new, |words| words.iter().map(|&w| decode_float(w)).collect())
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// What a strand hangs from: the path, how far along it, and its depth
#[derive(Clone, Copy, Debug, PartialEq)]
struct Source {
    id: i32,
    alpha: f32,
    depth: i32,
}

/// One attachment: a path, or points in a row that may go around
struct Curve<'a> {
    points: Vec<Vec3>,
    radius: Vec<f32>,
    sources: Vec<Source>,
    closed: bool,
    /// The encoded path, when the attachment is one
    path: Option<&'a [i32]>,
    /// Length along the curve to every point
    along: Vec<f32>,
}

/// A spot on an attachment
struct Spot {
    position: Vec3,
    radius: f32,
    tangent: Vec3,
    source: Source,
}

impl<'a> Curve<'a> {
    fn new(points: Vec<Vec3>, radius: Vec<f32>, sources: Vec<Source>, closed: bool, path: Option<&'a [i32]>) -> Self {
        let mut along = vec![0.0; points.len()];
        for i in 1..points.len() {
            along[i] = along[i - 1] + points[i].distance(points[i - 1]);
        }
        Curve { points, radius, sources, closed, path, along }
    }

    fn from_path(item: &'a [i32]) -> Self {
        let path = wrap_path(item);
        let points: Vec<Vec3> = path.points.chunks_exact(4).map(|p| Vec3::new(p[0], p[1], p[2])).collect();
        let radius = path.points.chunks_exact(4).map(|p| p[3].abs()).collect();
        let n = points.len();
        let mut curve = Curve::new(points, radius, Vec::new(), false, Some(item));
        let length = curve.length();
        curve.sources = (0..n)
            .map(|i| Source {
                id: path.link.id,
                alpha: if length > 0.0 { curve.along[i] / length } else { 0.0 },
                depth: path.depth,
            })
            .collect();
        curve
    }

    fn length(&self) -> f32 {
        self.along.last().copied().unwrap_or(0.0)
    }

    fn centre(&self) -> Vec3 {
        self.points.iter().copied().sum::<Vec3>() / self.points.len().max(1) as f32
    }

    /// The segments between neighbouring points, with the one back to the start of a ring
    fn segments(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        let n = self.points.len();
        let closing = (self.closed && n > 2).then_some((n - 1, 0));
        (0..n.saturating_sub(1)).map(|i| (i, i + 1)).chain(closing)
    }

    /// The spot of the curve closest to `p`, as a share of its length (0 for a single point)
    fn closest(&self, p: Vec3) -> (f32, f32) {
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

    /// The spot `u` of the way along the curve, by length. On a path the spot is on the path,
    /// between points of a row it takes the nearer point as its source.
    fn at(&self, u: f32) -> Spot {
        let n = self.points.len();
        let u = u.clamp(0.0, 1.0);
        if n == 1 {
            return Spot { position: self.points[0], radius: self.radius[0], tangent: Vec3::Y, source: self.sources[0] };
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
    fn by_angle(&self, centre: Vec3, e1: Vec3, e2: Vec3) -> Vec<(f32, usize)> {
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
    fn at_angle(&self, sorted: &[(f32, usize)], angle: f32) -> (Vec3, Source) {
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
fn read_curves(arg: &[i32]) -> Vec<Curve<'_>> {
    let (mut paths, mut points) = (Vec::new(), Vec::new());
    collect(arg, &mut paths, &mut points);
    let mut curves: Vec<Curve> = paths.into_iter().filter(|p| wrap_path(p).length > 0).map(Curve::from_path).collect();

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
        curves.push(Curve::new(
            row.iter().map(|p| p.position).collect(),
            row.iter().map(|p| p.scale.abs()).collect(),
            row.iter().map(|p| Source { id: p.parent_id, alpha: p.alpha, depth: p.depth }).collect(),
            closed,
            None,
        ));
        start = end;
    }
    curves
}

/// Where strand `k` is attached on both sides, and where it sits along the membrane (0..1)
struct Anchor {
    b: Vec3,
    b_source: Source,
    a: Vec3,
    a_source: Source,
    place: f32,
}

/// Where on a path in b the membrane wraps around it, as a share of its length: at `along` when
/// it is set, so it keeps its share while the path grows. Else `anchor` metres from its base, so
/// it stays there while the path grows and a moves. Otherwise at `attachment`, between the level
/// of a (0) and the tip of the path (1), or its base (-1).
fn wrap_alpha(a: &Curve, b: &Curve, attachment: f32, anchor: f32, along: f32) -> f32 {
    if along > 0.0 {
        return along.min(1.0);
    }
    let length = b.length();
    if anchor > 0.0 && length > 0.0 {
        return (anchor / length).min(1.0);
    }
    let level = b.closest(a.centre()).1;
    if attachment >= 0.0 {
        level + attachment * (1.0 - level)
    } else {
        level * (1.0 + attachment)
    }
}

/// The centre and direction of the axis a membrane goes around
type Axis = (Vec3, Vec3);

/// A ring of points around a path: the membrane wraps around the path at `alpha` of its length.
/// The strands start inside the surface of the path by `inset`, so the skin grows out of it.
fn around_path(a: &Curve, b: &Curve, alpha: f32, n: usize, inset: f32) -> (Vec<Anchor>, Axis) {
    let ring = b.at(alpha);
    let e1 = orthogonal_to(ring.tangent);
    let e2 = ring.tangent.cross(e1);
    let sorted = a.by_angle(ring.position, e1, e2);
    let reach = (ring.radius - inset).max(0.0);
    let anchors = (0..n)
        .map(|k| {
            let angle = TAU * k as f32 / n as f32;
            let (position, a_source) = a.at_angle(&sorted, angle);
            Anchor {
                b: ring.position + (e1 * angle.cos() + e2 * angle.sin()) * reach,
                b_source: ring.source,
                a: position,
                a_source,
                place: k as f32 / n as f32,
            }
        })
        .collect();
    (anchors, (ring.position, ring.tangent))
}

/// A ring of points to a ring of points or a single point, matched by their angle around the
/// axis of the ring in a
fn ring_to_ring(a: &Curve, b: &Curve, n: usize) -> (Vec<Anchor>, Axis) {
    // Newell's normal of the ring
    let m = a.points.len();
    let normal = (0..m).fold(Vec3::ZERO, |sum, i| sum + a.points[i].cross(a.points[(i + 1) % m]));
    let axis = normal.try_normalize().unwrap_or(Vec3::Y);
    let centre = b.centre();
    let e1 = orthogonal_to(axis);
    let e2 = axis.cross(e1);
    let (sorted_a, sorted_b) = (a.by_angle(centre, e1, e2), b.by_angle(centre, e1, e2));
    let anchors = (0..n)
        .map(|k| {
            let angle = TAU * k as f32 / n as f32;
            let (a_position, a_source) = a.at_angle(&sorted_a, angle);
            let (b_position, b_source) = b.at_angle(&sorted_b, angle);
            Anchor { b: b_position, b_source, a: a_position, a_source, place: k as f32 / n as f32 }
        })
        .collect();
    (anchors, (centre, axis))
}

/// Two attachments joined from end to end, the strands spread evenly along both
fn end_to_end(a: &Curve, b: &Curve, n: usize) -> Vec<Anchor> {
    let (a0, a1, b0, b1) = (a.at(0.0).position, a.at(1.0).position, b.at(0.0).position, b.at(1.0).position);
    let reversed = a0.distance(b1) + a1.distance(b0) < a0.distance(b0) + a1.distance(b1);
    (0..n)
        .map(|k| {
            let u = if n == 1 { 0.5 } else { k as f32 / (n - 1) as f32 };
            let (sa, sb) = (a.at(u), b.at(if reversed { 1.0 - u } else { u }));
            Anchor { b: sb.position, b_source: sb.source, a: sa.position, a_source: sa.source, place: u }
        })
        .collect()
}

/// A torn part that falls from where it is attached: every segment turns from where it lay in
/// the skin towards hanging down, flared towards `side` by the flare curve, and keeps its length
/// up to the recoil
fn fall(points: &[Vec3], side: Vec3, flare: &[f32], relax: f32) -> Vec<Vec3> {
    let n = points.len();
    let mut out = Vec::with_capacity(n);
    let mut p = points[0];
    out.push(p);
    for j in 0..n - 1 {
        let step = points[j + 1] - points[j];
        let along = (j as f32 + 0.5) / (n - 1) as f32;
        let angle = curve(flare, along).clamp(0.0, 1.0) * FRAC_PI_2;
        let hanging = DOWN * angle.cos() + side * angle.sin();
        let direction = step.try_normalize().map_or(hanging, |d| d.slerp(hanging, relax));
        p += direction * step.length() * (1.0 - RECOIL * relax);
        out.push(p);
    }
    out
}

fn horizontal(v: Vec3) -> Vec3 {
    (v - DOWN * v.dot(DOWN)).try_normalize().unwrap_or_else(|| orthogonal_to(DOWN))
}

fn encode(link: PathLink, depth: i32, points: &[Vec3], radius: impl Fn(f32) -> f32) -> Vec<i32> {
    let mut data = create_path(points.len(), depth);
    set_path_link(&mut data, link);
    let path = wrap_path_mut(&mut data);
    let last = (points.len() - 1).max(1) as f32;
    for (i, p) in points.iter().enumerate() {
        path.points[i * 4..i * 4 + 4].copy_from_slice(&[p.x, p.y, p.z, radius(i as f32 / last)]);
    }
    data
}

struct Skin<'f> {
    thickness: f32,
    sag: f32,
    tear: f32,
    tear_position: f32,
    /// How long the skin was when it tore, 0 for as long as the span
    torn_length: f32,
    flare: &'f [f32],
    resolution: usize,
}

/// One strand's share of a piece of the skin mesh: its points from where it is attached to its
/// other end, and at every point how much longer the circle around the axis it lay on in the
/// skin is than the one it hangs on now, as a share of the latter (see [`add_skin`])
type Column = Option<Vec<(Vec3, f32)>>;

/// The pieces of the skin, strand by strand: whole while intact, the parts on b and on a once torn
struct SkinColumns {
    intact: Vec<Column>,
    on_b: Vec<Column>,
    on_a: Vec<Column>,
    /// Where every strand sits around the membrane (0..1) and a smooth noise there, for the folds
    place: Vec<f32>,
    wobble: Vec<f32>,
}

/// The strands of one membrane: whole while intact, the part on b and the part on a once torn.
/// Also the same pieces with `resolution` points each for the skin mesh. `axis` is the centre
/// and direction of a membrane that goes around, the skin folds around it.
fn strands(
    anchors: &[Anchor],
    axis: Option<Axis>,
    skin: &Skin,
    seed: i32,
    creator: i32,
    offset: usize,
) -> (Vec<Vec<i32>>, Vec<Vec<i32>>, SkinColumns) {
    let n = anchors.len();
    let closed = axis.is_some();
    let mut columns = SkinColumns {
        intact: vec![None; n],
        on_b: vec![None; n],
        on_a: vec![None; n],
        place: anchors.iter().map(|a| a.place).collect(),
        wobble: vec![0.0; n],
    };
    if n == 0 {
        return (Vec::new(), Vec::new(), columns);
    }
    let key = path_id(creator, anchors[0].b_source.id, offset as i32);
    let noise = OpenSimplex::new((seed as u32) ^ (key as u32));
    // smooth along the membrane, around a ring it closes on itself
    let wave = |place: f32, scale: f64, salt: f64| {
        let angle = TAU as f64 * place as f64;
        let q = if closed { [scale * angle.cos(), scale * angle.sin(), salt] } else { [scale * angle, 0.0, salt] };
        (2.0 * noise.get(q) as f32).clamp(-1.0, 1.0)
    };
    let link_b = |k: usize| {
        let s = anchors[k].b_source;
        PathLink::child(creator, s.id, s.alpha, 2 * (offset + k) as i32)
    };
    let random = |k: usize, salt: i32| path_random(seed, link_b(k).id, salt);

    // the tear starts where the wave is lowest and runs from there, strands tear one after the
    // other until all of them have at TEAR_SPREAD
    let mut order: Vec<(f32, usize)> = (0..n).map(|k| (wave(anchors[k].place, ONSET_SCALE, 0.0) + 0.05 * (random(k, 1) - 0.5), k)).collect();
    order.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut onset = vec![0.0; n];
    for (rank, &(_, k)) in order.iter().enumerate() {
        onset[k] = TEAR_SPREAD * rank as f32 / n as f32;
    }

    // the skin is one sheet: how far its parts have fallen is smoothed over a few neighbouring
    // strands, and its tear line keeps only some of the jitter of single strands, so it doesn't
    // pleat from strand to strand
    let relax_of = |k: usize| smoothstep(0.0, 1.0, (skin.tear - onset[k]) / (1.0 - TEAR_SPREAD));
    let near = |k: usize| {
        (-SKIN_SMOOTH..=SKIN_SMOOTH).filter_map(move |d| {
            let j = k as i32 + d;
            if closed {
                Some(j.rem_euclid(n as i32) as usize)
            } else {
                (0..n as i32).contains(&j).then_some(j as usize)
            }
        })
    };
    let skin_relax: Vec<f32> = (0..n).map(|k| near(k).map(relax_of).sum::<f32>() / near(k).count() as f32).collect();
    let from_axis = |p: Vec3| {
        axis.map_or(0.0, |(centre, direction)| {
            let q = p - centre;
            (q - direction * q.dot(direction)).length()
        })
    };
    for (k, anchor) in anchors.iter().enumerate() {
        columns.wobble[k] = wave(anchor.place, FOLD_SCALE, 2.0);
    }

    let radius = 0.5 * skin.thickness;
    let edge = |u: f32| radius * (1.0 - EDGE_THIN * smoothstep(1.0 - EDGE, 1.0, u));
    let res = skin.resolution;
    let (mut on_b, mut on_a) = (Vec::new(), Vec::new());
    for (k, anchor) in anchors.iter().enumerate() {
        let chord = anchor.a - anchor.b;
        let span = chord.length();
        let sag = chord.try_normalize().map_or(Vec3::ZERO, |c| (DOWN - c * DOWN.dot(c)).normalize_or_zero()) * skin.sag * span;
        let skin_at = |t: f32| anchor.b + chord * t + sag * (4.0 * t * (1.0 - t));
        let depth_b = anchor.b_source.depth + 1;

        if skin.tear <= onset[k] {
            let points: Vec<Vec3> = (0..res).map(|i| skin_at(i as f32 / (res - 1) as f32)).collect();
            on_b.push(encode(link_b(k), depth_b, &points, |_| radius));
            columns.intact[k] = Some(points.into_iter().map(|p| (p, 0.0)).collect());
            continue;
        }
        let relax = smoothstep(0.0, 1.0, (skin.tear - onset[k]) / (1.0 - TEAR_SPREAD));
        let ragged = 0.6 * wave(anchor.place, RAGGED_SCALE, 1.0) + 0.4 * (2.0 * random(k, 2) - 1.0);
        let cut = (skin.tear_position + RAGGED * ragged).clamp(0.0, 1.0);
        let count = |share: f32| ((res as f32 * share).round() as usize).max(MIN_POINTS);
        // a torn part keeps the length it tore at: past the torn length it stops stretching with
        // the span, and the parts take only their share of it from either end
        let kept = if skin.torn_length > 0.0 && span > skin.torn_length { skin.torn_length / span } else { 1.0 };
        let (on_b_share, on_a_share) = (cut * kept, (1.0 - cut) * kept);

        let part = |m: usize, at: &dyn Fn(usize, usize) -> f32, side: Vec3| {
            let points: Vec<Vec3> = (0..m).map(|i| skin_at(at(i, m))).collect();
            fall(&points, side, skin.flare, relax)
        };
        if cut > MIN_PART {
            let to_b = |i: usize, m: usize| on_b_share * i as f32 / (m - 1) as f32;
            on_b.push(encode(link_b(k), depth_b, &part(count(cut), &to_b, horizontal(chord)), edge));
        }
        if cut < 1.0 - MIN_PART {
            let to_a = |i: usize, m: usize| 1.0 - on_a_share * i as f32 / (m - 1) as f32;
            let s = anchor.a_source;
            let link = PathLink::child(creator, s.id, s.alpha, 2 * (offset + k) as i32 + 1);
            on_a.push(encode(link, s.depth + 1, &part(count(1.0 - cut), &to_a, horizontal(-chord)), edge));
        }

        // the skin takes the same parts with `res` points for every strand, so neighbours line up
        let skin_ragged = 0.6 * wave(anchor.place, RAGGED_SCALE, 1.0) + 0.4 * SKIN_JITTER * (2.0 * random(k, 2) - 1.0);
        let skin_cut = (skin.tear_position + RAGGED * skin_ragged).clamp(0.0, 1.0);
        let skin_part = |share: &dyn Fn(f32) -> f32, side: Vec3| {
            let rest: Vec<Vec3> = (0..res).map(|i| skin_at(share(i as f32 / (res - 1) as f32))).collect();
            let fallen = fall(&rest, side, skin.flare, skin_relax[k]);
            // a part that falls towards the axis has more skin than the circle it hangs on
            rest.iter().zip(fallen).map(|(r, p)| (p, (from_axis(*r) / from_axis(p).max(1e-6) - 1.0).max(0.0))).collect()
        };
        if skin_cut > MIN_PART {
            columns.on_b[k] = Some(skin_part(&|u| skin_cut * kept * u, horizontal(chord)));
        }
        if skin_cut < 1.0 - MIN_PART {
            columns.on_a[k] = Some(skin_part(&|u| 1.0 - (1.0 - skin_cut) * kept * u, horizontal(-chord)));
        }
    }
    (on_b, on_a, columns)
}

/// Adds the skin of one membrane to `mesh` as thin closed sheets: one for every run of
/// neighbouring strands that have the same piece (whole, on b or on a). Around a ring a run of
/// every strand closes on itself. A run that ends is half a strand spacing wider on either side,
/// as wide as its strands cover, so a single torn strand still gives a narrow flap.
///
/// A torn part that hangs closer to the axis than it lay in the skin has more skin than the
/// circle it hangs on, and a thin sheet bends rather than squeezes: it folds (Sharon et al.
/// 2002), out and in across the sheet, see [`fold`].
fn add_skin(mesh: &mut Mesh, anchors: &[Anchor], axis: Option<Axis>, columns: &SkinColumns, thickness: f32) {
    let n = anchors.len();
    let closed = axis.is_some();
    let radius = 0.5 * thickness;
    let torn_edge = |u: f32| radius * (1.0 - EDGE_THIN * smoothstep(1.0 - EDGE, 1.0, u));
    // the spacing of the strands where they are attached, across the skin
    let spacing = |k: usize, on: fn(&Anchor) -> Vec3| {
        let (prev, next, steps) = if closed {
            ((k + n - 1) % n, (k + 1) % n, 2.0)
        } else {
            let (prev, next) = (k.saturating_sub(1), (k + 1).min(n - 1));
            (prev, next, ((next - prev) as f32).max(1.0))
        };
        (on(&anchors[next]) - on(&anchors[prev])) / steps
    };
    let from_axis = |p: Vec3| {
        axis.map_or(0.0, |(centre, direction)| {
            let q = p - centre;
            (q - direction * q.dot(direction)).length()
        })
    };
    /// The columns of a piece, where its strands are attached and whether it is torn
    type Piece<'c> = (&'c [Column], fn(&Anchor) -> Vec3, bool);
    let pieces: [Piece; 3] = [
        (&columns.intact, |a: &Anchor| a.b, false),
        (&columns.on_b, |a: &Anchor| a.b, true),
        (&columns.on_a, |a: &Anchor| a.a, true),
    ];
    for (piece, attached, torn) in pieces {
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
                            let across = if m > 1 { grid[c][i] - grid[towards][i] } else { spacing(k, attached) * sign };
                            grid[c][i] + 0.5 * across
                        })
                        .collect()
                };
                // not past the ends of a membrane that doesn't go around
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
            if torn && closed {
                fold(&mut grid, &excess, &run, wrap, columns, &from_axis);
            }
            let half: Vec<f32> =
                (0..rows).map(|i| if torn { torn_edge(i as f32 / (rows - 1) as f32) } else { radius }).collect();
            let normals = sheet_normals(&grid, wrap);
            let sheet = SheetGrid { columns: grid.len(), rows, wrap };
            add_sheet(mesh, sheet, |c, i| grid[c][i] + normals[c][i] * half[i], |c, i| grid[c][i] - normals[c][i] * half[i], None);
        }
    }
}

/// Folds a torn piece of a skin that goes around: about one fold for every `FOLD_LENGTH` part
/// lengths around it, a little uneven, deep enough to take up `FOLD_DEPTH` of the extra length of
/// every circle. `run` is the strand of every column.
fn fold(grid: &mut [Vec<Vec3>], excess: &[Vec<f32>], run: &[usize], wrap: bool, columns: &SkinColumns, from_axis: &dyn Fn(Vec3) -> f32) {
    let (m, rows) = (grid.len(), grid[0].len());
    let length = |c: usize| grid[c].windows(2).map(|w| w[0].distance(w[1])).sum::<f32>();
    let part = (0..m).map(length).sum::<f32>() / m as f32;
    let around = (0..m).map(|c| from_axis(grid[c][rows - 1])).sum::<f32>() / m as f32 * TAU;
    if part <= 0.0 || around <= 0.0 {
        return;
    }
    let folds = (around / (FOLD_LENGTH * part)).round().max(2.0);
    let normals = sheet_normals(grid, wrap);
    for c in 0..m {
        let k = run[c];
        let wave = (TAU * folds * columns.place[k] + FOLD_WOBBLE * columns.wobble[k]).sin();
        for i in 0..rows {
            // a wave of `folds` waves around a circle is longer by (π depth / wavelength)²
            let wavelength = TAU * from_axis(grid[c][i]) / folds;
            let depth = FOLD_DEPTH * wavelength / PI * excess[c][i].sqrt();
            grid[c][i] += normals[c][i] * (depth * wave);
        }
    }
}

/// The runs of neighbouring strands that have a piece, and whether a run goes all the way round
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

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    build(args, STRANDS).1
}

/// The strands across the skin and the skin. Only the skin is an output, the strands are what the
/// tests look at.
fn build(args: &[&[i32]], strand_count: i32) -> (Vec<i32>, Vec<i32>) {
    let mut ctx = Ctx::default();
    // the skin as thin sheets, every one closed
    let mut sheets = Mesh::default();
    let a = read_curves(args[0]);
    let b = read_curves(args[1]);
    let flare = read_floats(args[10]);
    let seed = int(args[11]);
    let resolution = int(args[12]).max(4) as usize;
    // the graph node that makes the membrane, it is the last input
    let creator = int(args[13]);

    // every attachment in a joins the attachment in b closest to it
    let pairs: Vec<(usize, usize)> = if b.is_empty() {
        Vec::new()
    } else {
        a.iter()
            .enumerate()
            .map(|(i, curve)| {
                let centre = curve.centre();
                let nearest = (0..b.len()).min_by(|&x, &y| b[x].closest(centre).0.total_cmp(&b[y].closest(centre).0)).unwrap();
                (i, nearest)
            })
            .collect()
    };

    let mut output = Vec::new();
    let mut strands_on = vec![0usize; b.len()];
    for (rank, &(i, j)) in pairs.iter().enumerate() {
        let (curve_a, curve_b) = (&a[i], &b[j]);
        match curve_b.path {
            Some(path) => ctx.set_path(path, rank, pairs.len()),
            None => {
                ctx.set_path_info(PathInfo {
                    index: rank,
                    rank,
                    count: pairs.len(),
                    depth: curve_b.sources[0].depth,
                    parent_position: 0.0,
                    length: curve_b.length(),
                });
                ctx.seed_path(path_id(creator, curve_b.sources[0].id, rank as i32));
            }
        }
        let attachment = ctx.float(args[2]).clamp(-1.0, 1.0);
        let anchor = ctx.float(args[3]).max(0.0);
        let along = ctx.float(args[4]).clamp(0.0, 1.0);
        let skin = Skin {
            thickness: ctx.float(args[5]).max(0.0),
            sag: ctx.float(args[6]).max(0.0),
            tear: ctx.float(args[7]).clamp(0.0, 1.0),
            tear_position: ctx.float(args[8]).clamp(0.0, 1.0),
            torn_length: ctx.float(args[9]).max(0.0),
            flare: &flare,
            resolution,
        };
        let n = strand_count.clamp(1, MAX_STRANDS) as usize;

        let (anchors, axis) = if curve_a.closed && curve_b.path.is_some() {
            let alpha = wrap_alpha(curve_a, curve_b, attachment, anchor, along);
            let (anchors, axis) = around_path(curve_a, curve_b, alpha, n, 0.5 * skin.thickness);
            (anchors, Some(axis))
        } else if curve_a.closed && (curve_b.closed || curve_b.points.len() == 1) {
            let (anchors, axis) = ring_to_ring(curve_a, curve_b, n);
            (anchors, Some(axis))
        } else {
            (end_to_end(curve_a, curve_b, n), None)
        };
        // a second membrane on the same attachment numbers its strands after the first
        let (on_b, on_a, columns) = strands(&anchors, axis, &skin, seed, creator, strands_on[j]);
        add_skin(&mut sheets, &anchors, axis, &columns, skin.thickness);
        strands_on[j] += n;
        output.extend(on_b);
        output.extend(on_a);
    }
    smooth_normals(&mut sheets);

    (concat_arg_vecs(output), sheets.encode())
}

#[cfg(test)]
mod tests {
    use super::build;
    use glam::Vec3;
    use myzel_core::{sheet_checks, Mesh};
    use nodarium_utils::{
        concat_arg_vecs, encode_float, encode_points,
        geometry::{create_multiple_paths, path_id, path_link, wrap_multiple_paths, wrap_path},
        split_args, Point,
    };
    use std::f32::consts::TAU;

    const CREATOR: i32 = 11;
    const STIPE_TOP: f32 = 0.05;
    const STIPE_RADIUS: f32 = 0.005;
    const MARGIN_RADIUS: f32 = 0.02;
    const MARGIN_HEIGHT: f32 = 0.04;

    /// Upright stipes 5 cm tall and 5 mm in radius, at these x
    fn stipes(xs: &[f32]) -> Vec<i32> {
        tall_stipes(xs, STIPE_TOP)
    }

    fn tall_stipes(xs: &[f32], top: f32) -> Vec<i32> {
        let mut data = create_multiple_paths(xs.len(), 12, 1, 3);
        for (k, path) in wrap_multiple_paths(&mut data).iter_mut().enumerate() {
            for i in 0..12 {
                path.points[i * 4..i * 4 + 4].copy_from_slice(&[xs[k], top * i as f32 / 11.0, 0.0, STIPE_RADIUS]);
            }
        }
        data
    }

    fn stipe_ids(data: &[i32]) -> Vec<i32> {
        split_args(data).iter().map(|p| wrap_path(p).link.id).collect()
    }

    /// A margin like the cap gives it: points around the axis at `x`, each on its own ray
    fn ring(x: f32, radius: f32, height: f32, count: usize, stipe: i32) -> Vec<Point> {
        (0..count)
            .map(|i| {
                let angle = TAU * i as f32 / count as f32;
                let out = Vec3::new(angle.cos(), 0.0, angle.sin());
                Point {
                    position: Vec3::new(x, height, 0.0) + out * radius,
                    direction: Vec3::NEG_Y,
                    tangent: out,
                    scale: 0.001,
                    parent_id: path_id(9, stipe, i as i32),
                    alpha: 1.0,
                    depth: 2,
                    index: 0,
                }
            })
            .collect()
    }

    fn floats(values: &[f32]) -> Vec<i32> {
        let mut words = vec![0, values.len() as i32 + 1];
        words.extend(values.iter().map(|&v| encode_float(v)));
        words.extend([1, 1]);
        words
    }

    #[derive(Clone)]
    struct Params {
        attachment: f32,
        anchor: f32,
        along: f32,
        thickness: f32,
        sag: f32,
        tear: f32,
        tear_position: f32,
        torn_length: f32,
        flare: Vec<f32>,
        strands: i32,
        seed: i32,
    }

    fn veil() -> Params {
        Params {
            attachment: 0.0,
            anchor: 0.0,
            along: 0.0,
            thickness: 0.001,
            sag: 0.1,
            tear: 0.0,
            tear_position: 0.5,
            torn_length: 0.0,
            flare: vec![0.0, 0.15, 0.6, 0.3, 1.0, 0.5],
            strands: 160,
            seed: 3,
        }
    }

    fn execute_with(a: &[i32], b: &[i32], p: &Params, flare: &[i32]) -> (Vec<i32>, Vec<i32>) {
        build(&[
            a,
            b,
            &[encode_float(p.attachment)],
            &[encode_float(p.anchor)],
            &[encode_float(p.along)],
            &[encode_float(p.thickness)],
            &[encode_float(p.sag)],
            &[encode_float(p.tear)],
            &[encode_float(p.tear_position)],
            &[encode_float(p.torn_length)],
            flare,
            &[p.seed],
            &[16],
            &[CREATOR],
        ], p.strands)
    }

    fn run(a: &[i32], b: &[i32], p: &Params) -> Vec<Vec<i32>> {
        split_args(&execute_with(a, b, p, &floats(&p.flare)).0).iter().map(|s| s.to_vec()).collect()
    }

    fn skin(a: &[i32], b: &[i32], p: &Params) -> Mesh {
        Mesh::decode(&execute_with(a, b, p, &floats(&p.flare)).1)
    }

    /// The skin of one cap's veil
    fn veil_skin(p: &Params) -> Mesh {
        let stipe = stipes(&[0.0]);
        let margin = encode_points(&ring(0.0, MARGIN_RADIUS, MARGIN_HEIGHT, 96, stipe_ids(&stipe)[0]));
        skin(&margin, &stipe, p)
    }

    /// The veil of one cap: its margin in a, its stipe in b
    fn run_veil(p: &Params) -> (Vec<Vec<i32>>, i32) {
        let stipe = stipes(&[0.0]);
        let id = stipe_ids(&stipe)[0];
        let margin = encode_points(&ring(0.0, MARGIN_RADIUS, MARGIN_HEIGHT, 96, id));
        (run(&margin, &stipe, p), id)
    }

    fn points(path: &[i32]) -> Vec<Vec3> {
        wrap_path(path).points.chunks_exact(4).map(|p| Vec3::new(p[0], p[1], p[2])).collect()
    }

    fn from_axis(p: Vec3) -> f32 {
        (p.x * p.x + p.z * p.z).sqrt()
    }

    #[test]
    fn intact_at_tear_0_without_a_gap() {
        let p = veil();
        let (strands, stipe) = run_veil(&p);
        assert_eq!(strands.len(), p.strands as usize);
        for s in &strands {
            assert_eq!(path_link(s).parent_id, stipe);
            let q = points(s);
            // from the stipe, grown out of its surface, to the margin
            assert!((from_axis(q[0]) - (STIPE_RADIUS - 0.5 * p.thickness)).abs() < 1e-5);
            let end = q[q.len() - 1];
            assert!((end.y - MARGIN_HEIGHT).abs() < 1e-5);
            assert!(from_axis(end) > MARGIN_RADIUS * 0.999 && from_axis(end) < MARGIN_RADIUS + 1e-6);
        }
        // neighbours touch all the way round, so the skin is closed
        for k in 0..strands.len() {
            let (s, t) = (points(&strands[k]), points(&strands[(k + 1) % strands.len()]));
            assert_eq!(s.len(), t.len());
            let gap = s.iter().zip(&t).map(|(a, b)| a.distance(*b)).fold(0.0, f32::max);
            assert!(gap <= p.thickness, "strands {} and {} are {} apart", k, k + 1, gap);
        }
        let mut ids: Vec<i32> = strands.iter().map(|s| path_link(s).id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), strands.len());
    }

    #[test]
    fn the_skin_sags_between_the_attachments() {
        let flat = run_veil(&Params { sag: 0.0, ..veil() }).0;
        let sagging = run_veil(&Params { sag: 0.2, ..veil() }).0;
        let (a, b) = (points(&flat[0]), points(&sagging[0]));
        let middle = a.len() / 2;
        let span = a[0].distance(a[a.len() - 1]);
        let drop = a[middle].distance(b[middle]);
        assert!((drop - 0.2 * span).abs() < 0.02 * span, "sags {} for a span of {}", drop, span);
        assert!(b[middle].y < a[middle].y);
    }

    #[test]
    fn two_separate_parts_at_tear_1() {
        let p = Params { tear: 1.0, ..veil() };
        let (strands, stipe) = run_veil(&p);
        let (ring, fringe): (Vec<_>, Vec<_>) = strands.iter().partition(|s| path_link(s).parent_id == stipe);
        assert_eq!(ring.len(), p.strands as usize);
        assert_eq!(fringe.len(), p.strands as usize);
        let ring_points: Vec<Vec3> = ring.iter().flat_map(|s| points(s)).collect();
        let fringe_points: Vec<Vec3> = fringe.iter().flat_map(|s| points(s)).collect();
        let closest = ring_points
            .iter()
            .flat_map(|a| fringe_points.iter().map(move |b| a.distance(*b)))
            .fold(f32::INFINITY, f32::min);
        assert!(closest > 2.0 * p.thickness, "ring and fringe come within {}", closest);
        // both parts hang down from where they are attached: the ring flares out from the
        // stipe, the fringe hangs from the margin
        for s in &ring {
            let q = points(s);
            let (start, end) = (q[0], q[q.len() - 1]);
            assert!(end.y < start.y - 0.002 && from_axis(end) > from_axis(start));
        }
        for s in &fringe {
            let q = points(s);
            let (start, end) = (q[0], q[q.len() - 1]);
            assert!((from_axis(start) - MARGIN_RADIUS).abs() < 2e-4);
            assert!(end.y < start.y - 0.002);
        }
    }

    #[test]
    fn the_ring_stays_on_the_stipe_at_attachment() {
        for attachment in [0.0, 0.5, 1.0, -0.5] {
            let height = if attachment >= 0.0 {
                MARGIN_HEIGHT + attachment * (STIPE_TOP - MARGIN_HEIGHT)
            } else {
                MARGIN_HEIGHT * (1.0 + attachment)
            };
            for tear in [0.0, 0.3, 1.0] {
                let p = Params { attachment, tear, ..veil() };
                let (strands, stipe) = run_veil(&p);
                let ring: Vec<_> = strands.iter().filter(|s| path_link(s).parent_id == stipe).collect();
                assert_eq!(ring.len(), p.strands as usize);
                for s in ring {
                    let start = points(s)[0];
                    assert!((start.y - height).abs() < 1e-5, "ring at {} for {}, expected {}", start.y, attachment, height);
                    assert!((from_axis(start) - (STIPE_RADIUS - 0.5 * p.thickness)).abs() < 1e-5);
                    assert!((path_link(s).parent_alpha - height / STIPE_TOP).abs() < 1e-5);
                }
            }
        }
    }

    #[test]
    fn the_anchor_holds_the_ring_while_the_stipe_grows() {
        // the stipe grows from 4 to 6 cm and lifts the margin with it, 1 cm under its top
        let anchor = 0.025;
        let mut followed = Vec::new();
        for top in [0.04, 0.05, 0.06] {
            let stipe = tall_stipes(&[0.0], top);
            let id = stipe_ids(&stipe)[0];
            let margin = encode_points(&ring(0.0, MARGIN_RADIUS, top - 0.01, 96, id));
            for tear in [0.0, 0.3, 1.0] {
                let p = Params { anchor, tear, attachment: 0.5, ..veil() };
                let strands = run(&margin, &stipe, &p);
                let ring: Vec<_> = strands.iter().filter(|s| path_link(s).parent_id == id).collect();
                assert_eq!(ring.len(), p.strands as usize);
                for s in ring {
                    let start = points(s)[0];
                    assert!((start.y - anchor).abs() < 1e-5, "ring at {} on a stipe {} tall", start.y, top);
                    assert!((from_axis(start) - (STIPE_RADIUS - 0.5 * p.thickness)).abs() < 1e-5);
                    assert!((path_link(s).parent_alpha - anchor / top).abs() < 1e-5);
                }
            }
            // without the anchor the ring rides up with the margin
            let p = Params { attachment: 0.5, ..veil() };
            followed.push(points(&run(&margin, &stipe, &p)[0])[0].y);
        }
        assert!(followed[2] - followed[0] > 0.015, "the ring rides from {} to {}", followed[0], followed[2]);
        // an anchor past the tip holds it at the tip
        let p = Params { anchor: 0.08, ..veil() };
        let (strands, _) = run_veil(&p);
        assert!((points(&strands[0])[0].y - STIPE_TOP).abs() < 1e-5);
    }

    #[test]
    fn torn_parts_keep_the_length_they_tore_at() {
        fn length(path: &[i32]) -> f32 {
            points(path).windows(2).map(|w| w[0].distance(w[1])).sum()
        }
        // the parts of every strand by its index, on the stipe and on the margin, as the margin
        // moves out from 2 to 3 cm
        let parts = |radius: f32, torn_length: f32| {
            let stipe = stipes(&[0.0]);
            let id = stipe_ids(&stipe)[0];
            let margin = encode_points(&ring(0.0, radius, MARGIN_HEIGHT, 96, id));
            // a flat skin, so its length along it is the span
            let p = Params { tear: 1.0, torn_length, sag: 0.0, ..veil() };
            let mut ring = vec![0.0; p.strands as usize];
            let mut fringe = vec![0.0; p.strands as usize];
            for s in run(&margin, &stipe, &p) {
                let k = (path_link(&s).index / 2) as usize;
                if path_link(&s).parent_id == id {
                    ring[k] = length(&s);
                } else {
                    fringe[k] = length(&s);
                }
            }
            (ring, fringe)
        };
        let torn_length = 0.008;
        let (ring, fringe) = parts(0.02, torn_length);
        let (wider_ring, wider_fringe) = parts(0.03, torn_length);
        for k in 0..ring.len() {
            // both parts together are the torn length, less the recoil
            let together = ring[k] + fringe[k];
            assert!(together <= torn_length * (1.0 - super::RECOIL) * 1.01 && together > 0.4 * torn_length, "strand {} is {} long", k, together);
            assert!((wider_ring[k] - ring[k]).abs() < 1e-3 * torn_length, "ring part {} grows from {} to {}", k, ring[k], wider_ring[k]);
            assert!((wider_fringe[k] - fringe[k]).abs() < 1e-3 * torn_length);
        }
        // without it they stretch with the span
        let (free, _) = parts(0.02, 0.0);
        let (wider_free, _) = parts(0.03, 0.0);
        let mean = |v: &[f32]| v.iter().sum::<f32>() / v.len() as f32;
        assert!(mean(&wider_free) > 1.4 * mean(&free));
        // and a span shorter than the torn length keeps its own length
        let (short, _) = parts(0.02, 0.1);
        assert_eq!(short, free);
    }

    #[test]
    fn the_tear_runs_around_from_a_few_places() {
        for seed in 0..8 {
            // half way to torn all round, half of the strands have torn
            let p = Params { tear: 0.25, seed, ..veil() };
            let (strands, stipe) = run_veil(&p);
            let n = p.strands as usize;
            // the parts on the margin are the strands that have torn
            let torn: Vec<i32> =
                strands.iter().filter(|s| path_link(s).parent_id != stipe).map(|s| path_link(s).index / 2).collect();
            assert_eq!(torn.len(), n / 2);
            // in a few runs around the ring, not here and there
            let is_torn = |k: usize| torn.contains(&(k as i32));
            let starts = (0..n).filter(|&k| is_torn(k) && !is_torn((k + n - 1) % n)).count();
            assert!((1..=5).contains(&starts), "the tear runs from {} places", starts);
            // the more it tears, the more strands have torn
            let more = run_veil(&Params { tear: 0.4, ..p }).0;
            assert!(more.len() > strands.len());
        }
    }

    #[test]
    fn the_same_inputs_give_the_same_membrane() {
        let p = Params { tear: 0.3, ..veil() };
        assert_eq!(run_veil(&p).0, run_veil(&p).0);
        assert_ne!(run_veil(&p).0, run_veil(&Params { seed: 4, ..p }).0);
    }

    #[test]
    fn reads_a_connected_curve_node() {
        let p = Params { tear: 1.0, ..veil() };
        let stipe = stipes(&[0.0]);
        let id = stipe_ids(&stipe)[0];
        let margin = encode_points(&ring(0.0, MARGIN_RADIUS, MARGIN_HEIGHT, 96, id));
        let flare = floats(&p.flare);
        let wrapped = nodarium_utils::concat_args(vec![&flare]);
        assert_eq!(super::read_floats(&wrapped), p.flare);
        assert_eq!(execute_with(&margin, &stipe, &p, &wrapped), execute_with(&margin, &stipe, &p, &flare));
        // the flare reaches the strands: straight out keeps the ring level
        let out = run(&margin, &stipe, &Params { flare: vec![0.0, 1.0, 1.0, 1.0], ..p.clone() });
        for s in out.iter().filter(|s| path_link(s).parent_id == id) {
            let q = points(s);
            assert!((q[q.len() - 1].y - q[0].y).abs() < 1e-4);
        }
    }

    #[test]
    fn every_margin_joins_its_own_stipe() {
        // two caps whose margins overlap, the second one higher
        let stipe = stipes(&[0.0, 0.03]);
        let ids = stipe_ids(&stipe);
        let margins = encode_points(
            &[ring(0.0, MARGIN_RADIUS, MARGIN_HEIGHT, 96, ids[0]), ring(0.03, MARGIN_RADIUS, 0.045, 96, ids[1])].concat(),
        );
        let p = Params { strands: 32, ..veil() };
        let strands = run(&margins, &stipe, &p);
        assert_eq!(strands.len(), 64);
        for (k, id) in ids.iter().enumerate() {
            let own: Vec<_> = strands.iter().filter(|s| path_link(s).parent_id == *id).collect();
            assert_eq!(own.len(), 32);
            for s in own {
                let start = points(s)[0];
                let off_axis = Vec3::new(start.x - 0.03 * k as f32, 0.0, start.z).length();
                assert!((off_axis - (STIPE_RADIUS - 0.5 * p.thickness)).abs() < 1e-5);
            }
        }
    }

    #[test]
    fn a_loft_between_two_paths() {
        // two parallel lines, the second one drawn the other way round
        let mut lines = create_multiple_paths(2, 8, 1, 5);
        for (k, path) in wrap_multiple_paths(&mut lines).iter_mut().enumerate() {
            for i in 0..8 {
                let x = 0.1 * i as f32 / 7.0;
                let (x, y) = if k == 0 { (x, 0.1) } else { (0.1 - x, 0.0) };
                path.points[i * 4..i * 4 + 4].copy_from_slice(&[x, y, 0.0, 0.002]);
            }
        }
        let paths = split_args(&lines);
        let (a, b) = (concat_arg_vecs(vec![paths[0].to_vec()]), concat_arg_vecs(vec![paths[1].to_vec()]));
        let p = Params { strands: 21, sag: 0.0, ..veil() };
        let strands = run(&a, &b, &p);
        assert_eq!(strands.len(), 21);
        for (k, s) in strands.iter().enumerate() {
            let q = points(s);
            let x = 0.1 * k as f32 / 20.0;
            // straight across, from the line in b up to the line in a
            assert!(q[0].distance(Vec3::new(x, 0.0, 0.0)) < 1e-5, "strand {} starts at {:?}", k, q[0]);
            assert!(q[q.len() - 1].distance(Vec3::new(x, 0.1, 0.0)) < 1e-5);
            assert_eq!(path_link(s).parent_id, wrap_path(paths[1]).link.id);
        }
    }

    #[test]
    fn a_ring_of_points_to_a_smaller_one() {
        let outer = ring(0.0, MARGIN_RADIUS, MARGIN_HEIGHT, 96, 1);
        let inner = ring(0.0, 0.008, 0.03, 48, 2);
        let p = Params { strands: 64, ..veil() };
        let strands = run(&encode_points(&outer), &encode_points(&inner), &p);
        assert_eq!(strands.len(), 64);
        for (k, s) in strands.iter().enumerate() {
            let q = points(s);
            let (start, end) = (q[0], q[q.len() - 1]);
            assert!((from_axis(start) - 0.008).abs() < 1e-4 && (start.y - 0.03).abs() < 1e-5);
            assert!((from_axis(end) - MARGIN_RADIUS).abs() < 1e-4 && (end.y - MARGIN_HEIGHT).abs() < 1e-5);
            // across, not twisted round the axis
            let turn = (start.z.atan2(start.x) - end.z.atan2(end.x)).abs();
            assert!(turn.min(TAU - turn) < 0.1, "strand {} turns {}", k, turn);
        }
    }

    #[test]
    fn nothing_to_attach_to_gives_nothing() {
        let stipe = stipes(&[0.0]);
        let margin = encode_points(&ring(0.0, MARGIN_RADIUS, MARGIN_HEIGHT, 96, stipe_ids(&stipe)[0]));
        let empty = [0, 1, 1, 1];
        assert!(run(&margin, &empty, &veil()).is_empty());
        assert!(run(&empty, &stipe, &veil()).is_empty());
    }

    /// The sheets of a skin mesh: every one is a contiguous block of vertices, its front and then
    /// its back, `rows` per column. The front and back vertex at the same spot, and its row.
    fn sheet_pairs(mesh: &Mesh, rows: usize) -> Vec<(Vec3, Vec3, usize)> {
        // a sheet's vertices end where no face reaches further
        let mut reach = vec![0usize; mesh.vertex_count()];
        for f in mesh.faces.chunks_exact(3) {
            let top = f.iter().map(|&v| v as usize).max().unwrap();
            for &v in f {
                reach[v as usize] = reach[v as usize].max(top);
            }
        }
        let vertex = |v: usize| Vec3::from_slice(&mesh.positions[v * 3..v * 3 + 3]);
        let mut pairs = Vec::new();
        let mut start = 0;
        while start < mesh.vertex_count() {
            let mut end = start;
            let mut v = start;
            while v <= end {
                end = end.max(reach[v]);
                v += 1;
            }
            let count = end + 1 - start;
            assert_eq!(count % (2 * rows), 0, "a sheet of {count} vertices with {rows} rows");
            let half = count / 2;
            for v in start..start + half {
                pairs.push((vertex(v), vertex(v + half), (v - start) % rows));
            }
            start = end + 1;
        }
        pairs
    }

    #[test]
    fn the_skin_is_closed_and_as_thick_as_thickness() {
        // resolution_curve is 16 in these tests
        let rows = 16;
        for tear in [0.0, 0.3, 1.0] {
            for thickness in [0.0005, 0.002] {
                let p = Params { tear, thickness, ..veil() };
                let mesh = veil_skin(&p);
                assert!(mesh.face_count() > 0);
                assert_eq!(sheet_checks::open_edge(&mesh), None, "tear {tear}");
                // the faces look out of the sheets: they enclose about the skin's area times
                // its thickness, and no sheet is turned inside out
                assert!(sheet_checks::volume(&mesh) > 0.0);
                let pairs = sheet_pairs(&mesh, rows);
                for &(front, back, row) in &pairs {
                    let apart = front.distance(back);
                    if row == 0 {
                        // as thick as thickness where it is attached
                        assert!((apart - thickness).abs() < 1e-3 * thickness, "{apart} apart at tear {tear}");
                    }
                    assert!(apart <= thickness * 1.001 && apart >= (1.0 - super::EDGE_THIN) * thickness * 0.999);
                }
                // a torn edge thins, an intact skin keeps its thickness to the margin
                let edges: Vec<f32> = pairs.iter().filter(|q| q.2 == rows - 1).map(|q| q.0.distance(q.1)).collect();
                let thinnest = edges.iter().copied().fold(f32::INFINITY, f32::min);
                if tear == 0.0 {
                    assert!((thinnest - thickness).abs() < 1e-3 * thickness);
                } else {
                    assert!((thinnest - (1.0 - super::EDGE_THIN) * thickness).abs() < 1e-3 * thickness);
                }
                let n = |v: usize| Vec3::from_slice(&mesh.normals[v * 3..v * 3 + 3]);
                assert!((0..mesh.vertex_count()).all(|v| (n(v).length() - 1.0).abs() < 1e-4));
            }
        }
    }

    #[test]
    fn the_skin_lies_where_the_strands_are() {
        for tear in [0.0, 0.45, 1.0] {
            let p = Params { tear, ..veil() };
            let strands = run_veil(&p).0;
            let mesh = veil_skin(&p);
            let vertices: Vec<Vec3> = mesh.positions.chunks_exact(3).map(Vec3::from_slice).collect();
            // the attached ends of every strand have the skin next to them, within a strand spacing
            let spacing = TAU * MARGIN_RADIUS / p.strands as f32;
            for s in &strands {
                let start = points(s)[0];
                let closest = vertices.iter().map(|v| v.distance(start)).fold(f32::INFINITY, f32::min);
                assert!(closest < spacing + p.thickness, "a strand starts {closest} from the skin at tear {tear}");
            }
            // and the skin reaches no further than the strands do
            let (lo, hi) = strands.iter().flat_map(|s| points(s)).fold((Vec3::INFINITY, Vec3::NEG_INFINITY), |(lo, hi), q| (lo.min(q), hi.max(q)));
            for v in &vertices {
                assert!(v.cmpge(lo - Vec3::splat(spacing + p.thickness)).all() && v.cmple(hi + Vec3::splat(spacing + p.thickness)).all());
            }
        }
    }

    #[test]
    fn a_ring_skin_goes_around_without_seams_and_fringe_flecks_are_flaps() {
        // a tear near the margin: every strand leaves a part on the stipe, the margin keeps
        // flecks with gaps between them
        let p = Params { tear: 1.0, tear_position: 0.95, ..veil() };
        let (strands, stipe) = run_veil(&p);
        let ring = strands.iter().filter(|s| path_link(s).parent_id == stipe).count();
        let fringe = strands.len() - ring;
        assert_eq!(ring, p.strands as usize);
        assert!(fringe > 0 && fringe < ring, "{fringe} fringe parts");
        let mesh = veil_skin(&p);
        assert_eq!(sheet_checks::open_edge(&mesh), None);
        let rows = 16;
        let pairs = sheet_pairs(&mesh, rows);
        // the ring is one band of every strand, the flecks are flaps half a spacing wider on
        // either side than their strands
        let ring_vertices = p.strands as usize * rows;
        assert!(pairs.len() > ring_vertices);
        let fleck_columns = (pairs.len() - ring_vertices) / rows;
        assert!(fleck_columns > fringe, "{fleck_columns} columns for {fringe} fringe parts");
    }

    #[test]
    fn along_holds_the_ring_at_its_share_while_the_stipe_grows() {
        // a ring in the upper third, on stipes from 4 to 6 cm with the margin 5 mm under the top
        let along = 0.7;
        for top in [0.04, 0.05, 0.06] {
            let stipe = tall_stipes(&[0.0], top);
            let id = stipe_ids(&stipe)[0];
            let margin = encode_points(&ring(0.0, MARGIN_RADIUS, top - 0.005, 96, id));
            for tear in [0.0, 0.3, 1.0] {
                // along wins over the anchor
                let p = Params { along, anchor: 0.01, tear, ..veil() };
                let strands = run(&margin, &stipe, &p);
                let ring: Vec<_> = strands.iter().filter(|s| path_link(s).parent_id == id).collect();
                assert_eq!(ring.len(), p.strands as usize);
                for s in ring {
                    let start = points(s)[0];
                    // on the stipe, at its share of the height, in the upper third
                    assert!((start.y - along * top).abs() < 1e-5, "ring at {} on a stipe {} tall", start.y, top);
                    assert!(start.y > top * 2.0 / 3.0 && start.y < top - 0.005);
                    assert!((from_axis(start) - (STIPE_RADIUS - 0.5 * p.thickness)).abs() < 1e-5);
                    assert!((path_link(s).parent_alpha - along).abs() < 1e-5);
                }
                // the skin starts there too
                let mesh = skin(&margin, &stipe, &p);
                let lowest_start = sheet_pairs(&mesh, 16).iter().filter(|q| q.2 == 0).map(|q| q.0.y).fold(f32::INFINITY, f32::min);
                assert!((lowest_start - along * top).abs() < p.thickness, "skin starts at {lowest_start}");
            }
        }
    }

    #[test]
    fn the_torn_edge_is_ragged_and_the_seed_sets_it() {
        let edge = |seed: i32| {
            let p = Params { tear: 1.0, seed, torn_length: 0.01, sag: 0.0, ..veil() };
            let (strands, stipe) = run_veil(&p);
            let mut lengths = vec![0.0; p.strands as usize];
            for s in strands.iter().filter(|s| path_link(s).parent_id == stipe) {
                lengths[(path_link(s).index / 2) as usize] = points(s).windows(2).map(|w| w[0].distance(w[1])).sum::<f32>();
            }
            lengths
        };
        let (a, b) = (edge(3), edge(4));
        let mean = |v: &[f32]| v.iter().sum::<f32>() / v.len() as f32;
        // the ring hangs about as long for every seed, its edge goes up and down around it,
        // also from one strand to the next
        assert!((mean(&a) - mean(&b)).abs() < 0.2 * mean(&a));
        let (lo, hi) = a.iter().fold((f32::INFINITY, 0.0f32), |(lo, hi), &l| (lo.min(l), hi.max(l)));
        assert!(hi - lo > 0.2 * mean(&a), "the edge only goes from {lo} to {hi}");
        let jagged = a.windows(2).map(|w| (w[1] - w[0]).abs()).sum::<f32>() / a.len() as f32;
        assert!(jagged > 0.01 * mean(&a));
        assert_ne!(a, b);
    }

    #[test]
    fn the_skin_is_empty_without_attachments_and_the_same_for_the_same_inputs() {
        let stipe = stipes(&[0.0]);
        let margin = encode_points(&ring(0.0, MARGIN_RADIUS, MARGIN_HEIGHT, 96, stipe_ids(&stipe)[0]));
        let empty = [0, 1, 1, 1];
        assert_eq!(skin(&margin, &empty, &veil()).vertex_count(), 0);
        let p = Params { tear: 0.45, ..veil() };
        assert_eq!(veil_skin(&p), veil_skin(&p));
        // a loft between two paths gives one sheet that ends at their ends
        let mut lines = create_multiple_paths(2, 8, 1, 5);
        for (k, path) in wrap_multiple_paths(&mut lines).iter_mut().enumerate() {
            for i in 0..8 {
                path.points[i * 4..i * 4 + 4].copy_from_slice(&[0.1 * i as f32 / 7.0, 0.1 * k as f32, 0.0, 0.002]);
            }
        }
        let paths = split_args(&lines);
        let (a, b) = (concat_arg_vecs(vec![paths[0].to_vec()]), concat_arg_vecs(vec![paths[1].to_vec()]));
        let mesh = skin(&a, &b, &Params { strands: 21, sag: 0.0, ..veil() });
        assert_eq!(sheet_checks::open_edge(&mesh), None);
        let xs = mesh.positions.chunks_exact(3).map(|v| v[0]);
        let (lo, hi) = xs.fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), x| (lo.min(x), hi.max(x)));
        assert!(lo > -1e-4 && hi < 0.1 + 1e-4, "the sheet runs from {lo} to {hi}");
    }
}
