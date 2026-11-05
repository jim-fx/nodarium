use glam::Vec3;

use crate::field::Field;
use crate::mesh::{pack_rgba, Mesh};

const BOTTOM_COLOR: [f32; 3] = [0.93, 0.89, 0.80];
const TOP_COLOR: [f32; 3] = [0.55, 0.38, 0.24];

// corner offsets of a cell and the corner pairs of its 12 edges
const CORNERS: [[usize; 3]; 8] =
    [[0, 0, 0], [1, 0, 0], [0, 1, 0], [1, 1, 0], [0, 0, 1], [1, 0, 1], [0, 1, 1], [1, 1, 1]];
const EDGES: [[usize; 2]; 12] =
    [[0, 1], [2, 3], [4, 5], [6, 7], [0, 2], [1, 3], [4, 6], [5, 7], [0, 4], [1, 5], [2, 6], [3, 7]];

/// Meshes the surface of a field with Surface Nets. `resolution` is the number
/// of cells along the longest side of the bounding box.
pub fn polygonize(field: &Field, resolution: usize) -> Mesh {
    let program = field.program();
    let longest = field.bbox.size().max_element();
    if field.ops.is_empty() || longest.is_nan() || longest <= 0.0 {
        return Mesh::default();
    }
    let cell = longest / resolution.max(2) as f32;

    // one cell of padding keeps the outermost samples outside, so the surface closes
    let origin = field.bbox.min - Vec3::splat(cell);
    let size = field.bbox.size() + Vec3::splat(2.0 * cell);
    let cells = [0, 1, 2].map(|a| ((size[a] / cell).ceil() as usize).max(1));
    let samples = cells.map(|c| c + 1);
    let sample_index = |p: [usize; 3]| p[0] + samples[0] * (p[1] + samples[1] * p[2]);
    let cell_index = |c: [usize; 3]| c[0] + cells[0] * (c[1] + cells[1] * c[2]);
    let position = |p: [f32; 3]| origin + Vec3::from(p) * cell;

    let mut distances = Vec::with_capacity(samples[0] * samples[1] * samples[2]);
    for z in 0..samples[2] {
        for y in 0..samples[1] {
            for x in 0..samples[0] {
                distances.push(program.eval(position([x as f32, y as f32, z as f32])));
            }
        }
    }

    // one vertex per cell the surface passes through, at the mean of its edge crossings
    let mut mesh = Mesh::default();
    let mut cell_vertex = vec![u32::MAX; cells[0] * cells[1] * cells[2]];
    for z in 0..cells[2] {
        for y in 0..cells[1] {
            for x in 0..cells[0] {
                let d = CORNERS.map(|o| distances[sample_index([x + o[0], y + o[1], z + o[2]])]);
                let inside = d.iter().filter(|&&v| v < 0.0).count();
                if inside == 0 || inside == 8 {
                    continue;
                }
                let mut sum = Vec3::ZERO;
                let mut crossings = 0.0;
                for [a, b] in EDGES {
                    if (d[a] < 0.0) == (d[b] < 0.0) {
                        continue;
                    }
                    let t = d[a] / (d[a] - d[b]);
                    let (ca, cb) = (Vec3::from(CORNERS[a].map(|v| v as f32)), Vec3::from(CORNERS[b].map(|v| v as f32)));
                    sum += ca + (cb - ca) * t;
                    crossings += 1.0;
                }
                let p = position([x as f32, y as f32, z as f32]) + sum / crossings * cell;
                cell_vertex[cell_index([x, y, z])] = mesh.vertex_count() as u32;
                mesh.positions.extend([p.x, p.y, p.z]);
            }
        }
    }

    // a quad for every grid edge the surface crosses, joining the four cells around it
    for z in 0..samples[2] {
        for y in 0..samples[1] {
            for x in 0..samples[0] {
                let p = [x, y, z];
                let inside = distances[sample_index(p)] < 0.0;
                for axis in 0..3 {
                    let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
                    if p[axis] >= cells[axis] || p[u] == 0 || p[v] == 0 || p[u] >= cells[u] || p[v] >= cells[v] {
                        continue;
                    }
                    let mut next = p;
                    next[axis] += 1;
                    if inside == (distances[sample_index(next)] < 0.0) {
                        continue;
                    }
                    let corner = |du: usize, dv: usize| {
                        let mut c = p;
                        c[u] = p[u] - 1 + du;
                        c[v] = p[v] - 1 + dv;
                        cell_vertex[cell_index(c)]
                    };
                    // counter-clockwise around +axis, flipped when the outside is on the other end
                    let quad = [corner(0, 0), corner(1, 0), corner(1, 1), corner(0, 1)];
                    if inside {
                        mesh.faces.extend([quad[0], quad[1], quad[2], quad[0], quad[2], quad[3]]);
                    } else {
                        mesh.faces.extend([quad[0], quad[2], quad[1], quad[0], quad[3], quad[2]]);
                    }
                }
            }
        }
    }

    let eps = cell * 0.5;
    let low = field.bbox.min.y;
    let height = field.bbox.size().y.max(f32::EPSILON);
    for vertex in mesh.positions.chunks_exact(3) {
        let p = Vec3::from_slice(vertex);
        let gradient = Vec3::new(
            program.eval(p + Vec3::X * eps) - program.eval(p - Vec3::X * eps),
            program.eval(p + Vec3::Y * eps) - program.eval(p - Vec3::Y * eps),
            program.eval(p + Vec3::Z * eps) - program.eval(p - Vec3::Z * eps),
        );
        let n = gradient.try_normalize().unwrap_or(Vec3::Y);
        mesh.normals.extend([n.x, n.y, n.z]);

        let t = ((p.y - low) / height).clamp(0.0, 1.0);
        let [r, g, b] = [0, 1, 2].map(|i| BOTTOM_COLOR[i] + (TOP_COLOR[i] - BOTTOM_COLOR[i]) * t);
        mesh.colors.push(pack_rgba(r, g, b, 1.0));
    }

    mesh
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn vertex(mesh: &Mesh, i: u32) -> Vec3 {
        Vec3::from_slice(&mesh.positions[i as usize * 3..i as usize * 3 + 3])
    }

    #[test]
    fn sphere_vertices_lie_on_the_radius() {
        let mesh = polygonize(&Field::sphere(1.0), 32);
        assert!(mesh.vertex_count() > 100);
        let cell = 2.0 / 32.0;
        for i in 0..mesh.vertex_count() as u32 {
            let r = vertex(&mesh, i).length();
            assert!((r - 1.0).abs() < cell * 0.5, "vertex at radius {}", r);
        }
    }

    #[test]
    fn sphere_mesh_is_closed_and_faces_outwards() {
        let mesh = polygonize(&Field::sphere(1.0).translate(Vec3::new(0.3, -0.2, 0.1)), 24);
        assert!(mesh.face_count() > 0);

        // every directed edge has exactly one partner going the other way
        let mut edges: HashMap<(u32, u32), i32> = HashMap::new();
        for f in mesh.faces.chunks_exact(3) {
            for (a, b) in [(f[0], f[1]), (f[1], f[2]), (f[2], f[0])] {
                *edges.entry((a, b)).or_default() += 1;
            }
        }
        for (&(a, b), &count) in &edges {
            assert_eq!(count, 1, "edge {}-{} used {} times in the same direction", a, b, count);
            assert_eq!(edges.get(&(b, a)), Some(&1), "edge {}-{} has no partner", a, b);
        }

        // positive volume means the faces wind counter-clockwise seen from outside
        let volume: f32 = mesh
            .faces
            .chunks_exact(3)
            .map(|f| vertex(&mesh, f[0]).dot(vertex(&mesh, f[1]).cross(vertex(&mesh, f[2]))) / 6.0)
            .sum();
        let expected = 4.0 / 3.0 * std::f32::consts::PI;
        assert!((volume - expected).abs() < expected * 0.05, "volume {} vs {}", volume, expected);
    }

    #[test]
    fn normals_point_away_from_the_center() {
        let mesh = polygonize(&Field::sphere(1.0), 16);
        for i in 0..mesh.vertex_count() {
            let p = Vec3::from_slice(&mesh.positions[i * 3..i * 3 + 3]);
            let n = Vec3::from_slice(&mesh.normals[i * 3..i * 3 + 3]);
            assert!(n.dot(p.normalize()) > 0.99);
        }
        assert_eq!(mesh.colors.len(), mesh.vertex_count());
    }

    #[test]
    fn same_input_gives_the_same_output() {
        let field = || {
            Field::capsule(Vec3::ZERO, Vec3::Y, 0.2)
                .smooth_union(Field::sphere(0.6).translate(Vec3::Y), 0.3)
                .bend(0.4)
                .warp(0.05, 4.0, 9)
        };
        assert_eq!(polygonize(&field(), 24).encode(), polygonize(&field(), 24).encode());
    }

    #[test]
    fn empty_field_gives_an_empty_mesh() {
        let empty = Field { bbox: Field::sphere(1.0).bbox, ops: vec![] };
        assert_eq!(polygonize(&empty, 16), Mesh::default());
    }
}
