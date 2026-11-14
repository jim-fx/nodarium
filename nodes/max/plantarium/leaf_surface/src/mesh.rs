use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

use glam::Vec2;
use spade::handles::FixedUndirectedEdgeHandle;
use spade::{ConstrainedDelaunayTriangulation, Point2, PositionInTriangulation, Triangulation};

use crate::outline::Outline;
use crate::polygon::Polygon;
use crate::veins::{closest, groove, Vein};

/// The blade before it is bent: triangles in leaf space, counter-clockwise seen from above.
pub struct FlatMesh {
    pub positions: Vec<Vec2>,
    pub faces: Vec<[u32; 3]>,
}

/// How much longer an edge may get for every unit it lies away from the margin.
const GRADE: f32 = 0.5;

/// The smallest angle refinement aims for, in degrees.
const ANGLE: f64 = 22.0;

/// How often the bad triangles are collected and split.
const PASSES: usize = 12;

type Cdt = ConstrainedDelaunayTriangulation<Point2<f64>>;

/// How fine the mesh is, from the cell of the field.
pub struct Density {
    pub cell: f32,
    /// How far the margin may stray from the line of the field.
    pub tolerance: f32,
    /// The longest edge along the margin and at the ends of the veins.
    pub margin: f32,
    /// The longest edge in the middle of the blade.
    pub interior: f32,
}

impl Density {
    pub fn new(cell: f32) -> Density {
        Density {
            cell,
            tolerance: 0.15 * cell,
            margin: 2.5 * cell,
            interior: 8.0 * cell,
        }
    }

    /// The length of an edge `distance` away from the margin or the end of a vein.
    fn size(&self, distance: f32) -> f32 {
        (self.margin + GRADE * distance).min(self.interior)
    }

    /// How far from the margin the edges stop growing.
    fn reach(&self) -> f32 {
        (self.interior - self.margin) / GRADE
    }
}

/// The length of an edge at a point of the blade, from how far it lies from the margin and from
/// the ends of the veins.
fn sizes<'a, 'p>(
    margin: &'a Polygon<'p>,
    outline: &Outline,
    veins: &[Vein],
    density: &'a Density,
) -> impl Fn(Vec2) -> f32 + use<'a, 'p> {
    let ends: Vec<Vec2> = veins
        .iter()
        .map(|v| v.end())
        .filter(|&e| margin.contains(e))
        .chain(outline.tips.iter().flatten().copied())
        .collect();
    move |p: Vec2| {
        let end = ends.iter().map(|&e| (e - p).length_squared()).fold(f32::MAX, f32::min).sqrt();
        density.size(margin.distance_within(p, density.reach().min(end)))
    }
}

