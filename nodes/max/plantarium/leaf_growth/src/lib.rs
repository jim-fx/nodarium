mod mesh;
mod sim;
mod veins;

use glam::{Vec2, Vec3};
use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;
use nodarium_utils::geometry::NO_PATH;
use nodarium_utils::{encode_float, wrap_arg, Ctx};

nodarium_definition_file!("src/definition.json");

/// Length of the leaf from the base to the tip before scaling by `size`,
/// matching the plantarium leaf node.
const LEAF_LENGTH: f32 = 100.0;
/// Total length of the veins that get into the mesh per unit of resolution, in
/// units of LEAF_LENGTH, the thickest first. The finest ones would just add
/// vertices.
const VEIN_LENGTH_PER_RESOLUTION: f32 = 0.4;
/// Distance of the points the veins are smoothed at, a 200th of the leaf.
const VEIN_STEP: f32 = LEAF_LENGTH / 200.0;
/// Smoothing passes over the veins. The kinks get rounded off over about
/// VEIN_STEP * sqrt(passes / 2).
const VEIN_SMOOTHING: usize = 20;

/// The shape inputs of the node. They are named after what they do to the
/// leaf and go from 0 to 1, `params` turns them into the parameters of the
/// model in `sim.rs`. The mappings were tuned so that the leaf changes about
/// evenly along every slider.
struct Controls {
    age: usize,
    width: f32,
    teeth: f32,
    lobes: f32,
    vein_density: f32,
    petiole: f32,
    tooth_shape: f32,
    tooth_direction: f32,
    /// In degrees.
    vein_angle: f32,
}

impl Controls {
    fn params(&self) -> sim::Params {
        let lobes = self.lobes;
        sim::Params {
            steps: self.age,
            // Evenly on a log scale, from a few veins (2.5) to many (0.4).
            vein_spacing: 2.5 * (0.4f32 / 2.5).powf(self.vein_density),
            // Growth at the vein tips alone also makes compound teeth, the
            // outward growth of the margin rounds the leaf instead.
            tip_growth: 0.02 + 0.05 * self.width,
            margin_growth: 0.002 + 0.028 * self.width,
            // Evenly on a log scale from a smooth margin (0.6) to deep teeth
            // (0.015). Stronger webbing pulls the margin by the tip onto the
            // midrib, which leaves a bare spike there.
            webbing: 0.6 * 0.025f32.powf(self.teeth),
            // Any sinus morphogen keeps veins out of the sinuses, so the first
            // sinuses always make large lobes. Small values start the sinuses
            // later, when there are more veins, so they stay small. Lobing has
            // most of its effect close to 1, where the sinuses stop smoothing.
            lobing: if lobes > 0.0 {
                1.0 - 0.4 * (1.0 - lobes).powi(3)
            } else {
                0.0
            },
            lobe_delay: (15.0 * (1.0 - lobes)).round() as usize,
            lobe_steps: (1.0 + 19.0 * lobes).round() as usize,
            petiole: self.petiole,
            roundness: self.tooth_shape,
            serration: self.tooth_direction,
            branch_angle: self.vein_angle.to_radians(),
            // Parameters of the model without an input of their own.
            max_vein_angle: 60f32.to_radians(),
            base_growth: 0.04,
            growth_zone: 4.0,
            basipetal: true,
            expansion: 0.02,
        }
    }
}

