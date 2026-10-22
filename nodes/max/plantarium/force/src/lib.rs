use glam::Vec3;
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{concat_arg_vecs, int, Ctx, Force, ForceKind};

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let mut ctx = Ctx::default();

    let kind = ForceKind::from_index(int(args[0])).unwrap_or(ForceKind::Point);
    let position = ctx.vec3(args[1]);
    let force = Force {
        kind,
        position: Vec3::from(position),
        strength: ctx.float(args[2]),
        radius: ctx.float(args[3]).max(0.0),
        falloff: ctx.float(args[4]).clamp(0.0, 1.0),
    };

    concat_arg_vecs(vec![force.encode()])
}

#[cfg(test)]
mod tests {
    use nodarium_utils::{decode_forces, encode_float, Force};

    fn vec3(x: f32, y: f32, z: f32) -> Vec<i32> {
        vec![
            0,
            4,
            encode_float(x),
            encode_float(y),
            encode_float(z),
            1,
            4,
        ]
    }

    #[test]
    fn the_node_gives_one_force_with_its_inputs() {
        let result = super::__nodarium_execute_inner(&[
            &[0],
            &vec3(1.0, 2.0, 3.0),
            &[encode_float(-0.5)],
            &[encode_float(4.0)],
            &[encode_float(0.25)],
        ]);
        let forces = decode_forces(&result);

        assert_eq!(forces.len(), 1);
        let Force {
            position,
            strength,
            radius,
            falloff,
            ..
        } = forces[0];
        assert_eq!(position.to_array(), [1.0, 2.0, 3.0]);
        assert_eq!(strength, -0.5);
        assert_eq!(radius, 4.0);
        assert_eq!(falloff, 0.25);
    }
}
