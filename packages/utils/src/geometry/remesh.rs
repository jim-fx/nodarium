use std::collections::{HashMap, HashSet, VecDeque};
use std::f32::consts::TAU;

use glam::{Quat, Vec3};

use super::{create_geometry_data, set_geometry_path_id, wrap_geometry_data, PathData};

/// How far the hole in the parent reaches past the radius of the branch, as a part of that
/// radius. The band between the edge of the hole and the branch becomes the fillet.
const FILLET: f32 = 0.6;
/// A branch thinner than this part of the width of a quad of its parent would get a hole much
/// wider than itself, so it stays a separate tube.
const MIN_BRANCH_TO_QUAD: f32 = 0.35;
/// Branches that leave their parent at a smaller angle (sine) run inside it for too long.
const MIN_SIN_ANGLE: f32 = 0.2;
/// The fillet may not use more than this part of the length of the branch.
const MAX_FILLET_LENGTH: f32 = 0.5;
const MAX_EXTRA_RINGS: usize = 12;
const SMOOTH_ITERATIONS: usize = 3;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RemeshStats {
    pub joined: usize,
    pub separate: usize,
}

struct Mesh {
    positions: Vec<Vec3>,
    faces: Vec<[u32; 3]>,
    /// vertices of the fillets and the edges of the holes, only these are smoothed
    junction: Vec<u32>,
}

impl Mesh {
    fn vertex(&mut self, p: Vec3) -> u32 {
        self.positions.push(p);
        (self.positions.len() - 1) as u32
    }
    /// The quad between `l0 l1` (towards the root) and `u0 u1`, wound like `extrude_path`.
    fn quad(&mut self, l0: u32, l1: u32, u0: u32, u1: u32) {
        self.faces.push([u1, l1, l0]);
        self.faces.push([l0, u0, u1]);
    }
}

struct Ring {
    s: f32,
    center: Vec3,
    radius: f32,
    rotation: Quat,
}

impl Ring {
    fn point(&self, angle: f32) -> Vec3 {
        self.center
            + self.rotation * Vec3::new(angle.cos() * self.radius, 0.0, angle.sin() * self.radius)
    }
    fn u(&self) -> Vec3 {
        self.rotation * Vec3::X
    }
    fn v(&self) -> Vec3 {
        self.rotation * Vec3::Z
    }
}

struct Path<'a> {
    data: &'a PathData<'a>,
    /// arc length at every point
    cum: Vec<f32>,
}

impl Path<'_> {
    fn count(&self) -> usize {
        self.cum.len()
    }
    fn length(&self) -> f32 {
        *self.cum.last().unwrap()
    }
    fn point(&self, i: usize) -> Vec3 {
        Vec3::from_slice(&self.data.points[i * 4..i * 4 + 3])
    }
    fn thickness(&self, i: usize) -> f32 {
        self.data.points[i * 4 + 3]
    }
    fn segment_at(&self, s: f32) -> (usize, f32) {
        let n = self.count();
        for k in 0..n - 1 {
            if s <= self.cum[k + 1] || k == n - 2 {
                let len = self.cum[k + 1] - self.cum[k];
                let t = if len > 0.0 { ((s - self.cum[k]) / len).clamp(0.0, 1.0) } else { 0.0 };
                return (k, t);
            }
        }
        (0, 0.0)
    }
    fn sample(&self, s: f32) -> (Vec3, f32, Vec3) {
        let (k, t) = self.segment_at(s);
        let a = self.point(k);
        let b = self.point(k + 1);
        let pos = a.lerp(b, t);
        let thickness = self.thickness(k) + (self.thickness(k + 1) - self.thickness(k)) * t;
        (pos, thickness, (b - a).try_normalize().unwrap_or(Vec3::Y))
    }
    /// The direction `extrude_path` gives the rings at the points, it points towards the root.
    fn point_directions(&self) -> Vec<Vec3> {
        let n = self.count();
        let mut last = Vec3::NEG_Y;
        (0..n)
            .map(|i| {
                let d = if i == 0 {
                    self.point(0) - self.point(1)
                } else if i == n - 1 {
                    self.point(i - 1) - self.point(i)
                } else {
                    self.point(i - 1) - self.point(i + 1)
                };
                last = d.try_normalize().unwrap_or(last);
                last
            })
            .collect()
    }
    fn ring_at(&self, s: f32) -> Ring {
        let (center, thickness, forward) = self.sample(s);
        ring(s, center, thickness, -forward)
    }
    fn closest(&self, p: Vec3) -> (f32, f32) {
        let mut best = (f32::MAX, 0.0);
        for k in 0..self.count() - 1 {
            let a = self.point(k);
            let ab = self.point(k + 1) - a;
            let t = if ab.length_squared() > 0.0 {
                ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let d = (a + ab * t).distance(p);
            if d < best.0 {
                best = (d, self.cum[k] + t * ab.length());
            }
        }
        best
    }
}

