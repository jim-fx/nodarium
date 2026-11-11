use glam::Vec3;
use myzel_core::{
    smooth_normals,
    veil::{add_skin, from_axis, horizontal, read_attachments, Attachment, Axis, Collar, Column, Skin, Source, Spot, DOWN},
    Mesh,
};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{
    decode_float,
    geometry::{orthogonal_to, path_id, path_random},
    int, Ctx, PathInfo,
};
use noise::{NoiseFn, OpenSimplex};
use std::f32::consts::{FRAC_PI_2, TAU};

nodarium_definition_file!("src/definition.json");

const MAX_STRANDS: i32 = 4096;
/// Strands around the skin, as many as the membrane has
const STRANDS: i32 = 256;
/// How far the torn edge wanders from `length`, as a share of it
const RAGGED: f32 = 0.35;
/// The edge takes this share of its wandering from strand to strand, the rest from a smooth noise
/// around the skin
const JITTER: f32 = 0.15;
/// Radius of the circle the noise of the edge is read on, it sets how many lobes it has
const RAGGED_SCALE: f64 = 4.0;
/// The same for the noise that makes the folds uneven
const FOLD_SCALE: f64 = 1.5;
/// Steps along the skin at which a connected flare is read
const FLARE_STEPS: usize = 32;

/// The flare along the skin, from where it is attached (0) to its edge (1)
enum Flare {
    Plain(f32),
    /// A connected input, read at even steps and linearly in between
    Read(Vec<f32>),
}

impl Flare {
    fn read(ctx: &mut Ctx, arg: &[i32]) -> Flare {
        if arg.len() == 1 {
            Flare::Plain(decode_float(arg[0]))
        } else {
            Flare::Read(ctx.float_along(arg, (0..=FLARE_STEPS).map(|i| i as f32 / FLARE_STEPS as f32)))
        }
    }

    fn at(&self, alpha: f32) -> f32 {
        let value = match self {
            Flare::Plain(value) => *value,
            Flare::Read(values) => {
                let x = alpha.clamp(0.0, 1.0) * (values.len() - 1) as f32;
                let i = (x as usize).min(values.len() - 2);
                values[i] + (values[i + 1] - values[i]) * (x - i as f32)
            }
        };
        value.clamp(0.0, 1.0)
    }
}

/// Where a strand of the skin is attached, and the way out from there: away from the path it
/// goes around, or in from a ring
struct Root {
    at: Vec3,
    out: Vec3,
}

/// A skin that goes around a path, hanging from its collar
fn on_collar(collar: &Collar, n: usize) -> (Vec<Root>, Axis) {
    let roots = (0..n)
        .map(|k| {
            let angle = TAU * k as f32 / n as f32;
            Root { at: collar.at(angle), out: collar.out(angle) }
        })
        .collect();
    (roots, (collar.centre, collar.tangent))
}

/// A skin that hangs on the inside of a ring of points
fn on_ring(ring: &Attachment, n: usize) -> (Vec<Root>, Axis) {
    let axis = ring.axis();
    let e1 = orthogonal_to(axis.1);
    let e2 = axis.1.cross(e1);
    let sorted = ring.by_angle(axis.0, e1, e2);
    let roots = (0..n)
        .map(|k| {
            let at = ring.at_angle(&sorted, TAU * k as f32 / n as f32).0;
            let inwards = axis.0 - at;
            let out = (inwards - axis.1 * inwards.dot(axis.1)).try_normalize().unwrap_or(e1);
            Root { at, out }
        })
        .collect();
    (roots, axis)
}

/// One strand of the skin, `length` long: every step hangs down, flared out by the flare read
/// along it. With every point how much longer the circle around the axis is that it would lie on
/// spread straight out than the one it hangs on, see `add_skin`.
fn hang(root: &Root, length: f32, flare: &Flare, axis: Axis, resolution: usize) -> Vec<(Vec3, f32)> {
    let side = horizontal(root.out);
    let step = length / (resolution - 1) as f32;
    let mut p = root.at;
    let mut column = Vec::with_capacity(resolution);
    for j in 0..resolution {
        let spread = root.at + root.out * step * j as f32;
        column.push((p, (from_axis(Some(axis), spread) / from_axis(Some(axis), p).max(1e-6) - 1.0).max(0.0)));
        let angle = flare.at((j as f32 + 0.5) / (resolution - 1) as f32) * FRAC_PI_2;
        p += (DOWN * angle.cos() + side * angle.sin()) * step;
    }
    column
}

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    build(args, STRANDS).1
}

