use glam::{Vec2, Vec3};
use myzel_core::{Cap, CapFrame, CapRay};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{
    concat_arg_vecs, decode_points,
    geometry::{create_path, is_path, path_link, path_random, set_path_link, wrap_path, wrap_path_mut, PathLink},
    int, split_args, Ctx,
};
use std::f32::consts::{PI, TAU};

nodarium_definition_file!("src/definition.json");

/// Points on the turn from the stipe into the cap, and on a branch on its way to its own place
const TURN_SAMPLES: usize = 12;
/// A stipe strand that ends lower than this many stipe radii under the stipe top stays in the
/// stipe, it was shortened on purpose
const REACH_DEPTH: f32 = 1.0;
/// How far along the ray a branch takes to reach its own place, in radii of the ray
const BRANCH_REACH: f32 = 3.0;
/// How far round from the underside of a ray the rim strands at the edge of its wedge reach
const FAN: f32 = 0.75 * PI;
/// The core of the stipe turns tighter than its rim, or the top of the cap over the stipe would
/// stay empty. The tightest turn, as a share of the turn of the rim.
const CORE_TURN: f32 = 0.25;
const MIN_STRANDS_FOR_RIM: usize = 12;
const RIM_GAP: f32 = 0.8;
const GOLDEN_ANGLE: f32 = PI * (3.0 - 2.236_068);
const SALT: i32 = 0x464c_0000;

fn wrap_pi(a: f32) -> f32 {
    (a + PI).rem_euclid(TAU) - PI
}

/// Where across its ray (x to the top of the flesh, y to the side) a stipe strand runs, from
/// its distance to the axis as a share of the stipe radius and its angle beside the ray as a
/// share of the way to the next ray. The wedge of stipe a ray takes fans out from the top of
/// the ray: the core of the stipe runs along the top, its rim around the underside and up the
/// sides. A fan grows by the distance like the stipe does, so strands keep their spacing, and
/// neighbours on the stipe stay neighbours in the cap.
fn home_in_ray(rho: f32, lateral: f32) -> Vec2 {
    let rim = Vec2::new(-(FAN * lateral).cos(), (FAN * lateral).sin());
    Vec2::X + (rim - Vec2::X) * rho
}

/// The places of `n` strands across a ray as `strands` puts them: an even ring at the wall of the
/// tube, so its surface has no gaps, and a sunflower inside, turned by `turn`
fn places(n: usize, turn: f32) -> Vec<Vec2> {
    let spacing = (PI / n as f32).sqrt();
    let rim = if n >= MIN_STRANDS_FOR_RIM { ((TAU / spacing).round() as usize).min(n) } else { 0 };
    let outer = if rim > 0 { 1.0 - RIM_GAP * spacing } else { 1.0 };
    (0..n)
        .map(|k| {
            if k < rim {
                return Vec2::from_angle(turn + TAU * k as f32 / rim as f32);
            }
            let (j, inner) = (k - rim, n - rim);
            Vec2::from_angle(turn + j as f32 * GOLDEN_ANGLE) * outer * ((j as f32 + 0.5) / inner as f32).sqrt()
        })
        .collect()
}

/// A curve from `p0` leaving along `t0` to `p1` arriving along `t1`, with the tangents of a
/// circular arc through both ends, so the curve turns evenly
fn hermite(p0: Vec3, t0: Vec3, p1: Vec3, t1: Vec3, t: f32) -> Vec3 {
    let turn = t0.angle_between(t1).min(0.95 * PI);
    let half = (0.5 * turn).sin();
    let k = if half > 1e-4 { 2.0 * (0.25 * turn).tan() / half } else { 1.0 } * p0.distance(p1);
    let (t2, t3) = (t * t, t * t * t);
    p0 * (2.0 * t3 - 3.0 * t2 + 1.0) + t0 * k * (t3 - 2.0 * t2 + t) + p1 * (-2.0 * t3 + 3.0 * t2) + t1 * k * (t3 - t2)
}