fn ring(s: f32, center: Vec3, thickness: f32, backward: Vec3) -> Ring {
    Ring {
        s,
        center,
        radius: thickness.max(0.000001),
        rotation: Quat::from_rotation_arc(Vec3::NEG_Y, backward),
    }
}

/// A branch that will be joined into its parent, found while the parent is meshed.
struct Join {
    /// the edge of the hole in the parent, in the order of the angle around the branch
    hole: Vec<u32>,
    /// where along the branch the fillet ends and the tube of the branch starts
    start: f32,
}

struct Attachment {
    child: usize,
    s: f32,
    /// the part of the parent, as arc lengths, the hole can reach
    from: f32,
    to: f32,
    radius: f32,
    direction: Vec3,
    base: Vec3,
}

/// How far from the axis of the branch the hole reaches. Around the parent it can't grow past
/// its sides, so a thick branch gets a narrower fillet.
fn hole_radius(radius: f32, parent_radius: f32) -> f32 {
    radius + FILLET * radius.min(parent_radius - radius).max(0.0)
}

/// The quads cut out of the parent as (ring, column), the edge of the hole and where the tube of
/// the branch starts.
type Hole = (Vec<(usize, usize)>, Vec<u32>, f32);

fn angle_in(ring: &Ring, p: Vec3) -> f32 {
    let q = p - ring.center;
    q.dot(ring.v()).atan2(q.dot(ring.u()))
}

/// Meshes every tree of paths into one connected geometry: each branch thick enough for it is
/// stitched into a hole in its parent with a fillet, the others stay separate closed tubes in
/// the same geometry. Only branches up to `levels` below a root are joined, 1 joins the branches
/// that grow on the stems. One geometry per path that grows on its own, it carries its id.
pub fn remesh_paths(
    paths: &[PathData],
    res: usize,
    levels: usize,
) -> (Vec<Vec<i32>>, RemeshStats) {
    let mut stats = RemeshStats::default();
    let res = res.max(3);

    let paths: Vec<Path> = paths
        .iter()
        .filter(|p| p.length >= 2)
        .map(|data| {
            let mut cum = vec![0.0];
            for i in 1..data.length {
                let a = Vec3::from_slice(&data.points[(i - 1) * 4..][..3]);
                let b = Vec3::from_slice(&data.points[i * 4..][..3]);
                cum.push(cum[i - 1] + a.distance(b));
            }
            Path { data, cum }
        })
        .collect();

    let by_id: HashMap<i32, usize> = paths
        .iter()
        .enumerate()
        .map(|(i, p)| (p.data.link.id, i))
        .collect();
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); paths.len()];
    let mut roots = Vec::new();
    for (i, p) in paths.iter().enumerate() {
        match by_id.get(&p.data.link.parent_id) {
            Some(&parent) if p.data.link.has_parent() && parent != i => children[parent].push(i),
            _ => roots.push(i),
        }
    }

    let mut geometries = Vec::new();
    for root in roots {
        let mut mesh = Mesh {
            positions: Vec::new(),
            faces: Vec::new(),
            junction: Vec::new(),
        };
        let mut joins: HashMap<usize, Join> = HashMap::new();
        let mut queue = VecDeque::from([(root, 0)]);
        let mut seen = HashSet::new();
        while let Some((index, level)) = queue.pop_front() {
            if !seen.insert(index) {
                continue;
            }
            let join = joins.remove(&index);
            let joinable: &[usize] = if level < levels { &children[index] } else { &[] };
            let new_joins = mesh_path(&paths, index, joinable, join, res, &mut mesh);
            for child in &children[index] {
                if new_joins.iter().any(|(c, _)| c == child) {
                    stats.joined += 1;
                } else {
                    stats.separate += 1;
                }
            }
            joins.extend(new_joins);
            queue.extend(children[index].iter().map(|&c| (c, level + 1)));
        }
        smooth_junctions(&mut mesh);
        geometries.push(encode(&mesh, paths[root].data.link.id));
    }

    (geometries, stats)
}