/// The strands of every skin and the skins. Only the skins are an output, the strands are what
/// the tests look at.
fn build(args: &[&[i32]], strand_count: i32) -> (Vec<Vec<Vec3>>, Vec<i32>) {
    let mut ctx = Ctx::default();
    let mut mesh = Mesh::default();
    let attachments = read_attachments(args[0]);
    let seed = int(args[4]);
    let resolution = int(args[5]).max(4) as usize;
    // the graph node that makes the skirt, it is the last input
    let creator = int(args[6]);
    let n = strand_count.clamp(1, MAX_STRANDS) as usize;

    // a ring hangs one skin, every other point a skin around the path it sits on
    let mut skins: Vec<(Source, Option<Spot>, &Attachment)> = Vec::new();
    for attachment in attachments.iter().filter(|a| a.path.is_none()) {
        if attachment.is_ring() {
            skins.push((attachment.sources[0], None, attachment));
        } else {
            for i in 0..attachment.points.len() {
                let spot = attachment.spot(i);
                skins.push((spot.source, Some(spot), attachment));
            }
        }
    }

    let mut strands = Vec::new();
    for (rank, (source, spot, attachment)) in skins.iter().enumerate() {
        ctx.set_path_info(PathInfo {
            index: rank,
            rank,
            count: skins.len(),
            depth: source.depth,
            parent_position: source.alpha,
            length: 0.0,
        });
        let key = path_id(creator, source.id, 0);
        ctx.seed_path(key);
        let length = ctx.float(args[1]).max(0.0);
        let flare = Flare::read(&mut ctx, args[2]);
        let thickness = ctx.float(args[3]).max(0.0);
        if length <= 0.0 {
            continue;
        }
        let (roots, axis) = match spot {
            // grown out of the path, like a membrane
            Some(spot) => on_collar(&Collar::new(spot, 0.5 * thickness), n),
            None => on_ring(attachment, n),
        };

        let noise = OpenSimplex::new((seed as u32) ^ (key as u32));
        // smooth around the skin, it closes on itself
        let wave = |place: f32, scale: f64, salt: f64| {
            let angle = TAU as f64 * place as f64;
            (2.0 * noise.get([scale * angle.cos(), scale * angle.sin(), salt]) as f32).clamp(-1.0, 1.0)
        };
        let place: Vec<f32> = (0..n).map(|k| k as f32 / n as f32).collect();
        let wobble: Vec<f32> = place.iter().map(|&p| wave(p, FOLD_SCALE, 2.0)).collect();
        let columns: Vec<Column> = roots
            .iter()
            .enumerate()
            .map(|(k, root)| {
                let jitter = 2.0 * path_random(seed, key, k as i32) - 1.0;
                let ragged = (1.0 - JITTER) * wave(place[k], RAGGED_SCALE, 1.0) + JITTER * jitter;
                Some(hang(root, length * (1.0 + RAGGED * ragged), &flare, axis, resolution))
            })
            .collect();
        let attached: Vec<Vec3> = roots.iter().map(|root| root.at).collect();
        add_skin(&mut mesh, &Skin { columns: &columns, attached: &attached, axis: Some(axis), torn: Some((&place, &wobble)) }, thickness);
        strands.extend(columns.into_iter().flatten().map(|c| c.into_iter().map(|q| q.0).collect()));
    }
    smooth_normals(&mut mesh);

    (strands, mesh.encode())
}

#[cfg(test)]
mod tests {
    use super::{build, RAGGED};
    use glam::Vec3;
    use myzel_core::{sheet_checks, veil::EDGE_THIN, Mesh};
    use nodarium_utils::{concat_args, curve, encode_float, encode_points, Point};
    use std::f32::consts::{FRAC_PI_2, TAU};

    const CREATOR: i32 = 12;
    const STIPE_RADIUS: f32 = 0.005;
    const MARGIN_RADIUS: f32 = 0.02;
    const RING_HEIGHT: f32 = 0.035;
    const MARGIN_HEIGHT: f32 = 0.04;
    const RESOLUTION: i32 = 16;
    const LENGTH: f32 = 0.007;
    /// The flare the membrane drew with its own curve up to graph version 12
    const OLD_FLARE: [f32; 6] = [0.0, 0.45, 0.4, 0.25, 1.0, 0.12];

