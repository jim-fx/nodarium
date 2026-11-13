use std::collections::HashMap;

use glam::Vec3;
use myzel_core::Mesh;
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{
    encode_points,
    geometry::{path_random, NO_PARENT},
    int, Ctx, Point,
};

nodarium_definition_file!("src/definition.json");

/// Candidates tried per point before the surface counts as full
const TRIES_PER_POINT: usize = 64;

struct Surface<'a> {
    mesh: &'a Mesh,
    /// The faces that face up enough, and the area of the faces up to and with each
    faces: Vec<usize>,
    cumulative: Vec<f32>,
}

impl<'a> Surface<'a> {
    fn new(mesh: &'a Mesh, facing_up: f32) -> Self {
        let (mut faces, mut cumulative, mut total) = (Vec::new(), Vec::new(), 0.0);
        for f in 0..mesh.face_count() {
            let [a, b, c] = Self::corners(mesh, f);
            let cross = (b - a).cross(c - a);
            let area = cross.length() * 0.5;
            if area > 0.0 && cross.y / (2.0 * area) >= facing_up {
                total += area;
                faces.push(f);
                cumulative.push(total);
            }
        }
        Surface { mesh, faces, cumulative }
    }

    fn corners(mesh: &Mesh, f: usize) -> [Vec3; 3] {
        [0, 1, 2].map(|k| Self::vec(&mesh.positions, mesh.faces[f * 3 + k] as usize))
    }

    fn vec(values: &[f32], v: usize) -> Vec3 {
        Vec3::from_slice(&values[v * 3..v * 3 + 3])
    }

    /// The spot of candidate `i`: a face picked by its area, a place on it and the normal there
    fn candidate(&self, seed: i32, i: i32) -> (Vec3, Vec3) {
        let total = *self.cumulative.last().unwrap();
        let pick = path_random(seed, i, 0) * total;
        let k = self.cumulative.partition_point(|&c| c < pick).min(self.faces.len() - 1);
        let f = self.faces[k];
        let (r1, r2) = (path_random(seed, i, 1).sqrt(), path_random(seed, i, 2));
        let weights = [1.0 - r1, r1 * (1.0 - r2), r1 * r2];
        let [a, b, c] = Self::corners(self.mesh, f);
        let position = a * weights[0] + b * weights[1] + c * weights[2];
        let face_normal = (b - a).cross(c - a).normalize();
        let normal = (0..3)
            .map(|k| Self::vec(&self.mesh.normals, self.mesh.faces[f * 3 + k] as usize) * weights[k])
            .sum::<Vec3>()
            .try_normalize()
            .filter(|n| n.dot(face_normal) > 0.0)
            .unwrap_or(face_normal);
        (position, normal)
    }
}

/// Keeps points at least `spacing` apart, in cells as large as the spacing
struct Spacing {
    spacing: f32,
    cells: HashMap<[i32; 3], Vec<Vec3>>,
}

impl Spacing {
    fn cell(&self, p: Vec3) -> [i32; 3] {
        (p / self.spacing).floor().as_ivec3().to_array()
    }

    fn has_room(&self, p: Vec3) -> bool {
        if self.spacing <= 0.0 {
            return true;
        }
        let [x, y, z] = self.cell(p);
        (-1..=1).all(|dx| {
            (-1..=1).all(|dy| {
                (-1..=1).all(|dz| {
                    self.cells
                        .get(&[x + dx, y + dy, z + dz])
                        .is_none_or(|points| points.iter().all(|q| q.distance_squared(p) >= self.spacing * self.spacing))
                })
            })
        })
    }

    fn add(&mut self, p: Vec3) {
        if self.spacing > 0.0 {
            self.cells.entry(self.cell(p)).or_default().push(p);
        }
    }
}

/// A direction along the surface, the projection of the x axis, or of the z axis where the normal
/// lies along x
fn tangent(normal: Vec3) -> Vec3 {
    (Vec3::X - normal * normal.x)
        .try_normalize()
        .unwrap_or_else(|| (Vec3::Z - normal * normal.z).normalize())
}