fn attachment(parent: &Path, child: usize, path: &Path, res: usize) -> Option<Attachment> {
    let base = path.point(0);
    let direction = (path.point(1) - base).try_normalize()?;
    let (distance, s) = parent.closest(base);
    let (_, parent_radius, tangent) = parent.sample(s);
    let radius = path.thickness(0);
    let sin = direction.cross(tangent).length();
    let quad = TAU * parent_radius / res as f32;
    if distance > parent_radius * 0.5
        || sin < MIN_SIN_ANGLE
        || radius < quad * MIN_BRANCH_TO_QUAD
        || radius > parent_radius * 1.05
    {
        return None;
    }
    // the axis of the branch leaves the parent this far along it, the hole is centered there
    let center = s + parent_radius * direction.dot(tangent) / sin;
    let half = hole_radius(radius, parent_radius) / sin + quad * 0.5;
    Some(Attachment {
        child,
        s,
        from: center - half,
        to: center + half,
        radius,
        direction,
        base,
    })
}

/// The arc lengths of the rings of a path: its points from `start` on, and more rings around
/// the spots where branches attach, about as far apart as the quads are wide.
fn ring_positions(path: &Path, start: f32, attachments: &[Attachment], res: usize) -> Vec<(f32, Option<usize>)> {
    let mut rings: Vec<(f32, Option<usize>)> = Vec::new();
    if start > 0.0 {
        rings.push((start, None));
    }
    for (i, &s) in path.cum.iter().enumerate() {
        if start == 0.0 || s > start + 1e-6 {
            rings.push((s, Some(i)));
        }
    }
    let mut extra = Vec::new();
    for a in attachments {
        let (_, radius, _) = path.sample(a.s);
        let step = (TAU * radius / res as f32).max((a.to - a.from) / MAX_EXTRA_RINGS as f32);
        let count = ((a.to - a.from) / step).ceil() as i32;
        for k in 0..=count {
            let s = a.from + k as f32 * step;
            if s > start && s < path.length() {
                extra.push((s, step));
            }
        }
    }
    for (s, step) in extra {
        if rings.iter().all(|r| (r.0 - s).abs() > step * 0.3) {
            rings.push((s, None));
        }
    }
    rings.sort_by(|a, b| a.0.total_cmp(&b.0));
    rings
}

fn mesh_path(
    paths: &[Path],
    index: usize,
    children: &[usize],
    join: Option<Join>,
    res: usize,
    mesh: &mut Mesh,
) -> Vec<(usize, Join)> {
    let path = &paths[index];
    let start = join.as_ref().map_or(0.0, |j| j.start);

    let mut attachments: Vec<Attachment> = children
        .iter()
        .filter_map(|&c| attachment(path, c, &paths[c], res))
        .filter(|a| a.from.min(a.s) > start)
        .collect();
    attachments.sort_by(|a, b| b.radius.total_cmp(&a.radius));

    let directions = path.point_directions();
    let rings: Vec<Ring> = ring_positions(path, start, &attachments, res)
        .into_iter()
        .map(|(s, point)| match point {
            Some(i) => ring(s, path.point(i), path.thickness(i), directions[i]),
            None => path.ring_at(s),
        })
        .collect();

    let first = mesh.positions.len() as u32;
    for r in &rings {
        for j in 0..res {
            mesh.vertex(r.point(TAU * j as f32 / res as f32));
        }
    }
    let at = |ring: usize, col: usize| first + (ring * res + col % res) as u32;

    let mut removed: HashSet<(usize, usize)> = HashSet::new();
    let mut claimed: HashSet<(usize, usize)> = HashSet::new();
    let mut joins = Vec::new();
    for a in &attachments {
        if let Some((quads, hole, branch_start)) = cut_hole(mesh, &rings, a, &paths[a.child], res, first, &claimed) {
            for &(r, c) in &quads {
                removed.insert((r, c));
                for dr in -1i32..=1 {
                    for dc in -1i32..=1 {
                        let rr = r as i32 + dr;
                        if rr >= 0 {
                            claimed.insert((rr as usize, (c as i32 + dc).rem_euclid(res as i32) as usize));
                        }
                    }
                }
            }
            mesh.junction.extend(&hole);
            joins.push((
                a.child,
                Join {
                    hole,
                    start: branch_start,
                },
            ));
        }
    }

    for r in 0..rings.len() - 1 {
        for c in 0..res {
            if !removed.contains(&(r, c)) {
                mesh.quad(at(r, c), at(r, c + 1), at(r + 1, c), at(r + 1, c + 1));
            }
        }
    }

    let last = rings.len() - 1;
    let apex = mesh.vertex(path.point(path.count() - 1));
    for c in 0..res {
        mesh.faces.push([apex, at(last, c + 1), at(last, c)]);
    }

    match join {
        Some(join) => fillet(mesh, &join, &rings[0], paths, index, (0..res).map(|c| at(0, c)).collect()),
        None => {
            let base = mesh.vertex(rings[0].center);
            for c in 0..res {
                mesh.faces.push([base, at(0, c), at(0, c + 1)]);
            }
        }
    }

    joins
}

