use glam::{Mat3, Mat4, Vec3, Vec4, Vec4Swizzles};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{
    abi::{self, Output},
    concat_args, encode_float,
    geometry::{
        colors_at_lengths, extrude_path, extrude_path_sections, extruded_size, geometry_flags,
        geometry_path_id, is_collapsed, is_geometry, is_path, path_link, split_items,
        wrap_geometry_data, wrap_instance_data, wrap_path, GeometryData, PathData,
        PathStructure, GEOMETRY_HEAD, GEOMETRY_TAG, NO_PATH,
    },
    int, wrap_arg, NO_COLOR,
};
use std::collections::HashMap;

nodarium_definition_file!("src/definition.json");

const INSTANCES_TAG: i32 = 2;

#[derive(Default)]
struct Mesh {
    positions: Vec<f32>,
    normals: Vec<f32>,
    faces: Vec<i32>,
    path_ids: Vec<i32>,
    /// Empty, or an RGBA8 color per vertex once a part with colors was added
    colors: Vec<u32>,
}

/// A part to bake: the vertices, faces and colors of a geometry
struct Part<'a> {
    positions: &'a [f32],
    normals: &'a [f32],
    faces: &'a [i32],
    /// Empty for a part without colors
    colors: &'a [u32],
}

impl Mesh {
    fn add(&mut self, part: &Part, transform: Mat4, path_id: i32) {
        let Part { positions, normals, faces, colors } = *part;
        let offset = (self.positions.len() / 3) as i32;
        // the vertices of a part without colors keep the material, as do those before the first
        if !colors.is_empty() || !self.colors.is_empty() {
            self.colors.resize(offset as usize, NO_COLOR);
            if colors.is_empty() {
                self.colors.extend(std::iter::repeat_n(NO_COLOR, positions.len() / 3));
            } else {
                self.colors.extend_from_slice(colors);
            }
        }
        let normal_matrix = Mat3::from_mat4(transform).inverse().transpose();
        for (position, normal) in positions.chunks_exact(3).zip(normals.chunks_exact(3)) {
            let position = transform.transform_point3(Vec3::from_slice(position));
            let normal = (normal_matrix * Vec3::from_slice(normal)).normalize_or_zero();
            self.positions.extend_from_slice(&position.to_array());
            self.normals.extend_from_slice(&normal.to_array());
        }
        self.faces.extend(faces.iter().map(|i| i + offset));
        self.path_ids.push(path_id);
    }

    /// `[1, vertex_amount, face_amount, path_id, flags, ...faces, ...positions, ...normals,
    /// ...colors?]` in brackets, without groups: a geometry with groups would be joined with the
    /// others by `join`, and the variants of an instance node would become one. The path id is
    /// kept when every part comes from the same path.
    fn encode(self) -> Vec<i32> {
        let mut out = vec![
            1,
            (self.positions.len() / 3) as i32,
            (self.faces.len() / 3) as i32,
            common_path_id(&self.path_ids),
            if self.colors.is_empty() { 0 } else { geometry_flags::COLORS },
        ];
        out.extend(self.faces);
        out.extend(self.positions.iter().map(|&f| encode_float(f)));
        out.extend(self.normals.iter().map(|&f| encode_float(f)));
        out.extend(self.colors.iter().map(|&c| c as i32));
        wrap_arg(&out)
    }
}

/// The id of the path every part comes from, `NO_PATH` when they come from several
fn common_path_id(path_ids: &[i32]) -> i32 {
    match path_ids.split_first() {
        Some((first, rest)) if rest.iter().all(|id| id == first) => *first,
        _ => NO_PATH,
    }
}

/// The meshes the parts are baked into. A part goes into the mesh of the path it was made
/// from, a part of no known path goes into every mesh.
struct Variants {
    meshes: Vec<Mesh>,
    mesh_of_path: HashMap<i32, usize>,
}

impl Variants {
    /// One mesh for everything.
    fn single() -> Self {
        Self {
            meshes: vec![Mesh::default()],
            mesh_of_path: HashMap::new(),
        }
    }

