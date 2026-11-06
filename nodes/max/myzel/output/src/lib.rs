use myzel_core::{mesh_segments, polygonize, read_field, read_float, read_hyphae, read_int, FIELD_TAG, HYPHAE_TAG};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};

nodarium_definition_file!("src/definition.json");

const SHOW_SURFACE: i32 = 0;
const SHOW_HYPHAE: i32 = 1;
const CUT_OPEN: i32 = 1;

/// Wraps items in the list the viewer reads: [0, 1, [0, n + 1, ...item, 1, 1]..., 1, 1]
fn list(items: &[Vec<i32>]) -> Vec<i32> {
    let mut output = Vec::with_capacity(items.iter().map(|i| i.len() + 4).sum::<usize>() + 4);
    output.extend([0, 1]);
    for item in items {
        output.extend([0, item.len() as i32 + 1]);
        output.extend(item);
        output.extend([1, 1]);
    }
    output.extend([1, 1]);
    output
}

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let resolution = read_int(args[1]).clamp(16, 256) as usize;
    match args[0].first() {
        Some(&HYPHAE_TAG) => {
            let show = read_int(args[2]);
            let mut hyphae = read_hyphae(args[0]);
            let cut = read_int(args[4]) == CUT_OPEN;
            if cut {
                hyphae = hyphae.retain(|start, s| start.z <= 0.0 && s.end.z <= 0.0);
            }
            let mut items = Vec::new();
            if show != SHOW_HYPHAE {
                items.push(mesh_segments(&hyphae.to_segments(), resolution, read_float(args[3])).encode());
            }
            if show != SHOW_SURFACE {
                items.push(if cut { hyphae.encode() } else { args[0].to_vec() });
            }
            list(&items)
        }
        Some(&FIELD_TAG) => list(&[polygonize(&read_field(args[0]), resolution).encode()]),
        _ => list(&[]),
    }
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use glam::Vec3;
    use myzel_core::{flags, Field, HyphaSegment, Hyphae, Mesh, Tissue, HYPHAE_TAG, MESH_TAG};
    use nodarium_utils::{encode_float, split_args};

    fn run(input: &[i32], show: i32) -> Vec<Vec<i32>> {
        run_cut(input, show, 0)
    }

    fn run_cut(input: &[i32], show: i32, cut: i32) -> Vec<Vec<i32>> {
        let output = __nodarium_execute_inner(&[input, &[32], &[show], &[encode_float(3.0)], &[cut]]);
        split_args(&output).iter().map(|item| item[2..item.len() - 2].to_vec()).collect()
    }

    fn hyphae() -> Vec<i32> {
        let mut h = Hyphae::default();
        let root = h.push(HyphaSegment {
            end: Vec3::ZERO,
            radius: 0.1,
            parent: None,
            birth: 0.0,
            tissue: Tissue::Stipe,
            flags: 0,
        });
        h.push(HyphaSegment {
            end: Vec3::Y,
            radius: 0.1,
            parent: Some(root),
            birth: 1.0,
            tissue: Tissue::Stipe,
            flags: flags::ACTIVE_TIP,
        });
        h.encode()
    }

    #[test]
    fn wraps_the_mesh_of_a_field_in_a_list() {
        let items = run(&Field::sphere(1.0).encode(), 0);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0][0], MESH_TAG);
        assert!(Mesh::decode(&items[0]).face_count() > 0);
    }

    #[test]
    fn shows_hyphae_as_surface_lines_or_both() {
        let input = hyphae();
        let surface = run(&input, 0);
        assert_eq!(surface.len(), 1);
        assert!(Mesh::decode(&surface[0]).face_count() > 0);

        let lines = run(&input, 1);
        assert_eq!(lines, vec![input.clone()]);
        assert_eq!(lines[0][0], HYPHAE_TAG);

        let both = run(&input, 2);
        assert_eq!(both.len(), 2);
        assert_eq!(both[0][0], MESH_TAG);
        assert_eq!(both[1], input);
    }

    #[test]
    fn cutting_open_leaves_out_the_front_half() {
        let mut h = Hyphae::decode(&hyphae());
        h.push(HyphaSegment { end: Vec3::new(0.0, 1.0, 0.5), parent: Some(1), ..h.segments[1] });
        let lines = run_cut(&h.encode(), 1, 1);
        assert_eq!(Hyphae::decode(&lines[0]).len(), 2);
        assert_eq!(run_cut(&h.encode(), 1, 0)[0], h.encode());
    }

    #[test]
    fn nothing_connected_shows_nothing() {
        assert!(run(&[], 0).is_empty());
    }
}
