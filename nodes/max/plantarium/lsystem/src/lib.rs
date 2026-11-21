mod grammar;
mod turtle;

use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{encode_points, int, read_text, Ctx};

nodarium_definition_file!("src/definition.json");

struct Args<'a> {
    length: &'a [i32],
    angle: &'a [i32],
    thickness: &'a [i32],
    tropism: &'a [i32],
}

impl turtle::Inputs for Args<'_> {
    fn length(&mut self, ctx: &mut Ctx) -> f32 {
        ctx.float(self.length)
    }
    fn angle(&mut self, ctx: &mut Ctx) -> f32 {
        ctx.float(self.angle)
    }
    fn thickness(&mut self, ctx: &mut Ctx) -> f32 {
        ctx.float(self.thickness)
    }
    fn tropism(&mut self, ctx: &mut Ctx) -> f32 {
        ctx.float(self.tropism)
    }
}

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> (Vec<i32>, Vec<i32>) {
    let axiom = read_text(args[0]);
    let rules = read_text(args[1]);
    let generations = Ctx::default().float(args[2]);
    let taper = Ctx::default().float(args[7]);
    let seed = int(args[8]);
    let creator = int(args[9]);

    let modules = grammar::parse_modules(&axiom)
        .map_err(|e| format!("in the axiom, {e}"))
        .and_then(|axiom| {
            let rules = grammar::parse_rules(&rules)?;
            grammar::rewrite(&axiom, &rules, generations, seed)
        })
        .unwrap_or_else(|e| panic!("{e}"));

    let mut inputs = Args { length: args[3], angle: args[4], thickness: args[5], tropism: args[6] };
    let drawing = turtle::draw(&modules, &mut inputs, taper, creator);
    (drawing.paths, encode_points(&drawing.points))
}

#[cfg(test)]
mod tests;