    /// One mesh per path without a parent in `items`, in the order of the list. The paths
    /// growing on it, and theirs, go into the same mesh.
    fn per_stem(items: &[&[i32]]) -> Self {
        let paths: Vec<Vec<i32>> =
            items.iter().filter(|item| is_path(item)).map(|item| item.to_vec()).collect();
        let structure = PathStructure::new(&paths);
        let root = |mut index: usize| {
            for _ in 0..paths.len() {
                match structure.parent(index) {
                    Some(parent) => index = parent,
                    None => break,
                }
            }
            index
        };

        let mesh_of_stem: HashMap<usize, usize> = (0..paths.len())
            .filter(|&index| structure.parent(index).is_none())
            .enumerate()
            .map(|(mesh, stem)| (stem, mesh))
            .collect();

        Self {
            meshes: (0..mesh_of_stem.len()).map(|_| Mesh::default()).collect(),
            mesh_of_path: paths
                .iter()
                .enumerate()
                .filter_map(|(index, path)| {
                    let mesh = *mesh_of_stem.get(&root(index))?;
                    Some((path_link(path).id, mesh))
                })
                .collect(),
        }
    }

    fn add(&mut self, part: &Part, transform: Mat4, path_id: i32) {
        match self.mesh_of_path.get(&path_id) {
            Some(&mesh) => self.meshes[mesh].add(part, transform, path_id),
            None => {
                for mesh in &mut self.meshes {
                    mesh.add(part, transform, path_id);
                }
            }
        }
    }

    fn add_geometry(&mut self, item: &[i32]) {
        let mut data = item.to_vec();
        let geometry = wrap_geometry_data(&mut data);
        let part = Part {
            positions: geometry.positions,
            normals: geometry.normals,
            faces: geometry.faces,
            colors: geometry.colors,
        };
        self.add(&part, Mat4::IDENTITY, geometry_path_id(item));
    }

    fn add_instances(&mut self, item: &[i32]) {
        let mut data = item.to_vec();
        let instances = wrap_instance_data(&mut data);
        let part = Part {
            positions: instances.positions,
            normals: instances.normals,
            faces: instances.faces,
            colors: instances.colors,
        };
        for (matrix, path_id) in instances.instances.chunks_exact(16).zip(instances.path_ids.iter()) {
            let transform = Mat4::from_cols_slice(matrix);
            if is_collapsed(&transform) {
                continue;
            }
            self.add(&part, transform, *path_id);
        }
    }
}

/// The points of a path spread evenly along its length, `count` of them.
fn resample(points: &[f32], count: usize) -> Vec<f32> {
    let points: Vec<Vec4> = points.chunks_exact(4).map(Vec4::from_slice).collect();
    let mut distances = vec![0.0];
    for pair in points.windows(2) {
        distances.push(distances[distances.len() - 1] + pair[0].xyz().distance(pair[1].xyz()));
    }
    let total = distances[distances.len() - 1];

    let mut resampled = Vec::with_capacity(count * 4);
    let mut segment = 0;
    for i in 0..count {
        let target = total * i as f32 / (count - 1) as f32;
        while segment + 2 < points.len() && distances[segment + 1] < target {
            segment += 1;
        }
        let length = distances[segment + 1] - distances[segment];
        let t = if length > 0.0 {
            ((target - distances[segment]) / length).clamp(0.0, 1.0)
        } else {
            0.0
        };
        resampled.extend(points[segment].lerp(points[segment + 1], t).to_array());
    }
    resampled
}

/// The points and colors of a path resampled to `max_points`, `None` when it has no more
fn resampled(path: &PathData, max_points: usize) -> Option<(Vec<f32>, Vec<u32>)> {
    if max_points < 2 || path.length <= max_points {
        return None;
    }
    let points = resample(path.points, max_points);
    let colors = if path.colors.is_empty() {
        Vec::new()
    } else {
        let shares = (0..max_points).map(|i| i as f32 / (max_points - 1) as f32);
        colors_at_lengths(path.points, path.colors, shares)
    };
    Some((points, colors))
}

/// The paths that are drawn, a bud, a path that has not started to grow, is not
fn is_drawn(item: &[i32]) -> bool {
    is_path(item) && !path_link(item).is_bud()
}

