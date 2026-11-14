use glam::{Vec2, Vec3};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{
    concat_arg_vecs,
    geometry::{is_geometry, split_items, wrap_geometry_data},
    int, Ctx,
};
use std::f32::consts::TAU;

nodarium_definition_file!("src/definition.json");

/// How often bend and twist read the amount from the base to the tip
const ALONG: usize = 256;
/// How often curl reads the amount from the base to the tip, and from the middle to the edge
const CURL_ROWS: usize = 32;
const CURL_COLUMNS: usize = 65;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Mode {
    Bend,
    Curl,
    Twist,
}

impl Mode {
    fn from_index(index: i32) -> Self {
        match index {
            1 => Mode::Curl,
            2 => Mode::Twist,
            _ => Mode::Bend,
        }
    }
}

/// The angle a line has turned by at evenly spaced places from 0 to 1 of its length, where it
/// turns `rates[k]` turns per length
fn turned(rates: &[f32]) -> Vec<f32> {
    let step = 1.0 / (rates.len() - 1) as f32;
    let mut angle = vec![0.0f32; rates.len()];
    for k in 1..rates.len() {
        angle[k] = angle[k - 1] + TAU * 0.5 * (rates[k - 1] + rates[k]) * step;
    }
    angle
}

/// A line of length 1 that starts at the origin along +u and turns towards +v, sampled evenly.
/// Before its start and after its end it goes on straight.
struct Line {
    angle: Vec<f32>,
    point: Vec<Vec2>,
}

impl Line {
    fn new(rates: &[f32]) -> Self {
        let angle = turned(rates);
        let step = 1.0 / (rates.len() - 1) as f32;
        let mut point = vec![Vec2::ZERO; rates.len()];
        for k in 1..rates.len() {
            point[k] = point[k - 1] + Vec2::from_angle(0.5 * (angle[k - 1] + angle[k])) * step;
        }
        Line { angle, point }
    }

    fn at(&self, t: f32) -> (Vec2, f32) {
        let last = self.point.len() - 1;
        if t <= 0.0 {
            return (Vec2::new(t, 0.0), 0.0);
        }
        if t >= 1.0 {
            let a = self.angle[last];
            return (self.point[last] + Vec2::from_angle(a) * (t - 1.0), a);
        }
        let u = t * last as f32;
        let k = (u as usize).min(last - 1);
        let f = u - k as f32;
        (self.point[k].lerp(self.point[k + 1], f), self.angle[k] + (self.angle[k + 1] - self.angle[k]) * f)
    }
}

/// Where a point `offset` above `t` of a line goes, and how far it turned
fn place(line: &Line, t: f32, offset: f32) -> (Vec2, f32) {
    let (p, a) = line.at(t);
    (p + Vec2::from_angle(a).perp() * offset, a)
}

/// The widest vertex in evenly spaced bins from the base to the tip, read between the middles
/// of the bins, so the width does not jump from one bin to the next
struct Width {
    bins: Vec<f32>,
}

impl Width {
    fn new(positions: &[f32], length: f32) -> Self {
        let n = positions.len() / 3;
        let count = ((n as f32).sqrt().round() as usize).clamp(4, 64);
        let mut bins = vec![-1.0f32; count];
        for p in positions.chunks_exact(3) {
            let b = ((-p[2] / length).clamp(0.0, 1.0) * count as f32) as usize;
            let b = b.min(count - 1);
            bins[b] = bins[b].max(p[0].abs());
        }
        // a grid with fewer rows than bins leaves bins without a vertex, they take the width
        // between the bins next to them
        let filled: Vec<usize> = (0..count).filter(|&b| bins[b] >= 0.0).collect();
        for b in 0..count {
            if bins[b] >= 0.0 {
                continue;
            }
            let after = filled.partition_point(|&f| f < b);
            bins[b] = match (after.checked_sub(1).map(|i| filled[i]), filled.get(after)) {
                (Some(lo), Some(&hi)) => bins[lo] + (bins[hi] - bins[lo]) * (b - lo) as f32 / (hi - lo) as f32,
                (Some(lo), None) => bins[lo],
                (None, Some(&hi)) => bins[hi],
                (None, None) => 0.0,
            };
        }
        Width { bins }
    }

    fn at(&self, t: f32) -> f32 {
        let u = (t.clamp(0.0, 1.0) * self.bins.len() as f32 - 0.5).clamp(0.0, (self.bins.len() - 1) as f32);
        let k = (u as usize).min(self.bins.len() - 2);
        let f = u - k as f32;
        self.bins[k] + (self.bins[k + 1] - self.bins[k]) * f
    }
}

