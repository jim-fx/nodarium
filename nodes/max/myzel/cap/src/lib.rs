use glam::{Vec2, Vec3};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{
    concat_arg_vecs, curve, decode_float, encode_points,
    geometry::{create_path, orthogonal_to, path_random, set_path_link, wrap_path, wrap_path_mut, DeepestPaths, PathLink},
    int, split_args, Ctx, Point,
};
use std::f32::consts::{PI, TAU};

nodarium_definition_file!("src/definition.json");

/// Points the drawn outline is resampled to before it is offset and bent
const OUTLINE_SAMPLES: usize = 96;
/// The share of the cap, counted from the margin, that `margin_curl` rolls under
const CURL_REACH: f32 = 0.35;
/// Rays added where they spread never take a cap past this many
const MAX_RAYS: usize = 1024;
/// How far down into the stipe the rays reach, in radii of the stipe top. They overlap the ends
/// of the stipe strands there, so stipe and cap close into one body without a seam.
const ROOT_DEPTH: f32 = 0.6;
/// How far across the axis the rays start in the stipe, as a share of the flesh half thickness
/// (or the stipe radius, when it is thinner)
const ROOT_OFFSET: f32 = 0.5;
const ROOT_SAMPLES: usize = 8;
/// An added ray grows to full thickness over this many flesh half thicknesses, so its start
/// sinks into its neighbours instead of showing a cut end
const RAMP_LENGTH: f32 = 2.0;
/// How much a ray may turn away from its even place between its neighbours, as a share of the gap
const ANGLE_JITTER: f32 = 0.3;

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

/// The drawn outline as (distance from the axis, height), with the top over the axis at 0.
/// The shape editor draws the left half of a 100 x 100 box, mirrored at x = 50, with y going
/// down. Its half width is the radius and both directions share the scale, so the cap keeps the
/// proportions it is drawn with.
fn outline(points: &[f32], radius: f32) -> Vec<Vec2> {
    let scale = radius / 50.0;
    let mut outline: Vec<Vec2> = points
        .chunks_exact(2)
        .map(|p| Vec2::new((50.0 - p[0].min(100.0 - p[0])) * scale, -p[1] * scale))
        .collect();
    outline.sort_by(|a, b| a.x.total_cmp(&b.x));
    outline.dedup_by(|a, b| a.x - b.x <= f32::EPSILON * radius);
    if outline.len() < 2 {
        return vec![Vec2::ZERO, Vec2::new(radius, 0.0)];
    }
    if outline[0].x > 0.0 {
        outline.insert(0, Vec2::new(0.0, outline[0].y));
    }
    let top = outline[0].y;
    outline.iter().map(|p| p - Vec2::Y * top).collect()
}