/// The pieces of the veins inside the outline as chains of points, spaced by the size of the mesh
/// there. A vein that branches from another starts on a point of its chain, and a vein whose tip
/// is on the margin ends there.
fn chains(
    outline: &Outline,
    margin: &Polygon,
    veins: &[Vein],
    density: &Density,
    size: &impl Fn(Vec2) -> f32,
) -> Vec<(usize, Vec<Vec2>)> {
    let clearance = 0.25 * density.margin;
    let step = 0.5 * density.cell;

    // the piece of every vein in the blade, by length
    let ranges: Vec<Option<(f32, f32)>> = veins
        .iter()
        .zip(&outline.tips)
        .map(|(vein, tip)| {
            let end = tip.map_or(vein.length(), |t| vein.project(t).0);
            let inside = |s: f32| {
                let p = vein.at(s).0;
                margin.contains(p) && (tip.is_some() || margin.distance_within(p, clearance) >= clearance)
            };
            let count = (end / step).ceil().max(1.0) as usize;
            let at = |k: usize| (k as f32 * step).min(end);
            let first = (0..=count).find(|&k| inside(at(k)))?;
            let last = (first..=count).take_while(|&k| inside(at(k))).last()?;
            Some((at(first), at(last)))
        })
        .collect();

    let mut breaks: Vec<Vec<f32>> = ranges.iter().map(|r| r.map_or(vec![], |(a, b)| vec![a, b])).collect();
    let mut starts: Vec<Option<Vec2>> = vec![None; veins.len()];
    for (i, vein) in veins.iter().enumerate() {
        let (Some(parent), Some((from, _))) = (vein.parent, ranges[i]) else {
            continue;
        };
        let Some((a, b)) = ranges[parent] else {
            continue;
        };
        let (s, d) = veins[parent].project(vein.at(from).0);
        if d < density.cell && s > a && s < b {
            breaks[parent].push(s);
            starts[i] = Some(veins[parent].at(s).0);
        }
    }

    let mut chains = Vec::new();
    for (i, vein) in veins.iter().enumerate() {
        let mut breaks = std::mem::take(&mut breaks[i]);
        if breaks.is_empty() {
            continue;
        }
        breaks.sort_by(f32::total_cmp);
        let mut points = vec![starts[i].unwrap_or(vein.at(breaks[0]).0)];
        for w in breaks.windows(2) {
            let (a, b) = (w[0], w[1]);
            if b - a < 1e-6 {
                continue;
            }
            // as many points as edges of the local size fit, spaced evenly in units of that size
            let n = ((b - a) / step).ceil().max(1.0) as usize;
            let mut walked = vec![0.0];
            for k in 0..n {
                let s = a + (k as f32 + 0.5) * (b - a) / n as f32;
                walked.push(walked[k] + (b - a) / n as f32 / size(vein.at(s).0));
            }
            let total = walked[n];
            let edges = total.round().max(1.0) as usize;
            for e in 1..edges {
                let target = total * e as f32 / edges as f32;
                let k = walked.iter().position(|&w| w >= target).unwrap_or(n).max(1);
                let t = (target - walked[k - 1]) / (walked[k] - walked[k - 1]).max(1e-9);
                points.push(vein.at(a + (k as f32 - 1.0 + t) * (b - a) / n as f32).0);
            }
            points.push(vein.at(b).0);
        }
        if let Some(tip) = outline.tips[i] {
            let last = points[points.len() - 1];
            if points.len() > 1 && last.distance(tip) < 0.5 * size(last) {
                points.pop();
            }
            points.push(tip);
        }
        if points.len() > 1 {
            chains.push((i, points));
        }
    }
    chains
}

/// Points beside the middle of every edge of the chains, where the groove `relief` sinks the vein
/// into ends, or a third of the edge away for a long edge. They keep the groove narrow in the
/// middle of the blade, where the edges are long, while the triangles between a chain and its
/// banks stay wide enough for refinement to leave them be.
fn banks(chains: &[(usize, Vec<Vec2>)], veins: &[Vein], margin: &Polygon, density: &Density) -> Vec<Vec2> {
    let near_other = |p: Vec2, own: usize, width: f32| {
        chains.iter().filter(|(v, _)| *v != own).any(|(_, chain)| {
            chain.windows(2).any(|w| closest(w[0], w[1], p).1 < width)
        })
    };
    let mut banks = Vec::new();
    for (vein, chain) in chains {
        for w in chain.windows(2) {
            let (a, b) = (w[0], w[1]);
            let middle = (a + b) * 0.5;
            let length = (b - a).length();
            let width = groove(veins[*vein].at(veins[*vein].project(middle).0).2, density.cell).max(0.3 * length);
            if length < width {
                continue;
            }
            let side = (b - a).normalize().perp() * width;
            for bank in [middle + side, middle - side] {
                if margin.contains(bank)
                    && margin.distance_within(bank, width) >= width
                    && !near_other(bank, *vein, width)
                {
                    banks.push(bank);
                }
            }
        }
    }
    banks
}