/// Moves every vertex and returns the rotation of the surface there, which the old normal
/// follows
fn move_vertices(ctx: &mut Ctx, positions: &mut [f32], mode: Mode, amount: &[i32]) -> Option<Vec<Rotation>> {
    let length = positions.chunks_exact(3).map(|p| -p[2]).fold(0.0f32, f32::max);
    if length <= 0.0 {
        return None;
    }
    let alpha = |z: f32| -z / length;
    let mut turns = Vec::with_capacity(positions.len() / 3);

    match mode {
        Mode::Bend => {
            let rates = ctx.floats_at(amount, (0..ALONG).map(|k| (k as f32 / (ALONG - 1) as f32, 0.0)));
            let line = Line::new(&rates);
            for p in positions.chunks_exact_mut(3) {
                let (q, a) = place(&line, alpha(p[2]), p[1] / length);
                p[1] = q.y * length;
                p[2] = -q.x * length;
                turns.push(Rotation::AroundX(a));
            }
        }
        Mode::Twist => {
            let rates = ctx.floats_at(amount, (0..ALONG).map(|k| (k as f32 / (ALONG - 1) as f32, 0.0)));
            let angle = turned(&rates);
            let last = (ALONG - 1) as f32;
            for p in positions.chunks_exact_mut(3) {
                let u = alpha(p[2]).clamp(0.0, 1.0) * last;
                let k = (u as usize).min(ALONG - 2);
                let a = angle[k] + (angle[k + 1] - angle[k]) * (u - k as f32);
                let q = Vec2::from_angle(a).rotate(Vec2::new(p[0], p[1]));
                p[0] = q.x;
                p[1] = q.y;
                turns.push(Rotation::AroundZ(a));
            }
        }
        Mode::Curl => {
            let at = (0..CURL_ROWS).flat_map(|r| {
                let along = r as f32 / (CURL_ROWS - 1) as f32;
                (0..CURL_COLUMNS).map(move |c| (along, c as f32 / (CURL_COLUMNS - 1) as f32))
            });
            let rates = ctx.floats_at(amount, at);
            let rows: Vec<Line> = rates.chunks_exact(CURL_COLUMNS).map(Line::new).collect();
            let width = Width::new(positions, length);
            let last = (CURL_ROWS - 1) as f32;
            for p in positions.chunks_exact_mut(3) {
                let t = alpha(p[2]);
                let w = width.at(t);
                if w <= 0.0 {
                    turns.push(Rotation::AroundZ(0.0));
                    continue;
                }
                let u = t.clamp(0.0, 1.0) * last;
                let k = (u as usize).min(CURL_ROWS - 2);
                let f = u - k as f32;
                let across = p[0].abs() / w;
                let (q0, a0) = rows[k].at(across);
                let (q1, a1) = rows[k + 1].at(across);
                let (q, a) = (q0.lerp(q1, f), a0 + (a1 - a0) * f);
                let side = if p[0] < 0.0 { -1.0 } else { 1.0 };
                // the height is added rather than turned with the roll: a margin rolls tighter
                // than a bent or cupped leaf is high, turned along it would fold over
                p[0] = side * q.x * w;
                p[1] += q.y * w;
                turns.push(Rotation::AroundZ(side * a));
            }
        }
    }
    Some(turns)
}

#[derive(Clone, Copy)]
enum Rotation {
    /// Turns -z towards +y
    AroundX(f32),
    /// Turns +x towards +y
    AroundZ(f32),
}

impl Rotation {
    fn apply(self, n: Vec3) -> Vec3 {
        match self {
            Rotation::AroundX(a) => {
                let q = Vec2::from_angle(a).rotate(Vec2::new(-n.z, n.y));
                Vec3::new(n.x, q.y, -q.x)
            }
            Rotation::AroundZ(a) => {
                let q = Vec2::from_angle(a).rotate(Vec2::new(n.x, n.y));
                Vec3::new(q.x, q.y, n.z)
            }
        }
    }
}

/// Area weighted normals of the moved surface. They keep the side of the old normals, turned
/// along, so a geometry whose faces are wound against its normals keeps facing the same way.
fn set_normals(positions: &[f32], normals: &mut [f32], faces: &[i32], turns: &[Rotation]) {
    let position = |i: usize| Vec3::from_slice(&positions[i * 3..i * 3 + 3]);
    let mut sum = vec![Vec3::ZERO; turns.len()];
    for face in faces.chunks_exact(3) {
        let [a, b, c] = [face[0] as usize, face[1] as usize, face[2] as usize];
        let n = (position(b) - position(a)).cross(position(c) - position(a));
        sum[a] += n;
        sum[b] += n;
        sum[c] += n;
    }
    for (i, (s, turn)) in sum.iter().zip(turns).enumerate() {
        let old = turn.apply(Vec3::from_slice(&normals[i * 3..i * 3 + 3]));
        let n = match s.try_normalize() {
            Some(n) if n.dot(old) < 0.0 => -n,
            Some(n) => n,
            None => old,
        };
        normals[i * 3..i * 3 + 3].copy_from_slice(&n.to_array());
    }
}

fn deform(ctx: &mut Ctx, geometry: &[i32], mode: Mode, amount: &[i32]) -> Vec<i32> {
    let mut out = geometry.to_vec();
    let data = wrap_geometry_data(&mut out);
    if let Some(turns) = move_vertices(ctx, data.positions, mode, amount) {
        set_normals(data.positions, data.normals, data.faces, &turns);
    }
    out
}

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let mut ctx = Ctx::default();
    let mode = Mode::from_index(int(args[1]));
    let amount = args[2];
    let unchanged = amount.len() == 1 && ctx.float(amount) == 0.0;

    let items: Vec<Vec<i32>> = split_items(args[0])
        .into_iter()
        .map(|item| {
            if is_geometry(item) && !unchanged {
                deform(&mut ctx, item, mode, amount)
            } else {
                item.to_vec()
            }
        })
        .collect();

    // a single geometry stays one, it is no list
    if is_geometry(args[0]) {
        return items.into_iter().next().unwrap();
    }
    concat_arg_vecs(items)
}

#[cfg(test)]
mod tests;
