use glam::Vec2;

use crate::field::{Corner, Field};

/// The blade before it is bent: triangles in leaf space, counter-clockwise seen from above.
pub struct FlatMesh {
    pub positions: Vec<Vec2>,
    pub faces: Vec<[u32; 3]>,
}

#[derive(Clone, Copy)]
enum Vertex {
    Corner(Corner),
    Edge(usize),
}

/// Triangles over the cells of the field inside the margin. The margin vertices sit where the
/// field crosses zero, the others on the corners of the grid.
pub fn marching_squares(field: &Field) -> FlatMesh {
    let mut positions = Vec::new();
    let mut faces = Vec::new();
    let mut corner_vertex = vec![u32::MAX; field.values.len()];
    let mut edge_vertex = vec![u32::MAX; field.edge_count()];

    for j in 0..field.ny {
        for i in 0..field.nx {
            let (corners, edges) = field.cell(i, j);
            let inside = corners.map(|c| field.inside(c));
            if !inside.iter().any(|&x| x) {
                continue;
            }
            let mut vertex = |v: Vertex, positions: &mut Vec<Vec2>| -> u32 {
                let (slot, p) = match v {
                    Vertex::Corner(c) => (&mut corner_vertex[field.index(c)], field.point(c)),
                    Vertex::Edge(e) => (&mut edge_vertex[e], field.crossing(e)),
                };
                if *slot == u32::MAX {
                    *slot = positions.len() as u32;
                    positions.push(p);
                }
                *slot
            };

            let crossings = (0..4).filter(|&k| inside[k] != inside[(k + 1) % 4]).count();
            if crossings == 4 && !field.center_inside(&corners) {
                // a saddle with its middle outside: two corners apart
                for k in (0..4).filter(|&k| inside[k]) {
                    let a = vertex(Vertex::Edge(edges[(k + 3) % 4]), &mut positions);
                    let b = vertex(Vertex::Corner(corners[k]), &mut positions);
                    let c = vertex(Vertex::Edge(edges[k]), &mut positions);
                    faces.push([a, b, c]);
                }
                continue;
            }

            let mut polygon = Vec::with_capacity(6);
            for k in 0..4 {
                if inside[k] {
                    polygon.push(vertex(Vertex::Corner(corners[k]), &mut positions));
                }
                if inside[k] != inside[(k + 1) % 4] {
                    polygon.push(vertex(Vertex::Edge(edges[k]), &mut positions));
                }
            }
            for k in 1..polygon.len() - 1 {
                faces.push([polygon[0], polygon[k], polygon[k + 1]]);
            }
        }
    }
    FlatMesh { positions, faces }
}