/// `count` points evenly spaced along a polyline, with how far along it each one is (0..1)
fn resample<T: Copy>(points: &[Vec2], values: &[T], count: usize, mix: impl Fn(T, T, f32) -> T) -> Vec<(Vec2, T, f32)> {
    let mut along = vec![0.0f32; points.len()];
    for i in 1..points.len() {
        along[i] = along[i - 1] + points[i].distance(points[i - 1]);
    }
    let length = along[points.len() - 1];
    let mut j = 0;
    (0..count)
        .map(|k| {
            let a = k as f32 / (count - 1).max(1) as f32;
            let s = a * length;
            while j + 2 < points.len() && along[j + 1] < s {
                j += 1;
            }
            let span = along[j + 1] - along[j];
            let t = if span > 0.0 { ((s - along[j]) / span).clamp(0.0, 1.0) } else { 0.0 };
            (points[j].lerp(points[j + 1], t), mix(values[j], values[j + 1], t), a)
        })
        .collect()
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The line the rays follow through the middle of the flesh, from the axis to the margin, as
/// steps that can be bent: every step keeps its length and turns by how far along it is.
struct Midline {
    start: Vec2,
    steps: Vec<(f32, f32)>,
    length: f32,
    /// Half the flesh thickness at every point, the radius of a ray there
    half: Vec<f32>,
}

impl Midline {
    /// The outline moved in by half the flesh thickness, so the outside of the flesh is the
    /// drawn outline. The hinge, where the flesh meets the stipe top, is at the origin.
    fn new(outline: &[Vec2], thickness: f32, thickness_curve: &[f32]) -> Self {
        let samples = resample(outline, &vec![(); outline.len()], OUTLINE_SAMPLES, |a, _, _| a);
        let n = samples.len();
        let half: Vec<f32> =
            samples.iter().map(|&(_, _, a)| 0.5 * thickness * curve(thickness_curve, a).max(0.0)).collect();
        let lift = Vec2::Y * 2.0 * half[0];
        let points: Vec<Vec2> = (0..n)
            .map(|i| {
                let (a, b) = (samples[i.saturating_sub(1)].0, samples[(i + 1).min(n - 1)].0);
                let tangent = (b - a).try_normalize().unwrap_or(Vec2::X);
                let normal = if i == 0 { Vec2::Y } else { tangent.perp() };
                samples[i].0 - normal * half[i] + lift
            })
            .collect();
        let steps: Vec<(f32, f32)> = points.windows(2).map(|w| ((w[1] - w[0]).length(), (w[1] - w[0]).to_angle())).collect();
        let length = steps.iter().map(|s| s.0).sum();
        Midline { start: points[0], steps, length, half }
    }

    /// The midline turned down by `closing` radians spread evenly along it, and its outer part
    /// rolled under by `curl` (1 = half a turn)
    fn bend(&self, closing: f32, curl: f32) -> Vec<Vec2> {
        let mut points = Vec::with_capacity(self.steps.len() + 1);
        let (mut p, mut s) = (self.start, 0.0);
        points.push(p);
        for &(length, angle) in &self.steps {
            let a = if self.length > 0.0 { (s + 0.5 * length) / self.length } else { 0.0 };
            let turn = closing * a + curl * PI * smoothstep(1.0 - CURL_REACH, 1.0, a);
            p += Vec2::from_angle(angle - turn) * length;
            s += length;
            points.push(p);
        }
        points
    }

    /// How far the midline has to close to bring the margin in to `reach` from the axis: the
    /// first turn that gets it there, or the one that comes closest
    fn closing_turn(&self, curl: f32, reach: f32) -> f32 {
        let margin = |turn: f32| self.bend(turn, curl).last().unwrap().x;
        const STEPS: usize = 64;
        let (mut best, mut best_margin) = (0.0, margin(0.0));
        if best_margin <= reach {
            return 0.0;
        }
        for i in 1..=STEPS {
            let turn = TAU * i as f32 / STEPS as f32;
            let m = margin(turn);
            if m <= reach {
                let (mut lo, mut hi) = (TAU * (i - 1) as f32 / STEPS as f32, turn);
                for _ in 0..30 {
                    let mid = 0.5 * (lo + hi);
                    if margin(mid) <= reach {
                        hi = mid;
                    } else {
                        lo = mid;
                    }
                }
                return hi;
            }
            if m < best_margin {
                (best, best_margin) = (turn, m);
            }
        }
        best
    }
}

/// A ray in the plane through the axis: points as (distance from the axis, height over the
/// hinge) and the radius at every point
struct Ray2 {
    points: Vec<Vec2>,
    radius: Vec<f32>,
}

/// Where a cap's rays start along the midline and at which angle. The first `rays` leave the
/// stipe, the others start between two neighbours where these drift more than `spacing` flesh
/// thicknesses apart. Decided on the open cap, so rays don't come and go while it opens.
fn place_rays(open: &[Vec2], half: &[f32], rays: usize, spacing: f32, seed: i32, id: i32) -> Vec<(f32, usize)> {
    let jitter = |k: usize, gap: f32| (path_random(seed, id, k as i32) - 0.5) * ANGLE_JITTER * gap;
    let gap = TAU / rays as f32;
    let mut placed: Vec<(f32, usize)> = (0..rays).map(|k| (k as f32 * gap + jitter(k, gap), 0)).collect();
    if spacing <= 0.0 {
        return placed;
    }
    let mut ring: Vec<f32> = placed.iter().map(|r| r.0).collect();
    let mut from = 1;
    while ring.len() * 2 <= MAX_RAYS {
        let count = ring.len() as f32;
        let Some(start) = (from..open.len()).find(|&i| half[i] > 0.0 && TAU * open[i].x / count > spacing * 2.0 * half[i])
        else {
            break;
        };
        // a ray needs a few points to the margin
        if start + 4 >= open.len() {
            break;
        }
        let gap = TAU / count;
        let mut next = Vec::with_capacity(ring.len() * 2);
        for (i, &angle) in ring.iter().enumerate() {
            let following = if i + 1 < ring.len() { ring[i + 1] } else { ring[0] + TAU };
            let between = 0.5 * (angle + following) + jitter(placed.len(), gap * 0.5);
            next.push(angle);
            next.push(between);
            placed.push((between, start));
        }
        ring = next;
        from = start + 1;
    }
    placed
}

struct Cap {
    radius: f32,
    profile: Vec<f32>,
    thickness: f32,
    thickness_curve: Vec<f32>,
    curl: f32,
    opening: f32,
    rays: usize,
    spacing: f32,
    resolution: usize,
}

/// The rays of one cap in the plane through the axis, with the angle each one points at
fn cap_rays(cap: &Cap, stipe_radius: f32, seed: i32, id: i32) -> Vec<(f32, Ray2)> {
    let midline = Midline::new(&outline(&cap.profile, cap.radius), cap.thickness, &cap.thickness_curve);
    let half = &midline.half;
    let open = midline.bend(0.0, 0.0);
    let margin_half = half[half.len() - 1];
    let closing = midline.closing_turn(cap.curl, stipe_radius + margin_half);
    let line = midline.bend((1.0 - cap.opening.clamp(0.0, 1.0)) * closing, cap.curl);

    // a ray starts on the far side of the axis and crosses it at the top of the cap, so every
    // ray passes over the centre and the flesh there is as thick as the rays
    let root_start = Vec2::new(-ROOT_OFFSET * half[0].min(stipe_radius), -ROOT_DEPTH * stipe_radius);
    // inside the stipe the rays stay within it, a thicker root would show as a collar under the cap
    let root_radius = half[0].min(stipe_radius + root_start.x);
    let root = {
        let end = line[0];
        let end_tangent = (line[1] - end).try_normalize().unwrap_or(Vec2::X);
        let k = root_start.distance(end);
        (0..ROOT_SAMPLES).map(move |i| {
            let t = i as f32 / ROOT_SAMPLES as f32;
            let (t2, t3) = (t * t, t * t * t);
            root_start * (2.0 * t3 - 3.0 * t2 + 1.0)
                + Vec2::Y * k * (t3 - 2.0 * t2 + t)
                + end * (-2.0 * t3 + 3.0 * t2)
                + end_tangent * k * (t3 - t2)
        })
    };

    let full_length: f32 = open.windows(2).map(|w| w[0].distance(w[1])).sum();
    place_rays(&open, half, cap.rays, cap.spacing, seed, id)
        .into_iter()
        .map(|(angle, start)| {
            let (points, radius): (Vec<Vec2>, Vec<f32>) = if start == 0 {
                root.clone()
                    .enumerate()
                    .map(|(i, p)| (p, root_radius + (half[0] - root_radius) * smoothstep(0.0, 1.0, i as f32 / ROOT_SAMPLES as f32)))
                    .chain((0..line.len()).map(|i| (line[i], half[i])))
                    .unzip()
            } else {
                let ramp = RAMP_LENGTH * half[start];
                let mut s = 0.0;
                (start..line.len())
                    .map(|i| {
                        if i > start {
                            s += open[i].distance(open[i - 1]);
                        }
                        (line[i], half[i] * (0.5 + 0.5 * smoothstep(0.0, ramp, s)))
                    })
                    .unzip()
            };
            let length: f32 = points.windows(2).map(|w| w[0].distance(w[1])).sum();
            let count = if start == 0 {
                cap.resolution
            } else {
                ((cap.resolution as f32 * length / full_length.max(f32::EPSILON)).round() as usize).clamp(4, cap.resolution)
            };
            let samples = resample(&points, &radius, count, |a, b, t| a + (b - a) * t);
            (angle, Ray2 { points: samples.iter().map(|s| s.0).collect(), radius: samples.iter().map(|s| s.1).collect() })
        })
        .collect()
}

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> (Vec<i32>, Vec<i32>, Vec<i32>) {
    let mut ctx = Ctx::default();
    let plant = split_args(args[0]);
    let seed = int(args[9]);
    let resolution = int(args[10]).max(4) as usize;
    // the graph node that makes the cap, it is the last input
    let creator = int(args[11]);

    let stipes = DeepestPaths::new(&plant, 1);
    let selected = |p: &[i32]| stipes.contains(p) && wrap_path(p).length > 1;
    let count = plant.iter().filter(|p| selected(p)).count();

    let (mut output, mut hinges, mut margins) = (Vec::new(), Vec::new(), Vec::new());
    for (rank, item) in plant.iter().filter(|p| selected(p)).enumerate() {
        ctx.set_path(item, rank, count);
        // the cap sits at the tip of its stipe
        ctx.alpha = 1.0;

        let cap = Cap {
            radius: ctx.float(args[1]).max(0.0),
            profile: read_floats(args[2]),
            thickness: ctx.float(args[3]).max(0.0),
            thickness_curve: read_floats(args[4]),
            curl: ctx.float(args[5]).clamp(0.0, 1.0),
            opening: ctx.float(args[6]),
            rays: ctx.int(args[7]).max(1) as usize,
            spacing: ctx.float(args[8]).max(0.0),
            resolution,
        };

        let stipe = wrap_path(item);
        let n = stipe.length;
        let point = |i: usize| Vec3::from_slice(&stipe.points[i * 4..i * 4 + 3]);
        let hinge = point(n - 1);
        let axis = (hinge - point(n - 2)).try_normalize().unwrap_or(Vec3::Y);
        let stipe_radius = stipe.points[(n - 1) * 4 + 3].abs();
        let side = orthogonal_to(axis);
        let around = axis.cross(side);

        hinges.push(Point {
            position: hinge,
            direction: side,
            tangent: axis,
            scale: stipe_radius,
            parent_id: stipe.link.id,
            alpha: 1.0,
            depth: stipe.depth,
            index: 0,
        });

        let mut ring = Vec::new();
        for (index, (angle, ray)) in cap_rays(&cap, stipe_radius, seed, stipe.link.id).into_iter().enumerate() {
            let out = side * angle.cos() + around * angle.sin();
            let link = PathLink::child(creator, stipe.link.id, 1.0, index as i32);
            let depth = stipe.depth + 1;
            let mut data = create_path(ray.points.len(), depth);
            set_path_link(&mut data, link);
            let path = wrap_path_mut(&mut data);
            let mut last = [hinge; 2];
            for (j, (p, r)) in ray.points.iter().zip(&ray.radius).enumerate() {
                let q = hinge + out * p.x + axis * p.y;
                path.points[j * 4..j * 4 + 4].copy_from_slice(&[q.x, q.y, q.z, *r]);
                last = [last[1], q];
            }
            let tangent = (last[1] - last[0]).try_normalize().unwrap_or(out);
            ring.push((
                angle.rem_euclid(TAU),
                Point {
                    position: last[1],
                    direction: (out - tangent * out.dot(tangent)).try_normalize().unwrap_or_else(|| orthogonal_to(tangent)),
                    tangent,
                    scale: *ray.radius.last().unwrap(),
                    parent_id: link.id,
                    alpha: 1.0,
                    depth,
                    index: 0,
                },
            ));
            output.push(data);
        }
        // around the axis, so the margin can be followed as a ring
        ring.sort_by(|a, b| a.0.total_cmp(&b.0));
        margins.extend(ring.into_iter().map(|r| r.1));
    }

    (concat_arg_vecs(output), encode_points(&hinges), encode_points(&margins))
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use glam::{Vec2, Vec3};
    use nodarium_utils::{
        decode_points, encode_float,
        geometry::{create_multiple_paths, path_link, wrap_multiple_paths, wrap_path, PathLink},
        split_args, Point,
    };
    use std::f32::consts::TAU;

    const CREATOR: i32 = 7;
    const STIPE_TOP: f32 = 0.05;
    const STIPE_RADIUS: f32 = 0.005;

    /// An upright stipe 5 cm tall, 5 mm in radius
    fn stipe() -> Vec<i32> {
        let mut data = create_multiple_paths(1, 12, 1, 3);
        for path in wrap_multiple_paths(&mut data).iter_mut() {
            for i in 0..12 {
                path.points[i * 4..i * 4 + 4].copy_from_slice(&[0.0, STIPE_TOP * i as f32 / 11.0, 0.0, STIPE_RADIUS]);
            }
        }
        data
    }

    fn floats(values: &[f32]) -> Vec<i32> {
        let mut words = vec![0, values.len() as i32 + 1];
        words.extend(values.iter().map(|&v| encode_float(v)));
        words.extend([1, 1]);
        words
    }

    #[derive(Clone)]
    struct Params {
        radius: f32,
        profile: Vec<f32>,
        thickness: f32,
        thickness_curve: Vec<f32>,
        curl: f32,
        opening: f32,
        rays: i32,
        spacing: f32,
        seed: i32,
    }

    /// A cone from the top at y = 20 down to the margin at y = 60 of the shape editor: the margin
    /// is 0.8 radii below the top. The flesh thins to nothing at the margin, so the rays end on
    /// the drawn outline.
    fn cone() -> Params {
        Params {
            radius: 0.03,
            profile: vec![0.0, 60.0, 50.0, 20.0],
            thickness: 0.006,
            thickness_curve: vec![0.0, 1.0, 1.0, 0.0],
            curl: 0.0,
            opening: 1.0,
            rays: 10,
            spacing: 0.5,
            seed: 3,
        }
    }

    struct Out {
        rays: Vec<Vec<i32>>,
        hinges: Vec<Point>,
        margins: Vec<Point>,
    }

    fn execute(p: &Params, profile: &[i32]) -> (Vec<i32>, Vec<i32>, Vec<i32>) {
        __nodarium_execute_inner(&[
            &stipe(),
            &[encode_float(p.radius)],
            profile,
            &[encode_float(p.thickness)],
            &floats(&p.thickness_curve),
            &[encode_float(p.curl)],
            &[encode_float(p.opening)],
            &[p.rays],
            &[encode_float(p.spacing)],
            &[p.seed],
            &[24],
            &[CREATOR],
        ])
    }

    fn run(p: &Params) -> Out {
        let (paths, hinges, margins) = execute(p, &floats(&p.profile));
        Out {
            rays: split_args(&paths).iter().map(|s| s.to_vec()).collect(),
            hinges: decode_points(&hinges),
            margins: decode_points(&margins),
        }
    }

    fn points(path: &[i32]) -> Vec<[f32; 4]> {
        wrap_path(path).points.chunks_exact(4).map(|p| [p[0], p[1], p[2], p[3]]).collect()
    }

    /// Distance from the axis and height over the stipe top
    fn place(p: Vec3) -> Vec2 {
        Vec2::new(Vec2::new(p.x, p.z).length(), p.y - STIPE_TOP)
    }

    fn place_of(q: [f32; 4]) -> Vec2 {
        place(Vec3::new(q[0], q[1], q[2]))
    }

    #[test]
    fn the_margin_lies_on_the_radius_and_height_of_the_profile() {
        let p = cone();
        let out = run(&p);
        assert_eq!(out.margins.len(), out.rays.len());
        // the top of the flesh over the stipe is a thickness above the stipe top
        let height = p.thickness - 0.8 * p.radius;
        for m in &out.margins {
            let q = place(m.position);
            assert!((q.x - p.radius).abs() < 1e-4, "margin {} from the axis, radius {}", q.x, p.radius);
            assert!((q.y - height).abs() < 1e-4, "margin at {} over the stipe top, expected {}", q.y, height);
        }

        let wide = run(&Params { radius: 0.05, ..cone() });
        for m in &wide.margins {
            assert!((place(m.position).x - 0.05).abs() < 1e-4);
        }
    }

    #[test]
    fn the_outside_of_the_flesh_is_the_drawn_outline() {
        let p = Params { thickness_curve: vec![0.0, 1.0, 1.0, 1.0], spacing: 0.0, ..cone() };
        let out = run(&p);
        let slope = Vec2::new(1.0, -0.8).normalize();
        // the drawn outline runs from the top at (0, thickness) down to the margin
        let top = Vec2::new(0.0, p.thickness);
        for ray in &out.rays {
            for q in points(ray).iter().skip(12).take(8) {
                let above = (place_of(*q) - top).perp_dot(slope);
                assert!((q[3] - above).abs() < 2e-4, "flesh reaches {} over the outline", q[3] - above);
            }
        }
    }

    #[test]
    fn opening_moves_the_margin_from_under_the_cap_to_the_end_of_the_profile() {
        let p = Params { thickness_curve: vec![0.0, 1.0, 1.0, 0.4], ..cone() };
        let margin = |opening: f32| {
            let out = run(&Params { opening, ..p.clone() });
            out.margins.iter().map(|m| place(m.position)).fold(Vec2::ZERO, |a, b| a + b) / out.margins.len() as f32
        };
        let closed = margin(0.0);
        // against the stipe, below its top and the cap
        let margin_half = 0.5 * 0.4 * p.thickness;
        assert!((closed.x - (STIPE_RADIUS + margin_half)).abs() < 5e-4, "closed margin {} from the axis", closed.x);
        assert!(closed.y < 0.0, "closed margin {} over the stipe top", closed.y);

        let open = margin(1.0);
        let end = Vec2::new(p.radius, p.thickness - 0.8 * p.radius);
        // the margin is half its flesh thickness inside the drawn end
        assert!(open.distance(end) < margin_half + 1e-4, "open margin at {:?}, profile ends at {:?}", open, end);

        let mut previous = closed.x;
        for k in 1..=10 {
            let m = margin(k as f32 / 10.0);
            assert!(m.x > previous, "the margin moves in from {} to {} while opening", previous, m.x);
            previous = m.x;
        }
    }

    #[test]
    fn margin_curl_rolls_the_margin_under() {
        let flat = run(&Params { curl: 0.0, ..cone() });
        let curled = run(&Params { curl: 1.0, ..cone() });
        let reach = |out: &Out| place(out.margins[0].position).x;
        assert!(reach(&curled) < reach(&flat));
        // rolled under, the margin points back towards the stipe
        let outward = |m: &Point| Vec3::new(m.position.x, 0.0, m.position.z).normalize();
        assert!(curled.margins[0].tangent.dot(outward(&curled.margins[0])) < 0.0);
        assert!(flat.margins[0].tangent.dot(outward(&flat.margins[0])) > 0.0);
    }

    #[test]
    fn rays_leave_the_stipe_top_inside_the_stipe() {
        let out = run(&cone());
        let stipe_id = wrap_path(split_args(&stipe())[0]).link.id;
        for (k, ray) in out.rays.iter().enumerate() {
            assert_eq!(path_link(ray), PathLink::child(CREATOR, stipe_id, 1.0, k as i32));
            assert_eq!(ray[3], 2);
        }
        // the first rays leave the stipe, the others start between them further out
        let in_stipe = |q: Vec2| q.x < STIPE_RADIUS && q.y < -0.5 * STIPE_RADIUS;
        let leaving = out.rays.iter().filter(|r| in_stipe(place_of(points(r)[0]))).count();
        assert_eq!(leaving, 10);
        for ray in &out.rays[..10] {
            let (start, next) = (points(ray)[0], points(ray)[1]);
            let (a, b) = (place_of(start), place_of(next));
            assert!(a.x + start[3] < STIPE_RADIUS + 1e-6, "ray starts {} out of a stipe of {}", a.x + start[3], STIPE_RADIUS);
            // it leaves upwards, along the stipe strands
            assert!(b.y - a.y > b.x - a.x, "ray leaves from {:?} to {:?}", a, b);
        }
        assert_eq!(out.hinges.len(), 1);
        let hinge = &out.hinges[0];
        assert!(hinge.position.distance(Vec3::new(0.0, STIPE_TOP, 0.0)) < 1e-6);
        assert!(hinge.tangent.distance(Vec3::Y) < 1e-5);
        assert_eq!(hinge.parent_id, stipe_id);
        assert!((hinge.scale - STIPE_RADIUS).abs() < 1e-7);
    }

    #[test]
    fn rays_are_added_where_they_drift_apart() {
        let p = Params { thickness_curve: vec![0.0, 1.0, 1.0, 0.5], ..cone() };
        let few = run(&Params { spacing: 0.0, ..p.clone() });
        assert_eq!(few.rays.len(), 10);
        let many = run(&p);
        assert!(many.rays.len() > 40, "{} rays", many.rays.len());
        // the margin points go around the axis, neighbours about `spacing` flesh thicknesses
        // apart and not more
        let m = &many.margins;
        let largest = (0..m.len()).map(|i| m[i].position.distance(m[(i + 1) % m.len()].position)).fold(0.0, f32::max);
        let flesh = 0.5 * p.thickness;
        assert!(largest < 1.4 * p.spacing * flesh, "margin gap {} for a flesh of {}", largest, flesh);
        let around = TAU * p.radius / m.len() as f32;
        assert!(largest < 1.5 * around, "margin gap {}, {} if spaced evenly", largest, around);
        // added rays start out on the cap where their neighbours spread, each round of them
        // further out than the one before
        let starts: Vec<f32> = many.rays[10..].iter().map(|r| place_of(points(r)[0]).x).collect();
        assert!(starts[0] > 0.003, "the first added ray starts {} from the axis", starts[0]);
        assert!(starts.windows(2).all(|w| w[1] >= w[0] - 1e-6));
        assert!(starts[starts.len() - 1] > starts[0] + 0.005);
    }

    #[test]
    fn a_cap_without_flesh_adds_no_rays() {
        let out = run(&Params { thickness: 0.0, ..cone() });
        assert_eq!(out.rays.len(), 10);
    }

    #[test]
    fn rays_stay_put_while_the_cap_opens() {
        let half_open = run(&Params { opening: 0.5, ..cone() });
        let open = run(&cone());
        assert_eq!(half_open.rays.len(), open.rays.len());
        for (a, b) in half_open.rays.iter().zip(&open.rays) {
            assert_eq!(path_link(a), path_link(b));
            assert_eq!(points(a).len(), points(b).len());
        }
    }

    #[test]
    fn the_same_inputs_give_the_same_cap() {
        let p = Params { curl: 0.4, opening: 0.6, ..cone() };
        let (a, b) = (run(&p), run(&p));
        assert_eq!(a.rays, b.rays);
        assert_eq!(a.margins, b.margins);
        let other = run(&Params { seed: 4, ..p });
        assert_ne!(a.rays, other.rays);
    }

    #[test]
    fn reads_a_connected_shape_node() {
        let p = cone();
        let profile = floats(&p.profile);
        let wrapped = nodarium_utils::concat_args(vec![&profile]);
        assert_eq!(super::read_floats(&wrapped), p.profile);
        let (paths, _, _) = execute(&p, &wrapped);
        assert_eq!(split_args(&paths), split_args(&execute(&p, &profile).0));
    }

    /// The underside the gills hang from, read back from the rays: in the plane through the
    /// axis it lies on the lower side of the flesh, one ray radius from the middle of the
    /// nearest ray. The cap is round, so all rays share that plane.
    fn underside_gap(out: &Out) -> f32 {
        let rays: Vec<&[f32]> = out.rays.iter().map(|r| wrap_path(r).points).collect();
        let cap = myzel_core::CapUnderside::new(&out.hinges[0], &rays).expect("a cap with rays from the stipe");
        let mut worst = 0.0f32;
        for k in 0..200 {
            let (f, angle) = (0.1 + 0.85 * (k % 20) as f32 / 19.0, TAU * k as f32 / 200.0);
            let u = place(cap.at(f, angle).position);
            let (mut nearest, mut radius) = (f32::MAX, 0.0);
            for ray in &out.rays {
                for w in points(ray).windows(2) {
                    let (a, b) = (place_of(w[0]), place_of(w[1]));
                    let t = ((u - a).dot(b - a) / (b - a).length_squared().max(1e-12)).clamp(0.0, 1.0);
                    let d = u.distance(a.lerp(b, t));
                    if d < nearest {
                        (nearest, radius) = (d, w[0][3] + (w[1][3] - w[0][3]) * t);
                    }
                }
            }
            worst = worst.max((nearest - radius).abs() / radius.max(1e-4));
        }
        worst
    }

    #[test]
    fn gills_stay_on_the_underside_however_the_cap_opens_or_is_drawn() {
        use myzel_core::{lamellae, reaction_diffusion, CapUnderside, LamellaeParams, Mesh, ReactionDiffusionParams};
        let pattern = reaction_diffusion(&ReactionDiffusionParams { radius: 0.03, steps: 1500, ..Default::default() });
        let params = LamellaeParams::default();
        let flat = Params { thickness_curve: vec![0.0, 1.0, 1.0, 0.4], spacing: 0.0, ..cone() };
        let shapes = [
            flat.clone(),
            // a dome that falls steeply to the margin
            Params { profile: vec![0.0, 85.0, 10.0, 45.0, 30.0, 25.0, 50.0, 20.0], ..flat.clone() },
            Params { thickness: 0.012, ..flat.clone() },
            Params { radius: 0.045, curl: 0.6, ..flat.clone() },
        ];
        for shape in &shapes {
            let mut margins = Vec::new();
            for opening in [0.0, 0.5, 1.0] {
                let out = run(&Params { opening, ..shape.clone() });
                let gap = underside_gap(&out);
                assert!(gap < 0.15, "underside {} ray radii off the flesh at opening {}", gap, opening);

                let rays: Vec<&[f32]> = out.rays.iter().map(|r| wrap_path(r).points).collect();
                let cap = CapUnderside::new(&out.hinges[0], &rays).unwrap();
                let margin = place(cap.at(1.0, 0.0).position);
                margins.push(margin.x);
                let mesh = lamellae(&[cap], &pattern, &params, Mesh::default());
                assert!(mesh.face_count() > 100, "no gills at opening {}", opening);
                // every gill vertex hangs within its depth below the flesh of a ray
                let reach = params.depth * 1.5 + shape.thickness;
                for v in mesh.positions.chunks_exact(3) {
                    let v = Vec3::from_slice(v);
                    let nearest = out
                        .rays
                        .iter()
                        .flat_map(|r| points(r))
                        .map(|q| v.distance(Vec3::new(q[0], q[1], q[2])))
                        .fold(f32::MAX, f32::min);
                    assert!(nearest < reach, "a gill {} mm from the cap at opening {}", nearest * 1e3, opening);
                }
            }
            // the gills went along as the margin came out from the stipe
            assert!(margins[0] < margins[1] && margins[1] < margins[2], "{:?}", margins);
        }
    }
}