/// Candidate `i` only depends on the seed and `i`, and is kept when it has room from the points
/// kept before it, so a larger amount only adds points after the ones it had
pub fn sample(mesh: &Mesh, amount: usize, spacing: f32, facing_up: f32, seed: i32) -> Vec<Point> {
    let surface = Surface::new(mesh, facing_up);
    let mut points = Vec::new();
    if surface.faces.is_empty() {
        return points;
    }
    let mut kept = Spacing { spacing, cells: HashMap::new() };
    for i in 0..(amount * TRIES_PER_POINT) as i32 {
        if points.len() == amount {
            break;
        }
        let (position, normal) = surface.candidate(seed, i);
        if !kept.has_room(position) {
            continue;
        }
        kept.add(position);
        points.push(Point {
            position,
            direction: normal,
            tangent: tangent(normal),
            scale: 1.0,
            parent_id: NO_PARENT,
            alpha: 0.0,
            depth: 0,
            index: points.len() as i32,
        });
    }
    points
}

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    // an input nothing is connected to is not a mesh
    let Some(mesh) = Mesh::try_decode(args[0]) else {
        return encode_points(&[]);
    };
    let mut ctx = Ctx::default();
    let amount = ctx.int(args[1]).max(0) as usize;
    let spacing = ctx.float(args[2]).max(0.0);
    let facing_up = ctx.float(args[3]);
    let seed = int(args[4]);
    encode_points(&sample(&mesh, amount, spacing, facing_up, seed))
}

#[cfg(test)]
mod tests {
    use super::{__nodarium_execute_inner, sample};
    use glam::Vec3;
    use myzel_core::{smooth_normals, Mesh};
    use nodarium_utils::{decode_points, encode_float, Point};

    /// A bumpy patch of ground 0.4 m across, and a wall standing on it
    fn ground() -> Mesh {
        let mut mesh = Mesh::default();
        let n = 20;
        let height = |x: f32, z: f32| 0.02 * (x * 17.0).sin() * (z * 13.0).cos();
        for i in 0..=n {
            for j in 0..=n {
                let (x, z) = (i as f32 / n as f32 * 0.4 - 0.2, j as f32 / n as f32 * 0.4 - 0.2);
                mesh.positions.extend([x, height(x, z), z]);
            }
        }
        let at = |i: u32, j: u32| i * (n + 1) + j;
        for i in 0..n {
            for j in 0..n {
                mesh.faces.extend([at(i, j), at(i, j + 1), at(i + 1, j + 1), at(i, j), at(i + 1, j + 1), at(i + 1, j)]);
            }
        }
        let wall = mesh.vertex_count() as u32;
        mesh.positions.extend([0.0, 0.0, -0.1, 0.0, 0.0, 0.1, 0.0, 0.3, 0.1, 0.0, 0.3, -0.1]);
        mesh.faces.extend([wall, wall + 1, wall + 2, wall, wall + 2, wall + 3]);
        mesh.groups = vec![0; mesh.vertex_count()];
        smooth_normals(&mut mesh);
        mesh
    }

    fn run(mesh: &Mesh, amount: i32, spacing: f32, facing_up: f32, seed: i32) -> Vec<Point> {
        let args = [mesh.encode(), vec![amount], vec![encode_float(spacing)], vec![encode_float(facing_up)], vec![seed]];
        let refs: Vec<&[i32]> = args.iter().map(|a| a.as_slice()).collect();
        decode_points(&__nodarium_execute_inner(&refs))
    }