    /// A point on an upright stipe, as points_on_path gives it
    fn spot(x: f32, height: f32) -> Point {
        Point {
            position: Vec3::new(x, height, 0.0),
            direction: Vec3::X,
            tangent: Vec3::Y,
            scale: STIPE_RADIUS,
            parent_id: 7 + x.to_bits() as i32 % 100,
            alpha: 0.7,
            depth: 1,
            index: 0,
        }
    }

    /// A margin like cap_margin gives it
    fn margin() -> Vec<Point> {
        (0..96)
            .map(|i| {
                let angle = TAU * i as f32 / 96.0;
                let out = Vec3::new(angle.cos(), 0.0, angle.sin());
                Point {
                    position: Vec3::new(0.0, MARGIN_HEIGHT, 0.0) + out * MARGIN_RADIUS,
                    direction: Vec3::NEG_Y,
                    tangent: out,
                    scale: 0.001,
                    parent_id: 100 + i,
                    alpha: 1.0,
                    depth: 2,
                    index: 0,
                }
            })
            .collect()
    }

    fn run_with(attachment: &[Point], length: f32, flare: &[i32], seed: i32) -> (Vec<Vec<Vec3>>, Mesh) {
        let (strands, mesh) = build(
            &[
                &encode_points(attachment),
                &[encode_float(length)],
                flare,
                &[encode_float(0.001)],
                &[seed],
                &[RESOLUTION],
                &[CREATOR],
            ],
            128,
        );
        (strands, Mesh::decode(&mesh))
    }

    fn run(attachment: &[Point], flare: f32) -> (Vec<Vec<Vec3>>, Mesh) {
        run_with(attachment, LENGTH, &[encode_float(flare)], 3)
    }

    fn from_axis(p: Vec3) -> f32 {
        (p.x * p.x + p.z * p.z).sqrt()
    }

    fn length(strand: &[Vec3]) -> f32 {
        strand.windows(2).map(|w| w[0].distance(w[1])).sum()
    }

    #[test]
    fn hangs_around_the_stipe_from_a_point_on_it() {
        let (strands, mesh) = run(&[spot(0.0, RING_HEIGHT)], 0.25);
        assert_eq!(strands.len(), 128);
        for s in &strands {
            // grown out of the stipe where the point is, hanging down and out from it
            assert!((s[0].y - RING_HEIGHT).abs() < 1e-6);
            assert!((from_axis(s[0]) - (STIPE_RADIUS - 0.0005)).abs() < 1e-6);
            let end = s[s.len() - 1];
            assert!(end.y < RING_HEIGHT - 0.5 * LENGTH && from_axis(end) > from_axis(s[0]) + 0.1 * LENGTH);
            assert!((length(s) - LENGTH).abs() <= RAGGED * LENGTH + 1e-6);
        }
        assert_eq!(sheet_checks::open_edge(&mesh), None);
        assert!(sheet_checks::volume(&mesh) > 0.0);
    }

    #[test]
    fn hangs_on_the_inside_of_a_ring() {
        // flare 0 hangs straight down from the margin, 1 straight in towards the axis
        for s in &run(&margin(), 0.0).0 {
            assert!((from_axis(s[0]) - MARGIN_RADIUS).abs() < 1e-4);
            let end = s[s.len() - 1];
            assert!((from_axis(end) - from_axis(s[0])).abs() < 1e-5);
            assert!((s[0].y - end.y - length(s)).abs() < 1e-5);
        }
        for s in &run(&margin(), 1.0).0 {
            let end = s[s.len() - 1];
            assert!((end.y - MARGIN_HEIGHT).abs() < 1e-5);
            assert!((from_axis(s[0]) - from_axis(end) - length(s)).abs() < 1e-4);
        }
    }

    #[test]
    fn a_curve_drawing_the_old_flare_hangs_the_old_skirt() {
        // what a curve node sends: the curve read at alpha, from 0 to 1
        let alpha = concat_args(vec![&[2], &[0]]);
        let points: Vec<[i32; 1]> = OLD_FLARE.iter().map(|&v| [encode_float(v)]).collect();
        let (min, max) = ([encode_float(0.0)], [encode_float(1.0)]);
        let mut parts: Vec<&[i32]> = vec![&[3], &alpha, &min, &max];
        parts.extend(points.iter().map(|p| &p[..]));
        let flare = concat_args(parts);
        for s in &run_with(&[spot(0.0, RING_HEIGHT)], LENGTH, &flare, 3).0 {
            let out = (s[0] * Vec3::new(1.0, 0.0, 1.0)).normalize();
            for (j, w) in s.windows(2).enumerate() {
                // every step turns from straight down towards straight out by the old curve
                let along = (j as f32 + 0.5) / (s.len() - 1) as f32;
                let d = (w[1] - w[0]).normalize();
                let angle = d.dot(out).atan2(-d.y);
                assert!((angle - curve(&OLD_FLARE, along) * FRAC_PI_2).abs() < 0.01, "step {j} at {angle}");
            }
        }
    }

