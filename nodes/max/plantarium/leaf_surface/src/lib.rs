use std::collections::HashMap;
use std::f32::consts::PI;

use glam::{Vec2, Vec3};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{
    concat_arg_vecs,
    geometry::{create_geometry_data, is_path, path_link, wrap_geometry_data, wrap_path},
    int, split_args, Ctx,
};

nodarium_definition_file!("src/definition.json");

/// The leaf node is 100 units long at size 1.
const LEAF_LENGTH: f32 = 100.0;

/// A piece of a vein in leaf space: x across the leaf, y along it. `h` is the height above the
/// leaf plane, `r` how far the blade reaches around the vein, both at the two ends.
struct Segment {
    a: Vec2,
    b: Vec2,
    ha: f32,
    hb: f32,
    ra: f32,
    rb: f32,
}

impl Segment {
    /// Where along the segment (0 to 1) the point closest to `p` lies, and how far it is.
    fn closest(&self, p: Vec2) -> (f32, f32) {
        let ab = self.b - self.a;
        let len2 = ab.length_squared();
        let t = if len2 > 0.0 {
            ((p - self.a).dot(ab) / len2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        (t, (self.a + ab * t - p).length())
    }
}

struct Vein {
    segments: Vec<Segment>,
    min: Vec2,
    max: Vec2,
    reach: f32,
}

impl Vein {
    /// No point of the blade around this vein is closer to `p` than this.
    fn lower_bound(&self, p: Vec2) -> f32 {
        let d = (self.min - p).max(p - self.max).max(Vec2::ZERO);
        d.length() - self.reach
    }
}

/// Moves the veins into leaf space: the base of the main veins at the origin, the leaf along +y.
/// The main veins are the paths whose parent is not among the veins.
fn leaf_space(paths: &[&[i32]]) -> Vec<Vein> {
    let ids: Vec<i32> = paths.iter().map(|p| path_link(p).id).collect();
    let roots: Vec<&[i32]> = paths
        .iter()
        .copied()
        .filter(|p| !ids.contains(&path_link(p).parent_id))
        .collect();

    let point = |p: &[f32], i: usize| Vec3::new(p[i * 4], p[i * 4 + 1], p[i * 4 + 2]);
    let (mut base, mut direction) = (Vec3::ZERO, Vec3::ZERO);
    for root in &roots {
        let path = wrap_path(root);
        base += point(path.points, 0);
        direction += point(path.points, path.length - 1) - point(path.points, 0);
    }
    base /= roots.len().max(1) as f32;
    let along = Vec3::new(direction.x, 0.0, direction.z)
        .try_normalize()
        .unwrap_or(Vec3::NEG_Z);
    let across = along.cross(Vec3::Y);

    paths
        .iter()
        .map(|p| {
            let path = wrap_path(p);
            let local: Vec<(Vec2, f32, f32)> = (0..path.length)
                .map(|i| {
                    let d = point(path.points, i) - base;
                    (
                        Vec2::new(d.dot(across), d.dot(along)),
                        d.y,
                        path.points[i * 4 + 3].max(0.0),
                    )
                })
                .collect();
            let segments: Vec<Segment> = local
                .windows(2)
                .map(|w| Segment {
                    a: w[0].0,
                    b: w[1].0,
                    ha: w[0].1,
                    hb: w[1].1,
                    ra: w[0].2,
                    rb: w[1].2,
                })
                .collect();
            let min = local.iter().fold(Vec2::MAX, |m, l| m.min(l.0));
            let max = local.iter().fold(Vec2::MIN, |m, l| m.max(l.0));
            let reach = local.iter().fold(0.0_f32, |m, l| m.max(l.2));
            Vein {
                segments,
                min,
                max,
                reach,
            }
        })
        .filter(|v| !v.segments.is_empty())
        .collect()
}

fn smin(a: f32, b: f32, k: f32) -> f32 {
    if k <= 0.0 {
        return a.min(b);
    }
    let h = (k - (a - b).abs()).max(0.0) / k;
    a.min(b) - h * h * k * 0.25
}

/// About the signed distance from `p` to the margin, negative on the blade: every vein is a
/// round cone as wide as its reach, a vein is joined to the others with a fillet of `k`.
fn blade(veins: &[Vein], p: Vec2, k: f32) -> f32 {
    let mut f = f32::MAX;
    for vein in veins {
        if vein.lower_bound(p) >= f + k {
            continue;
        }
        let mut d = f32::MAX;
        for s in &vein.segments {
            let (t, dist) = s.closest(p);
            d = d.min(dist - (s.ra + (s.rb - s.ra) * t));
        }
        f = smin(f, d, k);
    }
    f
}

struct Grid {
    nx: usize,
    ny: usize,
    origin: Vec2,
    cell: f32,
    values: Vec<f32>,
}

impl Grid {
    fn index(&self, (i, j): (usize, usize)) -> usize {
        j * (self.nx + 1) + i
    }

    fn point(&self, (i, j): (usize, usize)) -> Vec2 {
        self.origin + Vec2::new(i as f32, j as f32) * self.cell
    }

    fn inside(&self, corner: (usize, usize)) -> bool {
        self.values[self.index(corner)] < 0.0
    }

    fn horizontal_edges(&self) -> usize {
        self.nx * (self.ny + 1)
    }

    fn edge_count(&self) -> usize {
        self.horizontal_edges() + (self.nx + 1) * self.ny
    }

    /// The corners and edges of a cell, counter-clockwise from its lower left corner. Edge k
    /// goes from corner k to corner k + 1.
    fn cell(&self, i: usize, j: usize) -> ([(usize, usize); 4], [usize; 4]) {
        let h = self.horizontal_edges();
        (
            [(i, j), (i + 1, j), (i + 1, j + 1), (i, j + 1)],
            [
                j * self.nx + i,
                h + j * (self.nx + 1) + i + 1,
                (j + 1) * self.nx + i,
                h + j * (self.nx + 1) + i,
            ],
        )
    }

    fn edge_ends(&self, edge: usize) -> ((usize, usize), (usize, usize)) {
        let h = self.horizontal_edges();
        if edge < h {
            let (j, i) = (edge / self.nx, edge % self.nx);
            ((i, j), (i + 1, j))
        } else {
            let e = edge - h;
            let (j, i) = (e / (self.nx + 1), e % (self.nx + 1));
            ((i, j), (i, j + 1))
        }
    }

    /// Where the margin crosses an edge.
    fn crossing(&self, edge: usize) -> Vec2 {
        let (a, b) = self.edge_ends(edge);
        let (fa, fb) = (self.values[self.index(a)], self.values[self.index(b)]);
        let t = if fa != fb { fa / (fa - fb) } else { 0.5 };
        self.point(a).lerp(self.point(b), t.clamp(0.0, 1.0))
    }

    fn center_inside(&self, corners: &[(usize, usize); 4]) -> bool {
        corners.iter().map(|&c| self.values[self.index(c)]).sum::<f32>() < 0.0
    }

    /// A leaf has no holes: the gaps the veins close in on every side become blade.
    fn fill_holes(&mut self) {
        let mut outside = vec![false; self.values.len()];
        let mut stack: Vec<(usize, usize)> = Vec::new();
        for i in 0..=self.nx {
            stack.push((i, 0));
            stack.push((i, self.ny));
        }
        for j in 0..=self.ny {
            stack.push((0, j));
            stack.push((self.nx, j));
        }
        while let Some((i, j)) = stack.pop() {
            let index = self.index((i, j));
            if outside[index] || self.values[index] < 0.0 {
                continue;
            }
            outside[index] = true;
            if i > 0 {
                stack.push((i - 1, j));
            }
            if i < self.nx {
                stack.push((i + 1, j));
            }
            if j > 0 {
                stack.push((i, j - 1));
            }
            if j < self.ny {
                stack.push((i, j + 1));
            }
        }
        let inside = -0.5 * self.cell;
        for (value, outside) in self.values.iter_mut().zip(outside) {
            if !outside && *value >= 0.0 {
                *value = inside;
            }
        }
    }

    /// The pieces of the margin in every cell, as pairs of the edges they cross.
    fn contour_segments(&self) -> Vec<[usize; 2]> {
        let mut segments = Vec::new();
        for j in 0..self.ny {
            for i in 0..self.nx {
                let (corners, edges) = self.cell(i, j);
                let inside = corners.map(|c| self.inside(c));
                let crossed: Vec<usize> = (0..4)
                    .filter(|&k| inside[k] != inside[(k + 1) % 4])
                    .map(|k| edges[k])
                    .collect();
                match crossed.len() {
                    2 => segments.push([crossed[0], crossed[1]]),
                    4 if inside[0] == self.center_inside(&corners) => {
                        segments.push([edges[0], edges[1]]);
                        segments.push([edges[2], edges[3]]);
                    }
                    4 => {
                        segments.push([edges[3], edges[0]]);
                        segments.push([edges[1], edges[2]]);
                    }
                    _ => {}
                }
            }
        }
        segments
    }

    /// The closed lines of the margin.
    fn contour_loops(&self) -> Vec<Vec<Vec2>> {
        let segments = self.contour_segments();
        let mut by_edge: HashMap<usize, Vec<usize>> = HashMap::new();
        for (s, seg) in segments.iter().enumerate() {
            for &e in seg {
                by_edge.entry(e).or_default().push(s);
            }
        }
        let mut used = vec![false; segments.len()];
        let mut loops = Vec::new();
        for start in 0..segments.len() {
            if used[start] {
                continue;
            }
            used[start] = true;
            let mut edges = vec![segments[start][0]];
            let mut edge = segments[start][1];
            while edge != edges[0] {
                edges.push(edge);
                let next = by_edge[&edge].iter().copied().find(|&s| !used[s]);
                let Some(s) = next else { break };
                used[s] = true;
                edge = if segments[s][0] == edge {
                    segments[s][1]
                } else {
                    segments[s][0]
                };
            }
            loops.push(edges.into_iter().map(|e| self.crossing(e)).collect());
        }
        loops
    }

    /// Triangles over the part of the grid inside the margin, counter-clockwise.
    fn mesh(&self) -> (Vec<Vec2>, Vec<[u32; 3]>) {
        let mut positions = Vec::new();
        let mut faces = Vec::new();
        let mut corner_vertex = vec![u32::MAX; self.values.len()];
        let mut edge_vertex = vec![u32::MAX; self.edge_count()];

        #[derive(Clone, Copy)]
        enum V {
            Corner((usize, usize)),
            Edge(usize),
        }

        for j in 0..self.ny {
            for i in 0..self.nx {
                let (corners, edges) = self.cell(i, j);
                let inside = corners.map(|c| self.inside(c));
                if !inside.iter().any(|&x| x) {
                    continue;
                }
                let mut vertex = |v: V, positions: &mut Vec<Vec2>| -> u32 {
                    let (slot, p) = match v {
                        V::Corner(c) => (&mut corner_vertex[self.index(c)], self.point(c)),
                        V::Edge(e) => (&mut edge_vertex[e], self.crossing(e)),
                    };
                    if *slot == u32::MAX {
                        *slot = positions.len() as u32;
                        positions.push(p);
                    }
                    *slot
                };

                let crossings = (0..4).filter(|&k| inside[k] != inside[(k + 1) % 4]).count();
                if crossings == 4 && !self.center_inside(&corners) {
                    // a saddle with its middle outside: two separate corners
                    for k in (0..4).filter(|&k| inside[k]) {
                        let a = vertex(V::Edge(edges[(k + 3) % 4]), &mut positions);
                        let b = vertex(V::Corner(corners[k]), &mut positions);
                        let c = vertex(V::Edge(edges[k]), &mut positions);
                        faces.push([a, b, c]);
                    }
                    continue;
                }

                let mut polygon = Vec::with_capacity(6);
                for k in 0..4 {
                    if inside[k] {
                        polygon.push(vertex(V::Corner(corners[k]), &mut positions));
                    }
                    if inside[k] != inside[(k + 1) % 4] {
                        polygon.push(vertex(V::Edge(edges[k]), &mut positions));
                    }
                }
                for k in 1..polygon.len() - 1 {
                    faces.push([polygon[0], polygon[k], polygon[k + 1]]);
                }
            }
        }
        (positions, faces)
    }
}

/// Where every point of a closed margin sits between the base of the leaf (0) and its tip (1),
/// by the length of the margin on either side.
fn margin_alphas(margin: &[Vec2]) -> Vec<f32> {
    let n = margin.len();
    let mut alphas = vec![0.0; n];
    if n < 3 {
        return alphas;
    }
    let base = (0..n)
        .min_by(|&a, &b| margin[a].length().total_cmp(&margin[b].length()))
        .unwrap();
    let tip = (0..n)
        .max_by(|&a, &b| margin[a].y.total_cmp(&margin[b].y))
        .unwrap();
    for step in [1, n - 1] {
        let mut walked = vec![(base, 0.0)];
        let (mut i, mut length) = (base, 0.0);
        while i != tip {
            let next = (i + step) % n;
            length += (margin[next] - margin[i]).length();
            i = next;
            walked.push((i, length));
        }
        for (i, l) in walked {
            alphas[i] = if length > 0.0 { l / length } else { 0.0 };
        }
    }
    alphas
}

/// How far the margin is cut in at `p`, read from the closest piece of the margin.
fn cut_at(margin: &[Vec2], cuts: &[f32], p: Vec2) -> f32 {
    let n = margin.len();
    let mut best = (f32::MAX, 0.0);
    for i in 0..n {
        let s = Segment {
            a: margin[i],
            b: margin[(i + 1) % n],
            ha: 0.0,
            hb: 0.0,
            ra: 0.0,
            rb: 0.0,
        };
        let (t, d) = s.closest(p);
        if d < best.0 {
            best = (d, cuts[i] + (cuts[(i + 1) % n] - cuts[i]) * t);
        }
    }
    best.1
}

fn longest(loops: Vec<Vec<Vec2>>) -> Vec<Vec2> {
    let length = |l: &Vec<Vec2>| -> f32 {
        (0..l.len()).map(|i| (l[(i + 1) % l.len()] - l[i]).length()).sum()
    };
    loops
        .into_iter()
        .max_by(|a, b| length(a).total_cmp(&length(b)))
        .unwrap_or_default()
}

/// The blade as the union of the veins, meshed with marching squares.
fn field_mesh(
    ctx: &mut Ctx,
    veins: &[Vein],
    k: f32,
    margin_arg: &[i32],
    resolution: usize,
) -> (Vec<Vec2>, Vec<[u32; 3]>, f32) {
    let reach = veins.iter().fold(0.0_f32, |m, v| m.max(v.reach));
    let min = veins.iter().fold(Vec2::MAX, |m, v| m.min(v.min));
    let max = veins.iter().fold(Vec2::MIN, |m, v| m.max(v.max));
    let extent = (max - min).max_element() + 2.0 * reach;
    let cell = extent / resolution as f32;
    let pad = reach + k + 2.0 * cell;
    let origin = min - Vec2::splat(pad);
    let size = max - min + Vec2::splat(2.0 * pad);
    let nx = ((size.x / cell).ceil() as usize).clamp(2, 1024);
    let ny = ((size.y / cell).ceil() as usize).clamp(2, 1024);

    let mut grid = Grid {
        nx,
        ny,
        origin,
        cell,
        values: Vec::with_capacity((nx + 1) * (ny + 1)),
    };
    for j in 0..=ny {
        for i in 0..=nx {
            let p = grid.point((i, j));
            grid.values.push(blade(veins, p, k));
        }
    }
    grid.fill_holes();

    let margin = longest(grid.contour_loops());
    let alphas = margin_alphas(&margin);
    let cuts = ctx.float_along(margin_arg, alphas.iter().copied());
    let band = cuts.iter().fold(0.0_f32, |m, c| m.max(c.abs()));
    if band > 0.0 {
        let band = band + 2.0 * cell;
        for j in 0..=ny {
            for i in 0..=nx {
                let index = grid.index((i, j));
                if grid.values[index].abs() < band {
                    let p = grid.point((i, j));
                    grid.values[index] += cut_at(&margin, &cuts, p);
                }
            }
        }
        grid.fill_holes();
    }

    let (positions, faces) = grid.mesh();
    (positions, faces, cell)
}

/// The blade as the outline around the veins seen from the middle of the leaf, meshed in rings.
fn hull_mesh(
    ctx: &mut Ctx,
    veins: &[Vein],
    k: f32,
    margin_arg: &[i32],
    resolution: usize,
) -> (Vec<Vec2>, Vec<[u32; 3]>, f32) {
    let min = veins.iter().fold(Vec2::MAX, |m, v| m.min(v.min));
    let max = veins.iter().fold(Vec2::MIN, |m, v| m.max(v.max));
    let center = (min + max) * 0.5;
    let spokes = resolution * 2;
    let rings = (resolution / 6).max(2);
    let step = PI * 2.0 / spokes as f32;
    let sample = (max - min).max_element() / (resolution as f32 * 2.0);

    let mut radii = vec![0.0_f32; spokes];
    for vein in veins {
        for s in &vein.segments {
            let n = ((s.b - s.a).length() / sample).ceil().max(1.0) as usize;
            for q in 0..=n {
                let t = q as f32 / n as f32;
                let p = s.a.lerp(s.b, t) - center;
                let r = (s.ra + (s.rb - s.ra) * t).max(1e-4);
                let d = p.length();
                let phi = p.y.atan2(p.x);
                let half = if d <= r { PI } else { (r / d).asin() };
                let first = ((phi - half) / step).floor() as i64;
                let last = ((phi + half) / step).ceil() as i64;
                for b in first..=last {
                    let delta = b as f32 * step - phi;
                    let side = d * delta.sin();
                    if side.abs() > r {
                        continue;
                    }
                    let far = d * delta.cos() + (r * r - side * side).sqrt();
                    let slot = &mut radii[b.rem_euclid(spokes as i64) as usize];
                    *slot = slot.max(far);
                }
            }
        }
    }

    let mean = radii.iter().sum::<f32>() / spokes as f32;
    let window = ((k / (mean.max(1e-6) * step)).round() as usize).min(spokes / 4);
    if window > 0 {
        let source = radii.clone();
        for (b, r) in radii.iter_mut().enumerate() {
            let sum: f32 = (0..=2 * window)
                .map(|o| source[(b + spokes + o - window) % spokes])
                .sum();
            *r = sum / (2 * window + 1) as f32;
        }
    }

    let direction = |b: usize| Vec2::from_angle(b as f32 * step);
    let margin: Vec<Vec2> = (0..spokes).map(|b| center + direction(b) * radii[b]).collect();
    let alphas = margin_alphas(&margin);
    let cuts = ctx.float_along(margin_arg, alphas.iter().copied());
    for (r, c) in radii.iter_mut().zip(cuts) {
        *r = (*r - c).max(0.0);
    }

    let mut positions = vec![center];
    for ring in 1..=rings {
        let f = ring as f32 / rings as f32;
        for b in 0..spokes {
            positions.push(center + direction(b) * radii[b] * f);
        }
    }
    let at = |ring: usize, b: usize| (1 + (ring - 1) * spokes + b % spokes) as u32;
    let mut faces = Vec::new();
    for b in 0..spokes {
        faces.push([0, at(1, b), at(1, b + 1)]);
        for ring in 1..rings {
            faces.push([at(ring, b), at(ring + 1, b), at(ring + 1, b + 1)]);
            faces.push([at(ring, b), at(ring + 1, b + 1), at(ring, b + 1)]);
        }
    }
    (positions, faces, mean / rings as f32)
}

/// The height of the blade over `p`, blended from the veins close to it, and how far it lies
/// inside the nearest vein (1 on its middle, 0 at its edge and beyond).
fn heights(veins: &[Vein], p: Vec2, cell: f32) -> (f32, f32) {
    let (mut sum, mut weights, mut vein) = (0.0, 0.0, 0.0_f32);
    let eps = cell * cell;
    for v in veins {
        for s in &v.segments {
            let (t, d) = s.closest(p);
            let w = 1.0 / (d * d + eps).powi(2);
            sum += w * (s.ha + (s.hb - s.ha) * t);
            weights += w;
            let width = (0.15 * (s.ra + (s.rb - s.ra) * t)).max(1.5 * cell);
            vein = vein.max(1.0 - (d / width).min(1.0).powi(2));
        }
    }
    (if weights > 0.0 { sum / weights } else { 0.0 }, vein * vein)
}

fn build(ctx: &mut Ctx, args: &[&[i32]]) -> Vec<i32> {
    let paths: Vec<&[i32]> = split_args(args[0])
        .into_iter()
        .filter(|p| is_path(p))
        .collect();
    let veins = leaf_space(&paths);
    if veins.is_empty() {
        return concat_arg_vecs(vec![]);
    }
    let size = ctx.float(args[1]);
    let k = ctx.float(args[2]).max(0.0);
    let relief = ctx.float(args[4]);
    let resolution = int(args[6]).clamp(8, 512) as usize;

    let (flat, faces, cell) = match int(args[5]) {
        1 => hull_mesh(ctx, &veins, k, args[3], resolution),
        _ => field_mesh(ctx, &veins, k, args[3], resolution),
    };

    let bent = veins
        .iter()
        .flat_map(|v| &v.segments)
        .any(|s| s.ha.abs() > 1e-6 || s.hb.abs() > 1e-6);
    let scale = LEAF_LENGTH * size;
    let positions: Vec<Vec3> = flat
        .iter()
        .map(|&p| {
            let (h, vein) = if bent || relief != 0.0 {
                heights(&veins, p, cell)
            } else {
                (0.0, 0.0)
            };
            Vec3::new(p.x, h - relief * vein, -p.y) * scale
        })
        .collect();

    let mut normals = vec![Vec3::ZERO; positions.len()];
    for f in &faces {
        let [a, b, c] = f.map(|i| positions[i as usize]);
        let n = (b - a).cross(c - a);
        for &i in f {
            normals[i as usize] += n;
        }
    }

    let mut out = create_geometry_data(positions.len(), faces.len());
    let geometry = wrap_geometry_data(&mut out);
    for (i, f) in faces.iter().enumerate() {
        geometry.faces[i * 3..i * 3 + 3].copy_from_slice(&f.map(|v| v as i32));
    }
    for (i, (p, n)) in positions.iter().zip(&normals).enumerate() {
        geometry.positions[i * 3..i * 3 + 3].copy_from_slice(&p.to_array());
        let n = n.try_normalize().unwrap_or(Vec3::Y);
        geometry.normals[i * 3..i * 3 + 3].copy_from_slice(&n.to_array());
    }
    out
}

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let mut ctx = Ctx::default();
    build(&mut ctx, args)
}

#[cfg(test)]
mod tests;