    /// The distance from `p` to the nearest face of the mesh, and the normal of that face
    fn nearest_face(mesh: &Mesh, p: Vec3) -> (f32, Vec3) {
        let v = |i: u32| Vec3::from_slice(&mesh.positions[i as usize * 3..i as usize * 3 + 3]);
        mesh.faces
            .chunks_exact(3)
            .map(|f| {
                let (a, b, c) = (v(f[0]), v(f[1]), v(f[2]));
                let n = (b - a).cross(c - a).normalize();
                let on_plane = p - n * n.dot(p - a);
                let inside = [(a, b), (b, c), (c, a)].iter().all(|&(s, e)| (e - s).cross(on_plane - s).dot(n) >= -1e-6);
                let distance = if inside { n.dot(p - a).abs() } else { f32::MAX };
                (distance, n)
            })
            .min_by(|x, y| x.0.total_cmp(&y.0))
            .unwrap()
    }

    #[test]
    fn the_points_lie_on_the_surface_and_point_out_of_it() {
        let mesh = ground();
        let points = run(&mesh, 40, 0.02, -1.0, 3);
        assert_eq!(points.len(), 40);
        for point in &points {
            let (distance, normal) = nearest_face(&mesh, point.position);
            assert!(distance < 1e-5, "{:?} is {} m off the surface", point.position, distance);
            assert!((point.direction.length() - 1.0).abs() < 1e-5);
            assert!(point.direction.dot(normal) > 0.5, "{:?} doesn't point out of the face {:?}", point.direction, normal);
            assert!(point.tangent.dot(point.direction).abs() < 1e-5);
            assert_eq!((point.parent_id, point.depth), (-1, 0));
        }
        assert!(points.iter().enumerate().all(|(i, p)| p.index == i as i32));
    }

    #[test]
    fn the_same_seed_gives_the_same_points_and_another_seed_others() {
        let mesh = ground();
        assert_eq!(run(&mesh, 12, 0.03, 0.5, 7), run(&mesh, 12, 0.03, 0.5, 7));
        assert_ne!(run(&mesh, 12, 0.03, 0.5, 7), run(&mesh, 12, 0.03, 0.5, 8));
    }

    #[test]
    fn a_larger_amount_adds_points_and_keeps_the_ones_before() {
        let mesh = ground();
        let few = run(&mesh, 5, 0.03, 0.5, 11);
        let many = run(&mesh, 30, 0.03, 0.5, 11);
        assert_eq!(few.len(), 5);
        assert_eq!(many.len(), 30);
        assert_eq!(few[..], many[..5]);
    }

    #[test]
    fn the_points_keep_their_spacing() {
        let points = run(&ground(), 200, 0.05, -1.0, 1);
        // a patch of 0.16 m² has room for far fewer than 200 points 5 cm apart
        assert!(points.len() < 200 && points.len() > 20, "{} points", points.len());
        for (i, a) in points.iter().enumerate() {
            for b in &points[i + 1..] {
                assert!(a.position.distance(b.position) >= 0.05);
            }
        }
    }

    #[test]
    fn only_the_surface_that_faces_up_is_sampled() {
        let mesh = ground();
        let everywhere = run(&mesh, 300, 0.0, -1.0, 5);
        assert!(everywhere.iter().any(|p| p.direction.x.abs() > 0.99), "the wall is sampled");
        for point in run(&mesh, 300, 0.0, 0.8, 5) {
            assert!(nearest_face(&mesh, point.position).1.y >= 0.8);
        }
    }

    #[test]
    fn nothing_to_sample_gives_no_points() {
        let mesh = ground();
        assert!(run(&mesh, 0, 0.0, 0.0, 1).is_empty());
        assert!(run(&mesh, 10, 0.0, 1.1, 1).is_empty());
        assert!(run(&Mesh::default(), 10, 0.0, 0.0, 1).is_empty());
        // the list the host sends for nothing connected
        let refs: [&[i32]; 5] = [&[0, 1, 1, 1], &[10], &[encode_float(0.0)], &[encode_float(0.0)], &[1]];
        assert!(decode_points(&__nodarium_execute_inner(&refs)).is_empty());
        assert_eq!(sample(&mesh, 3, 0.0, 0.0, 1).len(), 3);
    }
}
