use myzel_core::{
    lamellae, read_float, read_floats, read_int, read_mesh, CapUnderside, LamellaForm, LamellaeParams, Mesh, Pattern,
};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{
    decode_points,
    geometry::{is_path, wrap_path},
    split_args,
};

nodarium_definition_file!("src/definition.json");

const FORM_RIDGES: i32 = 1;

/// The paths in a list, also from lists inside it
fn collect_paths<'a>(list: &'a [i32], out: &mut Vec<&'a [i32]>) {
    if list.len() < 4 {
        return;
    }
    for item in split_args(list) {
        if is_path(item) {
            out.push(item);
        } else if item.len() >= 4 && item[0] == 0 {
            collect_paths(item, out);
        }
    }
}

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let base = if args[0].is_empty() { Mesh::default() } else { read_mesh(args[0]) };
    let mut rays = Vec::new();
    collect_paths(args[1], &mut rays);
    let mut stipes = Vec::new();
    collect_paths(args[4], &mut stipes);
    let caps: Vec<CapUnderside> = decode_points(args[2])
        .iter()
        .filter_map(|hinge| {
            let points: Vec<&[f32]> = rays
                .iter()
                .map(|r| wrap_path(r))
                .filter(|p| p.link.parent_id == hinge.parent_id)
                .map(|p| p.points)
                .collect();
            let cap = CapUnderside::new(hinge, &points)?;
            Some(match stipes.iter().map(|s| wrap_path(s)).find(|s| s.link.id == hinge.parent_id) {
                Some(stipe) => cap.with_stipe(stipe.points),
                None => cap,
            })
        })
        .collect();
    let params = LamellaeParams {
        form: if read_int(args[5]) == FORM_RIDGES { LamellaForm::Ridges } else { LamellaForm::Plates },
        depth: read_float(args[6]).max(0.0),
        depth_curve: read_floats(args[7]),
        thickness: read_float(args[8]).max(0.0),
        free_distance: read_float(args[9]).max(0.0),
        decurrent: read_float(args[10]).max(0.0),
        edge_wave: read_float(args[11]).clamp(0.0, 1.0),
        irregularity: read_float(args[12]).clamp(0.0, 1.0),
        forking: read_float(args[13]).clamp(0.0, 1.0),
        seed: read_int(args[14]) as u32,
    };
    lamellae(&caps, &Pattern::decode(args[3]), &params, base).encode()
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use glam::Vec3;
    use myzel_core::{reaction_diffusion, LamellaeParams, Mesh, ReactionDiffusionParams};
    use nodarium_utils::{
        concat_arg_vecs, concat_args, encode_float, encode_points,
        geometry::{create_path, set_path_link, wrap_path_mut, PathLink},
        Point,
    };
    use std::f32::consts::TAU;

    const STIPE_ID: i32 = 1234;

    /// A flat cap of 16 rays, 2.5 cm wide, on a stipe 4 mm thick with its top at the origin
    fn cap() -> (Vec<i32>, Vec<i32>) {
        let rays = (0..16)
            .map(|k| {
                let angle = TAU * k as f32 / 16.0;
                let out = Vec3::new(angle.cos(), 0.0, angle.sin());
                let mut data = create_path(32, 2);
                set_path_link(&mut data, PathLink::child(9, STIPE_ID, 1.0, k));
                let path = wrap_path_mut(&mut data);
                for i in 0..32 {
                    let x = -0.001 + 0.026 * i as f32 / 31.0;
                    let q = out * x + Vec3::Y * 0.002;
                    path.points[i * 4..i * 4 + 4].copy_from_slice(&[q.x, q.y, q.z, 0.002]);
                }
                data
            })
            .collect();
        let hinge = Point {
            position: Vec3::ZERO,
            direction: Vec3::X,
            tangent: Vec3::Y,
            scale: 0.004,
            parent_id: STIPE_ID,
            alpha: 1.0,
            depth: 1,
            index: 0,
        };
        (concat_arg_vecs(rays), encode_points(&[hinge]))
    }

    fn floats(values: &[f32]) -> Vec<i32> {
        let mut words = vec![0, values.len() as i32 + 1];
        words.extend(values.iter().map(|&v| encode_float(v)));
        words.extend([1, 1]);
        words
    }

    fn run(mesh: &[i32], depth_curve: &[i32], d: &LamellaeParams) -> Vec<i32> {
        let (rays, hinges) = cap();
        let pattern = reaction_diffusion(&ReactionDiffusionParams { steps: 1500, ..Default::default() }).encode();
        let args: Vec<Vec<i32>> = vec![
            mesh.to_vec(),
            rays,
            hinges,
            pattern,
            vec![],
            vec![0],
            vec![encode_float(d.depth)],
            depth_curve.to_vec(),
            vec![encode_float(d.thickness)],
            vec![encode_float(d.free_distance)],
            vec![encode_float(d.decurrent)],
            vec![encode_float(d.edge_wave)],
            vec![encode_float(d.irregularity)],
            vec![encode_float(d.forking)],
            vec![d.seed as i32],
        ];
        let refs: Vec<&[i32]> = args.iter().map(|a| a.as_slice()).collect();
        __nodarium_execute_inner(&refs)
    }

    #[test]
    fn the_defaults_match_the_core_defaults() {
        let d = LamellaeParams::default();
        let def: serde_json::Value = serde_json::from_str(include_str!("definition.json")).unwrap();
        let value = |key: &str| def["inputs"][key]["value"].as_f64().unwrap() as f32;
        for (key, expected) in [
            ("depth", d.depth),
            ("thickness", d.thickness),
            ("free_distance", d.free_distance),
            ("decurrent", d.decurrent),
            ("edge_wave", d.edge_wave),
            ("irregularity", d.irregularity),
            ("forking", d.forking),
        ] {
            assert!((value(key) - expected).abs() < 1e-7, "{}", key);
        }
        let curve: Vec<f32> =
            def["inputs"]["depth_curve"]["value"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap() as f32).collect();
        assert_eq!(curve, d.depth_curve);
        assert_eq!(def["inputs"]["form"]["value"], 0);
    }

    #[test]
    fn hangs_gills_under_the_cap_and_keeps_the_mesh() {
        let d = LamellaeParams::default();
        let gills = Mesh::decode(&run(&[], &floats(&d.depth_curve), &d));
        assert!(gills.face_count() > 1000);
        assert!(gills.groups.iter().all(|&g| g == 0));
        assert!(gills.positions.chunks_exact(3).all(|p| p[1] < 0.002 && p[1] > -0.01));

        let base = Mesh {
            faces: vec![0, 1, 2],
            positions: vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            normals: vec![0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0],
            colors: Vec::new(),
            groups: vec![0, 0, 0],
            tissue_groups: false,
        };
        let joined = Mesh::decode(&run(&base.encode(), &floats(&d.depth_curve), &d));
        assert_eq!(joined.vertex_count(), gills.vertex_count() + 3);
        assert_eq!(&joined.positions[..9], &base.positions[..]);
        assert!(joined.groups[3..].iter().all(|&g| g == 1));
    }

    #[test]
    fn reads_a_connected_curve_node() {
        let d = LamellaeParams::default();
        let curve = floats(&[0.0, 0.5, 1.0, 0.5]);
        assert_eq!(run(&[], &concat_args(vec![&curve]), &d), run(&[], &curve, &d));
    }
}
