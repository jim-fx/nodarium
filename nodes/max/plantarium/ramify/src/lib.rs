use glam::Vec3;
use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;
use nodarium_utils::{
    concat_arg_vecs, decode_points, int,
    geometry::{
        create_path, orthogonal_to, rotate_vector_by_angle, set_path_link, wrap_path_mut,
        PathLink, BUD_LENGTH,
    },
    Ctx, PathInfo,
};

nodarium_definition_file!("src/definition.json");

const POINTS: usize = 0;
const ARCHITECTURE: usize = 1;
const GENERATIONS: usize = 2;
const LENGTH: usize = 3;
const RATIO: usize = 4;
const LATERALS: usize = 5;
const ANGLE: usize = 6;
const DIVERGENCE: usize = 7;
const DOMINANCE: usize = 8;
const THICKNESS: usize = 9;
const GROWTH: usize = 10;
const RESOLUTION: usize = 11;
const CREATOR: usize = 12;

const SYMPODIAL: i32 = 1;
const MAX_LATERALS: i32 = 16;
/// Every generation multiplies the paths, a few more would freeze the app before the geometry
/// is drawn.
pub const MAX_PATHS: usize = 8192;
const MIN_LENGTH: f32 = 1e-6;

/// One axis of the plant, a straight path: a monopodial axis is the leader with all the
/// internodes it adds, a sympodial one a single module.
struct Axis {
    link: PathLink,
    depth: i32,
    parent: Option<usize>,
    /// How far from the base of the full-grown parent the axis starts.
    attach: f32,
    /// The generation of its first internode.
    start: usize,
    base: Vec3,
    heading: Vec3,
    /// The side the first lateral points to before the divergence turns it.
    side: Vec3,
    first: f32,
    internodes: Vec<f32>,
    /// Its place among the daughters of the node that made it.
    rank: usize,
    count: usize,
    clock: f32,
    thickness: f32,
}

impl Axis {
    fn length(&self) -> f32 {
        self.internodes.iter().sum()
    }

    fn end(&self) -> usize {
        self.start + self.internodes.len()
    }

    /// Internode k grows while the clock goes from its generation to the next one.
    fn grown_length(&self) -> f32 {
        self.internodes
            .iter()
            .enumerate()
            .map(|(k, l)| l * (self.clock - (self.start + k) as f32).clamp(0.0, 1.0))
            .sum()
    }

    fn grown(&self, grown_length: f32) -> f32 {
        if grown_length <= 0.0 {
            0.0
        } else if self.clock >= self.end() as f32 {
            1.0 + (self.clock - self.end() as f32) / self.internodes.len() as f32
        } else {
            (grown_length / self.length()).min(1.0)
        }
    }
}