    #[test]
    fn the_edge_is_ragged_and_the_seed_sets_it() {
        let edge = |seed: i32| -> Vec<f32> {
            run_with(&[spot(0.0, RING_HEIGHT)], LENGTH, &[encode_float(0.25)], seed).0.iter().map(|s| length(s)).collect()
        };
        let (a, b) = (edge(3), edge(4));
        let mean = |v: &[f32]| v.iter().sum::<f32>() / v.len() as f32;
        // about as long for every seed, the edge goes up and down around it, also a little from
        // one strand to the next
        assert!((mean(&a) - LENGTH).abs() < 0.2 * LENGTH && (mean(&b) - LENGTH).abs() < 0.2 * LENGTH);
        let (lo, hi) = a.iter().fold((f32::INFINITY, 0.0f32), |(lo, hi), &l| (lo.min(l), hi.max(l)));
        assert!(hi - lo > 0.2 * LENGTH, "the edge only goes from {lo} to {hi}");
        assert!(a.windows(2).any(|w| (w[1] - w[0]).abs() > 1e-6));
        assert_ne!(a, b);
        assert_eq!(a, edge(3));
    }

    #[test]
    fn a_skirt_that_hangs_in_from_where_it_would_lie_folds() {
        // straight out it lies flat, hanging down from the stipe it has more skin than the circle
        // it hangs on and folds: its edge goes in and out around the stipe
        let spread = |flare: f32| {
            let mesh = run(&[spot(0.0, RING_HEIGHT)], flare).1;
            let radii: Vec<f32> = mesh.positions.chunks_exact(3).map(|v| from_axis(Vec3::from_slice(v))).collect();
            radii.iter().fold(0.0f32, |m, &r| m.max(r)) - radii.iter().fold(f32::INFINITY, |m, &r| m.min(r))
        };
        assert!(spread(0.0) > 0.5 * STIPE_RADIUS, "folds {}", spread(0.0));
    }

    #[test]
    fn the_skin_is_closed_and_thins_to_its_edge() {
        let (_, mesh) = run(&margin(), 0.3);
        assert!(mesh.face_count() > 0);
        assert_eq!(sheet_checks::open_edge(&mesh), None);
        assert!(sheet_checks::volume(&mesh) > 0.0);
        // front and back: as thick as thickness where it hangs from, thinner at its edge
        let vertex = |v: usize| Vec3::from_slice(&mesh.positions[v * 3..v * 3 + 3]);
        let rows = RESOLUTION as usize;
        let half = mesh.vertex_count() / 2;
        assert_eq!(half % rows, 0);
        let apart = |v: usize| vertex(v).distance(vertex(v + half));
        assert!((0..half).step_by(rows).all(|v| (apart(v) - 0.001).abs() < 1e-6));
        assert!((0..half).skip(rows - 1).step_by(rows).all(|v| (apart(v) - (1.0 - EDGE_THIN) * 0.001).abs() < 1e-6));
    }

    #[test]
    fn every_point_and_ring_hangs_its_own_skin() {
        let mut attachment = margin();
        attachment.extend([spot(0.0, RING_HEIGHT), spot(0.05, RING_HEIGHT)]);
        let (strands, _) = run(&attachment, 0.25);
        assert_eq!(strands.len(), 3 * 128);
        let starts_near = |x: f32| strands.iter().filter(|s| (Vec3::new(s[0].x - x, 0.0, s[0].z).length() - (STIPE_RADIUS - 0.0005)).abs() < 1e-5).count();
        assert_eq!(starts_near(0.0), 128);
        assert_eq!(starts_near(0.05), 128);
    }

    #[test]
    fn no_length_or_nothing_to_hang_from_gives_nothing() {
        assert_eq!(run_with(&[spot(0.0, RING_HEIGHT)], 0.0, &[encode_float(0.25)], 3).1.vertex_count(), 0);
        assert_eq!(run(&[], 0.25).1.vertex_count(), 0);
    }
}