/// Picks the quads of the parent around the branch, as one interval of columns per row so the
/// edge of the hole is a single loop, and returns them with that loop ordered by the angle
/// around the branch and where the tube of the branch starts.
fn cut_hole(
    mesh: &Mesh,
    rings: &[Ring],
    a: &Attachment,
    child: &Path,
    res: usize,
    first: u32,
    claimed: &HashSet<(usize, usize)>,
) -> Option<Hole> {
    let pos = |r: usize, c: usize| mesh.positions[(first as usize) + r * res + c % res];
    let attach_ring = rings
        .iter()
        .enumerate()
        .min_by(|x, y| (x.1.s - a.s).abs().total_cmp(&(y.1.s - a.s).abs()))?
        .0;
    let center_col = (0..res)
        .max_by(|&x, &y| {
            let dx = (pos(attach_ring, x) - rings[attach_ring].center).dot(a.direction);
            let dy = (pos(attach_ring, y) - rings[attach_ring].center).dot(a.direction);
            dx.total_cmp(&dy)
        })
        .unwrap();
    let limit = hole_radius(a.radius, rings[attach_ring].radius);
    let half = res as i32 / 2;

    let mut rows: Vec<(usize, i32, i32)> = Vec::new();
    for r in 0..rings.len() - 1 {
        if rings[r + 1].s < a.from || rings[r].s > a.to {
            continue;
        }
        let mut interval: Option<(i32, i32)> = None;
        for o in -half..res as i32 - half {
            let c = (center_col as i32 + o).rem_euclid(res as i32) as usize;
            let center = (pos(r, c) + pos(r, c + 1) + pos(r + 1, c) + pos(r + 1, c + 1)) * 0.25;
            let q = center - a.base;
            let axial = q.dot(a.direction);
            if axial > 0.0 && (q - a.direction * axial).length() < limit {
                interval = Some(interval.map_or((o, o), |(lo, hi)| (lo.min(o), hi.max(o))));
            }
        }
        if let Some((lo, hi)) = interval {
            if let Some(&(prev, _, _)) = rows.last() {
                if prev + 1 != r {
                    return None;
                }
            }
            rows.push((r, lo, hi));
        }
    }
    let (first_row, last_row) = (rows.first()?.0, rows.last()?.0);
    if first_row == 0 || last_row + 2 >= rings.len() {
        return None;
    }
    for i in 1..rows.len() {
        let (lo, hi) = (rows[i].1, rows[i].2);
        let (plo, phi) = (rows[i - 1].1, rows[i - 1].2);
        if hi < plo {
            rows[i].2 = plo;
        }
        if lo > phi {
            rows[i].1 = phi;
        }
    }
    if rows.iter().any(|&(_, lo, hi)| hi - lo + 1 > res as i32 - 3) {
        return None;
    }

    let col = |o: i32| (center_col as i32 + o).rem_euclid(res as i32) as usize;
    let mut quads = Vec::new();
    for &(r, lo, hi) in &rows {
        for o in lo..=hi {
            if claimed.contains(&(r, col(o))) {
                return None;
            }
            quads.push((r, col(o)));
        }
    }

    // walk the edge of the hole: along the lowest ring, up the right side, back along the
    // highest ring and down the left side, in grid coordinates (ring, column offset)
    let mut walk: Vec<(usize, i32)> = Vec::new();
    let (r0, lo0, hi0) = rows[0];
    for o in lo0..=hi0 + 1 {
        walk.push((r0, o));
    }
    for i in 0..rows.len() {
        let (r, _, hi) = rows[i];
        walk.push((r + 1, hi + 1));
        if let Some(&(_, _, next_hi)) = rows.get(i + 1) {
            let step = if next_hi > hi { 1 } else { -1 };
            let mut o = hi + 1;
            while o != next_hi + 1 {
                o += step;
                walk.push((r + 1, o));
            }
        }
    }
    let (rn, lon, hin) = *rows.last().unwrap();
    for o in (lon..=hin).rev() {
        walk.push((rn + 1, o));
    }
    for i in (0..rows.len()).rev() {
        let (r, lo, _) = rows[i];
        walk.push((r, lo));
        if i > 0 {
            let prev_lo = rows[i - 1].1;
            let step = if prev_lo > lo { 1 } else { -1 };
            let mut o = lo;
            while o != prev_lo {
                o += step;
                walk.push((r, o));
            }
        }
    }
    walk.dedup();
    if walk.first() == walk.last() {
        walk.pop();
    }
    let mut hole: Vec<u32> = walk
        .iter()
        .map(|&(r, o)| first + (r * res + col(o)) as u32)
        .collect();
    if hole.iter().collect::<HashSet<_>>().len() != hole.len() {
        return None;
    }

    // the tube of the branch starts past the farthest point of the hole along the branch
    let farthest = hole
        .iter()
        .map(|&v| (mesh.positions[v as usize] - a.base).dot(a.direction))
        .fold(0.0_f32, f32::max);
    let branch_start = farthest + a.radius * 0.5;
    if branch_start > child.length() * MAX_FILLET_LENGTH {
        return None;
    }

    let start_ring = child.ring_at(branch_start);
    let mut angles: Vec<f32> = hole
        .iter()
        .map(|&v| angle_in(&start_ring, mesh.positions[v as usize]))
        .collect();
    let mut total = 0.0;
    for i in 0..angles.len() {
        total += wrap(angles[(i + 1) % angles.len()] - angles[i]);
    }
    if total < 0.0 {
        hole.reverse();
        angles.reverse();
    }
    // the corners of the staircase can turn back a little, the fillet evens that out
    let backwards: f32 = (0..angles.len())
        .map(|i| (-wrap(angles[(i + 1) % angles.len()] - angles[i])).max(0.0))
        .sum();
    if (total.abs() - TAU).abs() > 0.5 || backwards > std::f32::consts::FRAC_PI_4 {
        return None;
    }

    Some((quads, hole, branch_start))
}