fn direction(points: &[Vec3], i: usize, fallback: Vec3) -> Vec3 {
    let n = points.len();
    (points[(i + 1).min(n - 1)] - points[i.saturating_sub(1)]).try_normalize().unwrap_or(fallback)
}

/// The part of a cap ray outside the stipe, with a frame across it: `normal` points to the top
/// of the flesh, `across` to the next ray around the axis
struct Tube {
    id: i32,
    depth: i32,
    angle: f32,
    crosses: bool,
    /// Which of the cap paths it is, rays that fill_gaps added come after the ones they fork from
    path: usize,
    centre: Vec<Vec3>,
    radius: Vec<f32>,
    normal: Vec<Vec3>,
    across: Vec3,
    /// Distance from the axis
    x: Vec<f32>,
    along: Vec<f32>,
}

impl Tube {
    fn new(frame: &CapFrame, ray: &CapRay, item: &[i32]) -> Option<Tube> {
        let path = wrap_path(item);
        let link = path_link(item);
        // the root of a ray inside the stipe is left to the stipe strands
        let start = if ray.crosses { ray.plane.iter().position(|p| p.x >= 0.0).unwrap_or(0) } else { 0 };
        let points: Vec<&[f32]> = path.points.chunks_exact(4).skip(start).collect();
        if points.len() < 3 {
            return None;
        }
        let centre: Vec<Vec3> = points.iter().map(|p| Vec3::new(p[0], p[1], p[2])).collect();
        let radius = points.iter().map(|p| p[3].abs()).collect();
        let across = frame.axis.cross(frame.out(ray.angle));
        let normal = (0..centre.len())
            .map(|i| direction(&centre, i, frame.out(ray.angle)).cross(across).try_normalize().unwrap_or(frame.axis))
            .collect();
        let mut along = vec![0.0f32; centre.len()];
        for i in 1..centre.len() {
            along[i] = along[i - 1] + centre[i].distance(centre[i - 1]);
        }
        Some(Tube {
            id: link.id,
            depth: path.depth,
            angle: ray.angle,
            crosses: ray.crosses,
            path: ray.path,
            centre,
            radius,
            normal,
            across,
            x: ray.plane[start..].iter().map(|p| p.x).collect(),
            along,
        })
    }

    fn len(&self) -> usize {
        self.centre.len()
    }

    /// The point `home` across the tube at point `j`, for a strand of radius `r`
    fn at(&self, j: usize, home: Vec2, r: f32) -> Vec3 {
        self.centre[j] + (self.normal[j] * home.x + self.across * home.y) * (self.radius[j] - r).max(0.0)
    }

    /// A strand of radius `r` at point `from` thins where the tube thins after it
    fn thickness(&self, from: usize, r: f32, j: usize) -> f32 {
        let base = self.radius[from];
        let share = if base > 0.0 { (self.radius[j] / base).min(1.0) } else { 1.0 };
        (r * share).min(self.radius[j])
    }

    /// The part of a strand from point `from` to the tip, at `home` across the tube
    fn follow(&self, from: usize, home: Vec2, r: f32, points: &mut Vec<Vec3>, radius: &mut Vec<f32>) {
        for j in from..self.len() {
            let rj = self.thickness(from, r, j);
            points.push(self.at(j, home, rj));
            radius.push(rj);
        }
    }

    fn arrival(&self, j: usize, home: Vec2, r: f32) -> Vec3 {
        let (a, b) = (j.saturating_sub(1), (j + 1).min(self.len() - 1));
        (self.at(b, home, r) - self.at(a, home, r)).try_normalize().unwrap_or(self.normal[j].cross(self.across))
    }

    fn index_at(&self, along: f32) -> usize {
        self.along.partition_point(|&a| a < along).min(self.len() - 1)
    }
}

/// A strand in the cap: a turn or a branch first, then it follows its ray
struct Grown {
    points: Vec<Vec3>,
    radius: Vec<f32>,
    home: Vec2,
    alpha: f32,
}

