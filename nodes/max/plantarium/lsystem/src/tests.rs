use super::__nodarium_execute_inner;
use glam::Vec3;
use nodarium_utils::{
    concat_args, decode_points, encode_float,
    geometry::{path_link, wrap_path, PathLink, NO_PARENT},
    split_args, Point,
};

const CREATOR: i32 = 7;

fn text(text: &str) -> Vec<i32> {
    let mut words = vec![0, text.chars().count() as i32 + 1];
    words.extend(text.chars().map(|c| c as i32));
    words.extend([1, 1]);
    words
}

struct Run<'a> {
    axiom: &'a str,
    rules: &'a str,
    generations: f32,
    length: Vec<i32>,
    angle: f32,
    taper: f32,
    tropism: f32,
}

impl Default for Run<'_> {
    fn default() -> Self {
        Self { axiom: "F", rules: "", generations: 1.0, length: vec![encode_float(1.0)], angle: 0.5, taper: 0.5, tropism: 0.0 }
    }
}

struct Plant {
    paths: Vec<Vec<i32>>,
    points: Vec<Point>,
}

impl Plant {
    fn links(&self) -> Vec<PathLink> {
        self.paths.iter().map(|p| path_link(p)).collect()
    }
    fn points_of(&self, path: usize) -> Vec<[f32; 4]> {
        wrap_path(&self.paths[path]).points.chunks_exact(4).map(|c| [c[0], c[1], c[2], c[3]]).collect()
    }
    fn tip(&self, path: usize) -> Vec3 {
        let p = self.points_of(path);
        let last = p[p.len() - 1];
        Vec3::new(last[0], last[1], last[2])
    }
}

fn run(run: Run) -> Plant {
    let result = __nodarium_execute_inner(&[
        &text(run.axiom),
        &text(run.rules),
        &[encode_float(run.generations)],
        &run.length,
        &[encode_float(run.angle)],
        &[encode_float(0.1)],
        &[encode_float(run.tropism)],
        &[encode_float(run.taper)],
        &[3],
        &[CREATOR],
    ]);
    let (paths, points) = result;
    Plant { paths: split_args(&paths).iter().map(|p| p.to_vec()).collect(), points: decode_points(&points) }
}

fn close(a: Vec3, b: Vec3) -> bool {
    a.distance(b) < 1e-4
}

#[test]
fn a_line_of_steps_is_one_path_straight_up() {
    let plant = run(Run { axiom: "FFF", generations: 0.0, ..Default::default() });
    assert_eq!(plant.paths.len(), 1);
    let link = plant.links()[0];
    assert_eq!(link, PathLink::root(CREATOR, 0));
    assert_eq!(plant.paths[0][3], 1);
    assert!(close(plant.tip(0), Vec3::new(0.0, 3.0, 0.0)));
}

#[test]
fn a_bracket_is_a_child_path_that_knows_where_it_grows() {
    let plant = run(Run { axiom: "F[+F]F", generations: 0.0, ..Default::default() });
    assert_eq!(plant.paths.len(), 2);
    // the child comes right before its parent
    let [child, stem] = [plant.links()[0], plant.links()[1]];
    assert_eq!(stem.parent_id, NO_PARENT);
    assert_eq!(child.parent_id, stem.id);
    assert_eq!(child.parent_alpha, 0.5);
    assert_eq!(plant.paths[0][3], 2);
    assert!(close(Vec3::from_slice(&plant.points_of(0)[0][..3]), Vec3::new(0.0, 1.0, 0.0)));
    // + turns to the left, toward -x seen from the front
    let tip = plant.tip(0);
    assert!(tip.x < 0.0 && (tip.y - 1.0 - 0.5f32.cos()).abs() < 1e-4, "{tip}");
    assert!(close(plant.tip(1), Vec3::new(0.0, 2.0, 0.0)));
}

#[test]
fn every_turn_turns_around_its_own_axis() {
    let tip = |axiom| run(Run { axiom, generations: 0.0, angle: std::f32::consts::FRAC_PI_2, ..Default::default() }).tip(0);
    assert!(close(tip("+F"), Vec3::NEG_X));
    assert!(close(tip("-F"), Vec3::X));
    assert!(close(tip("&F"), Vec3::NEG_Z));
    assert!(close(tip("^F"), Vec3::Z));
    // a roll turns what comes after it around the heading
    assert!(close(tip("/+F"), Vec3::NEG_Z) || close(tip("/+F"), Vec3::Z));
    assert!(close(tip("|F"), Vec3::NEG_Y));
    assert!(close(tip("F(2)"), Vec3::new(0.0, 2.0, 0.0)));
    assert!(close(tip("+(0.5)F"), Vec3::new(-(0.25f32 * std::f32::consts::PI).sin(), (0.25f32 * std::f32::consts::PI).cos(), 0.0)));
}

#[test]
fn a_bracket_without_steps_lets_its_branches_grow_on_the_path_around_it() {
    let plant = run(Run { axiom: "F[+[F][-F]]F", generations: 0.0, ..Default::default() });
    let links = plant.links();
    assert_eq!(links.len(), 3);
    let stem = links[2];
    assert!(links[..2].iter().all(|l| l.parent_id == stem.id));
    assert_eq!([links[0].index, links[1].index], [0, 1]);
}

#[test]
fn ids_stay_when_a_branch_elsewhere_changes() {
    let a = run(Run { axiom: "F[+F]F[-F]", generations: 0.0, ..Default::default() }).links();
    let b = run(Run { axiom: "F[+FFF[F]]F[-F]", generations: 0.0, ..Default::default() }).links();
    assert_eq!(a.last().unwrap().id, b.last().unwrap().id);
    let second = |links: &[PathLink]| links.iter().find(|l| l.index == 1 && l.parent_id != NO_PARENT).unwrap().id;
    assert_eq!(second(&a), second(&b));
}