fn wrap(a: f32) -> f32 {
    (a + std::f32::consts::PI).rem_euclid(TAU) - std::f32::consts::PI
}

/// Bridges the edge of the hole to the first ring of the branch: two rings of a curve that
/// leaves the parent surface towards the branch and ends along it, then a band that zips the
/// vertex counts of the hole and the ring together by their angle around the branch.
fn fillet(mesh: &mut Mesh, join: &Join, start: &Ring, paths: &[Path], index: usize, ring: Vec<u32>) {
    let path = &paths[index];
    let base = path.point(0);
    let direction = (path.point(1) - base).normalize();
    let hole: Vec<Vec3> = join.hole.iter().map(|&v| mesh.positions[v as usize]).collect();
    let mut angles: Vec<f32> = hole.iter().map(|&p| angle_in(start, p)).collect();
    for i in 1..angles.len() {
        angles[i] = angles[i - 1] + wrap(angles[i] - angles[i - 1]).max(0.0);
    }

    let mut lower = join.hole.clone();
    for t in [1.0 / 3.0, 2.0 / 3.0] {
        let upper: Vec<u32> = hole
            .iter()
            .zip(&angles)
            .map(|(&h, &angle)| {
                let on_branch = start.point(angle);
                let axial = (h - base).dot(direction);
                let control = on_branch + direction * (axial - (start.center - base).dot(direction));
                let p = h * (1.0 - t) * (1.0 - t) + control * 2.0 * (1.0 - t) * t + on_branch * t * t;
                mesh.vertex(p)
            })
            .collect();
        mesh.junction.extend(&upper);
        let n = lower.len();
        for k in 0..n {
            mesh.quad(lower[k], lower[(k + 1) % n], upper[k], upper[(k + 1) % n]);
        }
        lower = upper;
    }

    zip(mesh, &lower, &angles, &ring, ring.len());
}