/// A path from `points` and `radius` with the place in the plant of `link`
fn write_path(points: &[Vec3], radius: &[f32], depth: i32, link: PathLink) -> Vec<i32> {
    let mut data = create_path(points.len(), depth);
    set_path_link(&mut data, link);
    let path = wrap_path_mut(&mut data);
    for (i, (p, r)) in points.iter().zip(radius).enumerate() {
        path.points[i * 4..i * 4 + 4].copy_from_slice(&[p.x, p.y, p.z, *r]);
    }
    data
}

/// The stipe strands that continue into the rays nearest them around the axis. Strands from the
/// rim of the stipe run along the underside of the flesh, strands from the core along its top,
/// and their angle beside the ray decides how far to the side of it they run.
fn continue_stipe(
    frame: &CapFrame,
    tubes: &[Tube],
    reach: f32,
    stipe: &[&[i32]],
    taken: &mut [bool],
    stipe_out: &mut [Vec<i32>],
    grown: &mut [Vec<Grown>],
) {
    let roots: Vec<usize> = (0..tubes.len()).filter(|&i| tubes[i].crosses).collect();
    if roots.is_empty() {
        return;
    }
    let r_stipe = frame.stipe_radius.max(f32::EPSILON);
    let height = |p: Vec3| (p - frame.hinge).dot(frame.axis);
    for (s, item) in stipe.iter().enumerate() {
        if taken[s] || !is_path(item) || path_link(item).parent_id != frame.stipe_id {
            continue;
        }
        let path = wrap_path(item);
        let n = path.length;
        if n < 2 {
            continue;
        }
        let pts: Vec<Vec3> = path.points.chunks_exact(4).map(|p| Vec3::new(p[0], p[1], p[2])).collect();
        let rad: Vec<f32> = path.points.chunks_exact(4).map(|p| p[3].abs()).collect();
        let tip = pts[n - 1];
        if height(tip) < -REACH_DEPTH * r_stipe {
            continue;
        }
        let rho = ((tip - frame.hinge) - frame.axis * height(tip)).length();
        let phi = frame.angle(tip);
        let (k, delta) = roots
            .iter()
            .map(|&i| (i, wrap_pi(phi - tubes[i].angle)))
            .min_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
            .unwrap();
        // the gap to the next ray on the strand's side, the strands of a ray fill its half of it
        let gap = roots
            .iter()
            .filter(|&&i| i != k)
            .map(|&i| wrap_pi(tubes[i].angle - tubes[k].angle))
            .filter(|d| (*d >= 0.0) == (delta >= 0.0))
            .map(f32::abs)
            .fold(TAU, f32::min);
        let lateral = (delta / (0.5 * gap)).clamp(-1.0, 1.0);
        let share = (rho / r_stipe).min(1.0);
        let home = home_in_ray(share, lateral);
        let turn = reach * share.max(CORE_TURN);

        let tube = &tubes[k];
        let m = tube.len();
        let join = tube.x.iter().position(|&x| x >= rho + turn).unwrap_or(m - 2).clamp(1, m - 2);
        let r_tip = rad[n - 1];
        let r_join = tube.thickness(join, r_tip, join);
        let q = tube.at(join, home, r_join);
        let q_dir = tube.arrival(join, home, r_join);
        // the turn starts as far under the strand's place in the flesh as it runs out to there
        let bottom = height(q) - turn;
        let cut = height(tip) > bottom;
        let (keep, p, r_p) = if cut {
            let i = (0..n - 1).rev().find(|&i| height(pts[i]) <= bottom).unwrap_or(0);
            let (h0, h1) = (height(pts[i]), height(pts[i + 1]));
            let t = if h1 > h0 { ((bottom - h0) / (h1 - h0)).clamp(0.0, 1.0) } else { 0.0 };
            (i + 1, pts[i].lerp(pts[i + 1], t), rad[i] + (rad[i + 1] - rad[i]) * t)
        } else {
            (n - 1, tip, r_tip)
        };
        let p_dir = (p - pts[keep - 1]).try_normalize().unwrap_or_else(|| direction(&pts, keep - 1, frame.axis));
        if cut {
            let mut points = pts[..keep].to_vec();
            points.push(p);
            let mut radius = rad[..keep].to_vec();
            radius.push(r_p);
            stipe_out[s] = write_path(&points, &radius, path.depth, path.link);
        }

        let mut points = Vec::new();
        let mut radius = Vec::new();
        for i in 0..TURN_SAMPLES {
            let t = i as f32 / TURN_SAMPLES as f32;
            points.push(hermite(p, p_dir, q, q_dir, t));
            radius.push(r_p + (r_join - r_p) * t);
        }
        tube.follow(join, home, r_tip, &mut points, &mut radius);
        grown[k].push(Grown { points, radius, home, alpha: 0.0 });
        taken[s] = true;
    }
}