/// Triangles inside the outline whose edges follow the veins: a constrained Delaunay
/// triangulation of the margin and the chains of the veins, refined until no angle is sharper than
/// `ANGLE` and no triangle larger than the interior size allows.
pub fn vein_mesh(outline: &Outline, veins: &[Vein], density: &Density) -> FlatMesh {
    let empty = FlatMesh {
        positions: vec![],
        faces: vec![],
    };
    if outline.points.len() < 3 {
        return empty;
    }
    let point = |p: Vec2| Point2::new(p.x as f64, p.y as f64);
    let mut cdt = Cdt::new();
    let insert = |cdt: &mut Cdt, line: &[Vec2], closed: bool| {
        let Ok(handles) = line.iter().map(|&p| cdt.insert(point(p))).collect::<Result<Vec<_>, _>>() else {
            return;
        };
        let n = handles.len();
        for k in 0..if closed { n } else { n - 1 } {
            let (a, b) = (handles[k], handles[(k + 1) % n]);
            if a != b {
                cdt.add_constraint_and_split(a, b, |p| p);
            }
        }
    };
    let margin = Polygon::new(&outline.points);
    let size = sizes(&margin, outline, veins, density);
    insert(&mut cdt, &outline.points, true);
    let chains = chains(outline, &margin, veins, density, &size);
    for (_, chain) in &chains {
        insert(&mut cdt, chain, false);
    }
    for bank in banks(&chains, veins, &margin, density) {
        let _ = cdt.insert(point(bank));
    }
    refine(&mut cdt, density, size);

    let mut index = vec![u32::MAX; cdt.num_vertices()];
    let mut positions = Vec::new();
    let mut faces = Vec::new();
    let outside = outside(&cdt);
    for face in cdt.inner_faces() {
        if outside[face.fix().index()] {
            continue;
        }
        let corners = face.vertices().map(|v| {
            let slot = &mut index[v.fix().index()];
            if *slot == u32::MAX {
                *slot = positions.len() as u32;
                let p = v.position();
                positions.push(Vec2::new(p.x as f32, p.y as f32));
            }
            *slot
        });
        faces.push(corners);
    }
    if faces.is_empty() {
        return empty;
    }
    FlatMesh { positions, faces }
}

fn vec2(p: Point2<f64>) -> Vec2 {
    Vec2::new(p.x as f32, p.y as f32)
}

fn centroid([a, b, c]: [Point2<f64>; 3]) -> Point2<f64> {
    Point2::new((a.x + b.x + c.x) / 3.0, (a.y + b.y + c.y) / 3.0)
}

fn length(a: Point2<f64>, b: Point2<f64>) -> f64 {
    ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt()
}

/// Whether `p` lies in the circle with the edge from `a` to `b` as its diameter.
fn encroaches(a: Point2<f64>, b: Point2<f64>, p: Point2<f64>) -> bool {
    (a.x - p.x) * (b.x - p.x) + (a.y - p.y) * (b.y - p.y) < 0.0
}

/// The faces outside the margin, by index: those reached from the convex hull without crossing a
/// constraint edge. The veins lie inside, so only the margin stops the way.
fn outside(cdt: &Cdt) -> Vec<bool> {
    let mut outside = vec![false; cdt.num_all_faces()];
    let mut todo: Vec<_> = cdt.convex_hull().filter(|e| !e.is_constraint_edge()).map(|e| e.rev()).collect();
    while let Some(edge) = todo.pop() {
        let Some(face) = edge.face().as_inner() else {
            continue;
        };
        if std::mem::replace(&mut outside[face.fix().index()], true) {
            continue;
        }
        for next in face.adjacent_edges() {
            if !next.is_constraint_edge() {
                todo.push(next.rev());
            }
        }
    }
    outside
}

/// A triangle refinement splits: how skinny it is, its circumradius, its centroid and its
/// circumcenter.
type Bad = (f64, f64, Point2<f64>, Point2<f64>);

/// Hashes the corners of a face, packed into one word.
#[derive(Default)]
struct Corners(u64);

impl Hasher for Corners {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.write_u64(b as u64);
        }
    }

    fn write_u64(&mut self, i: u64) {
        self.0 = (self.0.rotate_left(5) ^ i).wrapping_mul(0x517c_c1b7_2722_0a95);
    }
}