/// Turns `items` into baked meshes: one, or with `per_stem` one per stem
fn bake(items: &[&[i32]], circle_segments: usize, max_points: usize, per_stem: bool) -> Vec<i32> {
    let mut variants = if per_stem { Variants::per_stem(items) } else { Variants::single() };
    if variants.meshes.is_empty() {
        variants = Variants::single();
    }

    for &item in items {
        if is_drawn(item) {
            let path = wrap_path(item);
            let tube = match resampled(&path, max_points) {
                Some((points, colors)) => extrude_path(
                    PathData { length: max_points, points: &points, colors: &colors, ..path },
                    circle_segments,
                ),
                None => extrude_path(path, circle_segments),
            };
            variants.add_geometry(&tube);
        } else if is_geometry(item) {
            variants.add_geometry(item);
        } else if item.get(2) == Some(&INSTANCES_TAG) {
            variants.add_instances(item);
        }
    }

    let mut meshes: Vec<Vec<i32>> = variants.meshes.into_iter().map(Mesh::encode).collect();
    if meshes.len() == 1 {
        return meshes.remove(0);
    }
    concat_args(meshes.iter().map(Vec::as_slice).collect())
}

fn as_f32(words: &mut [i32]) -> &mut [f32] {
    // SAFETY: f32 has the size and alignment of i32 and takes any bits
    unsafe { std::slice::from_raw_parts_mut(words.as_mut_ptr() as *mut f32, words.len()) }
}

/// The vertices, faces, whether there are colors and the creators of the tubes of `paths`
fn tubes_len(paths: &[PathData], circle_segments: usize) -> (usize, usize, bool, Vec<i32>) {
    let (mut vertices, mut faces, mut colored) = (0, 0, false);
    let mut creators = Vec::new();
    for path in paths {
        let (v, f) = extruded_size(path.points.len() / 4, circle_segments);
        vertices += v;
        faces += f;
        colored |= v > 0 && !path.colors.is_empty();
        if v > 0 && !creators.contains(&path.link.creator) {
            creators.push(path.link.creator);
        }
    }
    (vertices, faces, colored, creators)
}

/// The tubes of `paths` as one geometry written into `words`, which have exactly the room for it:
/// a group for the paths of every creator, the colors of the paths that have them
fn write_tubes(paths: &[PathData], circle_segments: usize, words: &mut [i32]) {
    let (vertices, faces, colored, creators) = tubes_len(paths, circle_segments);
    let mut flags = geometry_flags::GROUPS | geometry_flags::CREATORS;
    if colored {
        flags |= geometry_flags::COLORS;
    }
    let path_ids: Vec<i32> = paths.iter().map(|path| path.link.id).collect();
    let len = words.len();
    words[..GEOMETRY_HEAD].copy_from_slice(&[
        0,
        len as i32 - 3,
        GEOMETRY_TAG,
        vertices as i32,
        faces as i32,
        common_path_id(&path_ids),
        flags,
    ]);
    words[len - 2..].copy_from_slice(&[1, 1]);

    let (face_words, rest) = words[GEOMETRY_HEAD..len - 2].split_at_mut(faces * 3);
    let (positions, rest) = rest.split_at_mut(vertices * 3);
    let (normals, rest) = rest.split_at_mut(vertices * 3);
    let (colors, rest) = rest.split_at_mut(if colored { vertices } else { 0 });
    let (groups, table) = rest.split_at_mut(vertices);
    table[0] = creators.len() as i32;
    table[1..].copy_from_slice(&creators);
    colors.fill(NO_COLOR as i32);

    let positions = as_f32(positions);
    let normals = as_f32(normals);
    // SAFETY: u32 has the size and alignment of i32
    let colors = unsafe { std::slice::from_raw_parts_mut(colors.as_mut_ptr() as *mut u32, colors.len()) };

    let (mut vertex, mut face) = (0, 0);
    for path in paths {
        let (v, f) = extruded_size(path.points.len() / 4, circle_segments);
        if v == 0 {
            continue;
        }
        let faces_of_path = &mut face_words[face * 3..(face + f) * 3];
        extrude_path_sections(
            path,
            circle_segments,
            GeometryData {
                positions: &mut positions[vertex * 3..(vertex + v) * 3],
                normals: &mut normals[vertex * 3..(vertex + v) * 3],
                faces: faces_of_path,
                colors: if path.colors.is_empty() { &mut [][..] } else { &mut colors[vertex..vertex + v] },
            },
        );
        for index in faces_of_path.iter_mut() {
            *index += vertex as i32;
        }
        let group = creators.iter().position(|&c| c == path.link.creator).unwrap_or(0) as i32;
        groups[vertex..vertex + v].fill(group);
        vertex += v;
        face += f;
    }
}

