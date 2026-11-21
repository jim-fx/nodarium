//! The turtle: walks the rewritten string and draws it as paths, a path for every branch.
//!
//! It walks twice. The first walk only follows the brackets and finds the paths, their parents,
//! depths and number of steps, so the second walk can read the inputs of every step with the
//! path it belongs to and with alpha along it.

use crate::grammar::{draws, Module};
use glam::{Quat, Vec3};
use nodarium_utils::{
    geometry::{create_path, path_id, set_path_link, wrap_path_mut, PathLink, BUD_LENGTH, NO_PARENT},
    Ctx, PathInfo, Point,
};
use std::f32::consts::PI;

/// What the inputs of the node give the turtle, read with the context of a path.
pub trait Inputs {
    fn length(&mut self, ctx: &mut Ctx) -> f32;
    fn angle(&mut self, ctx: &mut Ctx) -> f32;
    fn thickness(&mut self, ctx: &mut Ctx) -> f32;
    fn tropism(&mut self, ctx: &mut Ctx) -> f32;
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Kind {
    Forward { draw: bool },
    Yaw(f32),
    Pitch(f32),
    Roll(f32),
    TurnAround,
    Level,
    Push,
    Pop,
    Width,
    Length,
    Mark,
}

fn kind(symbol: char) -> Option<Kind> {
    Some(match symbol {
        s if draws(s) => Kind::Forward { draw: true },
        'f' | 'g' => Kind::Forward { draw: false },
        '+' => Kind::Yaw(1.0),
        '-' => Kind::Yaw(-1.0),
        '&' => Kind::Pitch(1.0),
        '^' => Kind::Pitch(-1.0),
        '\\' => Kind::Roll(1.0),
        '/' => Kind::Roll(-1.0),
        '|' => Kind::TurnAround,
        '$' => Kind::Level,
        '[' => Kind::Push,
        ']' => Kind::Pop,
        '!' => Kind::Width,
        '"' => Kind::Length,
        '*' => Kind::Mark,
        _ => return None,
    })
}

/// `!` and `"` without a number
const DEFAULT_FACTOR: f32 = 0.7;

struct Op {
    kind: Kind,
    arg: f32,
    share: f32,
    /// The path whose context the inputs are read with, `None` for the plant as a whole
    ctx: Option<usize>,
    /// The step this op comes after on that path
    step: usize,
    /// The path a step draws on, or a mark sits on
    path: Option<usize>,
}

struct PathPlan {
    parent: Option<usize>,
    depth: i32,
    index: i32,
    id: i32,
    steps: usize,
    full_steps: f32,
    grown_steps: f32,
    /// The steps of the parent before this path starts
    parent_steps: usize,
}

struct Frame {
    path: Option<usize>,
    /// Ops of this bracket before its path starts, they are read with that path
    pending: Vec<usize>,
}

struct Plan {
    ops: Vec<Op>,
    paths: Vec<PathPlan>,
}

fn plan(modules: &[Module], creator: i32) -> Plan {
    let mut ops: Vec<Op> = Vec::new();
    let mut paths: Vec<PathPlan> = Vec::new();
    let mut children: Vec<i32> = Vec::new();
    let mut roots = 0;
    let mut frames = vec![Frame { path: None, pending: Vec::new() }];

    let owner = |frames: &[Frame]| frames.iter().rev().find_map(|f| f.path);

    for module in modules {
        let Some(kind) = kind(module.symbol) else { continue };
        let default = if matches!(kind, Kind::Width | Kind::Length) { DEFAULT_FACTOR } else { 1.0 };
        let arg = module.arg.unwrap_or(default);
        let top = frames.len() - 1;
        let current = frames[top].path;
        let mut op = Op { kind, arg, share: module.share, ctx: owner(&frames), step: 0, path: current };

        match kind {
            Kind::Push => {
                frames.push(Frame { path: None, pending: Vec::new() });
            }
            Kind::Pop => {
                if frames.len() == 1 {
                    // a `]` without its `[` is left out
                    continue;
                }
                frames.pop();
                op.ctx = owner(&frames);
            }
            Kind::Forward { draw: true } => {
                let path = match current {
                    Some(path) => path,
                    None => {
                        let parent = owner(&frames);
                        let index = match parent {
                            Some(p) => {
                                children[p] += 1;
                                children[p] - 1
                            }
                            None => {
                                roots += 1;
                                roots - 1
                            }
                        };
                        let parent_id = parent.map_or(NO_PARENT, |p| paths[p].id);
                        paths.push(PathPlan {
                            parent,
                            depth: parent.map_or(1, |p| paths[p].depth + 1),
                            index,
                            id: path_id(creator, parent_id, index),
                            steps: 0,
                            full_steps: 0.0,
                            grown_steps: 0.0,
                            parent_steps: parent.map_or(0, |p| paths[p].steps),
                        });
                        children.push(0);
                        let path = paths.len() - 1;
                        for &pending in &frames[top].pending {
                            ops[pending].ctx = Some(path);
                        }
                        frames[top].pending.clear();
                        frames[top].path = Some(path);
                        path
                    }
                };
                op.path = Some(path);
                op.ctx = Some(path);
                op.step = paths[path].steps;
                paths[path].steps += 1;
                paths[path].full_steps += arg.abs();
                paths[path].grown_steps += arg.abs() * module.share;
            }
            Kind::Forward { draw: false } => {
                op.ctx = current.or(op.ctx);
                // a move ends the path, the next step starts a new one
                frames[top].path = None;
            }
            Kind::Mark => {
                op.path = current.or(op.ctx);
            }
            _ => {}
        }

        if let Some(path) = current {
            if !matches!(kind, Kind::Pop | Kind::Forward { draw: true }) {
                op.ctx = Some(path);
                op.step = paths[path].steps;
            }
        } else if !matches!(kind, Kind::Push | Kind::Pop | Kind::Forward { draw: true }) {
            frames[top].pending.push(ops.len());
        }
        ops.push(op);
    }
    Plan { ops, paths }
}

#[derive(Clone, Copy)]
struct Turtle {
    position: Vec3,
    heading: Vec3,
    left: Vec3,
    up: Vec3,
    width: f32,
    step: f32,
    /// The direction of the last step, the tangent of the path a mark sits on
    tangent: Vec3,
}

impl Turtle {
    fn rotate(&mut self, axis: Vec3, angle: f32) {
        let q = Quat::from_axis_angle(axis.normalize(), angle);
        self.heading = (q * self.heading).normalize();
        self.left = (q * self.left).normalize();
        self.up = (q * self.up).normalize();
    }
}

#[derive(Default)]
struct DrawnPath {
    points: Vec<Vec3>,
    widths: Vec<f32>,
    /// The length drawn so far and the length of the path once it has grown
    length: f32,
    full_length: f32,
    /// The children with the point they start on and how far along this path that is
    children: Vec<(usize, usize, f32)>,
    heading: Vec3,
}

pub struct Drawing {
    pub paths: Vec<i32>,
    pub points: Vec<Point>,
}

/// Walks `modules` and gives the paths, children before their parent, and a point on every `*`.
pub fn draw(modules: &[Module], inputs: &mut impl Inputs, taper: f32, creator: i32) -> Drawing {
    let Plan { ops, paths: plans } = plan(modules, creator);
    let count = plans.len();
    let mut contexts: Vec<Ctx> = plans
        .iter()
        .enumerate()
        .map(|(rank, plan)| {
            let mut ctx = Ctx::default();
            ctx.set_path_info(PathInfo {
                index: plan.index as usize,
                rank,
                count,
                depth: plan.depth,
                parent_position: plan.parent.map_or(0.0, |p| plan.parent_steps as f32 / plans[p].steps.max(1) as f32),
                length: 0.0,
                grown: if plan.full_steps > 0.0 { plan.grown_steps / plan.full_steps } else { 1.0 },
            });
            ctx.seed_path(plan.id);
            ctx
        })
        .collect();
    let mut plant = Ctx::default();

    let mut drawn: Vec<DrawnPath> = (0..count).map(|_| DrawnPath::default()).collect();
    let mut marks: Vec<(Option<usize>, Turtle, f32, f32)> = Vec::new();
    let mut turtle = Turtle {
        position: Vec3::ZERO,
        heading: Vec3::Y,
        left: Vec3::NEG_X,
        up: Vec3::Z,
        width: 1.0,
        step: 1.0,
        tangent: Vec3::Y,
    };
    let mut stack: Vec<Turtle> = Vec::new();

    for op in &ops {
        let ctx = match op.ctx {
            Some(path) => {
                let ctx = &mut contexts[path];
                ctx.alpha = (op.step as f32 / plans[path].steps.saturating_sub(1).max(1) as f32).min(1.0);
                ctx
            }
            None => &mut plant,
        };
        match op.kind {
            Kind::Forward { draw } => {
                let full = inputs.length(ctx) * op.arg * turtle.step;
                let length = full * op.share;
                match (draw, op.path) {
                    (true, Some(path)) => {
                        if drawn[path].points.is_empty() {
                            let parent = plans[path].parent;
                            if let Some(parent) = parent {
                                let at = drawn[parent].points.len().saturating_sub(1);
                                let along = drawn[parent].length;
                                drawn[parent].children.push((path, at, along));
                            }
                            drawn[path].points.push(turtle.position);
                            drawn[path].widths.push(turtle.width);
                            drawn[path].heading = turtle.heading;
                        }
                        drawn[path].full_length += full.abs();
                        if length.abs() >= BUD_LENGTH {
                            turtle.position += turtle.heading * length;
                            drawn[path].points.push(turtle.position);
                            drawn[path].widths.push(turtle.width);
                            drawn[path].length += length.abs();
                        }
                        turtle.tangent = turtle.heading * length.signum();
                    }
                    _ => turtle.position += turtle.heading * length,
                }
                let tropism = inputs.tropism(ctx) * op.share;
                let axis = turtle.heading.cross(Vec3::NEG_Y);
                if tropism != 0.0 && axis.length_squared() > 1e-8 {
                    turtle.rotate(axis, tropism * axis.length());
                }
            }
            Kind::Yaw(sign) => {
                let angle = inputs.angle(ctx) * op.arg * op.share * sign;
                turtle.rotate(turtle.up, angle);
            }
            Kind::Pitch(sign) => {
                let angle = inputs.angle(ctx) * op.arg * op.share * sign;
                turtle.rotate(turtle.left, angle);
            }
            Kind::Roll(sign) => {
                let angle = inputs.angle(ctx) * op.arg * op.share * sign;
                turtle.rotate(turtle.heading, angle);
            }
            Kind::TurnAround => turtle.rotate(turtle.up, PI * op.share),
            Kind::Level => {
                let left = Vec3::Y.cross(turtle.heading);
                if left.length_squared() > 1e-8 {
                    turtle.left = left.normalize();
                    turtle.up = turtle.heading.cross(turtle.left).normalize();
                }
            }
            Kind::Push => stack.push(turtle),
            Kind::Pop => {
                if let Some(saved) = stack.pop() {
                    turtle = saved;
                }
            }
            Kind::Width => turtle.width *= 1.0 + (op.arg - 1.0) * op.share,
            Kind::Length => turtle.step *= 1.0 + (op.arg - 1.0) * op.share,
            Kind::Mark => {
                let along = op.path.map_or(0.0, |p| drawn[p].length);
                marks.push((op.path, turtle, op.share, along));
            }
        }
    }

    let thickness_of_root: Vec<f32> = (0..count)
        .map(|p| if plans[p].parent.is_none() { inputs.thickness(&mut contexts[p]) } else { 0.0 })
        .collect();
    finish(&plans, &drawn, &marks, &thickness_of_root, taper, creator)
}

fn finish(
    plans: &[PathPlan],
    drawn: &[DrawnPath],
    marks: &[(Option<usize>, Turtle, f32, f32)],
    thickness_of_root: &[f32],
    taper: f32,
    creator: i32,
) -> Drawing {
    let count = plans.len();

    // what grows on a path and above it, as it is now and once it has grown: the share of it that
    // grows above a point sets the thickness there
    let mut load = vec![0.0f32; count];
    let mut full_load = vec![0.0f32; count];
    for p in (0..count).rev() {
        load[p] = drawn[p].length + drawn[p].children.iter().map(|&(c, _, _)| load[c]).sum::<f32>();
        full_load[p] = drawn[p].full_length + drawn[p].children.iter().map(|&(c, _, _)| full_load[c]).sum::<f32>();
    }
    let mut root = vec![0usize; count];
    for p in 0..count {
        root[p] = plans[p].parent.map_or(p, |parent| root[parent]);
    }

    let grown = |p: usize| {
        if drawn[p].length < BUD_LENGTH {
            0.0
        } else if drawn[p].full_length > 0.0 {
            (drawn[p].length / drawn[p].full_length).min(1.0)
        } else {
            1.0
        }
    };

    let mut encoded: Vec<Option<Vec<i32>>> = vec![None; count];
    for p in 0..count {
        let path = &drawn[p];
        let plan = &plans[p];
        let mut points = path.points.clone();
        let mut widths = path.widths.clone();
        let base = points.first().copied().unwrap_or(Vec3::ZERO);
        if points.len() < 2 {
            points = vec![base, base + path.heading * BUD_LENGTH];
            widths = vec![widths.first().copied().unwrap_or(1.0); 2];
        }

        let mut along = vec![0.0f32; points.len()];
        for i in 1..points.len() {
            along[i] = along[i - 1] + points[i].distance(points[i - 1]);
        }
        let total = along[along.len() - 1];
        let r = root[p];
        let link = PathLink {
            grown: grown(p),
            ..match plan.parent {
                Some(parent) => {
                    let at = path_at(drawn, parent, p);
                    let full = drawn[parent].full_length;
                    PathLink::child(creator, plans[parent].id, if full > 0.0 { (at / full).min(1.0) } else { 0.0 }, plan.index)
                }
                None => PathLink::root(creator, plan.index),
            }
        };
        debug_assert_eq!(link.id, plan.id);

        let mut data = create_path(points.len(), plan.depth);
        set_path_link(&mut data, link);
        let wrapped = wrap_path_mut(&mut data);
        // what the branches that start on a point or above it carry
        let mut branches_above = vec![0.0f32; points.len() + 1];
        for &(c, at, _) in &path.children {
            branches_above[at.min(points.len() - 1)] += load[c];
        }
        for i in (0..points.len()).rev() {
            branches_above[i] += branches_above[i + 1];
        }
        for (i, point) in points.iter().enumerate() {
            let above = (total - along[i]).max(0.0) + branches_above[i];
            let share = if full_load[r] > 0.0 { (above / full_load[r]).clamp(0.0, 1.0) } else { 1.0 };
            let thickness = thickness_of_root[r] * widths[i] * share.powf(taper.max(0.0));
            wrapped.points[i * 4..i * 4 + 4].copy_from_slice(&[point.x, point.y, point.z, thickness]);
        }
        encoded[p] = Some(data);
    }

    // children come right before their parent, as `branch` gives them
    let mut paths = Vec::with_capacity(count);
    fn emit(p: usize, drawn: &[DrawnPath], encoded: &mut [Option<Vec<i32>>], out: &mut Vec<Vec<i32>>) {
        for &(child, _, _) in &drawn[p].children {
            emit(child, drawn, encoded, out);
        }
        if let Some(data) = encoded[p].take() {
            out.push(data);
        }
    }
    for p in 0..count {
        if plans[p].parent.is_none() {
            emit(p, drawn, &mut encoded, &mut paths);
        }
    }
    // a child the walk never reached, because its parent drew nothing before it
    paths.extend(encoded.into_iter().flatten());

    let mut marks_on_path = vec![0i32; count];
    let mut marks_on_none = 0;
    let points = marks
        .iter()
        .map(|&(path, turtle, share, along)| {
            let index = match path {
                Some(p) => &mut marks_on_path[p],
                None => &mut marks_on_none,
            };
            *index += 1;
            let tangent = turtle.tangent - turtle.heading * turtle.tangent.dot(turtle.heading);
            let tangent = if tangent.length_squared() > 1e-6 { tangent.normalize() } else { turtle.up };
            Point {
                position: turtle.position,
                direction: turtle.heading,
                tangent,
                scale: share,
                parent_id: path.map_or(NO_PARENT, |p| plans[p].id),
                alpha: path.map_or(0.0, |p| {
                    let full = drawn[p].full_length;
                    if full > 0.0 { (along / full).min(grown(p)) } else { 0.0 }
                }),
                depth: path.map_or(0, |p| plans[p].depth),
                index: *index - 1,
            }
        })
        .collect();

    Drawing { paths: nodarium_utils::concat_arg_vecs(paths), points }
}

/// How far along `parent` the child starts, as it is drawn now.
fn path_at(drawn: &[DrawnPath], parent: usize, child: usize) -> f32 {
    drawn[parent].children.iter().find(|&&(c, _, _)| c == child).map_or(0.0, |&(_, _, along)| along)
}
