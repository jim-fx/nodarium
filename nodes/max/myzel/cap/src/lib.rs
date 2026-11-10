use myzel_core::{cap_alphas, grow_cap, CapFrame, CapShape};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{
    concat_arg_vecs, decode_float, decode_points,
    geometry::{is_path, path_link, wrap_path},
    int, split_args, Ctx,
};

nodarium_definition_file!("src/definition.json");

/// The words inside the brackets of a framed list `[0, n + 1, ...words, 1, 1]`
fn unframe(arg: &[i32]) -> Option<&[i32]> {
    let n = arg.len();
    let framed = n >= 4 && arg[0] == 0 && arg[1] as usize == n - 3 && arg[n - 2] == 1 && arg[n - 1] == 1;
    framed.then(|| &arg[2..n - 2])
}

/// A list of floats as the host sends a shape. The value on the node arrives as one frame, a
/// connected node sends `concat_args([frame])` = `[0, 1, ...frame, 1, 1]`. Not read with
/// `split_args`: a point at 0 would read as the start of a bracket.
fn read_floats(arg: &[i32]) -> Vec<f32> {
    let n = arg.len();
    let connected = n >= 8 && arg[..2] == [0, 1] && arg[n - 2..] == [1, 1] && unframe(&arg[2..n - 2]).is_some();
    let frame = if connected { &arg[2..n - 2] } else { arg };
    unframe(frame).map_or_else(Vec::new, |words| words.iter().map(|&w| decode_float(w)).collect())
}

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let mut ctx = Ctx::default();
    let plant = split_args(args[0]);
    let points = decode_points(args[1]);
    let profile = read_floats(args[3]);
    let resolution = int(args[6]).max(4) as usize;
    // the graph node that makes the cap, it is the last input
    let creator = int(args[7]);

    // the rays of every path, the ones of a point whose path is not in the plant last
    let mut rays: Vec<(i32, Vec<Vec<i32>>)> = Vec::new();
    for (rank, point) in points.iter().enumerate() {
        let stipe = plant.iter().copied().find(|p| is_path(p) && wrap_path(p).length > 1 && path_link(p).id == point.parent_id);
        // the frame the nodes after it find from the stipe alone
        let frame = match stipe {
            Some(stipe) => {
                ctx.set_path(stipe, rank, points.len());
                CapFrame::on(stipe, point.alpha)
            }
            None => {
                ctx.seed_path(point.parent_id);
                CapFrame::at(point)
            }
        };
        ctx.alpha = point.alpha;
        let radius = ctx.float(args[2]).max(0.0);
        let count = ctx.int(args[5]).max(1) as usize;
        // along the flesh from the centre to the margin, so a curve can thin it
        let thickness = ctx.float_along(args[4], cap_alphas());
        let shape = CapShape { profile: &profile, radius, thickness: &thickness, rays: count, resolution, creator };
        let own = match rays.iter().position(|r| r.0 == point.parent_id) {
            Some(i) => &mut rays[i].1,
            None => {
                rays.push((point.parent_id, Vec::new()));
                &mut rays.last_mut().unwrap().1
            }
        };
        // a second cap on the same stipe counts on, so every ray keeps its own id
        let grown = grow_cap(&frame, stipe.map_or(point.depth, |s| s[3]), own.len() as i32, &shape);
        own.extend(grown);
    }

    // like branch_from_points, the rays of a path come right before it
    let mut output: Vec<Vec<i32>> = Vec::new();
    for item in &plant {
        if is_path(item) {
            if let Some(i) = rays.iter().position(|r| r.0 == path_link(item).id) {
                output.append(&mut rays[i].1);
            }
        }
        output.push(item.to_vec());
    }
    for (_, own) in rays.iter_mut() {
        output.append(own);
    }
    concat_arg_vecs(output)
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use glam::{Vec2, Vec3};
    use myzel_core::{margin_points, Cap, CapFrame};
    use nodarium_utils::{
        concat_args, encode_float, encode_points,
        geometry::{create_multiple_paths, interpolate_along_path, is_path, orthogonal_to, path_link, wrap_multiple_paths, wrap_path, PathLink},
        split_args, Point,
    };

    const CREATOR: i32 = 7;
    const STIPE_TOP: f32 = 0.05;
    const STIPE_RADIUS: f32 = 0.005;

    /// An upright stipe 5 cm tall, 5 mm in radius
    fn stipe() -> Vec<i32> {
        stipes(1)
    }

    /// `count` of them, 10 cm apart, each leaning a little more
    fn stipes(count: usize) -> Vec<i32> {
        let mut data = create_multiple_paths(count, 12, 1, 3);
        for (s, path) in wrap_multiple_paths(&mut data).iter_mut().enumerate() {
            let tilt = glam::Quat::from_rotation_z(0.2 * s as f32);
            for i in 0..12 {
                let p = Vec3::new(0.1 * s as f32, 0.0, 0.0) + tilt * Vec3::new(0.0, STIPE_TOP * i as f32 / 11.0, 0.0);
                path.points[i * 4..i * 4 + 4].copy_from_slice(&[p.x, p.y, p.z, STIPE_RADIUS]);
            }
        }
        data
    }

    /// A point on every path at `alpha`, as points_on_path places them
    fn points_at(plant: &[i32], alpha: f32) -> Vec<Point> {
        split_args(plant)
            .iter()
            .map(|p| {
                let path = wrap_path(p);
                let (position, orthogonal, tangent) = interpolate_along_path(path.points, alpha);
                Point {
                    position: position.truncate(),
                    direction: orthogonal,
                    tangent,
                    scale: position.w,
                    parent_id: path.link.id,
                    alpha,
                    depth: path.depth,
                    index: 0,
                }
            })
            .collect()
    }

    fn floats(values: &[f32]) -> Vec<i32> {
        let mut words = vec![0, values.len() as i32 + 1];
        words.extend(values.iter().map(|&v| encode_float(v)));
        words.extend([1, 1]);
        words
    }

    /// The output of a curve node over alpha, from 0 to `max`
    fn curve_node(points: &[f32], max: f32) -> Vec<i32> {
        let mut args: Vec<Vec<i32>> =
            vec![vec![3], concat_args(vec![&[2], &[0]]), vec![encode_float(0.0)], vec![encode_float(max)]];
        args.extend(points.iter().map(|&p| vec![encode_float(p)]));
        concat_args(args.iter().map(|a| a.as_slice()).collect())
    }

    #[derive(Clone)]
    struct Params {
        radius: f32,
        profile: Vec<f32>,
        thickness: Vec<i32>,
        rays: i32,
    }

    /// A cone from the top at y = 20 down to the margin at y = 60 of the shape editor: the margin
    /// is 0.8 radii below the top. The flesh thins to nothing at the margin, so the rays end on
    /// the drawn outline.
    fn cone() -> Params {
        Params {
            radius: 0.03,
            profile: vec![0.0, 60.0, 50.0, 20.0],
            thickness: curve_node(&[0.0, 1.0, 1.0, 0.0], 0.006),
            rays: 10,
        }
    }

    fn execute_on(plant: &[i32], points: &[Point], p: &Params, profile: &[i32]) -> Vec<i32> {
        __nodarium_execute_inner(&[
            plant,
            &encode_points(points),
            &[encode_float(p.radius)],
            profile,
            &p.thickness,
            &[p.rays],
            &[24],
            &[CREATOR],
        ])
    }

    /// Every path the node gives, the stipes and the rays
    fn grow_on(plant: &[i32], points: &[Point], p: &Params) -> Vec<Vec<i32>> {
        split_args(&execute_on(plant, points, p, &floats(&p.profile))).iter().map(|s| s.to_vec()).collect()
    }

    /// The rays of a cap at the tip of the stipe
    fn run(p: &Params) -> Vec<Vec<i32>> {
        let plant = stipe();
        grow_on(&plant, &points_at(&plant, 1.0), p).into_iter().filter(|r| path_link(r).has_parent()).collect()
    }

    fn margins(paths: &[Vec<i32>]) -> Vec<Point> {
        margin_points(&paths.iter().map(|p| p.as_slice()).collect::<Vec<_>>())
    }

    fn with_stipe(rays: &[Vec<i32>]) -> Vec<Vec<i32>> {
        rays.iter().cloned().chain([split_args(&stipe())[0].to_vec()]).collect()
    }

    fn points(path: &[i32]) -> Vec<[f32; 4]> {
        wrap_path(path).points.chunks_exact(4).map(|p| [p[0], p[1], p[2], p[3]]).collect()
    }

    /// Distance from the axis and height over the stipe top
    fn place(p: Vec3) -> Vec2 {
        Vec2::new(Vec2::new(p.x, p.z).length(), p.y - STIPE_TOP)
    }

    fn place_of(q: [f32; 4]) -> Vec2 {
        place(Vec3::new(q[0], q[1], q[2]))
    }

    #[test]
    fn the_margin_lies_on_the_radius_and_height_of_the_profile() {
        let p = cone();
        let rays = run(&p);
        let margin = margins(&with_stipe(&rays));
        assert_eq!(margin.len(), rays.len());
        // the top of the flesh over the stipe is a thickness above the stipe top
        let height = 0.006 - 0.8 * p.radius;
        for m in &margin {
            let q = place(m.position);
            assert!((q.x - p.radius).abs() < 1e-4, "margin {} from the axis, radius {}", q.x, p.radius);
            assert!((q.y - height).abs() < 1e-4, "margin at {} over the stipe top, expected {}", q.y, height);
        }
        // in a ring around the axis, counted from the side of the stipe frame
        let frame = CapFrame::on(split_args(&stipe())[0], 1.0);
        let angles: Vec<f32> = margin.iter().map(|m| frame.angle(m.position)).collect();
        assert!(angles.windows(2).all(|w| w[1] > w[0]));

        for m in margins(&with_stipe(&run(&Params { radius: 0.05, ..cone() }))) {
            assert!((place(m.position).x - 0.05).abs() < 1e-4);
        }
    }

    #[test]
    fn the_outside_of_the_flesh_is_the_drawn_outline() {
        let p = Params { thickness: vec![encode_float(0.006)], ..cone() };
        let slope = Vec2::new(1.0, -0.8).normalize();
        // the drawn outline runs from the top at (0, thickness) down to the margin
        let top = Vec2::new(0.0, 0.006);
        for ray in &run(&p) {
            for q in points(ray).iter().skip(12).take(8) {
                let above = (place_of(*q) - top).perp_dot(slope);
                assert!((q[3] - above).abs() < 2e-4, "flesh reaches {} over the outline", q[3] - above);
            }
        }
    }

    #[test]
    fn the_thickness_follows_alpha_from_the_centre_to_the_margin() {
        let ray = points(&run(&cone())[0]);
        // the flesh thins to nothing at the margin, as the curve over alpha does
        assert!(ray.last().unwrap()[3] < 1e-4);
        let middle = ray[ray.len() / 2][3];
        assert!(middle > 0.001 && middle < 0.003, "half the flesh {middle} halfway out");
        let even = run(&Params { thickness: vec![encode_float(0.006)], ..cone() });
        assert!((points(&even[0]).last().unwrap()[3] - 0.003).abs() < 1e-6);
    }

    #[test]
    fn rays_leave_the_stipe_top_inside_the_stipe() {
        let rays = run(&cone());
        let stipe_id = wrap_path(split_args(&stipe())[0]).link.id;
        assert_eq!(rays.len(), 10);
        for (k, ray) in rays.iter().enumerate() {
            assert_eq!(path_link(ray), PathLink::child(CREATOR, stipe_id, 1.0, k as i32));
            assert_eq!(ray[3], 2);
            let (start, next) = (points(ray)[0], points(ray)[1]);
            let (a, b) = (place_of(start), place_of(next));
            assert!(a.x < STIPE_RADIUS && a.y < -0.5 * STIPE_RADIUS);
            assert!(a.x + start[3] < STIPE_RADIUS + 1e-6, "ray starts {} out of a stipe of {}", a.x + start[3], STIPE_RADIUS);
            // it leaves upwards, along the stipe strands
            assert!(b.y - a.y > b.x - a.x, "ray leaves from {:?} to {:?}", a, b);
        }
    }

    #[test]
    fn the_cap_at_the_tip_sits_on_the_stipe_top_whichever_way_the_point_faces() {
        let plant = stipes(2);
        let mut at = points_at(&plant, 1.0);
        let paths = grow_on(&plant, &at, &cone());
        // the frame of the cap is the stipe top and its last segment, its first ray turns out
        // from orthogonal_to that, as the nodes after the cap find it from the stipe alone
        let caps = Cap::all(&paths.iter().map(|p| p.as_slice()).collect::<Vec<_>>());
        assert_eq!(caps.len(), 2);
        for (cap, stipe) in caps.iter().zip(split_args(&plant)) {
            let p = points(stipe);
            let (tip, below) = (Vec3::from_slice(&p[11][..3]), Vec3::from_slice(&p[10][..3]));
            assert_eq!(cap.frame.hinge, tip);
            assert_eq!(cap.frame.axis, (tip - below).normalize());
            assert_eq!(cap.frame.side, orthogonal_to(cap.frame.axis));
            assert_eq!(cap.frame.stipe_radius, STIPE_RADIUS);
        }
        for point in &mut at {
            point.direction = -point.direction;
            point.position += Vec3::splat(1e-3);
        }
        assert_eq!(grow_on(&plant, &at, &cone()), paths);
    }

    #[test]
    fn gives_its_paths_with_the_rays_of_a_path_right_before_it() {
        let plant = stipes(3);
        let paths = grow_on(&plant, &points_at(&plant, 1.0), &cone());
        assert_eq!(paths.len(), 33);
        assert!(paths.iter().all(|p| is_path(p)));
        for (s, stipe) in split_args(&plant).iter().enumerate() {
            assert_eq!(&paths[11 * s + 10], stipe);
            let id = path_link(stipe).id;
            assert!(paths[11 * s..11 * s + 10].iter().all(|r| path_link(r).parent_id == id));
        }
        // only the paths of the points get a cap
        let one = grow_on(&plant, &points_at(&plant, 1.0)[1..2], &cone());
        assert_eq!(one.len(), 13);
        assert_eq!(&one[..1], &split_args(&plant)[..1]);
    }

    #[test]
    fn caps_on_one_stipe_keep_their_own_ids() {
        let plant = stipe();
        let mut at = points_at(&plant, 1.0);
        at.extend(points_at(&plant, 0.5));
        let paths = grow_on(&plant, &at, &cone());
        assert_eq!(paths.len(), 21);
        let ids: std::collections::HashSet<i32> = paths.iter().map(|p| path_link(p).id).collect();
        assert_eq!(ids.len(), 21);
        let caps = Cap::all(&paths.iter().map(|p| p.as_slice()).collect::<Vec<_>>());
        assert_eq!(caps.len(), 2);
        assert!((caps[1].frame.hinge.y - 0.5 * STIPE_TOP).abs() < 1e-6);
    }

    #[test]
    fn a_point_off_the_plant_grows_its_cap_at_the_point() {
        let elsewhere = stipes(2);
        let at = points_at(&elsewhere, 1.0);
        let paths = grow_on(&stipe(), &at[1..], &cone());
        assert_eq!(paths.len(), 11);
        assert_eq!(&paths[..1], &split_args(&stipe())[..]);
        let start = Vec3::from_slice(&points(&paths[1])[0][..3]);
        assert!(start.distance(at[1].position) < 0.01);
    }

    #[test]
    fn the_same_inputs_give_the_same_cap() {
        let p = cone();
        assert_eq!(run(&p), run(&p));
    }

    #[test]
    fn every_stipe_moves_its_rays_its_own_way() {
        let plant = stipes(2);
        let paths = grow_on(&plant, &points_at(&plant, 1.0), &cone());
        let caps = Cap::all(&paths.iter().map(|p| p.as_slice()).collect::<Vec<_>>());
        let gap = std::f32::consts::TAU / 10.0;
        let angles: Vec<Vec<f32>> = caps.iter().map(|c| c.rays.iter().map(|r| r.angle).collect()).collect();
        for (k, angle) in angles.iter().flatten().enumerate() {
            // up to 15 % of the gap from its even place
            let even = (k % 10) as f32 * gap;
            let off = (angle - even + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
            assert!(off.abs() <= 0.15 * gap + 1e-4, "ray {k} is {off} from its place");
        }
        assert!(angles[0].iter().zip(&angles[1]).any(|(a, b)| (a - b).abs() > 1e-3));
    }

    #[test]
    fn reads_a_connected_shape_node() {
        let p = cone();
        let profile = floats(&p.profile);
        let wrapped = concat_args(vec![&profile]);
        assert_eq!(super::read_floats(&wrapped), p.profile);
        let plant = stipe();
        let at = points_at(&plant, 1.0);
        assert_eq!(split_args(&execute_on(&plant, &at, &p, &wrapped)), split_args(&execute_on(&plant, &at, &p, &profile)));
    }
}
