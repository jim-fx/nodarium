use glam::Vec3;
use myzel_core::{
    smooth_normals,
    veil::{add_skin, read_attachments, Attachment, Axis, Collar, Column, Skin, DOWN},
    Mesh,
};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{
    geometry::{orthogonal_to, path_id},
    int, Ctx, PathInfo,
};
use std::f32::consts::TAU;

nodarium_definition_file!("src/definition.json");

const MAX_STRANDS: i32 = 4096;
/// Strands across the skin. Neighbours touch when there are about as many as the longer
/// attachment is long in thicknesses.
const STRANDS: i32 = 256;

/// Where a strand across the skin is attached on b and on a
struct Anchor {
    b: Vec3,
    a: Vec3,
}

/// A ring of points in a around the collar of a spot in b
fn around(a: &Attachment, collar: &Collar, n: usize) -> Vec<Anchor> {
    let sorted = a.by_angle(collar.centre, collar.e1, collar.e2);
    (0..n)
        .map(|k| {
            let angle = TAU * k as f32 / n as f32;
            Anchor { b: collar.at(angle), a: a.at_angle(&sorted, angle).0 }
        })
        .collect()
}

/// A ring of points to a ring of points around the same axis, matched by their angle around it
fn ring_to_ring(a: &Attachment, b: &Attachment, n: usize) -> (Vec<Anchor>, Axis) {
    let axis = (b.centre(), a.axis().1);
    let e1 = orthogonal_to(axis.1);
    let e2 = axis.1.cross(e1);
    let (sorted_a, sorted_b) = (a.by_angle(axis.0, e1, e2), b.by_angle(axis.0, e1, e2));
    let anchors = (0..n)
        .map(|k| {
            let angle = TAU * k as f32 / n as f32;
            Anchor { b: b.at_angle(&sorted_b, angle).0, a: a.at_angle(&sorted_a, angle).0 }
        })
        .collect();
    (anchors, axis)
}

/// Two attachments joined from end to end, the strands spread evenly along both
fn end_to_end(a: &Attachment, b: &Attachment, n: usize) -> Vec<Anchor> {
    let (a0, a1, b0, b1) = (a.at(0.0).position, a.at(1.0).position, b.at(0.0).position, b.at(1.0).position);
    let reversed = a0.distance(b1) + a1.distance(b0) < a0.distance(b0) + a1.distance(b1);
    (0..n)
        .map(|k| {
            let u = if n == 1 { 0.5 } else { k as f32 / (n - 1) as f32 };
            Anchor { b: b.at(if reversed { 1.0 - u } else { u }).position, a: a.at(u).position }
        })
        .collect()
}

/// The attachment in b that `attachment` of a joins: the one closest to it. A ring in a goes
/// around the point of a row in b closest to its centre, unless that row is a ring around the
/// same axis.
fn partner(attachment: &Attachment, b: &[Attachment]) -> Option<(usize, Option<usize>)> {
    let centre = attachment.centre();
    let mut best: Option<(f32, usize, Option<usize>)> = None;
    let mut offer = |distance: f32, j: usize, spot: Option<usize>| {
        if best.is_none_or(|(d, ..)| distance < d) {
            best = Some((distance, j, spot));
        }
    };
    for (j, other) in b.iter().enumerate() {
        if attachment.closed && other.path.is_none() && !other.around_axis_of(attachment) {
            for (i, p) in other.points.iter().enumerate() {
                offer(p.distance(centre), j, Some(i));
            }
        } else {
            offer(other.closest(centre).0, j, None);
        }
    }
    best.map(|(_, j, spot)| (j, spot))
}