/// Triangulates the band between two loops around the same axis, both ordered by increasing
/// angle, always advancing the loop whose next vertex comes first.
fn zip(mesh: &mut Mesh, lower: &[u32], lower_angles: &[f32], upper: &[u32], res: usize) {
    let m = lower.len();
    let start = (0..m)
        .min_by(|&a, &b| lower_angles[a].rem_euclid(TAU).total_cmp(&lower_angles[b].rem_euclid(TAU)))
        .unwrap();
    let mut la = Vec::with_capacity(m + 1);
    let mut acc = lower_angles[start];
    for i in 0..=m {
        if i > 0 {
            acc += wrap(lower_angles[(start + i) % m] - lower_angles[(start + i - 1) % m]).max(0.0);
        }
        la.push(acc);
    }
    let base = la[0];
    let step = TAU / res as f32;
    let first_upper = ((base.rem_euclid(TAU)) / step).round() as usize % res;
    let mut ua = Vec::with_capacity(res + 1);
    let offset = base - wrap(base - first_upper as f32 * step);
    for j in 0..=res {
        ua.push(offset + j as f32 * step);
    }

    let (mut i, mut j) = (0, 0);
    while i < m || j < res {
        let l = lower[(start + i) % m];
        let u = upper[(first_upper + j) % res];
        if j < res && (i == m || ua[j + 1] <= la[i + 1]) {
            mesh.faces.push([l, u, upper[(first_upper + j + 1) % res]]);
            j += 1;
        } else {
            mesh.faces.push([l, u, lower[(start + i + 1) % m]]);
            i += 1;
        }
    }
}

/// A few rounds of Laplacian smoothing on the vertices of the junctions, like the smoothing
/// pass of modular_tree, but only where the fillets are so the tubes keep their shape.
fn smooth_junctions(mesh: &mut Mesh) {
    if mesh.junction.is_empty() {
        return;
    }
    let junction: HashSet<u32> = mesh.junction.iter().copied().collect();
    let mut neighbours: HashMap<u32, Vec<u32>> = HashMap::new();
    for f in &mesh.faces {
        for k in 0..3 {
            let (a, b) = (f[k], f[(k + 1) % 3]);
            if junction.contains(&a) {
                let n = neighbours.entry(a).or_default();
                if !n.contains(&b) {
                    n.push(b);
                }
            }
            if junction.contains(&b) {
                let n = neighbours.entry(b).or_default();
                if !n.contains(&a) {
                    n.push(a);
                }
            }
        }
    }
    let mut order: Vec<u32> = junction.into_iter().collect();
    order.sort_unstable();
    for _ in 0..SMOOTH_ITERATIONS {
        let next: Vec<Vec3> = order
            .iter()
            .map(|v| {
                let n = &neighbours[v];
                let mean = n.iter().map(|&i| mesh.positions[i as usize]).sum::<Vec3>() / n.len() as f32;
                mesh.positions[*v as usize].lerp(mean, 0.5)
            })
            .collect();
        for (v, p) in order.iter().zip(next) {
            mesh.positions[*v as usize] = p;
        }
    }
}