#[test]
fn the_thickness_follows_what_grows_above() {
    let plant = run(Run { axiom: "FF[+F][-F]", generations: 0.0, ..Default::default() });
    let stem = plant.points_of(2);
    let child = plant.points_of(0);
    assert!((stem[0][3] - 0.1).abs() < 1e-6, "{:?}", stem);
    assert!(stem[1][3] < stem[0][3]);
    // the cross sections of the two children add up to the one of the stem at the fork
    assert!((2.0 * child[0][3].powi(2) - stem[2][3].powi(2)).abs() < 1e-5, "{:?} {:?}", child, stem);
    assert_eq!(child[1][3], 0.0);
    let even = run(Run { axiom: "FF[+F][-F]", generations: 0.0, taper: 0.0, ..Default::default() });
    assert!(even.points_of(0).iter().all(|p| (p[3] - 0.1).abs() < 1e-6));
}

#[test]
fn a_fraction_of_a_generation_grows_the_new_parts_from_their_buds() {
    let grown = |generations| run(Run { axiom: "A", rules: "A = F[+A]A", generations, ..Default::default() });
    let half = grown(1.5);
    let links = half.links();
    assert_eq!(links.len(), 2);
    // the steps of the second generation have half of their length, the path half of its own
    assert!((links[0].grown - 0.5).abs() < 1e-6, "{:?}", links);
    assert!((links[1].grown - 0.75).abs() < 1e-6, "{:?}", links);
    assert!(close(half.tip(1), Vec3::new(0.0, 1.5, 0.0)), "{}", half.tip(1));
    assert!(close(grown(2.0).tip(1), Vec3::new(0.0, 2.0, 0.0)));
    // a branch the last generation has only just started is a bud
    let bud = run(Run { axiom: "F[+A]", rules: "A = F", generations: 1e-9, ..Default::default() });
    let links = bud.links();
    assert_eq!([links[0].grown, links[1].grown], [0.0, 1.0]);
}

#[test]
fn a_step_that_becomes_two_stretches_without_a_jump() {
    let height = |generations| run(Run { axiom: "F", rules: "F = FF", generations, ..Default::default() }).tip(0).y;
    assert!((height(1.0) - 2.0).abs() < 1e-5);
    assert!((height(1.0001) - 2.0).abs() < 1e-3);
    assert!((height(1.5) - 3.0).abs() < 1e-5);
}

#[test]
fn a_mark_gives_a_point_on_its_path() {
    let plant = run(Run { axiom: "FF[+*]F*", generations: 0.0, ..Default::default() });
    let stem = plant.links()[0];
    assert_eq!(plant.points.len(), 2);
    let [side, tip] = [plant.points[0], plant.points[1]];
    assert_eq!([side.parent_id, tip.parent_id], [stem.id, stem.id]);
    assert_eq!([side.index, tip.index], [0, 1]);
    assert!((side.alpha - 2.0 / 3.0).abs() < 1e-5 && (tip.alpha - 1.0).abs() < 1e-5);
    assert!(close(side.position, Vec3::new(0.0, 2.0, 0.0)));
    // it points where the turtle heads, its tangent lies along the path
    assert!(side.direction.x < 0.0 && side.tangent.dot(side.direction).abs() < 1e-5);
    assert!(side.tangent.y > 0.0);
    assert_eq!(side.scale, 1.0);
    let alone = run(Run { axiom: "*", generations: 0.0, ..Default::default() });
    assert_eq!(alone.points[0].parent_id, NO_PARENT);
    assert!(alone.paths.is_empty());
}

#[test]
fn the_length_is_read_with_the_depth_of_the_path() {
    // path_info depth
    let depth = concat_args(vec![&[5], &[3]]);
    let plant = run(Run { axiom: "F[+F]", generations: 0.0, length: depth, ..Default::default() });
    let child = plant.points_of(0);
    let length = Vec3::from_slice(&child[1][..3]).distance(Vec3::from_slice(&child[0][..3]));
    assert!((length - 2.0).abs() < 1e-5, "{length}");
    assert!(close(plant.tip(1), Vec3::Y));
}

#[test]
fn unbalanced_brackets_and_unknown_symbols_draw_what_they_can() {
    let plant = run(Run { axiom: "]]F[+F XYZ", generations: 0.0, ..Default::default() });
    assert_eq!(plant.paths.len(), 2);
}

#[test]
#[should_panic(expected = "line 2")]
fn a_broken_rule_names_its_line() {
    run(Run { rules: "F = FF\nFF", ..Default::default() });
}

#[test]
#[should_panic(expected = "grows past")]
fn a_grammar_that_blows_up_stops_with_a_message() {
    run(Run { rules: "F = F[+F][-F]F", generations: 12.0, ..Default::default() });
}

#[test]
fn tropism_bends_the_steps_that_lean_toward_the_ground() {
    let tip = |axiom, tropism| run(Run { axiom, generations: 0.0, tropism, ..Default::default() }).tip(0);
    // an upright line doesn't bend
    assert!(close(tip("FF", 0.5), Vec3::new(0.0, 2.0, 0.0)));
    // + by three angles of 0.5 lays the line flat, toward -x
    assert!(close(tip("+(3.1416)FF", 0.0), Vec3::new(-2.0, 0.0, 0.0)));
    assert!(tip("+(3.1416)FF", 0.3).y < -0.1);
    assert!(tip("+(3.1416)FF", -0.3).y > 0.1);
}