/// The skin strand by strand, from b to a, sagging by `sag` of the span in the middle
fn columns(anchors: &[Anchor], sag: f32, resolution: usize) -> Vec<Column> {
    anchors
        .iter()
        .map(|anchor| {
            let chord = anchor.a - anchor.b;
            let hang = chord.try_normalize().map_or(Vec3::ZERO, |c| (DOWN - c * DOWN.dot(c)).normalize_or_zero()) * sag * chord.length();
            let points = (0..resolution).map(|i| {
                let t = i as f32 / (resolution - 1) as f32;
                (anchor.b + chord * t + hang * (4.0 * t * (1.0 - t)), 0.0)
            });
            Some(points.collect())
        })
        .collect()
}

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    build(args, STRANDS).1
}

/// The strands across the skin and the skin. Only the skin is an output, the strands are what the
/// tests look at.
fn build(args: &[&[i32]], strand_count: i32) -> (Vec<Vec<Vec3>>, Vec<i32>) {
    let mut ctx = Ctx::default();
    let mut mesh = Mesh::default();
    let a = read_attachments(args[0]);
    let b = read_attachments(args[1]);
    let resolution = int(args[4]).max(4) as usize;
    // the graph node that makes the membrane, it is the last input
    let creator = int(args[5]);

    let pairs: Vec<(usize, usize, Option<usize>)> =
        a.iter().enumerate().filter_map(|(i, attachment)| partner(attachment, &b).map(|(j, spot)| (i, j, spot))).collect();

    let mut strands = Vec::new();
    for (rank, &(i, j, spot)) in pairs.iter().enumerate() {
        let (on_a, on_b) = (&a[i], &b[j]);
        match on_b.path {
            Some(path) => ctx.set_path(path, rank, pairs.len()),
            None => {
                let source = on_b.sources[spot.unwrap_or(0)];
                ctx.set_path_info(PathInfo {
                    index: rank,
                    rank,
                    count: pairs.len(),
                    depth: source.depth,
                    parent_position: source.alpha,
                    length: if spot.is_some() { 0.0 } else { on_b.length() },
                });
                ctx.seed_path(path_id(creator, source.id, rank as i32));
            }
        }
        let thickness = ctx.float(args[2]).max(0.0);
        let sag = ctx.float(args[3]).max(0.0);
        if thickness <= 0.0 {
            continue;
        }
        let n = strand_count.clamp(1, MAX_STRANDS) as usize;
        let inset = 0.5 * thickness;

        let (anchors, axis) = if let Some(i) = spot {
            let collar = Collar::new(&on_b.spot(i), inset);
            let axis = (collar.centre, collar.tangent);
            (around(on_a, &collar, n), Some(axis))
        } else if on_a.closed && on_b.path.is_some() {
            // level with a
            let collar = Collar::new(&on_b.at(on_b.closest(on_a.centre()).1), inset);
            let axis = (collar.centre, collar.tangent);
            (around(on_a, &collar, n), Some(axis))
        } else if on_a.closed && on_b.closed {
            let (anchors, axis) = ring_to_ring(on_a, on_b, n);
            (anchors, Some(axis))
        } else {
            (end_to_end(on_a, on_b, n), None)
        };
        let columns = columns(&anchors, sag, resolution);
        let attached: Vec<Vec3> = anchors.iter().map(|anchor| anchor.b).collect();
        add_skin(&mut mesh, &Skin { columns: &columns, attached: &attached, axis, torn: None }, thickness);
        strands.extend(columns.into_iter().flatten().map(|c| c.into_iter().map(|q| q.0).collect()));
    }
    smooth_normals(&mut mesh);

    (strands, mesh.encode())
}

#[cfg(test)]
mod tests {
    use super::build;
    use glam::Vec3;
    use myzel_core::{sheet_checks, Mesh};
    use nodarium_utils::{
        concat_arg_vecs, encode_float, encode_points,
        geometry::{create_multiple_paths, path_id, wrap_multiple_paths, wrap_path},
        split_args, Point,
    };
    use std::f32::consts::TAU;