/// Where a branch forks: a point on a strand, its direction and radius there
struct Fork {
    point: Vec3,
    direction: Vec3,
    radius: f32,
    /// The point of the new strand's own ray it forks at
    at: usize,
}

/// The fork on the strand among `parents` that comes nearest to `home` at the start of the ray,
/// and the point of the ray it is next to
fn fork(tube: &Tube, parents: &[&[Grown]], home: Vec2) -> Option<Fork> {
    let target = tube.at(0, home, 0.0);
    let mut best: Option<(f32, &Grown, usize)> = None;
    for strand in parents.iter().flat_map(|g| g.iter()) {
        for (i, p) in strand.points.iter().enumerate() {
            let d = p.distance_squared(target);
            if best.as_ref().is_none_or(|b| d < b.0) {
                best = Some((d, strand, i));
            }
        }
    }
    let (_, strand, i) = best?;
    let point = strand.points[i];
    // the branch moves over to its place from the fork on, never back towards the axis
    let at = (0..tube.len()).min_by(|&a, &b| tube.centre[a].distance_squared(point).total_cmp(&tube.centre[b].distance_squared(point)))?;
    Some(Fork { point, direction: direction(&strand.points, i, tube.normal[at]), radius: strand.radius[i], at })
}

/// A strand that forks off at `fork` and moves over to `home` in its ray
fn branch(tube: &Tube, fork: &Fork, home: Vec2) -> Option<Grown> {
    let m = tube.len();
    if fork.at + 2 >= m {
        return None;
    }
    let end = tube.index_at(tube.along[fork.at] + BRANCH_REACH * tube.radius[fork.at]).clamp(fork.at + 1, m - 2);
    let r_end = tube.thickness(fork.at, fork.radius, end);
    let e = tube.at(end, home, r_end);
    let e_dir = tube.arrival(end, home, r_end);
    // a branch leaves its parent at an angle, half way between the parent and where it heads
    let leave = (fork.direction + (e - fork.point).normalize_or_zero()).try_normalize().unwrap_or(fork.direction);
    let mut points = Vec::new();
    let mut radius = Vec::new();
    for i in 0..TURN_SAMPLES {
        let t = i as f32 / TURN_SAMPLES as f32;
        points.push(hermite(fork.point, leave, e, e_dir, t));
        radius.push(fork.radius + (r_end - fork.radius) * t);
    }
    tube.follow(end, home, r_end, &mut points, &mut radius);
    let length = tube.along[m - 1];
    let alpha = if length > 0.0 { tube.along[fork.at] / length } else { 0.0 };
    Some(Grown { points, radius, home, alpha })
}

