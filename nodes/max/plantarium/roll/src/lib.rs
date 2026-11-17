use glam::{Quat, Vec3, Vec4Swizzles};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{
    concat_arg_vecs,
    geometry::{
        interpolate_along_path, is_path, path_link, wrap_path, wrap_path_mut, DeepestPaths,
        PathStructure,
    },
    split_args, Ctx,
};
use std::f32::consts::TAU;

nodarium_definition_file!("src/definition.json");

/// At a share `u` of the way from the start of the coil to the tip, a path has turned by
/// `u^TIGHTEN` of the whole turn. 2 lets the curvature grow evenly from nothing at the start to
/// the most at the tip, so the coil is a spiral that winds in and joins the rest of the path
/// without a kink.
const TIGHTEN: f32 = 2.0;

/// Up to how much bend, in radians, a path counts as nearly straight. A path that bends more rolls
/// up in its own plane, a nearly straight one in a plane between its own and the fallback, so the
/// coil doesn't jump from side to side with every wiggle.
const STRAIGHT: f32 = 0.1;

/// How a path is rolled up: the segment whose middle lies `s` along the path turns around `axis`
/// by `angle_at(s)`.
#[derive(Debug, Clone, Copy)]
struct Coil {
    axis: Vec3,
    start: f32,
    length: f32,
    angle: f32,
}

impl Coil {
    fn new(points: &[Vec3], length: f32, roll: f32, turns: f32) -> Option<Self> {
        let rolled = length * roll;
        // the turn shrinks with the rolled part, so the curl at the tip stays while a path unrolls
        let angle = TAU * turns * roll;
        if rolled <= 0.0 || angle == 0.0 {
            return None;
        }
        Some(Self {
            axis: bend_axis(points),
            start: length - rolled,
            length: rolled,
            angle,
        })
    }

    fn angle_at(&self, s: f32) -> f32 {
        let u = ((s - self.start) / self.length).clamp(0.0, 1.0);
        self.angle * u.powf(TIGHTEN)
    }

    fn roll(&self, points: &mut [f32]) {
        let original: Vec<Vec3> = points.chunks_exact(4).map(Vec3::from_slice).collect();
        let mut along = 0.0;
        let mut at = original[0];
        for (i, pair) in original.windows(2).enumerate() {
            let segment = pair[1] - pair[0];
            let length = segment.length();
            at += Quat::from_axis_angle(self.axis, self.angle_at(along + 0.5 * length)) * segment;
            along += length;
            points[(i + 1) * 4..(i + 1) * 4 + 3].copy_from_slice(&at.to_array());
        }
    }
}

/// The axis a path bends around: the sum of the turns from each segment to the next, those near
/// the tip weigh more, so the coil goes on the way the end already bends. A straight path curls
/// down, as under its weight.
fn bend_axis(points: &[Vec3]) -> Vec3 {
    let directions: Vec<Vec3> =
        points.windows(2).filter_map(|w| (w[1] - w[0]).try_normalize()).collect();
    let last = directions.len().max(2) as f32 - 1.0;
    let bend: Vec3 = directions
        .windows(2)
        .enumerate()
        .map(|(i, d)| d[0].cross(d[1]) * ((i + 1) as f32 / last))
        .sum();
    let chord = (points[points.len() - 1] - points[0]).try_normalize().unwrap_or(Vec3::Y);
    let fallback = chord.cross(Vec3::NEG_Y).try_normalize().unwrap_or(Vec3::Z);
    let straight = (STRAIGHT - bend.length()).max(0.0);
    (bend + fallback * straight).try_normalize().unwrap_or(fallback)
}

/// What happened to a path: it turned by `turn` with the spot on its parent, then rolled up.
struct Motion {
    turn: Quat,
    coil: Option<Coil>,
    length: f32,
}

impl Motion {
    /// How far the path is turned at `alpha` of its length
    fn turn_at(&self, alpha: f32) -> Quat {
        match self.coil {
            Some(coil) => {
                Quat::from_axis_angle(coil.axis, coil.angle_at(alpha * self.length)) * self.turn
            }
            None => self.turn,
        }
    }
}