/// Turns `heading` by `tilt` towards `side` and gives the new heading and side, like a turtle
/// that pitches, so the side keeps pointing away from where the axis came from.
fn pitch(heading: Vec3, side: Vec3, tilt: f32) -> (Vec3, Vec3) {
    let (sin, cos) = tilt.sin_cos();
    (
        (heading * cos + side * sin).normalize(),
        (side * cos - heading * sin).normalize(),
    )
}

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let mut ctx = Ctx::default();

    let sympodial = int(args[ARCHITECTURE]) == SYMPODIAL;
    let generations = ctx.int(args[GENERATIONS]).clamp(1, 12) as usize;
    let max_resolution = int(args[RESOLUTION]).max(4) as usize;
    let creator = int(args[CREATOR]);
    // alpha of the rule, from the first node a tip makes to the last
    let rule_alpha = |g: usize| (g as f32 / generations.saturating_sub(2).max(1) as f32).min(1.0);

    let points = decode_points(args[POINTS]);
    let mut axes: Vec<Axis> = Vec::new();
    if points.is_empty() {
        axes.push(root(PathLink::root(creator, 0), 1, Vec3::ZERO, Vec3::Y, 0, 1));
    } else {
        for (i, point) in points.iter().enumerate() {
            let mut link = PathLink::child(creator, point.parent_id, point.alpha, point.index);
            if !link.has_parent() {
                link.parent_alpha = 0.0;
            }
            let heading = point.direction.try_normalize().unwrap_or(Vec3::Y);
            axes.push(root(link, point.depth + 1, point.position, heading, i, points.len()));
        }
    }
    let roots = axes.len();

    // an axis only needs what its parent decided, so the axes are rewritten one after the
    // other, every one through all of its generations
    let mut a = 0;
    while a < axes.len() {
        let parent = axes[a].parent;
        let parent_length = parent.map(|p| axes[p].length()).unwrap_or(0.0);
        if parent.is_some() {
            axes[a].link.parent_alpha = if parent_length > 0.0 {
                (axes[a].attach / parent_length).min(1.0)
            } else {
                1.0
            };
        }
        let axis = &axes[a];
        ctx.set_path_info(PathInfo {
            index: axis.link.index.max(0) as usize,
            rank: axis.rank,
            count: axis.count,
            depth: axis.depth,
            parent_position: if axis.link.has_parent() { axis.link.parent_alpha } else { 0.0 },
            length: axis.first,
            grown: 1.0,
        });
        // the random values of an axis start from its id, so they stay when other axes change
        ctx.seed_path(axis.link.id);
        if a < roots {
            let first = ctx.float(args[LENGTH]).max(0.0);
            let thickness = ctx.float(args[THICKNESS]).max(0.0);
            ctx.path.length = first;
            axes[a].first = first;
            axes[a].thickness = thickness;
        }
        let axis = &axes[a];
        let (id, depth, start, heading, side, thickness) =
            (axis.link.id, axis.depth, axis.start, axis.heading, axis.side, axis.thickness);

        ctx.alpha = rule_alpha(start);
        let clock = ctx.float(args[GROWTH]).max(0.0) * generations as f32;
        let parent_clock = parent.map(|p| axes[p].clock).unwrap_or(f32::INFINITY);

        let mut internodes = Vec::new();
        let mut daughters = Vec::new();
        let mut length = axes[a].first;
        let mut distance = 0.0;
        let mut roll = 0.0;
        for (node, generation) in (start..generations).enumerate() {
            internodes.push(length);
            distance += length;
            if generation + 1 >= generations {
                break;
            }

            ctx.alpha = rule_alpha(generation);
            ctx.path.length = length;
            let laterals = ctx.int(args[LATERALS]).clamp(0, MAX_LATERALS);
            let next = length * ctx.float(args[RATIO]).max(0.0);
            let dominance = ctx.float(args[DOMINANCE]).clamp(0.0, 1.0);
            roll += ctx.float(args[DIVERGENCE]);

            for j in 0..laterals {
                // the angle is read for every lateral, so a random gives each its own
                let angle = ctx.float(args[ANGLE]);
                let leader = sympodial && j == 0;
                let (tilt, first) = if leader {
                    (angle * (1.0 - dominance), next)
                } else {
                    (angle, next * (1.0 - dominance))
                };
                if first <= MIN_LENGTH {
                    continue;
                }
                let around = roll + std::f32::consts::TAU * j as f32 / laterals as f32;
                let (heading, side) =
                    pitch(heading, rotate_vector_by_angle(side, heading, around), tilt);
                daughters.push(Axis {
                    // the id only depends on the parent, the node and the number of the lateral,
                    // so it stays when other nodes make more or fewer laterals
                    link: PathLink::child(creator, id, 1.0, node as i32 * MAX_LATERALS + j),
                    depth: depth + 1,
                    parent: Some(a),
                    attach: distance,
                    start: generation + 1,
                    base: Vec3::ZERO,
                    heading,
                    side,
                    first,
                    internodes: Vec::new(),
                    rank: j as usize,
                    count: laterals as usize,
                    clock: 0.0,
                    thickness,
                });
            }
            if sympodial {
                break;
            }
            length = next;
        }

        if axes.len() + daughters.len() > MAX_PATHS {
            panic!(
                "ramify: {} generations of this rule make more than {} paths. Lower generations or laterals, or raise dominance",
                generations, MAX_PATHS
            );
        }
        let axis = &mut axes[a];
        axis.internodes = internodes;
        // an axis never grows ahead of the one it sits on, so it starts when its node is reached
        axis.clock = clock.min(parent_clock);
        axes.extend(daughters);
        a += 1;
    }

    let grown_lengths: Vec<f32> = axes.iter().map(|a| a.grown_length()).collect();
    let longest = axes.iter().map(|a| a.length()).fold(0.0_f32, f32::max);
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); axes.len()];
    for (i, axis) in axes.iter().enumerate() {
        if let Some(p) = axis.parent {
            children[p].push(i);
        }
    }
    // the leaves of a tip, which the cross section carries too, weigh as much as its last
    // internode, so a tip ends blunt once it has grown and a young one stays closed
    let tips: Vec<(f32, f32)> = axes
        .iter()
        .enumerate()
        .map(|(i, axis)| {
            let length = axis.length();
            if children[i].iter().any(|&c| axes[c].attach >= length) {
                return (0.0, 0.0);
            }
            let last = axis.internodes.len() - 1;
            let l = axis.internodes[last];
            (l * (axis.clock - (axis.start + last) as f32).clamp(0.0, 1.0), l)
        })
        .collect();
    // what grows above an axis, its own length, its tip and everything on it
    let mut carried: Vec<f32> = grown_lengths.iter().zip(&tips).map(|(l, t)| l + t.0).collect();
    let mut carried_full: Vec<f32> = axes.iter().zip(&tips).map(|(a, t)| a.length() + t.1).collect();
    let mut plant = vec![0; axes.len()];
    for i in (0..axes.len()).rev() {
        if let Some(p) = axes[i].parent {
            carried[p] += carried[i];
            carried_full[p] += carried_full[i];
        }
    }
    for i in 0..axes.len() {
        if let Some(p) = axes[i].parent {
            plant[i] = plant[p];
        } else {
            plant[i] = i;
        }
    }

    // where every axis starts on its parent as it is now, at the tip of a parent that has not
    // grown that far
    for i in 0..axes.len() {
        if let Some(p) = axes[i].parent {
            axes[i].base = axes[p].base + axes[p].heading * axes[i].attach.min(grown_lengths[p]);
        }
    }

    // the pipe model: the cross section is the share of the plant that grows above it
    let thickness_at = |i: usize, s: f32| {
        let total = carried_full[plant[i]];
        if total <= 0.0 {
            return 0.0;
        }
        let above = (grown_lengths[i] - s).max(0.0)
            + tips[i].0
            + children[i].iter().filter(|&&c| axes[c].attach >= s).map(|&c| carried[c]).sum::<f32>();
        axes[i].thickness * (above / total).sqrt()
    };

    let mut output = Vec::with_capacity(axes.len());
    for (i, axis) in axes.iter().enumerate() {
        let full = axis.length();
        let resolution = if longest > 0.0 {
            ((max_resolution as f32 * full / longest).round() as usize).clamp(4, max_resolution)
        } else {
            max_resolution
        };
        let grown_length = grown_lengths[i];
        let length = grown_length.max(BUD_LENGTH);
        // the branches of a fork split the cross section of the axis they continue, they widen
        // to it over their first few radii so the fork has no step
        let flare = match axis.parent {
            Some(p) if axis.attach >= axes[p].length() => {
                let parent = thickness_at(p, axis.attach.min(grown_lengths[p]));
                (parent - thickness_at(i, 0.0)).max(0.0)
            }
            _ => 0.0,
        };
        let flare_length = 3.0 * (thickness_at(i, 0.0) + flare);

        let mut data = create_path(resolution, axis.depth);
        set_path_link(&mut data, PathLink { grown: axis.grown(grown_length), ..axis.link });
        let path = wrap_path_mut(&mut data);
        for j in 0..resolution {
            let s = length * j as f32 / (resolution - 1) as f32;
            let p = axis.base + axis.heading * s;
            path.points[j * 4..j * 4 + 3].copy_from_slice(&p.to_array());
            let widen = if flare_length > 0.0 { (1.0 - s / flare_length).max(0.0).powi(2) } else { 0.0 };
            path.points[j * 4 + 3] = thickness_at(i, s) + flare * widen;
        }
        output.push(data);
    }

    concat_arg_vecs(output)
}