    const CREATOR: i32 = 11;
    const STIPE_TOP: f32 = 0.05;
    const STIPE_RADIUS: f32 = 0.005;
    const MARGIN_RADIUS: f32 = 0.02;
    const MARGIN_HEIGHT: f32 = 0.04;
    const RESOLUTION: i32 = 16;

    /// Upright stipes `top` tall and 5 mm in radius, at these x
    fn stipes(xs: &[f32], top: f32) -> Vec<i32> {
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

    /// A point on a stipe like points_on_path gives it: at `alpha` of an upright stipe `top` tall
    fn spot(x: f32, alpha: f32, top: f32, tangent: Vec3, stipe: i32) -> Point {
        Point {
            position: Vec3::new(x, alpha * top, 0.0),
            direction: Vec3::X,
            tangent,
            scale: STIPE_RADIUS,
            parent_id: stipe,
            alpha,
            depth: 1,
            index: 0,
        }
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

    #[derive(Clone, Copy)]
    struct Params {
        thickness: f32,
        sag: f32,
        strands: i32,
    }

    const VEIL: Params = Params { thickness: 0.001, sag: 0.1, strands: 160 };

    fn run(a: &[i32], b: &[i32], p: Params) -> (Vec<Vec<Vec3>>, Mesh) {
        let (strands, mesh) =
            build(&[a, b, &[encode_float(p.thickness)], &[encode_float(p.sag)], &[RESOLUTION], &[CREATOR]], p.strands);
        (strands, Mesh::decode(&mesh))
    }

    /// The veil of one cap: its margin in a, a point at `alpha` of its stipe in b
    fn veil(alpha: f32, p: Params) -> (Vec<Vec<Vec3>>, Mesh) {
        let id = stipe_ids(&stipes(&[0.0], STIPE_TOP))[0];
        let margin = encode_points(&ring(0.0, MARGIN_RADIUS, MARGIN_HEIGHT, 96, id));
        run(&margin, &encode_points(&[spot(0.0, alpha, STIPE_TOP, Vec3::Y, id)]), p)
    }

    fn from_axis(p: Vec3) -> f32 {
        (p.x * p.x + p.z * p.z).sqrt()
    }

    #[test]
    fn closed_between_the_margin_and_the_point_on_the_stipe() {
        let p = VEIL;
        let (strands, mesh) = veil(0.6, p);
        assert_eq!(strands.len(), p.strands as usize);
        for s in &strands {
            // from the stipe, grown out of its surface, to the margin
            assert!((from_axis(s[0]) - (STIPE_RADIUS - 0.5 * p.thickness)).abs() < 1e-5);
            assert!((s[0].y - 0.6 * STIPE_TOP).abs() < 1e-5);
            let end = s[s.len() - 1];
            assert!((end.y - MARGIN_HEIGHT).abs() < 1e-5);
            assert!(from_axis(end) > MARGIN_RADIUS * 0.999 && from_axis(end) < MARGIN_RADIUS + 1e-6);
        }
        // neighbours touch all the way round
        for k in 0..strands.len() {
            let (s, t) = (&strands[k], &strands[(k + 1) % strands.len()]);
            let gap = s.iter().zip(t).map(|(a, b)| a.distance(*b)).fold(0.0, f32::max);
            assert!(gap <= p.thickness, "strands {} and {} are {} apart", k, k + 1, gap);
        }
        // one closed sheet that goes around, as thick as thickness all over
        assert!(mesh.face_count() > 0);
        assert_eq!(sheet_checks::open_edge(&mesh), None);
        assert!(sheet_checks::volume(&mesh) > 0.0);
        let normal = |v: usize| Vec3::from_slice(&mesh.normals[v * 3..v * 3 + 3]);
        assert!((0..mesh.vertex_count()).all(|v| (normal(v).length() - 1.0).abs() < 1e-4));
        let vertex = |v: usize| Vec3::from_slice(&mesh.positions[v * 3..v * 3 + 3]);
        let half = mesh.vertex_count() / 2;
        assert_eq!(half, p.strands as usize * RESOLUTION as usize);
        for v in 0..half {
            assert!((vertex(v).distance(vertex(v + half)) - p.thickness).abs() < 1e-3 * p.thickness);
        }
    }

    #[test]
    fn wraps_at_the_point_it_is_given_while_the_stipe_grows() {
        // a ring in the upper third, on stipes from 4 to 6 cm with the margin 5 mm under the top,
        // where points_on_path puts a point at 0.7
        for top in [0.04, 0.05, 0.06] {
            let id = stipe_ids(&stipes(&[0.0], top))[0];
            let margin = encode_points(&ring(0.0, MARGIN_RADIUS, top - 0.005, 96, id));
            let (strands, _) = run(&margin, &encode_points(&[spot(0.0, 0.7, top, Vec3::Y, id)]), VEIL);
            assert_eq!(strands.len(), VEIL.strands as usize);
            for s in &strands {
                assert!((s[0].y - 0.7 * top).abs() < 1e-5, "wraps at {} on a stipe {} tall", s[0].y, top);
                assert!((from_axis(s[0]) - (STIPE_RADIUS - 0.5 * VEIL.thickness)).abs() < 1e-5);
                assert!((s[s.len() - 1].y - (top - 0.005)).abs() < 1e-5);
            }
        }
    }

    #[test]
    fn wraps_across_the_tangent_of_the_point() {
        let id = stipe_ids(&stipes(&[0.0], STIPE_TOP))[0];
        let margin = encode_points(&ring(0.0, MARGIN_RADIUS, MARGIN_HEIGHT, 96, id));
        let tangent = Vec3::new(0.3, 1.0, 0.1).normalize();
        let at = spot(0.0, 0.6, STIPE_TOP, tangent, id);
        let (strands, _) = run(&margin, &encode_points(&[at]), VEIL);
        for s in &strands {
            let out = s[0] - at.position;
            assert!(out.dot(tangent).abs() < 1e-5);
            assert!((out.length() - (STIPE_RADIUS - 0.5 * VEIL.thickness)).abs() < 1e-5);
        }
    }

    #[test]
    fn the_skin_sags_between_the_attachments() {
        let flat = veil(0.6, Params { sag: 0.0, ..VEIL }).0;
        let sagging = veil(0.6, Params { sag: 0.2, ..VEIL }).0;
        let (a, b) = (&flat[0], &sagging[0]);
        let middle = a.len() / 2;
        let span = a[0].distance(a[a.len() - 1]);
        let drop = a[middle].distance(b[middle]);
        assert!((drop - 0.2 * span).abs() < 0.02 * span, "sags {} for a span of {}", drop, span);
        assert!(b[middle].y < a[middle].y);
    }

    #[test]
    fn around_a_path_it_wraps_level_with_a() {
        let stipe = stipes(&[0.0], STIPE_TOP);
        let margin = encode_points(&ring(0.0, MARGIN_RADIUS, MARGIN_HEIGHT, 96, stipe_ids(&stipe)[0]));
        let (strands, _) = run(&margin, &stipe, VEIL);
        assert_eq!(strands.len(), VEIL.strands as usize);
        for s in &strands {
            assert!((s[0].y - MARGIN_HEIGHT).abs() < 1e-5);
            assert!((from_axis(s[0]) - (STIPE_RADIUS - 0.5 * VEIL.thickness)).abs() < 1e-5);
        }
    }

    #[test]
    fn every_margin_wraps_the_point_on_its_own_stipe() {
        // two caps whose margins overlap, the second one higher, one point on every stipe
        let ids = stipe_ids(&stipes(&[0.0, 0.03], STIPE_TOP));
        let margins = encode_points(
            &[ring(0.0, MARGIN_RADIUS, MARGIN_HEIGHT, 96, ids[0]), ring(0.03, MARGIN_RADIUS, 0.045, 96, ids[1])].concat(),
        );
        let points = encode_points(&[spot(0.0, 0.7, STIPE_TOP, Vec3::Y, ids[0]), spot(0.03, 0.8, STIPE_TOP, Vec3::Y, ids[1])]);
        let p = Params { strands: 32, ..VEIL };
        let (strands, _) = run(&margins, &points, p);
        assert_eq!(strands.len(), 64);
        for (k, own) in strands.chunks(32).enumerate() {
            for s in own {
                let off_axis = Vec3::new(s[0].x - 0.03 * k as f32, 0.0, s[0].z).length();
                assert!((off_axis - (STIPE_RADIUS - 0.5 * p.thickness)).abs() < 1e-5);
                assert!((s[0].y - [0.7, 0.8][k] * STIPE_TOP).abs() < 1e-5);
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
        let (strands, mesh) = run(&a, &b, Params { strands: 21, sag: 0.0, ..VEIL });
        assert_eq!(strands.len(), 21);
        for (k, s) in strands.iter().enumerate() {
            let x = 0.1 * k as f32 / 20.0;
            // straight across, from the line in b up to the line in a
            assert!(s[0].distance(Vec3::new(x, 0.0, 0.0)) < 1e-5, "strand {} starts at {:?}", k, s[0]);
            assert!(s[s.len() - 1].distance(Vec3::new(x, 0.1, 0.0)) < 1e-5);
        }
        // one sheet that ends at their ends
        assert_eq!(sheet_checks::open_edge(&mesh), None);
        let xs = mesh.positions.chunks_exact(3).map(|v| v[0]);
        let (lo, hi) = xs.fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), x| (lo.min(x), hi.max(x)));
        assert!(lo > -1e-4 && hi < 0.1 + 1e-4, "the sheet runs from {lo} to {hi}");
    }

    #[test]
    fn a_ring_of_points_to_a_smaller_one() {
        let outer = ring(0.0, MARGIN_RADIUS, MARGIN_HEIGHT, 96, 1);
        let inner = ring(0.0, 0.008, 0.03, 48, 2);
        let (strands, _) = run(&encode_points(&outer), &encode_points(&inner), Params { strands: 64, ..VEIL });
        assert_eq!(strands.len(), 64);
        for (k, s) in strands.iter().enumerate() {
            let (start, end) = (s[0], s[s.len() - 1]);
            assert!((from_axis(start) - 0.008).abs() < 1e-4 && (start.y - 0.03).abs() < 1e-5);
            assert!((from_axis(end) - MARGIN_RADIUS).abs() < 1e-4 && (end.y - MARGIN_HEIGHT).abs() < 1e-5);
            // across, not twisted round the axis
            let turn = (start.z.atan2(start.x) - end.z.atan2(end.x)).abs();
            assert!(turn.min(TAU - turn) < 0.1, "strand {} turns {}", k, turn);
        }
    }

    #[test]
    fn nothing_to_attach_to_or_no_thickness_gives_nothing() {
        let id = stipe_ids(&stipes(&[0.0], STIPE_TOP))[0];
        let margin = encode_points(&ring(0.0, MARGIN_RADIUS, MARGIN_HEIGHT, 96, id));
        let point = encode_points(&[spot(0.0, 0.7, STIPE_TOP, Vec3::Y, id)]);
        let empty = [0, 1, 1, 1];
        assert_eq!(run(&margin, &empty, VEIL).1.vertex_count(), 0);
        assert_eq!(run(&empty, &point, VEIL).1.vertex_count(), 0);
        assert_eq!(run(&margin, &point, Params { thickness: 0.0, ..VEIL }).1.vertex_count(), 0);
    }

    #[test]
    fn the_same_inputs_give_the_same_skin() {
        assert_eq!(veil(0.7, VEIL).1, veil(0.7, VEIL).1);
    }
}