/// The paths of `items` as tubes in one geometry, the geometry and the instances passed on as
/// they are
fn keep_instances(input: &[i32], items: &[&[i32]], circle_segments: usize, max_points: usize) -> Output {
    let kept: Vec<&[i32]> = items
        .iter()
        .copied()
        .filter(|item| is_geometry(item) || item.get(2) == Some(&INSTANCES_TAG))
        .collect();
    let drawn: Vec<&[i32]> = items.iter().copied().filter(|item| is_drawn(item)).collect();

    if drawn.is_empty() {
        if kept.is_empty() {
            return wrap_arg(&[1, 0, 0, NO_PATH, 0]).into();
        }
        if kept.len() == items.len() {
            return abi::frame_input(input, &[], &[]).map_or_else(|| input.to_vec().into(), Output::from);
        }
    }

    let paths: Vec<PathData> = drawn.iter().map(|item| wrap_path(item)).collect();
    let resamples: Vec<Option<(Vec<f32>, Vec<u32>)>> =
        paths.iter().map(|path| resampled(path, max_points)).collect();
    let paths: Vec<PathData> = paths
        .into_iter()
        .zip(&resamples)
        .map(|(path, resample)| match resample {
            Some((points, colors)) => PathData { length: max_points, points, colors, ..path },
            None => path,
        })
        .collect();

    let tubes = if paths.is_empty() {
        0
    } else {
        let (v, f, colored, creators) = tubes_len(&paths, circle_segments);
        // a group per vertex, the number of creators and each of them, the closing bracket
        GEOMETRY_HEAD + f * 3 + v * (7 + usize::from(colored)) + 1 + creators.len() + 2
    };
    if kept.is_empty() {
        return abi::write_result(tubes, |words| {
            write_tubes(&paths, circle_segments, words);
            tubes
        })
        .into();
    }
    if tubes == 0 && kept.len() == 1 {
        return kept[0].to_vec().into();
    }

    // every item comes with its own brackets, so the list is just the items between [0, 1] and [1, 1]
    let len = 4 + tubes + kept.iter().map(|item| item.len()).sum::<usize>();
    abi::write_result(len, |words| {
        words[..2].copy_from_slice(&[0, 1]);
        let mut at = 2;
        if tubes > 0 {
            write_tubes(&paths, circle_segments, &mut words[at..at + tubes]);
            at += tubes;
        }
        for item in &kept {
            words[at..at + item.len()].copy_from_slice(item);
            at += item.len();
        }
        words[at..at + 2].copy_from_slice(&[1, 1]);
        len
    })
    .into()
}

#[nodarium_execute(arena)]
pub fn execute(args: &[&[i32]]) -> Output {
    let input = args[0];
    let mut circle_segments = int(args[1]) as usize;
    let max_circle_segments = int(args[2]) as usize;
    if max_circle_segments > 0 {
        circle_segments = circle_segments.min(max_circle_segments);
    }
    let max_points = int(args[3]) as usize;
    let bake_instances = int(args[4]) != 0;
    let per_stem = int(args[5]) != 0;

    let items = split_items(input);
    if bake_instances || per_stem {
        return bake(&items, circle_segments, max_points, per_stem).into();
    }
    keep_instances(input, &items, circle_segments, max_points)
}

#[cfg(test)]
mod tests {
    use glam::{Mat4, Vec3};
    use nodarium_utils::{
        concat_args,
        geometry::{
            create_instance_data, create_multiple_paths, create_path, extrude_path, geometry_path_id,
            has_geometry_colors, is_geometry, path_link, set_path_link, split_items, with_geometry_colors,
            with_path_colors, wrap_geometry_data, wrap_instance_data, wrap_multiple_paths, wrap_path,
            wrap_path_mut, Mesh, MeshWords, PathLink, NO_PATH,
        },
        split_args, wrap_arg, NO_COLOR,
    };