/// Splits a constraint edge in the middle into two.
fn split(cdt: &mut Cdt, edge: FixedUndirectedEdgeHandle) {
    let e = cdt.undirected_edge(edge);
    let [a, b] = e.vertices().map(|v| v.fix());
    let [pa, pb] = e.positions();
    cdt.remove_constraint_edge(edge);
    let Ok(m) = cdt.insert(Point2::new((pa.x + pb.x) * 0.5, (pa.y + pb.y) * 0.5)) else {
        cdt.add_constraint_and_split(a, b, |p| p);
        return;
    };
    cdt.add_constraint_and_split(a, m, |p| p);
    cdt.add_constraint_and_split(m, b, |p| p);
}

/// Delaunay refinement after Ruppert: a triangle in the outline that is larger than `size` allows
/// or has an angle sharper than `ANGLE` gets a point at the middle of its circumcircle, unless that
/// point would come too close to a vein or the margin, which is then split in the middle instead.
fn refine(cdt: &mut Cdt, density: &Density, size: impl Fn(Vec2) -> f32) {
    let ratio = 1.0 / (2.0 * ANGLE.to_radians().sin());
    let shortest = 0.3 * density.cell as f64;
    let limit = cdt.num_vertices() * 6 + 1000;
    // vertices never move, so a face with the same corners as in the pass before is as bad
    let mut seen: HashMap<u64, Option<Bad>, BuildHasherDefault<Corners>> = HashMap::default();
    for _ in 0..PASSES {
        let mut bad = Vec::new();
        let outside = outside(cdt);
        for face in cdt.inner_faces() {
            if outside[face.fix().index()] {
                continue;
            }
            let [u, v, w] = face.vertices().map(|v| v.fix().index() as u64);
            let found = *seen.entry(u | v << 21 | w << 42).or_insert_with(|| {
                let corners = face.positions();
                let middle = centroid(corners);
                let [a, b, c] = corners;
                let sides = [length(b, c), length(c, a), length(a, b)];
                let short = sides.iter().copied().fold(f64::MAX, f64::min);
                let doubled = ((b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)).abs();
                if doubled <= 0.0 {
                    return None;
                }
                let radius = sides[0] * sides[1] * sides[2] / (2.0 * doubled);
                let skinny = radius / short > ratio && short > shortest;
                // about the edge of an equilateral triangle with this circumcircle
                let edge = radius / 0.6;
                (skinny || (edge > density.margin as f64 && edge > size(vec2(middle)) as f64))
                    .then(|| (radius / short, radius, middle, face.circumcenter()))
            });
            bad.extend(found);
        }
        if bad.is_empty() {
            return;
        }
        bad.sort_by(|a, b| b.0.total_cmp(&a.0));
        for (_, radius, from, center) in bad {
            if cdt.num_vertices() > limit {
                return;
            }
            let mut encroached: Vec<FixedUndirectedEdgeHandle> = cdt
                .get_conflicting_edges_between_points(from, center)
                .map(|e| e.as_undirected().fix())
                .take(1)
                .collect();
            let face = match cdt.locate(center) {
                PositionInTriangulation::OnFace(face) => Some(face),
                PositionInTriangulation::OnEdge(edge) => cdt.directed_edge(edge).face().as_inner().map(|f| f.fix()),
                _ => None,
            };
            let Some(face) = face else {
                continue;
            };
            if encroached.is_empty() {
                let face = cdt.face(face);
                let near = face
                    .adjacent_edges()
                    .into_iter()
                    .chain(face.adjacent_edges().into_iter().flat_map(|e| {
                        e.rev().face().as_inner().into_iter().flat_map(|f| f.adjacent_edges())
                    }));
                for edge in near {
                    let [a, b] = edge.positions();
                    if edge.is_constraint_edge() && encroaches(a, b, center) {
                        encroached.push(edge.as_undirected().fix());
                    }
                }
            }
            if !encroached.is_empty() {
                encroached.sort();
                encroached.dedup();
                for edge in encroached {
                    let [a, b] = cdt.undirected_edge(edge).positions();
                    if cdt.is_constraint_edge(edge) && length(a, b) > 2.0 * shortest {
                        split(cdt, edge);
                    }
                }
                continue;
            }
            let crowded = cdt
                .face(face)
                .positions()
                .iter()
                .any(|&p| length(p, center) < 0.5 * radius);
            if !crowded {
                let _ = cdt.insert(center);
            }
        }
    }
}
