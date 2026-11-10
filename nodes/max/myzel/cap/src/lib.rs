use myzel_core::{cap_alphas, grow_cap, CapShape};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{
    concat_arg_vecs, decode_float, encode_points,
    geometry::{wrap_path, DeepestPaths},
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
pub fn execute(args: &[&[i32]]) -> (Vec<i32>, Vec<i32>) {
    let mut ctx = Ctx::default();
    let plant = split_args(args[0]);
    let profile = read_floats(args[2]);
    let resolution = int(args[5]).max(4) as usize;
    // the graph node that makes the cap, it is the last input
    let creator = int(args[6]);

    let stipes = DeepestPaths::new(&plant, 1);
    let selected = |p: &[i32]| stipes.contains(p) && wrap_path(p).length > 1;
    let count = plant.iter().filter(|p| selected(p)).count();

    let (mut output, mut hinges) = (Vec::new(), Vec::new());
    for (rank, item) in plant.iter().filter(|p| selected(p)).enumerate() {
        ctx.set_path(item, rank, count);
        // the cap sits at the tip of its stipe
        ctx.alpha = 1.0;
        let radius = ctx.float(args[1]).max(0.0);
        let rays = ctx.int(args[4]).max(1) as usize;
        // along the flesh from the centre to the margin, so a curve can thin it
        let thickness = ctx.float_along(args[3], cap_alphas());
        let shape = CapShape { profile: &profile, radius, thickness: &thickness, rays, resolution, creator };
        let (hinge, rays) = grow_cap(item, &shape);
        output.extend(rays);
        hinges.push(hinge);
    }

    // after the rays, so a ray keeps its place in the list
    output.extend(hinges.iter().map(|h| h.encode()));
    let points: Vec<_> = hinges.iter().map(|h| h.point()).collect();
    (concat_arg_vecs(output), encode_points(&points))
}

#[cfg(test)]
mod tests {
    use super::__nodarium_execute_inner;
    use glam::{Vec2, Vec3};
    use myzel_core::{margin_points, read_hinges, testing, CapHinge};
    use nodarium_utils::{
        concat_args, decode_points, encode_float,
        geometry::{create_multiple_paths, path_link, wrap_multiple_paths, wrap_path, PathLink},
        split_args, Point,
    };

    const CREATOR: i32 = 7;
    const STIPE_TOP: f32 = 0.05;
    const STIPE_RADIUS: f32 = 0.005;

    /// An upright stipe 5 cm tall, 5 mm in radius
    fn stipe() -> Vec<i32> {
        stipes(1)
    }

    /// `count` of them, 10 cm apart
    fn stipes(count: usize) -> Vec<i32> {
        let mut data = create_multiple_paths(count, 12, 1, 3);
        for (s, path) in wrap_multiple_paths(&mut data).iter_mut().enumerate() {
            for i in 0..12 {
                path.points[i * 4..i * 4 + 4].copy_from_slice(&[0.1 * s as f32, STIPE_TOP * i as f32 / 11.0, 0.0, STIPE_RADIUS]);
            }
        }
        data
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

    struct Out {
        rays: Vec<Vec<i32>>,
        hinges: Vec<Point>,
        /// The hinge items in the list of rays
        items: Vec<CapHinge>,
        margins: Vec<Point>,
    }

    fn execute(p: &Params, profile: &[i32]) -> (Vec<i32>, Vec<i32>) {
        execute_on(&stipe(), p, profile)
    }

    fn execute_on(plant: &[i32], p: &Params, profile: &[i32]) -> (Vec<i32>, Vec<i32>) {
        __nodarium_execute_inner(&[plant, &[encode_float(p.radius)], profile, &p.thickness, &[p.rays], &[24], &[CREATOR]])
    }

    fn run(p: &Params) -> Out {
        run_on(&stipe(), p)
    }

    fn run_on(plant: &[i32], p: &Params) -> Out {
        let (paths, hinges) = execute_on(plant, p, &floats(&p.profile));
        Out {
            rays: testing::paths(&paths),
            hinges: decode_points(&hinges),
            items: read_hinges(&paths),
            margins: margin_points(&split_args(&paths)),
        }
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
        let out = run(&p);
        assert_eq!(out.margins.len(), out.rays.len());
        // the top of the flesh over the stipe is a thickness above the stipe top
        let height = 0.006 - 0.8 * p.radius;
        for m in &out.margins {
            let q = place(m.position);
            assert!((q.x - p.radius).abs() < 1e-4, "margin {} from the axis, radius {}", q.x, p.radius);
            assert!((q.y - height).abs() < 1e-4, "margin at {} over the stipe top, expected {}", q.y, height);
        }
        // in a ring around the axis, counted from the side of the hinge
        let frame = myzel_core::CapFrame::new(&out.items[0]);
        let angles: Vec<f32> = out.margins.iter().map(|m| frame.angle(m.position)).collect();
        assert!(angles.windows(2).all(|w| w[1] > w[0]));

        let wide = run(&Params { radius: 0.05, ..cone() });
        for m in &wide.margins {
            assert!((place(m.position).x - 0.05).abs() < 1e-4);
        }
    }

    #[test]
    fn the_outside_of_the_flesh_is_the_drawn_outline() {
        let p = Params { thickness: vec![encode_float(0.006)], ..cone() };
        let out = run(&p);
        let slope = Vec2::new(1.0, -0.8).normalize();
        // the drawn outline runs from the top at (0, thickness) down to the margin
        let top = Vec2::new(0.0, 0.006);
        for ray in &out.rays {
            for q in points(ray).iter().skip(12).take(8) {
                let above = (place_of(*q) - top).perp_dot(slope);
                assert!((q[3] - above).abs() < 2e-4, "flesh reaches {} over the outline", q[3] - above);
            }
        }
    }

    #[test]
    fn the_thickness_follows_alpha_from_the_centre_to_the_margin() {
        let out = run(&cone());
        let ray = points(&out.rays[0]);
        // the flesh thins to nothing at the margin, as the curve over alpha does
        assert!(ray.last().unwrap()[3] < 1e-4);
        let middle = ray[ray.len() / 2][3];
        assert!(middle > 0.001 && middle < 0.003, "half the flesh {middle} halfway out");
        let even = run(&Params { thickness: vec![encode_float(0.006)], ..cone() });
        assert!((points(&even.rays[0]).last().unwrap()[3] - 0.003).abs() < 1e-6);
    }

    #[test]
    fn rays_leave_the_stipe_top_inside_the_stipe() {
        let out = run(&cone());
        let stipe_id = wrap_path(split_args(&stipe())[0]).link.id;
        assert_eq!(out.rays.len(), 10);
        for (k, ray) in out.rays.iter().enumerate() {
            assert_eq!(path_link(ray), PathLink::child(CREATOR, stipe_id, 1.0, k as i32));
            assert_eq!(ray[3], 2);
            let (start, next) = (points(ray)[0], points(ray)[1]);
            let (a, b) = (place_of(start), place_of(next));
            assert!(a.x < STIPE_RADIUS && a.y < -0.5 * STIPE_RADIUS);
            assert!(a.x + start[3] < STIPE_RADIUS + 1e-6, "ray starts {} out of a stipe of {}", a.x + start[3], STIPE_RADIUS);
            // it leaves upwards, along the stipe strands
            assert!(b.y - a.y > b.x - a.x, "ray leaves from {:?} to {:?}", a, b);
        }
        assert_eq!(out.hinges.len(), 1);
        let hinge = &out.hinges[0];
        assert!(hinge.position.distance(Vec3::new(0.0, STIPE_TOP, 0.0)) < 1e-6);
        assert!(hinge.tangent.distance(Vec3::Y) < 1e-5);
        assert_eq!(hinge.parent_id, stipe_id);
        assert!((hinge.scale - STIPE_RADIUS).abs() < 1e-7);
    }

    #[test]
    fn the_same_inputs_give_the_same_cap() {
        let p = cone();
        let (a, b) = (run(&p), run(&p));
        assert_eq!(a.rays, b.rays);
        assert_eq!(a.margins, b.margins);
    }

    /// Angle of every ray around the axis of its cap
    fn angles(out: &Out, cap: usize) -> Vec<f32> {
        let hinge = &out.items[cap];
        let frame = myzel_core::CapFrame::new(hinge);
        let own = out.rays.iter().filter(|r| path_link(r).parent_id == hinge.stipe);
        own.map(|r| frame.angle(Vec3::from_slice(&points(r).last().unwrap()[..3]))).collect()
    }

    #[test]
    fn every_stipe_moves_its_rays_its_own_way() {
        let out = run_on(&stipes(2), &cone());
        let gap = std::f32::consts::TAU / 10.0;
        let (a, b) = (angles(&out, 0), angles(&out, 1));
        assert_eq!((a.len(), b.len()), (10, 10));
        for (k, angle) in a.iter().chain(&b).enumerate() {
            // up to 15 % of the gap from its even place
            let even = (k % 10) as f32 * gap;
            let off = (angle - even + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
            assert!(off.abs() <= 0.15 * gap + 1e-4, "ray {k} is {off} from its place");
        }
        assert!(a.iter().zip(&b).any(|(x, y)| (x - y).abs() > 1e-3));
    }

    #[test]
    fn a_list_of_several_stipes_keeps_one_hinge_per_cap() {
        let plant = stipes(3);
        let out = run_on(&plant, &cone());
        assert_eq!(out.rays.len(), 30);
        assert_eq!(out.items.len(), 3);
        assert_eq!(out.items.iter().map(|h| h.point()).collect::<Vec<_>>(), out.hinges);
        for (stipe, hinge) in split_args(&plant).iter().zip(&out.items) {
            assert_eq!(hinge.stipe, wrap_path(stipe).link.id);
            assert_eq!(hinge.creator, CREATOR);
            assert_eq!(out.rays.iter().filter(|r| path_link(r).parent_id == hinge.stipe).count(), 10);
        }
        // the hinges come after the rays, every ray keeps its place in the list
        let (paths, _) = execute_on(&plant, &cone(), &floats(&cone().profile));
        let items = split_args(&paths);
        assert!(items[..30].iter().all(|i| nodarium_utils::geometry::is_path(i)));
        assert!(items[30..].iter().all(|i| CapHinge::decode(i).is_some()));
    }

    #[test]
    fn the_hinge_keeps_the_open_cap() {
        let p = cone();
        let out = run(&p);
        let hinge = &out.items[0];
        assert!((hinge.radius - p.radius).abs() < 1e-4, "radius {}", hinge.radius);
        // along the cone from the axis to the margin, and the bend from the root into the flesh
        let slant = Vec2::new(1.0, 0.8).length() * p.radius;
        assert!(hinge.length > slant && hinge.length < 1.15 * slant, "length {} of {slant}", hinge.length);
        assert_eq!(hinge.rest.len(), myzel_core::REST_SAMPLES);
        assert!(hinge.rest[0].abs() < 1e-6 && (hinge.rest.last().unwrap() - 1.0).abs() < 1e-6);
        // every point of a ray out on the cap is where the hinge says it was, up to the samples
        // it keeps. Near the top the root of the cone loops back over the axis once.
        let close = 0.01 * p.radius;
        let cap = &myzel_core::Cap::all(&split_args(&execute(&p, &floats(&p.profile)).0))[0];
        for ray in &cap.rays {
            for (q, &alpha) in ray.plane.iter().zip(&ray.alphas) {
                if alpha > 0.2 {
                    assert!((q.x - hinge.rest_distance(alpha)).abs() < close, "{} and {} at {alpha}", q.x, hinge.rest_distance(alpha));
                }
            }
        }
    }

    #[test]
    fn reads_a_connected_shape_node() {
        let p = cone();
        let profile = floats(&p.profile);
        let wrapped = concat_args(vec![&profile]);
        assert_eq!(super::read_floats(&wrapped), p.profile);
        let (paths, _) = execute(&p, &wrapped);
        assert_eq!(split_args(&paths), split_args(&execute(&p, &profile).0));
    }
}