    const OUTPUT: i32 = 0;
    const BAKE: i32 = 1;

    fn run(input: &[i32], circle: i32, max_circle: i32, max_points: i32, bake: i32, variants: i32) -> Vec<i32> {
        let args: [&[i32]; 6] = [input, &[circle], &[max_circle], &[max_points], &[bake], &[variants]];
        unsafe { super::__nodarium_execute_inner(&args).words() }.to_vec()
    }

    fn baked(input: &[i32]) -> Vec<i32> {
        run(input, 8, 0, 0, BAKE, 0)
    }

    fn variants(input: &[i32]) -> Vec<i32> {
        run(input, 8, 0, 0, BAKE, 1)
    }

    fn output(input: &[i32]) -> Vec<i32> {
        run(input, 8, 0, 0, OUTPUT, 0)
    }

    /// A triangle along +x and +y, its normals look along +z.
    fn triangle(path_id: i32) -> Vec<i32> {
        let mut data = vec![1, 3, 1, path_id, 0, 0, 1, 2];
        let positions = [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0f32];
        let normals = [0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0f32];
        data.extend(positions.iter().chain(&normals).map(|f| f.to_bits() as i32));
        wrap_arg(&data)
    }

    /// Two triangles in the groups 0 and 1, made by the creators 41 and 42
    fn tissues() -> Vec<i32> {
        Mesh {
            faces: vec![0, 1, 2, 3, 4, 5],
            positions: [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0].repeat(2),
            normals: [0.0, 0.0, 1.0].repeat(6),
            colors: Vec::new(),
            groups: vec![0, 0, 0, 1, 1, 1],
            creators: vec![41, 42],
        }
        .encode()
    }

    fn instances_of(geometry: &[i32], transforms: &[(Mat4, i32)]) -> Vec<i32> {
        let mut geometry = geometry.to_vec();
        let geo = wrap_geometry_data(&mut geometry);
        let mut data = create_instance_data(3, 1, transforms.len());
        let mut instances = wrap_instance_data(&mut data);
        instances.set_geometry(geo);
        for (i, (transform, path_id)) in transforms.iter().enumerate() {
            instances.set_transformation_matrix(i, &transform.to_cols_array());
            instances.set_path_id(i, *path_id);
        }
        data
    }

    fn vec3s(data: &[f32]) -> Vec<Vec3> {
        data.chunks_exact(3).map(Vec3::from_slice).collect()
    }

    /// `amount` stems of the node `creator` along +y, stem i is i + 1 meters long.
    fn stems(amount: usize, creator: i32) -> Vec<i32> {
        let mut stems = create_multiple_paths(amount, 3, 1, creator);
        for (i, stem) in wrap_multiple_paths(&mut stems).iter_mut().enumerate() {
            for j in 0..3 {
                let y = (i + 1) as f32 * j as f32 / 2.0;
                stem.points[j * 4..j * 4 + 4].copy_from_slice(&[0.0, y, 0.0, 0.1]);
            }
        }
        stems
    }

    /// A stem along +y with a point every meter.
    fn straight_stem(points: usize) -> Vec<i32> {
        let mut stems = create_multiple_paths(1, points, 1, 6);
        for stem in wrap_multiple_paths(&mut stems).iter_mut() {
            for i in 0..points {
                stem.points[i * 4..i * 4 + 4].copy_from_slice(&[0.0, i as f32, 0.0, 0.1]);
            }
        }
        stems
    }

    fn vertex_and_face_amount(geometry: &[i32]) -> (i32, i32) {
        (geometry[3], geometry[4])
    }

    fn highest_point(geometry: &[i32]) -> f32 {
        let mut geometry = geometry.to_vec();
        vec3s(wrap_geometry_data(&mut geometry).positions).iter().map(|p| p.y).fold(f32::MIN, f32::max)
    }

    fn a_branch_on(parent_id: i32) -> Vec<i32> {
        let mut branch = create_path(2, 2);
        set_path_link(&mut branch, PathLink::child(9, parent_id, 0.5, 0));
        wrap_path_mut(&mut branch).points.copy_from_slice(&[0.0, 10.0, 0.0, 0.1, 1.0, 10.0, 0.0, 0.1]);
        branch
    }