fn encode(mesh: &Mesh, path_id: i32) -> Vec<i32> {
    let mut normals = vec![Vec3::ZERO; mesh.positions.len()];
    for f in &mesh.faces {
        let [a, b, c] = f.map(|i| mesh.positions[i as usize]);
        let n = (b - a).cross(c - a);
        for &i in f {
            normals[i as usize] += n;
        }
    }
    let mut data = create_geometry_data(mesh.positions.len(), mesh.faces.len());
    set_geometry_path_id(&mut data, path_id);
    let geometry = wrap_geometry_data(&mut data);
    for (i, p) in mesh.positions.iter().enumerate() {
        geometry.positions[i * 3..i * 3 + 3].copy_from_slice(&p.to_array());
        let n = normals[i].try_normalize().unwrap_or(Vec3::Y);
        geometry.normals[i * 3..i * 3 + 3].copy_from_slice(&n.to_array());
    }
    for (i, f) in mesh.faces.iter().enumerate() {
        geometry.faces[i * 3..i * 3 + 3].copy_from_slice(&f.map(|v| v as i32));
    }
    data
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{create_path, set_path_link, wrap_path, wrap_path_mut, PathLink};

    fn path(points: &[[f32; 4]], link: PathLink) -> Vec<i32> {
        let mut data = create_path(points.len(), 0);
        set_path_link(&mut data, link);
        let p = wrap_path_mut(&mut data);
        for (i, point) in points.iter().enumerate() {
            p.points[i * 4..i * 4 + 4].copy_from_slice(point);
        }
        data
    }

    fn stem() -> (Vec<i32>, PathLink) {
        let link = PathLink::root(1, 0);
        let points: Vec<[f32; 4]> = (0..6).map(|i| [0.0, i as f32 * 0.2, 0.0, 0.1]).collect();
        (path(&points, link), link)
    }

    fn branch(parent: PathLink, alpha: f32, direction: Vec3, radius: f32) -> Vec<i32> {
        let base = Vec3::new(0.0, alpha, 0.0);
        let points: Vec<[f32; 4]> = (0..6)
            .map(|i| {
                let p = base + direction.normalize() * i as f32 * 0.12;
                [p.x, p.y, p.z, radius * (1.0 - i as f32 * 0.15)]
            })
            .collect();
        path(&points, PathLink::child(2, parent.id, alpha, 0))
    }

    /// Edges with other than two faces, and edges two faces walk in the same direction.
    fn manifold_errors(geometry: &mut [i32]) -> (usize, usize) {
        let data = wrap_geometry_data(geometry);
        let mut undirected: HashMap<(i32, i32), usize> = HashMap::new();
        let mut directed: HashMap<(i32, i32), usize> = HashMap::new();
        for f in data.faces.chunks(3) {
            for k in 0..3 {
                let (a, b) = (f[k], f[(k + 1) % 3]);
                *undirected.entry((a.min(b), a.max(b))).or_default() += 1;
                *directed.entry((a, b)).or_default() += 1;
            }
        }
        (
            undirected.values().filter(|&&n| n != 2).count(),
            directed.values().filter(|&&n| n > 1).count(),
        )
    }

    fn signed_volume(geometry: &mut [i32]) -> f32 {
        let data = wrap_geometry_data(geometry);
        let p = |i: i32| Vec3::from_slice(&data.positions[i as usize * 3..][..3]);
        data.faces
            .chunks(3)
            .map(|f| p(f[0]).dot(p(f[1]).cross(p(f[2]))) / 6.0)
            .sum()
    }

    #[test]
    fn a_y_junction_is_one_closed_manifold() {
        let (stem, link) = stem();
        let branch = branch(link, 0.5, Vec3::new(1.0, 0.8, 0.0), 0.07);
        let paths = [wrap_path(&stem), wrap_path(&branch)];

        for res in [8, 12, 24] {
            let (mut geometries, stats) = remesh_paths(&paths, res, usize::MAX);
            assert_eq!(stats, RemeshStats { joined: 1, separate: 0 }, "res {res}");
            assert_eq!(geometries.len(), 1);
            assert_eq!(manifold_errors(&mut geometries[0]), (0, 0), "res {res}");
            assert!(signed_volume(&mut geometries[0]) > 0.0, "res {res}");
        }
    }

    #[test]
    fn opposite_branches_are_both_joined() {
        let (stem, link) = stem();
        let a = branch(link, 0.5, Vec3::new(1.0, 0.5, 0.0), 0.07);
        let mut b = branch(link, 0.5, Vec3::new(-1.0, 0.5, 0.0), 0.07);
        set_path_link(&mut b, PathLink::child(2, link.id, 0.5, 1));
        let paths = [wrap_path(&stem), wrap_path(&a), wrap_path(&b)];

        let (mut geometries, stats) = remesh_paths(&paths, 16, usize::MAX);
        assert_eq!(stats.joined, 2);
        assert_eq!(manifold_errors(&mut geometries[0]), (0, 0));
    }

    #[test]
    fn a_thin_twig_stays_a_separate_closed_tube() {
        let (stem, link) = stem();
        let twig = branch(link, 0.5, Vec3::new(1.0, 0.5, 0.0), 0.005);
        let paths = [wrap_path(&stem), wrap_path(&twig)];

        let (mut geometries, stats) = remesh_paths(&paths, 8, usize::MAX);
        assert_eq!(stats, RemeshStats { joined: 0, separate: 1 });
        assert_eq!(manifold_errors(&mut geometries[0]), (0, 0));
    }
}
