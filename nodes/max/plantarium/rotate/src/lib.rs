use glam::{Mat4, Vec3};
use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;
use nodarium_utils::{
    concat_args, evaluate_float, evaluate_int, geometry::wrap_path_mut, log,
    split_args,
};

nodarium_definition_file!("src/input.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {

    log!("DEBUG args: {:?}", args);


    let plants = split_args(args[0]);
    let axis = evaluate_int(args[1]); // 0 =x, 1 = y, 2 = z
    let spread = evaluate_int(args[3]);

    let output: Vec<Vec<i32>> = plants
        .iter()
        .enumerate()
        .map(|(j, _path_data)| {
            let mut path_data = _path_data.to_vec();

            // if this is not a path don't modify it
            if path_data[2] != 0 {
                return path_data;
            }

            let path = wrap_path_mut(&mut path_data);

            let angle = evaluate_float(args[2]);

            let origin = [path.points[0], path.points[1], path.points[2]];

            let axis = match axis {
                0 => Vec3::X,
                1 => Vec3::Y,
                2 => Vec3::Z,
                _ => panic!("Invalid axis"),
            };

            let rotation = if spread == 1 {
                let angle = angle * (j as f32 / plants.len() as f32);
                Mat4::from_axis_angle(axis, angle)
            } else {
                Mat4::from_axis_angle(axis, angle)
            };

            for i in 0..path.length {
                let mut p = Vec3::new(
                    path.points[i * 4] - origin[0],
                    path.points[i * 4 + 1] - origin[1],
                    path.points[i * 4 + 2] - origin[2],
                );
                p = rotation.transform_vector3(p);
                path.points[i * 4] = p.x + origin[0];
                path.points[i * 4 + 1] = p.y + origin[1];
                path.points[i * 4 + 2] = p.z + origin[2];
            }
            path_data
        })
        .collect();

    concat_args(output.iter().map(|x| x.as_slice()).collect())
}