/// Bends a flat strip into an arc. `u` is the position along the strip and `h`
/// the height above it, `angle` is the bend reached at `u = extent`.
fn bend(u: f32, h: f32, angle: f32, extent: f32) -> (f32, f32) {
    if angle.abs() < 1e-4 || extent <= 0.0 {
        return (u, h);
    }
    let r = extent / angle;
    let (sin, cos) = (u / r).sin_cos();
    ((r - h) * sin, r - (r - h) * cos)
}

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let mut ctx = Ctx::default();

    let size = ctx.float(args[0]);
    let controls = Controls {
        age: ctx.int(args[1]).clamp(1, 200) as usize,
        width: ctx.float(args[2]).clamp(0.0, 1.0),
        teeth: ctx.float(args[3]).clamp(0.0, 1.0),
        lobes: ctx.float(args[4]).clamp(0.0, 1.0),
        vein_density: ctx.float(args[5]).clamp(0.0, 1.0),
        petiole: ctx.float(args[6]).clamp(0.0, 1.0),
        tooth_shape: ctx.float(args[7]).clamp(0.0, 1.0),
        tooth_direction: ctx.float(args[8]).clamp(-1.0, 1.0),
        vein_angle: ctx.float(args[9]).clamp(1.0, 89.0),
    };
    let vein_depth = ctx.float(args[10]);
    let vein_width = ctx.float(args[11]).max(0.0);
    let x_curve = ctx.float(args[12]).to_radians();
    let y_curve = ctx.float(args[13]).to_radians();
    let resolution = ctx.int(args[14]).clamp(2, 128) as f32;
    let params = controls.params();

    let mut leaf = sim::simulate(&params);

    // Normalise the leaf so that it reaches from the origin to LEAF_LENGTH.
    let mut outline = leaf.outline();
    let length = outline.iter().fold(0.0f32, |m, p| m.max(p.y)).max(1e-6);
    let scale = LEAF_LENGTH / length;
    for p in outline.iter_mut() {
        *p *= scale;
    }
    for strand in leaf.strands.iter_mut() {
        for p in strand.points.iter_mut() {
            *p *= scale;
        }
    }
    let half_width = outline.iter().fold(0.0f32, |m, p| m.max(p.x.abs()));

    let mut lines = veins::lines(&leaf.strands, VEIN_STEP, VEIN_SMOOTHING);
    // A line carries at most as much as the one it branches off, so the stable
    // sort keeps every line after its parent.
    lines.sort_by(|a, b| b.flux[0].total_cmp(&a.flux[0]));
    let budget = resolution * VEIN_LENGTH_PER_RESOLUTION * LEAF_LENGTH;
    let mut total = 0.0;
    lines.retain(|line| {
        let keep = total < budget;
        total += line.length();
        keep
    });

    let spacing = LEAF_LENGTH / resolution;
    let mesh = mesh::triangulate(&outline, &lines, spacing, vein_width);

    // The blade lies in the xz plane facing +y, growing from the origin along -z.
    // Veins run along mesh edges and are pressed into the upper side of the
    // blade, the thicker ones (supplying more CPs) deeper and wider.
    let positions: Vec<Vec3> = mesh
        .points
        .iter()
        .zip(&mesh.vein)
        .map(|(&p, &vein): (&Vec2, &f32)| {
            let (x, h) = bend(p.x, -vein_depth * vein, x_curve, half_width);
            let (y, h) = bend(p.y, h, y_curve, LEAF_LENGTH);
            Vec3::new(x, h, -y) * size
        })
        .collect();

    // Weighted by the angle every triangle has at the vertex, so the normals
    // don't depend on how a flat part is split into triangles. With weights
    // by area the walls of the grooves would shade unevenly. Points meant to
    // lie on the flat blade only take the flat triangles around them, else
    // where veins meet the tilt of a groove would spread over the large
    // triangles of the blade.
    let mut normals = vec![Vec3::ZERO; positions.len()];
    let mut flat_normals = vec![Vec3::ZERO; positions.len()];
    for t in &mesh.triangles {
        let [a, b, c] = t.map(|i| positions[i]);
        let n = (b - a).cross(c - a).normalize_or_zero();
        let flat = t.iter().all(|&i| mesh.vein[i] <= 0.0);
        for (k, &i) in t.iter().enumerate() {
            let p = positions[i];
            let (u, v) = (positions[t[(k + 1) % 3]] - p, positions[t[(k + 2) % 3]] - p);
            normals[i] += n * u.angle_between(v);
            if flat {
                flat_normals[i] += n * u.angle_between(v);
            }
        }
    }
    for (i, n) in normals.iter_mut().enumerate() {
        if mesh.flat[i] && mesh.vein[i] <= 0.0 && flat_normals[i] != Vec3::ZERO {
            *n = flat_normals[i];
        }
    }

    let vertex_amount = positions.len();
    let face_amount = mesh.triangles.len();
    let mut out = Vec::with_capacity(4 + face_amount * 3 + vertex_amount * 6);
    out.push(1);
    out.push(vertex_amount as i32);
    out.push(face_amount as i32);
    // not made from a path
    out.push(NO_PATH);
    for t in &mesh.triangles {
        out.extend(t.iter().map(|&i| i as i32));
    }
    for p in &positions {
        out.extend([encode_float(p.x), encode_float(p.y), encode_float(p.z)]);
    }
    for n in &normals {
        let n = n.normalize_or(Vec3::Y);
        out.extend([encode_float(n.x), encode_float(n.y), encode_float(n.z)]);
    }

    wrap_arg(&out)
}

#[cfg(test)]
mod tests {
    use super::{__nodarium_execute_inner, DEFINITION_DATA};
    use glam::Vec2;
    use nodarium_utils::{decode_float, encode_float, geometry::NO_PATH};