/// Fills every ray up to `amount` strands with branches. Rays that leave the stipe go first and
/// branch off their own stipe strands; the rays fill_gaps added, in the order it added them,
/// branch off the strands of the rays beside them.
fn branch_out(tubes: &[Tube], amount: usize, seed: i32, grown: &mut [Vec<Grown>]) {
    let mut order: Vec<usize> = (0..tubes.len()).collect();
    order.sort_by_key(|&i| (!tubes[i].crosses, tubes[i].path));
    let mut done = vec![false; tubes.len()];
    for &k in &order {
        let tube = &tubes[k];
        let have = grown[k].len();
        if have < amount {
            let mut free = places(amount, TAU * path_random(seed, tube.id, SALT));
            // how close every free place is to the strands already there
            let mut gaps: Vec<f32> = free
                .iter()
                .map(|p| grown[k].iter().map(|g| g.home.distance_squared(*p)).fold(f32::MAX, f32::min))
                .collect();
            let side = |sign: f32| {
                (0..tubes.len())
                    .filter(|&i| done[i] && i != k && !grown[i].is_empty())
                    .map(|i| (i, wrap_pi(tubes[i].angle - tube.angle) * sign))
                    .filter(|(_, d)| *d >= 0.0)
                    .min_by(|a, b| a.1.total_cmp(&b.1))
                    .map(|(i, _)| i)
            };
            let parents: Vec<&[Grown]> = if have > 0 {
                vec![grown[k].as_slice()]
            } else {
                [side(1.0), side(-1.0)].into_iter().flatten().map(|i| grown[i].as_slice()).collect()
            };
            let mut added = Vec::new();
            for _ in 0..amount - have {
                let pick = (0..free.len()).max_by(|&x, &y| gaps[x].total_cmp(&gaps[y])).unwrap();
                let home = free.swap_remove(pick);
                gaps.swap_remove(pick);
                for (gap, p) in gaps.iter_mut().zip(&free) {
                    *gap = gap.min(p.distance_squared(home));
                }
                if let Some(strand) = fork(tube, &parents, home).and_then(|f| branch(tube, &f, home)) {
                    added.push(strand);
                }
            }
            grown[k].extend(added);
        }
        done[k] = true;
    }
}

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> (Vec<i32>, Vec<i32>) {
    let mut ctx = Ctx::default();
    let stipe = split_args(args[0]);
    let cap_items = split_args(args[1]);
    let hinges = decode_points(args[2]);
    let seed = int(args[5]);
    // the graph node that makes the strands, it is the last input
    let creator = int(args[6]);

    let ray_items: Vec<usize> = (0..cap_items.len()).filter(|&i| is_path(cap_items[i])).collect();
    let rays: Vec<&[i32]> = ray_items.iter().map(|&i| cap_items[i]).collect();
    let mut stipe_out: Vec<Vec<i32>> = stipe.iter().map(|s| s.to_vec()).collect();
    let mut taken = vec![false; stipe.len()];
    let mut used = vec![false; cap_items.len()];
    let mut cap_out = Vec::new();

    let caps = Cap::all(&rays, &hinges);
    let count = caps.len();
    for (rank, cap) in caps.iter().enumerate() {
        for ray in &cap.rays {
            used[ray_items[ray.path]] = true;
        }
        ctx.set_path(rays[cap.rays[0].path], rank, count);
        ctx.seed_path(cap.frame.stipe_id);
        let amount = ctx.int(args[3]).max(1) as usize;
        let reach = ctx.float(args[4]).max(0.0) * cap.frame.stipe_radius;

        let tubes: Vec<Tube> = cap.rays.iter().filter_map(|ray| Tube::new(&cap.frame, ray, rays[ray.path])).collect();
        let mut grown: Vec<Vec<Grown>> = tubes.iter().map(|_| Vec::new()).collect();
        continue_stipe(&cap.frame, &tubes, reach, &stipe, &mut taken, &mut stipe_out, &mut grown);
        branch_out(&tubes, amount, seed, &mut grown);

        let mut by_path: Vec<usize> = (0..tubes.len()).collect();
        by_path.sort_by_key(|&i| tubes[i].path);
        for k in by_path {
            for (index, strand) in grown[k].iter().enumerate() {
                let link = PathLink::child(creator, tubes[k].id, strand.alpha, index as i32);
                cap_out.push(write_path(&strand.points, &strand.radius, tubes[k].depth + 1, link));
            }
        }
    }

    let unused = cap_items.iter().enumerate().filter(|(i, _)| !used[*i]).map(|(_, item)| item.to_vec());
    (concat_arg_vecs(stipe_out), concat_arg_vecs(unused.chain(cap_out).collect()))
}