/// Moves the path at `index` with the spot on its parent it grows from, as if it was fixed there,
/// and returns how far it turned.
fn follow(
    structure: &PathStructure,
    original: &[Vec<i32>],
    current: &mut [Vec<i32>],
    motions: &[Option<Motion>],
    index: usize,
) -> Quat {
    let Some((parent, motion)) =
        structure.parent(index).and_then(|parent| Some((parent, motions[parent].as_ref()?)))
    else {
        return Quat::IDENTITY;
    };
    let alpha = path_link(&original[parent]).spot_or_tip(path_link(&original[index]).parent_alpha);
    let before = interpolate_along_path(wrap_path(&original[parent]).points, alpha).0.xyz();
    let after = interpolate_along_path(wrap_path(&current[parent]).points, alpha).0.xyz();
    let turn = motion.turn_at(alpha);
    for point in wrap_path_mut(&mut current[index]).points.chunks_exact_mut(4) {
        let moved = after + turn * (Vec3::from_slice(point) - before);
        point[..3].copy_from_slice(&moved.to_array());
    }
    turn
}

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let mut ctx = Ctx::default();
    let original: Vec<Vec<i32>> = split_args(args[0]).iter().map(|p| p.to_vec()).collect();
    let depth = ctx.int(args[3]).max(1);
    let selected = DeepestPaths::new(&original, depth);
    let count = selected.count(&original);

    let mut rank = 0;
    let settings: Vec<Option<(f32, f32)>> = original
        .iter()
        .map(|path| {
            if !selected.contains(path) {
                return None;
            }
            ctx.set_path(path, rank, count);
            rank += 1;
            Some((ctx.float(args[1]).clamp(0.0, 1.0), ctx.float(args[2])))
        })
        .collect();

    // parents come first, so a path rolls up after it moved with the spot it grows from
    let structure = PathStructure::new(&original);
    let mut current = original.clone();
    let mut motions: Vec<Option<Motion>> = original.iter().map(|_| None).collect();
    for &index in structure.order() {
        if !is_path(&original[index]) || wrap_path(&original[index]).length < 2 {
            continue;
        }
        let turn = follow(&structure, &original, &mut current, &motions, index);
        let path = wrap_path_mut(&mut current[index]);
        let length = path.get_length();
        let positions: Vec<Vec3> = path.points.chunks_exact(4).map(Vec3::from_slice).collect();
        let coil = settings[index].and_then(|(roll, turns)| Coil::new(&positions, length, roll, turns));
        if let Some(coil) = &coil {
            coil.roll(path.points);
        }
        if coil.is_some() || turn != Quat::IDENTITY {
            motions[index] = Some(Motion { turn, coil, length });
        }
    }
    concat_arg_vecs(current)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodarium_utils::{
        encode_float,
        geometry::{create_path, set_path_link, PathLink},
    };
    use std::f32::consts::PI;

    const CREATOR: i32 = 3;

    fn path(link: PathLink, depth: i32, points: &[Vec3]) -> Vec<i32> {
        let mut data = create_path(points.len(), depth);
        set_path_link(&mut data, link);
        let path = wrap_path_mut(&mut data);
        for (i, p) in points.iter().enumerate() {
            let thickness = 0.02 * (1.0 - i as f32 / points.len() as f32);
            path.points[i * 4..i * 4 + 4].copy_from_slice(&[p.x, p.y, p.z, thickness]);
        }
        data
    }

    /// A straight line of `count` points from `from`, `step` apart
    fn line(from: Vec3, step: Vec3, count: usize) -> Vec<Vec3> {
        (0..count).map(|i| from + step * i as f32).collect()
    }

    fn stem(points: &[Vec3]) -> Vec<i32> {
        path(PathLink::root(CREATOR, 0), 1, points)
    }

    fn run(paths: &[Vec<i32>], roll: f32, turns: f32, depth: i32) -> Vec<Vec<i32>> {
        let input = concat_arg_vecs(paths.to_vec());
        let out = __nodarium_execute_inner(&[
            &input,
            &[encode_float(roll)],
            &[encode_float(turns)],
            &[depth],
        ]);
        split_args(&out).iter().map(|p| p.to_vec()).collect()
    }

    fn points(path: &[i32]) -> Vec<Vec3> {
        wrap_path(path).points.chunks_exact(4).map(Vec3::from_slice).collect()
    }

    fn directions(path: &[i32]) -> Vec<Vec3> {
        points(path).windows(2).map(|w| (w[1] - w[0]).normalize()).collect()
    }

    #[test]
    fn no_roll_or_no_turns_change_nothing() {
        let input = vec![stem(&line(Vec3::ZERO, Vec3::Y * 0.1, 20))];
        assert_eq!(run(&input, 0.0, 2.0, 1), input);
        assert_eq!(run(&input, 0.7, 0.0, 1), input);
    }

    #[test]
    fn the_path_keeps_its_header_thickness_and_segment_lengths() {
        let input = vec![stem(&line(Vec3::ZERO, Vec3::Y * 0.1, 20))];
        let out = run(&input, 0.8, 2.5, 1);
        assert_eq!(out[0].len(), input[0].len());
        assert_eq!(path_link(&out[0]), path_link(&input[0]));
        assert_eq!(out[0][..10], input[0][..10]);
        let thickness = |p: &[i32]| wrap_path(p).points.chunks_exact(4).map(|q| q[3]).collect::<Vec<_>>();
        assert_eq!(thickness(&out[0]), thickness(&input[0]));
        for (a, b) in points(&input[0]).windows(2).zip(points(&out[0]).windows(2)) {
            assert!((a[0].distance(a[1]) - b[0].distance(b[1])).abs() < 1e-5);
        }
    }

    #[test]
    fn only_the_rolled_share_at_the_tip_turns_by_the_turns() {
        // 64 segments, the outer half rolls up by half a turn
        let input = vec![stem(&line(Vec3::ZERO, Vec3::X * 0.1, 65))];
        let out = run(&input, 0.5, 1.0, 1);
        let (before, after) = (points(&input[0]), points(&out[0]));
        assert!(before[..=32].iter().zip(&after[..=32]).all(|(a, b)| a.distance(*b) < 1e-6));
        let tip = *directions(&out[0]).last().unwrap();
        assert!(tip.angle_between(Vec3::NEG_X) < 0.1, "the tip points along {}", tip);
    }

    #[test]
    fn a_straight_path_curls_down_in_a_plane() {
        let lying = run(&[stem(&line(Vec3::ZERO, Vec3::X * 0.1, 33))], 1.0, 1.0, 1);
        let lying = points(&lying[0]);
        assert!(lying.iter().all(|p| p.z.abs() < 1e-5));
        assert!(lying[16].y < 0.0, "it curls down");

        let standing = run(&[stem(&line(Vec3::ZERO, Vec3::Y * 0.1, 33))], 1.0, 1.0, 1);
        assert!(points(&standing[0]).iter().all(|p| p.z.abs() < 1e-5));
    }

    #[test]
    fn the_coil_goes_on_the_way_the_path_bends() {
        // a quarter circle in the ground plane, from +x towards +z
        let arc: Vec<Vec3> = (0..33)
            .map(|i| {
                let a = 0.5 * PI * i as f32 / 32.0;
                Vec3::new(a.sin(), 0.0, 1.0 - a.cos())
            })
            .collect();
        let out = run(&[stem(&arc)], 0.6, 1.5, 1);
        let rolled = points(&out[0]);
        assert!(rolled.iter().all(|p| p.y.abs() < 1e-4), "the coil stays in the plane of the bend");
        let turn = |d: &[Vec3]| d.windows(2).map(|w| w[0].cross(w[1]).y).fold(0.0, |a, b| a + b);
        // in the plane of x and z a bend from +x to +z is a turn around -y
        assert!(turn(&directions(&out[0])) < turn(&directions(&stem(&arc))));
    }

    #[test]
    fn the_coil_gets_tighter_towards_the_tip() {
        let out = run(&[stem(&line(Vec3::ZERO, Vec3::X * 0.1, 65))], 0.5, 3.0, 1);
        let bends: Vec<f32> =
            directions(&out[0]).windows(2).map(|w| w[0].angle_between(w[1])).collect();
        for pair in bends[33..].windows(2) {
            assert!(pair[1] > pair[0], "the bends grow towards the tip: {:?}", &bends[32..]);
        }
    }

    #[test]
    fn unrolling_moves_the_path_a_little_at_a_time() {
        let input = vec![stem(&line(Vec3::ZERO, Vec3::Y * 0.05, 33))];
        let mut last = points(&input[0]);
        for step in 1..=100 {
            let now = points(&run(&input, step as f32 / 100.0, 2.5, 1)[0]);
            let moved = last.iter().zip(&now).map(|(a, b)| a.distance(*b)).fold(0.0, f32::max);
            // the path is 1.6 long
            assert!(moved < 0.06, "the path jumps by {} at roll {}", moved, step as f32 / 100.0);
            last = now;
        }
    }

    #[test]
    fn a_rolled_path_keeps_the_curl_of_its_tip() {
        let input = vec![stem(&line(Vec3::ZERO, Vec3::X * 0.05, 65))];
        let last_bend = |roll: f32| {
            let d = directions(&run(&input, roll, 2.0, 1)[0]);
            d[d.len() - 2].angle_between(d[d.len() - 1])
        };
        let (rolled, unrolling) = (last_bend(1.0), last_bend(0.5));
        assert!((rolled - unrolling).abs() < 0.15 * rolled, "{} then {}", rolled, unrolling);
    }

    #[test]
    fn a_child_rolls_up_with_the_spot_it_grows_on() {
        let frond = stem(&line(Vec3::ZERO, Vec3::X * 0.1, 33));
        let id = path_link(&frond).id;
        let spot = Vec3::new(2.4, 0.0, 0.0);
        let pinna = path(PathLink::child(CREATOR + 1, id, 0.75, 0), 2, &line(spot, Vec3::Y * 0.05, 9));
        let out = run(&[frond, pinna], 0.5, 1.0, 2);

        let on_frond = interpolate_along_path(wrap_path(&out[0]).points, 0.75);
        let base = points(&out[1])[0];
        assert!(base.distance(on_frond.0.xyz()) < 1e-4, "{} is off the spot {}", base, on_frond.0);
        // the pinna stands across the frond before and after, up to the bend of half a segment
        let across = directions(&out[1])[0];
        assert!((across.angle_between(on_frond.2) - 0.5 * PI).abs() < 0.1);
        assert_eq!(path_link(&out[1]).parent_id, id);
    }

    #[test]
    fn a_child_below_the_coil_stays() {
        let frond = stem(&line(Vec3::ZERO, Vec3::X * 0.1, 33));
        let pinna = path(
            PathLink::child(CREATOR + 1, path_link(&frond).id, 0.25, 0),
            2,
            &line(Vec3::new(0.8, 0.0, 0.0), Vec3::Z * 0.05, 9),
        );
        let out = run(&[frond, pinna.clone()], 0.5, 1.0, 2);
        // it only rolls up itself, as it would without the frond
        assert_eq!(out[1], run(&[pinna], 0.5, 1.0, 1)[0]);
    }

    #[test]
    fn a_bud_and_a_lone_point_pass() {
        let mut bud = stem(&line(Vec3::ZERO, Vec3::Y * 1e-6, 8));
        wrap_path_mut(&mut bud).set_grown(0.0);
        let point = path(PathLink::root(CREATOR, 1), 1, &[Vec3::ONE]);
        let out = run(&[bud, point.clone()], 1.0, 2.0, 1);
        assert_eq!(out[1], point);
        assert!(points(&out[0]).iter().all(|p| p.is_finite()));
    }
}