    fn mesh(words: &[i32]) -> Mesh {
        MeshWords::read(words).expect("a geometry").to_mesh()
    }

    #[test]
    fn paths_become_one_geometry_with_a_group_per_creator() {
        let first = stems(2, 5);
        let second = stems(1, 7);
        let input = concat_args(split_args(&first).into_iter().chain(split_args(&second)).collect());

        let result = output(&input);
        let words = MeshWords::read(&result).expect("a geometry");
        assert_eq!(words.creators(), &[5, 7]);
        assert_eq!(words.path_id(), NO_PATH);
        let groups = words.groups().unwrap();
        let tube = vertex_and_face_amount(&extrude_path(wrap_path(split_args(&first)[0]), 8)).0 as usize;
        assert_eq!(groups.len(), 3 * tube);
        assert!(groups[..2 * tube].iter().all(|&g| g == 0));
        assert!(groups[2 * tube..].iter().all(|&g| g == 1));
    }

    #[test]
    fn the_tubes_are_the_ones_the_output_draws() {
        let input = stems(3, 5);
        let mut expected = Mesh::default();
        for path in split_args(&input) {
            let tube = mesh(&extrude_path(wrap_path(path), 8));
            expected.append(&tube, 0);
        }

        let result = mesh(&output(&input));

        assert_eq!(result.faces, expected.faces);
        assert_eq!(result.positions, expected.positions);
        assert_eq!(result.normals, expected.normals);
        assert!(result.colors.is_empty());
    }

    #[test]
    fn a_single_path_keeps_its_id() {
        let stem = straight_stem(3);
        let result = output(&stem);
        assert_eq!(geometry_path_id(&result), path_link(split_args(&stem)[0]).id);
        assert!((highest_point(&result) - 2.0).abs() < 1e-5);
    }

    #[test]
    fn instances_and_geometry_are_passed_on() {
        let leaves = instances_of(&triangle(NO_PATH), &[(Mat4::IDENTITY, 3)]);
        let surface = tissues();
        let paths = stems(1, 5);
        let input = concat_args(vec![split_args(&paths)[0], &leaves, &surface, &triangle(4)]);

        let result = output(&input);
        let items = split_args(&result);

        assert_eq!(items.len(), 4);
        assert_eq!(mesh(items[0]).creators, vec![5]);
        assert_eq!(items[1], leaves.as_slice());
        assert_eq!(items[2], surface.as_slice());
        assert_eq!(items[3], triangle(4).as_slice());
    }

    #[test]
    fn a_geometry_with_groups_passes_through_as_it_is() {
        let surface = tissues();
        assert_eq!(output(&surface), surface);
        let list = concat_args(vec![&surface, &triangle(NO_PATH)]);
        assert_eq!(output(&list), list);
    }

    #[test]
    fn colors_of_paths_are_kept_and_paths_without_them_keep_the_material() {
        let plain = stems(1, 5);
        let other = stems(1, 6);
        let colored = with_path_colors(split_args(&other)[0], &[7, 8, 9]);
        let input = concat_args(vec![split_args(&plain)[0], &colored]);

        let result = mesh(&output(&input));

        let tube = result.colors.len() / 2;
        assert!(result.colors[..tube].iter().all(|&c| c == NO_COLOR));
        assert_eq!(&result.colors[tube..tube + 8], &[7; 8]);
        assert_eq!(&result.colors[tube + 8..tube + 16], &[8; 8]);
        assert!(result.colors[tube + 16..].iter().all(|&c| c == 9));
    }

    #[test]
    fn a_bud_is_not_drawn() {
        let mut bud = create_path(2, 1);
        set_path_link(&mut bud, PathLink { grown: 0.0, ..PathLink::root(5, 0) });
        let input = concat_args(vec![&bud, &triangle(NO_PATH)]);
        assert_eq!(output(&input), triangle(NO_PATH));
    }