#[cfg(test)]
mod tests {
    use super::{__nodarium_execute_inner, TURN_SAMPLES};
    use glam::{Vec2, Vec3};
    use myzel_core::testing::{cone, grow, TestCap, STIPE_RADIUS, STIPE_TOP};
    use nodarium_utils::{
        concat_arg_vecs, decode_points, encode_float,
        geometry::{create_path, path_link, set_path_link, wrap_path, wrap_path_mut, PathLink},
        split_args,
    };
    use std::f32::consts::PI;

    const CREATOR: i32 = 61;
    const STRAND: f32 = 0.0003;

    /// `count` straight strands up the stipe of the test cap, spread evenly over it like
    /// `strands` spreads them, the first at the axis and the last at the rim
    fn stipe_strands(stipe_id: i32, count: usize) -> Vec<Vec<i32>> {
        let golden = PI * (3.0 - 5f32.sqrt());
        (0..count)
            .map(|k| {
                let share = ((k as f32 + 0.5) / count as f32).sqrt();
                let at = Vec2::from_angle(k as f32 * golden) * (STIPE_RADIUS - STRAND) * share;
                let mut data = create_path(16, 2);
                set_path_link(&mut data, PathLink::child(9, stipe_id, 0.0, k as i32));
                let path = wrap_path_mut(&mut data);
                for i in 0..16 {
                    path.points[i * 4..i * 4 + 4].copy_from_slice(&[at.x, STIPE_TOP * i as f32 / 15.0, at.y, STRAND]);
                }
                data
            })
            .collect()
    }

    struct Out {
        rays: Vec<Vec<i32>>,
        stipe_in: Vec<Vec<i32>>,
        stipe: Vec<Vec<i32>>,
        cap: Vec<Vec<i32>>,
    }

    fn list(data: &[i32]) -> Vec<Vec<i32>> {
        split_args(data).iter().map(|s| s.to_vec()).collect()
    }

    /// The test cone with fewer rays added, so the tests stay quick
    fn cap() -> TestCap {
        TestCap { spacing: 2.0, ..cone() }
    }

    fn run(count: usize, amount: i32, bend: f32) -> Out {
        let (rays, hinges) = grow(&cap());
        let stipe_in = stipe_strands(decode_points(&hinges)[0].parent_id, count);
        let (stipe, cap) = __nodarium_execute_inner(&[
            &concat_arg_vecs(stipe_in.clone()),
            &concat_arg_vecs(rays.clone()),
            &hinges,
            &[amount],
            &[encode_float(bend)],
            &[3],
            &[CREATOR],
        ]);
        Out { rays, stipe_in, stipe: list(&stipe), cap: list(&cap) }
    }

    fn points(path: &[i32]) -> Vec<(Vec3, f32)> {
        wrap_path(path).points.chunks_exact(4).map(|p| (Vec3::new(p[0], p[1], p[2]), p[3])).collect()
    }

    fn positions(path: &[i32]) -> Vec<Vec3> {
        points(path).iter().map(|p| p.0).collect()
    }

    /// The largest turn between neighbouring steps, in degrees
    fn largest_turn(points: &[Vec3]) -> f32 {
        let steps: Vec<Vec3> = points.windows(2).map(|w| w[1] - w[0]).filter(|s| s.length() > 1e-7).collect();
        steps.windows(2).map(|w| w[0].angle_between(w[1]).to_degrees()).fold(0.0, f32::max)
    }

