use glam::Vec3;
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{
    concat_arg_vecs, curve, decode_float,
    geometry::{create_path, is_path, orthogonal_to, path_id, path_random, set_path_link, wrap_path, wrap_path_mut, PathLink},
    int, split_args, Ctx, PathInfo, Point,
};
use noise::{NoiseFn, OpenSimplex};
use std::f32::consts::{FRAC_PI_2, TAU};

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
const MAX_STRANDS: i32 = 4096;
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

/// A ring of points around a path: the membrane wraps around the path at `attachment`, between
/// the level of the ring (0) and the tip of the path (1), or its base (-1). The strands start
/// inside the surface of the path by `inset`, so the skin grows out of it.
fn around_path(a: &Curve, b: &Curve, attachment: f32, n: usize, inset: f32) -> Vec<Anchor> {
    let level = b.closest(a.centre()).1;
    let alpha = if attachment >= 0.0 { level + attachment * (1.0 - level) } else { level * (1.0 + attachment) };
    let ring = b.at(alpha);
    let e1 = orthogonal_to(ring.tangent);
    let e2 = ring.tangent.cross(e1);
    let sorted = a.by_angle(ring.position, e1, e2);
    let reach = (ring.radius - inset).max(0.0);
    (0..n)
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
        .collect()
}

/// A ring of points to a ring of points or a single point, matched by their angle around the
/// axis of the ring in a
fn ring_to_ring(a: &Curve, b: &Curve, n: usize) -> Vec<Anchor> {
    // Newell's normal of the ring
    let m = a.points.len();
    let normal = (0..m).fold(Vec3::ZERO, |sum, i| sum + a.points[i].cross(a.points[(i + 1) % m]));
    let axis = normal.try_normalize().unwrap_or(Vec3::Y);
    let centre = b.centre();
    let e1 = orthogonal_to(axis);
    let e2 = axis.cross(e1);
    let (sorted_a, sorted_b) = (a.by_angle(centre, e1, e2), b.by_angle(centre, e1, e2));
    (0..n)
        .map(|k| {
            let angle = TAU * k as f32 / n as f32;
            let (a_position, a_source) = a.at_angle(&sorted_a, angle);
            let (b_position, b_source) = b.at_angle(&sorted_b, angle);
            Anchor { b: b_position, b_source, a: a_position, a_source, place: k as f32 / n as f32 }
        })
        .collect()
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
    flare: &'f [f32],
    resolution: usize,
}

/// The strands of one membrane: whole while intact, the part on b and the part on a once torn
fn strands(anchors: &[Anchor], closed: bool, skin: &Skin, seed: i32, creator: i32, offset: usize) -> (Vec<Vec<i32>>, Vec<Vec<i32>>) {
    let n = anchors.len();
    if n == 0 {
        return (Vec::new(), Vec::new());
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
            continue;
        }
        let relax = smoothstep(0.0, 1.0, (skin.tear - onset[k]) / (1.0 - TEAR_SPREAD));
        let ragged = 0.6 * wave(anchor.place, RAGGED_SCALE, 1.0) + 0.4 * (2.0 * random(k, 2) - 1.0);
        let cut = (skin.tear_position + RAGGED * ragged).clamp(0.0, 1.0);
        let count = |share: f32| ((res as f32 * share).round() as usize).max(MIN_POINTS);

        if cut > MIN_PART {
            let m = count(cut);
            let points: Vec<Vec3> = (0..m).map(|i| skin_at(cut * i as f32 / (m - 1) as f32)).collect();
            on_b.push(encode(link_b(k), depth_b, &fall(&points, horizontal(chord), skin.flare, relax), edge));
        }
        if cut < 1.0 - MIN_PART {
            let m = count(1.0 - cut);
            let points: Vec<Vec3> = (0..m).map(|i| skin_at(1.0 - (1.0 - cut) * i as f32 / (m - 1) as f32)).collect();
            let s = anchor.a_source;
            let link = PathLink::child(creator, s.id, s.alpha, 2 * (offset + k) as i32 + 1);
            on_a.push(encode(link, s.depth + 1, &fall(&points, horizontal(-chord), skin.flare, relax), edge));
        }
    }
    (on_b, on_a)
}

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let mut ctx = Ctx::default();
    let a = read_curves(args[0]);
    let b = read_curves(args[1]);
    let flare = read_floats(args[7]);
    let seed = int(args[9]);
    let resolution = int(args[10]).max(4) as usize;
    // the graph node that makes the membrane, it is the last input
    let creator = int(args[11]);

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
        let skin = Skin {
            thickness: ctx.float(args[3]).max(0.0),
            sag: ctx.float(args[4]).max(0.0),
            tear: ctx.float(args[5]).clamp(0.0, 1.0),
            tear_position: ctx.float(args[6]).clamp(0.0, 1.0),
            flare: &flare,
            resolution,
        };
        let n = ctx.int(args[8]).clamp(1, MAX_STRANDS) as usize;

        let (anchors, closed) = if curve_a.closed && curve_b.path.is_some() {
            (around_path(curve_a, curve_b, attachment, n, 0.5 * skin.thickness), true)
        } else if curve_a.closed && (curve_b.closed || curve_b.points.len() == 1) {
            (ring_to_ring(curve_a, curve_b, n), true)
        } else {
            (end_to_end(curve_a, curve_b, n), false)
        };
        // a second membrane on the same attachment numbers its strands after the first
        let (on_b, on_a) = strands(&anchors, closed, &skin, seed, creator, strands_on[j]);
        strands_on[j] += n;
        output.extend(on_b);
        output.extend(on_a);
    }

    concat_arg_vecs(output)
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use glam::Vec3;
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
        let mut data = create_multiple_paths(xs.len(), 12, 1, 3);
        for (k, path) in wrap_multiple_paths(&mut data).iter_mut().enumerate() {
            for i in 0..12 {
                path.points[i * 4..i * 4 + 4].copy_from_slice(&[xs[k], STIPE_TOP * i as f32 / 11.0, 0.0, STIPE_RADIUS]);
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
        thickness: f32,
        sag: f32,
        tear: f32,
        tear_position: f32,
        flare: Vec<f32>,
        strands: i32,
        seed: i32,
    }

    fn veil() -> Params {
        Params {
            attachment: 0.0,
            thickness: 0.001,
            sag: 0.1,
            tear: 0.0,
            tear_position: 0.5,
            flare: vec![0.0, 0.15, 0.6, 0.3, 1.0, 0.5],
            strands: 160,
            seed: 3,
        }
    }

    fn execute_with(a: &[i32], b: &[i32], p: &Params, flare: &[i32]) -> Vec<i32> {
        __nodarium_execute_inner(&[
            a,
            b,
            &[encode_float(p.attachment)],
            &[encode_float(p.thickness)],
            &[encode_float(p.sag)],
            &[encode_float(p.tear)],
            &[encode_float(p.tear_position)],
            flare,
            &[p.strands],
            &[p.seed],
            &[16],
            &[CREATOR],
        ])
    }

    fn run(a: &[i32], b: &[i32], p: &Params) -> Vec<Vec<i32>> {
        split_args(&execute_with(a, b, p, &floats(&p.flare))).iter().map(|s| s.to_vec()).collect()
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
}