    #[test]
    fn an_unconnected_input_gives_an_empty_geometry() {
        for mut result in [output(&[0, 1, 1, 1]), baked(&[0, 1, 1, 1]), variants(&[0, 1, 1, 1])] {
            let geometry = wrap_geometry_data(&mut result);
            assert!(geometry.positions.is_empty());
            assert!(geometry.faces.is_empty());
        }
    }

    #[test]
    fn the_limits_lower_the_resolution_of_paths() {
        let stems = straight_stem(10);

        for bake in [OUTPUT, BAKE] {
            let mut result = run(&stems, 8, 4, 3, bake, 0);

            // 3 rings of 4, a cap ring and its center
            assert_eq!(vertex_and_face_amount(&result), (3 * 4 + 4 + 1, 2 * 4 * 2 + 4));
            let geometry = wrap_geometry_data(&mut result);
            let heights: Vec<f32> = vec3s(geometry.positions).iter().map(|p| p.y).collect();
            assert!(heights.iter().any(|y| (y - 4.5).abs() < 1e-5));
            assert!(heights.iter().any(|y| (y - 9.0).abs() < 1e-5));
        }
    }

    #[test]
    fn without_limits_paths_get_the_full_resolution() {
        let stems = straight_stem(10);
        let expected = extrude_path(wrap_path(split_args(&stems)[0]), 8);
        assert_eq!(vertex_and_face_amount(&output(&stems)), vertex_and_face_amount(&expected));
        assert_eq!(vertex_and_face_amount(&baked(&stems)), vertex_and_face_amount(&expected));
    }

    #[test]
    fn a_lower_resolution_setting_wins_over_the_limit() {
        let stems = straight_stem(10);
        let expected = extrude_path(wrap_path(split_args(&stems)[0]), 4);
        let result = run(&stems, 4, 16, 0, OUTPUT, 0);
        assert_eq!(vertex_and_face_amount(&result), vertex_and_face_amount(&expected));
    }

    #[test]
    fn a_resampled_path_keeps_its_colors() {
        let mut stem = create_path(5, 1);
        for (i, point) in wrap_path_mut(&mut stem).points.chunks_exact_mut(4).enumerate() {
            point.copy_from_slice(&[0.0, i as f32, 0.0, 0.1]);
        }
        let colors = [0xff00_0000, 0xff00_0040, 0xff00_0080, 0xff00_00c0, 0xff00_00ff];
        let stem = concat_args(vec![&with_path_colors(&stem, &colors)]);
        for bake in [OUTPUT, BAKE] {
            let mut result = run(&stem, 4, 0, 3, bake, 0);
            let baked = wrap_geometry_data(&mut result).colors.to_vec();
            assert_eq!(&baked[..4], &[colors[0]; 4]);
            assert_eq!(&baked[4..8], &[colors[2]; 4]);
            assert!(baked[8..].iter().all(|&c| c == colors[4]));
        }
    }

    #[test]
    fn baked_instances_become_part_of_the_geometry() {
        let moved = Mat4::from_translation(Vec3::new(0.0, 5.0, 0.0));
        let turned = Mat4::from_scale_rotation_translation(
            Vec3::splat(2.0),
            glam::Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
            Vec3::ZERO,
        );
        let instances = instances_of(&triangle(NO_PATH), &[(moved, 7), (turned, 8)]);
        let input = concat_args(vec![&triangle(NO_PATH), &instances]);

        let mut result = baked(&input);
        assert_eq!(geometry_path_id(&result), NO_PATH);
        // nobody is known to have made them, so they are one group
        assert!(!MeshWords::read(&result).unwrap().has_groups());
        let geometry = wrap_geometry_data(&mut result);

        assert_eq!(geometry.positions.len() / 3, 9);
        assert_eq!(geometry.faces, &[0, 1, 2, 3, 4, 5, 6, 7, 8]);
        let positions = vec3s(geometry.positions);
        let normals = vec3s(geometry.normals);
        assert!((positions[4] - Vec3::new(1.0, 5.0, 0.0)).length() < 1e-5);
        assert!((positions[7] - Vec3::new(0.0, 0.0, -2.0)).length() < 1e-5);
        assert!((normals[3] - Vec3::Z).length() < 1e-5);
        assert!((normals[6] - Vec3::X).length() < 1e-5);
    }