    /// The cap strand a stipe strand goes on as
    fn continued<'a>(out: &'a Out, stipe: &[i32]) -> &'a Vec<i32> {
        let end = *positions(stipe).last().unwrap();
        out.cap.iter().find(|s| positions(s)[0].distance(end) < 1e-6)
            .expect("a stipe strand ends without going on")
    }

    #[test]
    fn every_cap_ray_gets_its_strands() {
        let out = run(200, 30, 1.0);
        for ray in &out.rays {
            let id = path_link(ray).id;
            let strands = out.cap.iter().filter(|s| path_link(s).parent_id == id).count();
            assert!(strands >= 30, "ray {} has {} strands", id, strands);
        }
        assert!(out.cap.iter().all(|s| path_link(s).creator == CREATOR));
    }

    #[test]
    fn every_stipe_strand_continues_into_the_cap_without_a_kink() {
        let out = run(200, 30, 1.0);
        assert_eq!(out.stipe.len(), out.stipe_in.len());
        for (strand, before) in out.stipe.iter().zip(&out.stipe_in) {
            assert_eq!(path_link(strand), path_link(before));
            let whole: Vec<Vec3> =
                positions(strand).into_iter().chain(positions(continued(&out, strand)).into_iter().skip(1)).collect();
            let turn = largest_turn(&whole);
            assert!(turn < 25.0, "a strand turns {} degrees in one step", turn);
        }
    }

    #[test]
    fn branches_turn_smoothly_too() {
        let out = run(60, 40, 1.0);
        assert!(out.cap.len() > 3 * out.stipe_in.len(), "{} strands in the cap", out.cap.len());
        for strand in &out.cap {
            let turn = largest_turn(&positions(strand));
            assert!(turn < 30.0, "a cap strand turns {} degrees in one step", turn);
        }
    }

    #[test]
    fn strands_stay_inside_their_ray_after_the_turn() {
        let out = run(200, 30, 1.0);
        for strand in &out.cap {
            let parent = path_link(strand).parent_id;
            let ray = out.rays.iter().find(|r| path_link(r).id == parent).expect("every strand is a child of a ray");
            let tube = points(ray);
            for &(p, r) in points(strand).iter().skip(TURN_SAMPLES) {
                let room = tube.iter().map(|&(c, radius)| p.distance(c) + r - radius).fold(f32::MAX, f32::min);
                assert!(room <= 1e-6, "a strand reaches {} out of its ray", room);
            }
        }
    }

    #[test]
    fn the_core_of_the_stipe_runs_along_the_top_of_the_flesh() {
        let out = run(200, 30, 1.0);
        let height_in_flesh = |k: usize| {
            let cap = continued(&out, &out.stipe[k]);
            let ray = out.rays.iter().find(|r| path_link(r).id == path_link(cap).parent_id).unwrap();
            let p = positions(cap);
            let q = p[p.len() * 2 / 3];
            let c = positions(ray).into_iter().min_by(|a, b| a.distance(q).total_cmp(&b.distance(q))).unwrap();
            q.y - c.y
        };
        // the rim of a wedge goes round the underside of its ray and up its sides
        let core = (0..20).map(height_in_flesh).sum::<f32>() / 20.0;
        let rim = (150..200).map(height_in_flesh).sum::<f32>() / 50.0;
        assert!(core > 0.0 && rim < 0.0, "core {} rim {}", core, rim);
    }

    #[test]
    fn the_same_inputs_give_the_same_strands() {
        let (a, b) = (run(120, 30, 1.5), run(120, 30, 1.5));
        assert_eq!(a.cap, b.cap);
        assert_eq!(a.stipe, b.stipe);
    }

    #[test]
    fn without_a_cap_the_stipe_stays_as_it_is() {
        let (_, hinges) = grow(&cap());
        let input = concat_arg_vecs(stipe_strands(decode_points(&hinges)[0].parent_id, 50));
        let none = concat_arg_vecs(vec![]);
        let (stipe, cap) = __nodarium_execute_inner(&[&input, &none, &hinges, &[30], &[encode_float(1.0)], &[3], &[CREATOR]]);
        assert_eq!(split_args(&stipe), split_args(&input));
        assert!(split_args(&cap).is_empty());
    }

    #[test]
    fn strands_of_another_stipe_pass_through() {
        let (rays, hinges) = grow(&cap());
        let input = concat_arg_vecs(stipe_strands(12345, 20));
        let (stipe, _) =
            __nodarium_execute_inner(&[&input, &concat_arg_vecs(rays), &hinges, &[30], &[encode_float(1.0)], &[3], &[CREATOR]]);
        assert_eq!(split_args(&stipe), split_args(&input));
    }
}