    /// Runs the node with the defaults of definition.json, `overrides` set by name.
    fn run(overrides: &[(&str, f32)]) -> Leaf {
        let text = std::str::from_utf8(&DEFINITION_DATA).unwrap();
        let definition: serde_json::Value = serde_json::from_str(text).unwrap();
        let inputs = definition["inputs"].as_object().unwrap();
        for (name, _) in overrides {
            assert!(inputs.contains_key(*name), "no input {name}");
        }
        // serde_json sorts the keys, the node takes its inputs in the order of the file
        let mut names: Vec<&String> = inputs.keys().collect();
        names.sort_by_key(|name| text.find(&format!("\"{name}\": {{")).unwrap());
        let args: Vec<[i32; 1]> = names
            .into_iter()
            .map(|name| {
                let input = &inputs[name];
                let value = overrides
                    .iter()
                    .find(|(n, _)| n == name)
                    .map(|&(_, v)| v)
                    .unwrap_or_else(|| match &input["value"] {
                        serde_json::Value::Bool(b) => *b as i32 as f32,
                        v => v.as_f64().unwrap() as f32,
                    });
                match input["type"].as_str().unwrap() {
                    "float" => [encode_float(value)],
                    _ => [value as i32],
                }
            })
            .collect();
        let args: Vec<&[i32]> = args.iter().map(|a| &a[..]).collect();
        Leaf::read(&__nodarium_execute_inner(&args))
    }

    struct Leaf {
        path_id: i32,
        triangles: Vec<[usize; 3]>,
        positions: Vec<[f32; 3]>,
    }

    impl Leaf {
        fn read(out: &[i32]) -> Self {
            let geometry = &out[2..out.len() - 2];
            let (vertices, faces) = (geometry[1] as usize, geometry[2] as usize);
            let positions = &geometry[4 + faces * 3..4 + faces * 3 + vertices * 3];
            Leaf {
                path_id: geometry[3],
                triangles: geometry[4..4 + faces * 3]
                    .chunks(3)
                    .map(|t| [t[0] as usize, t[1] as usize, t[2] as usize])
                    .collect(),
                positions: positions
                    .chunks(3)
                    .map(|p| [decode_float(p[0]), decode_float(p[1]), decode_float(p[2])])
                    .collect(),
            }
        }

        /// Signed area of every triangle seen from above, the blade faces +y.
        fn areas(&self) -> Vec<f32> {
            self.triangles
                .iter()
                .map(|t| {
                    let [a, b, c] = t.map(|i| Vec2::new(self.positions[i][0], -self.positions[i][2]));
                    0.5 * (b - a).perp_dot(c - a)
                })
                .collect()
        }
    }

    #[test]
    fn size_sets_the_length_of_the_leaf() {
        let leaf = run(&[("size", 2.0)]);
        let (min, max) = leaf
            .positions
            .iter()
            .fold((f32::MAX, f32::MIN), |(lo, hi), p| (lo.min(p[2]), hi.max(p[2])));

        assert_eq!(leaf.path_id, NO_PATH);
        assert!(!leaf.triangles.is_empty());
        assert!((min + 200.0).abs() < 1e-3 && max.abs() < 1e-3, "z from {min} to {max}");
    }

    #[test]
    fn a_smooth_leaf_reaches_its_tip() {
        // Too much webbing pulls the margin by the tip onto the midrib, the
        // leaf would end in a bare spike and look shorter than its size.
        let leaf = run(&[("teeth", 0.0)]);
        let tip = leaf
            .triangles
            .iter()
            .flatten()
            .fold(0.0f32, |m, &i| m.min(leaf.positions[i][2]));

        assert!(tip < -99.0, "the blade ends at {tip}");
    }

    #[test]
    fn more_lobes_cut_deeper_into_the_blade() {
        let blade = |lobes: f32| -> f32 { run(&[("lobes", lobes)]).areas().iter().sum() };
        let areas = [0.0, 0.25, 0.5, 0.75, 1.0].map(blade);

        assert!(areas.windows(2).all(|w| w[1] < w[0]), "areas {areas:?}");
    }

    #[test]
    fn a_crumpled_margin_gives_a_valid_mesh() {
        // The margin of a broad, old leaf gets pressed onto the midrib, where
        // it meets its mirror image.
        let leaf = run(&[("width", 1.0), ("age", 100.0)]);
        let folded = leaf.areas().iter().filter(|&&a| a <= 0.0).count();

        assert_eq!(folded, 0);
        assert!(leaf.triangles.len() <= 2 * leaf.positions.len());
    }
}