    #[test]
    fn baking_gives_one_geometry_without_groups() {
        let paths = stems(2, 5);
        let mut input = split_args(&paths);
        let surface = tissues();
        input.push(&surface);

        let result = baked(&concat_args(input));

        let words = MeshWords::read(&result).expect("a geometry");
        assert!(!words.has_groups());
        assert_eq!(words.vertex_count(), mesh(&output(&paths)).vertex_count() + 6);
    }

    #[test]
    fn a_single_geometry_passes_through() {
        let input = triangle(3);
        assert_eq!(baked(&input), input);
        assert_eq!(output(&input), input);
    }

    #[test]
    fn the_result_can_be_split_from_a_list_like_any_geometry() {
        let result = output(&stems(2, 5));
        let list = concat_args(vec![&result, &result]);
        let items = split_args(&list);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0], result.as_slice());
    }

    #[test]
    fn variants_give_a_geometry_per_stem() {
        let stems = stems(3, 5);
        let ids: Vec<i32> = split_args(&stems).iter().map(|s| path_link(s).id).collect();

        let result = variants(&stems);
        let variants = split_items(&result);

        assert_eq!(variants.len(), 3);
        for (i, variant) in variants.iter().enumerate() {
            assert!(is_geometry(variant));
            assert_eq!(geometry_path_id(variant), ids[i]);
            assert!((highest_point(variant) - (i + 1) as f32).abs() < 1e-5);
        }
    }

    #[test]
    fn variants_bake_even_without_bake() {
        let stems = stems(3, 5);
        assert_eq!(run(&stems, 8, 0, 0, OUTPUT, 1), variants(&stems));
    }

    #[test]
    fn without_variants_all_stems_are_one_geometry() {
        let stems = stems(3, 5);
        let result = baked(&stems);
        assert!(is_geometry(&result));
        assert_eq!(geometry_path_id(&result), NO_PATH);
        assert!((highest_point(&result) - 3.0).abs() < 1e-5);
    }

    #[test]
    fn what_grows_on_a_stem_belongs_to_its_variant() {
        let stems = stems(2, 5);
        let first = path_link(split_args(&stems)[0]).id;
        let branch = a_branch_on(first);
        let leaves = instances_of(
            &triangle(NO_PATH),
            &[(Mat4::from_translation(Vec3::new(0.0, 20.0, 0.0)), path_link(&branch).id)],
        );
        let mut input = split_args(&stems);
        input.push(&branch);
        input.push(&leaves);

        let result = variants(&concat_args(input));
        let variants = split_items(&result);

        assert_eq!(variants.len(), 2);
        assert!((highest_point(variants[0]) - 21.0).abs() < 1e-5);
        assert!((highest_point(variants[1]) - 2.0).abs() < 1e-5);
    }

    #[test]
    fn a_part_of_no_stem_goes_into_every_variant() {
        let stems = stems(2, 5);
        let mut input = split_args(&stems);
        let loose = triangle(NO_PATH);
        input.push(&loose);

        let alone = variants(&stems);
        let result = variants(&concat_args(input));

        for (with, without) in split_items(&result).iter().zip(split_items(&alone)) {
            assert_eq!(with[3], without[3] + 3);
            assert_eq!(geometry_path_id(with), NO_PATH);
        }
    }

    #[test]
    fn variants_of_a_single_stem_are_one_geometry() {
        let stems = straight_stem(10);
        assert_eq!(variants(&stems), baked(&stems));
    }

    #[test]
    fn variants_without_stems_bake_everything_into_one_geometry() {
        let input = triangle(3);
        assert_eq!(variants(&input), input);
    }

    #[test]
    fn baked_colors_and_parts_without_them_keep_the_material() {
        let colored = with_geometry_colors(&triangle(NO_PATH), &[1, 2, 3]);
        let input = concat_args(vec![&triangle(NO_PATH), &colored]);
        let mut result = baked(&input);
        assert_eq!(wrap_geometry_data(&mut result).colors, &[NO_COLOR, NO_COLOR, NO_COLOR, 1, 2, 3]);

        let plain = concat_args(vec![&triangle(NO_PATH), &triangle(NO_PATH)]);
        assert!(!has_geometry_colors(&baked(&plain)));
    }
}