fn root(link: PathLink, depth: i32, base: Vec3, heading: Vec3, rank: usize, count: usize) -> Axis {
    Axis {
        link,
        depth,
        parent: None,
        attach: 0.0,
        start: 0,
        base,
        heading,
        side: orthogonal_to(heading),
        first: 0.0,
        internodes: Vec::new(),
        rank,
        count,
        clock: 0.0,
        thickness: 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::{__nodarium_execute_inner, MAX_PATHS};
    use glam::Vec3;
    use nodarium_utils::{
        concat_args, encode_float,
        geometry::{path_link, wrap_path, PathLink, NO_PARENT},
        split_args,
    };
    use std::collections::HashMap;

    const NODE: i32 = 7;

    struct Rule {
        architecture: i32,
        generations: i32,
        laterals: i32,
        angle: Vec<i32>,
        dominance: f32,
        growth: f32,
    }

    impl Default for Rule {
        fn default() -> Self {
            Self {
                architecture: 0,
                generations: 4,
                laterals: 2,
                angle: vec![encode_float(0.7)],
                dominance: 0.3,
                growth: 1.0,
            }
        }
    }

    fn run(rule: &Rule) -> Vec<i32> {
        __nodarium_execute_inner(&[
            &[],
            &[rule.architecture],
            &[rule.generations],
            &[encode_float(0.5)],
            &[encode_float(0.8)],
            &[rule.laterals],
            &rule.angle,
            &[encode_float(2.3998)],
            &[encode_float(rule.dominance)],
            &[encode_float(0.05)],
            &[encode_float(rule.growth)],
            &[8],
            &[NODE],
        ])
    }

    struct Path {
        link: PathLink,
        depth: i32,
        points: Vec<[f32; 4]>,
    }

    impl Path {
        fn at(&self, i: usize) -> Vec3 {
            Vec3::from_slice(&self.points[i][..3])
        }

        fn length(&self) -> f32 {
            (1..self.points.len()).map(|i| self.at(i).distance(self.at(i - 1))).sum()
        }

        fn heading(&self) -> Vec3 {
            (self.at(self.points.len() - 1) - self.at(0)).normalize()
        }
    }

    fn paths(result: &[i32]) -> Vec<Path> {
        split_args(result)
            .iter()
            .map(|p| {
                let path = wrap_path(p);
                Path {
                    link: path_link(p),
                    depth: path.depth,
                    points: path.points.chunks_exact(4).map(|c| [c[0], c[1], c[2], c[3]]).collect(),
                }
            })
            .collect()
    }

    fn random_angle(seed: i32) -> Vec<i32> {
        concat_args(vec![&[1], &[encode_float(0.3)], &[encode_float(1.2)], &[seed]])
    }

    #[test]
    fn a_monopodial_rule_makes_laterals_plus_one_to_the_power_of_its_nodes() {
        for (generations, laterals) in [(1, 2), (4, 1), (4, 2), (5, 3)] {
            let rule = Rule { generations, laterals, ..Default::default() };
            let count = paths(&run(&rule)).len();
            assert_eq!(count, (laterals as usize + 1).pow(generations as u32 - 1), "{generations} {laterals}");
        }
    }

    #[test]
    fn a_sympodial_rule_ends_every_tip_and_forks_it() {
        let rule = Rule { architecture: 1, generations: 5, laterals: 2, ..Default::default() };
        let paths = paths(&run(&rule));
        assert_eq!(paths.len(), 1 + 2 + 4 + 8 + 16);
        // every module is one internode, the next generation sits at its tip
        for path in paths.iter().filter(|p| p.link.has_parent()) {
            assert_eq!(path.link.parent_alpha, 1.0);
        }
        assert_eq!(paths.iter().map(|p| p.depth).max(), Some(5));
    }

    #[test]
    fn without_dominance_a_sympodial_fork_is_symmetric() {
        let rule = Rule { architecture: 1, generations: 2, dominance: 0.0, ..Default::default() };
        let paths = paths(&run(&rule));
        assert_eq!(paths.len(), 3);
        let axis = paths[0].heading();
        let (a, b) = (paths[1].heading(), paths[2].heading());
        assert!((a.angle_between(axis) - 0.7).abs() < 1e-4);
        assert!((b.angle_between(axis) - 0.7).abs() < 1e-4);
        assert!((a + b).normalize().distance(axis) < 1e-4, "{a} {b} {axis}");
        assert!((paths[1].length() - paths[2].length()).abs() < 1e-5);
    }

    #[test]
    fn with_dominance_the_leader_of_a_sympodial_node_continues_the_line() {
        let rule = Rule { architecture: 1, generations: 2, dominance: 0.5, ..Default::default() };
        let paths = paths(&run(&rule));
        let axis = paths[0].heading();
        let (leader, lateral) = (&paths[1], &paths[2]);
        assert!((leader.heading().angle_between(axis) - 0.35).abs() < 1e-4);
        assert!((lateral.length() - leader.length() * 0.5).abs() < 1e-5);
    }

    #[test]
    fn full_dominance_leaves_a_monopodial_axis_unbranched() {
        let rule = Rule { generations: 6, dominance: 1.0, ..Default::default() };
        let paths = paths(&run(&rule));
        assert_eq!(paths.len(), 1);
        // 0.5 * (1 + 0.8 + ... + 0.8^5)
        let expected = 0.5 * (1.0 - 0.8_f32.powi(6)) / 0.2;
        assert!((paths[0].length() - expected).abs() < 1e-4);
    }

    #[test]
    fn every_axis_knows_its_parent_and_has_its_own_id() {
        let paths = paths(&run(&Rule { generations: 5, ..Default::default() }));
        let by_id: HashMap<i32, &Path> = paths.iter().map(|p| (p.link.id, p)).collect();
        assert_eq!(by_id.len(), paths.len());
        for path in &paths {
            assert_eq!(path.link.creator, NODE);
            if !path.link.has_parent() {
                assert_eq!(path.link.parent_id, NO_PARENT);
                assert_eq!(path.depth, 1);
                continue;
            }
            let parent = by_id[&path.link.parent_id];
            assert_eq!(path.depth, parent.depth + 1);
            assert!((0.0..=1.0).contains(&path.link.parent_alpha));
            // the base lies on the parent where the link says
            let spot = parent.at(0).lerp(parent.at(parent.points.len() - 1), path.link.parent_alpha);
            assert!(spot.distance(path.at(0)) < 1e-4);
        }
    }

    #[test]
    fn the_ids_stay_when_other_nodes_make_more_laterals() {
        let ids = |laterals: i32| -> Vec<i32> {
            paths(&run(&Rule { generations: 3, laterals, ..Default::default() })).iter().map(|p| p.link.id).collect()
        };
        let few = ids(1);
        let many = ids(2);
        assert!(few.iter().all(|id| many.contains(id)), "{few:?} {many:?}");
    }

    #[test]
    fn the_pipe_model_thins_every_axis_towards_its_tip() {
        let paths = paths(&run(&Rule { generations: 5, ..Default::default() }));
        assert!((paths[0].points[0][3] - 0.05).abs() < 1e-6);
        let by_id: HashMap<i32, &Path> = paths.iter().map(|p| (p.link.id, p)).collect();
        for path in &paths {
            assert!(path.points.windows(2).all(|w| w[1][3] <= w[0][3] + 1e-7));
            // a tip ends blunt, as thick as the leaves of a last internode need
            assert!(path.points.last().unwrap()[3] > 0.0);
            if let Some(parent) = by_id.get(&path.link.parent_id) {
                assert!(path.points[0][3] <= parent.points[0][3]);
            }
        }
    }

    #[test]
    fn the_generations_grow_one_after_the_other() {
        let rule = |growth: f32| Rule { generations: 4, laterals: 1, growth, ..Default::default() };
        let full = paths(&run(&rule(1.0)));
        let ids = |paths: &[Path]| paths.iter().map(|p| p.link.id).collect::<Vec<_>>();
        for growth in [0.0, 0.3, 0.5, 0.75, 1.0] {
            let grown = paths(&run(&rule(growth)));
            // the plant has the same paths at every time, only their lengths change
            assert_eq!(ids(&grown), ids(&full));
            let by_id: HashMap<i32, &Path> = grown.iter().map(|p| (p.link.id, p)).collect();
            for (path, whole) in grown.iter().zip(&full) {
                assert!(path.length() <= whole.length() + 1e-5);
                let Some(parent) = by_id.get(&path.link.parent_id) else { continue };
                if parent.link.share() < path.link.parent_alpha {
                    assert!(path.link.is_bud(), "a lateral grows before its node is reached");
                }
            }
        }
        // a quarter of the time of 4 generations is the first internode of the leader
        let quarter = paths(&run(&rule(0.25)));
        assert!((quarter[0].length() - 0.5).abs() < 1e-4);
        assert!(quarter[1..].iter().all(|p| p.link.is_bud()));
        assert!(paths(&run(&rule(0.0))).iter().all(|p| p.link.is_bud()));
    }

    #[test]
    fn a_young_plant_is_thinner() {
        let rule = |growth: f32| Rule { generations: 4, growth, ..Default::default() };
        let young = paths(&run(&rule(0.5)))[0].points[0][3];
        let old = paths(&run(&rule(1.0)))[0].points[0][3];
        assert!(young > 0.0 && young < old);
    }

    #[test]
    fn the_same_seed_gives_the_same_plant_and_another_seed_another() {
        let plant = |seed: i32| run(&Rule { angle: random_angle(seed), ..Default::default() });
        assert_eq!(plant(3), plant(3));
        assert_ne!(plant(3), plant(4));
    }

    #[test]
    #[should_panic(expected = "more than 8192 paths")]
    fn a_rule_that_would_make_too_many_paths_stops_with_a_message() {
        assert_eq!(MAX_PATHS, 8192);
        run(&Rule { generations: 12, laterals: 3, ..Default::default() });
    }
}
