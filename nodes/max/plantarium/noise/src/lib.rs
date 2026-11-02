use glam::Vec3;
use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;
use nodarium_utils::{
    concat_args, geometry::wrap_path_mut, split_args, Ctx,
};
use noise::{HybridMulti, MultiFractal, NoiseFn, OpenSimplex};

nodarium_definition_file!("src/definition.json");

/// The number of points a path had when the strength of the noise was tuned, the default of
/// the resolution.curve setting
const REFERENCE_RESOLUTION: f32 = 32.0;

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + t * (b - a)
}

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let mut ctx = Ctx::default();


    let plants = split_args(args[0]);
    let seed = args[4][0];

    let directional_args = split_args(args[5]);
    assert!(
        directional_args.len() == 3,
        "Failed to evaluate Vec3 - Expected 3 arguments, got {}",
        directional_args.len()
    );

    let depth = ctx.int(args[6]);

    let octaves = ctx.int(args[7]);

    let noise_x: HybridMulti<OpenSimplex> =
        HybridMulti::new(seed as u32 + 1).set_octaves(octaves as usize);
    let noise_y: HybridMulti<OpenSimplex> =
        HybridMulti::new(seed as u32 + 2).set_octaves(octaves as usize);
    let noise_z: HybridMulti<OpenSimplex> =
        HybridMulti::new(seed as u32 + 3).set_octaves(octaves as usize);

    let mut max_depth = 0;
    for path_data in plants.iter() {
        if path_data[2] != 0 {
            continue;
        }
        max_depth = max_depth.max(path_data[3]);
    }

    let path_count = plants
        .iter()
        .filter(|p| p[2] == 0 && p[3] >= (max_depth - depth + 1))
        .count();
    let mut path_index = 0;

    let output: Vec<Vec<i32>> = plants
        .iter()
        .enumerate()
        .map(|(j, _path_data)| {
            let mut path_data = _path_data.to_vec();

            // if this is not a path don't modify it
            if path_data[2] != 0 {
                return path_data;
            }

            if path_data[3] < (max_depth - depth + 1) {
                return path_data;
            }

            ctx.set_path(&path_data, path_index, path_count);
            path_index += 1;

            let path = wrap_path_mut(&mut path_data);

            let length = path.get_length() as f64;

            // alpha goes along the path, so every setting can change from base to tip
            let alphas: Vec<f32> = (0..path.length)
                .map(|i| i as f32 / (path.length - 1).max(1) as f32)
                .collect();
            let scale: Vec<f64> = ctx
                .float_along(args[1], alphas.iter().copied())
                .iter()
                .map(|v| (v * 0.1) as f64)
                .collect();
            let strength = ctx.float_along(args[2], alphas.iter().copied());
            let fix_bottom = ctx.float_along(args[3], alphas.iter().copied());
            let directional_strength: Vec<[f32; 3]> = {
                let x = ctx.float_along(directional_args[0], alphas.iter().copied());
                let y = ctx.float_along(directional_args[1], alphas.iter().copied());
                let z = ctx.float_along(directional_args[2], alphas.iter().copied());
                (0..path.length).map(|i| [x[i], y[i], z[i]]).collect()
            };

            // Snapshot original positions so we can derive each segment's original
            // direction even after we've modified earlier points.
            let orig: Vec<f32> = path.points[..path.length * 4].to_vec();

            // Anchor the base (fix_bottom=1 → scale=0, no displacement at root)
            let scale0 = lerp(1.0, 0.0, fix_bottom[0]);
            path.points[0] += noise_x.get([j as f64, 0.0]) as f32
                * directional_strength[0][0]
                * strength[0]
                * scale0;
            path.points[1] += noise_y.get([j as f64, 0.0]) as f32
                * directional_strength[0][1]
                * strength[0]
                * scale0;
            path.points[2] += noise_z.get([j as f64, 0.0]) as f32
                * directional_strength[0][2]
                * strength[0]
                * scale0;
            let mut prev = Vec3::new(path.points[0], path.points[1], path.points[2]);

            for i in 1..path.length {
                // sample the middle of the segment, so coarse paths follow the same noise
                let a = (i as f64 - 0.5) / (path.length - 1) as f64;
                let px = j as f64 + a * length * scale[i];
                let py = a * scale[i];
                let sf = lerp(1.0, a as f32, fix_bottom[i]);

                let orig_dir = Vec3::new(
                    orig[i * 4] - orig[(i - 1) * 4],
                    orig[i * 4 + 1] - orig[(i - 1) * 4 + 1],
                    orig[i * 4 + 2] - orig[(i - 1) * 4 + 2],
                );
                let orig_len = orig_dir.length();

                // The push grows with the segment, so the angle a segment turns by does
                // not change with the number of points. It matches the old look at the
                // default resolution.
                let segment_scale = if length > 0.0 {
                    orig_len * (REFERENCE_RESOLUTION - 1.0) / length as f32
                } else {
                    0.0
                };
                let perturb = Vec3::new(
                    noise_x.get([px, py]) as f32 * directional_strength[i][0] * strength[i] * sf,
                    noise_y.get([px, py]) as f32 * directional_strength[i][1] * strength[i] * sf,
                    noise_z.get([px, py]) as f32 * directional_strength[i][2] * strength[i] * sf,
                ) * segment_scale;

                // Perturb the original direction and rescale to original length.
                // Biasing toward orig_dir prevents the segment from folding back.
                let mut new_dir = orig_dir + perturb;
                let nd_len = new_dir.length();
                if nd_len > 0.0001 && orig_len > 0.0001 {
                    new_dir *= orig_len / nd_len;
                } else {
                    new_dir = orig_dir;
                }

                let cur = prev + new_dir;
                path.points[i * 4] = cur.x;
                path.points[i * 4 + 1] = cur.y;
                path.points[i * 4 + 2] = cur.z;
                prev = cur;
            }
            path_data
        })
        .collect();

    concat_args(output.iter().map(|x| x.as_slice()).collect())
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use glam::Vec3;
    use nodarium_utils::{
        concat_args, encode_float,
        geometry::{create_multiple_paths, wrap_multiple_paths, wrap_path},
        split_args,
    };

    /// A straight stem from (0, 0, 0) up to (0, 5, 0) with `resolution` points.
    fn stem(resolution: usize) -> Vec<i32> {
        let mut stems = create_multiple_paths(1, resolution, 1, 3);
        for stem in wrap_multiple_paths(&mut stems).iter_mut() {
            for i in 0..resolution {
                let y = 5.0 * i as f32 / (resolution - 1) as f32;
                stem.points[i * 4..i * 4 + 4].copy_from_slice(&[0.0, y, 0.0, 0.1]);
            }
        }
        stems
    }

    fn tip(plant: &[i32], resolution: usize) -> Vec3 {
        let one = [encode_float(1.0)];
        let direction = concat_args(vec![&one, &one, &one]);
        let out = __nodarium_execute_inner(&[
            plant,
            &[encode_float(1.0)],
            &[encode_float(0.5)],
            &[encode_float(1.0)],
            &[7],
            &direction,
            &[1],
            &[1],
        ]);
        let path = split_args(&out)[0].to_vec();
        let p = wrap_path(&path).points;
        let s = (resolution - 1) * 4;
        Vec3::new(p[s], p[s + 1], p[s + 2])
    }

    #[test]
    fn the_shape_does_not_depend_on_the_resolution() {
        let reference = tip(&stem(32), 32);
        for resolution in [8, 16, 64] {
            let t = tip(&stem(resolution), resolution);
            assert!(
                t.distance(reference) < 0.05,
                "tip at resolution {} is {:?}, at 32 it is {:?}",
                resolution,
                t,
                reference
            );
        }
    }
}
