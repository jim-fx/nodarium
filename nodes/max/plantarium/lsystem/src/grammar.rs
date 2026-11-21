//! The text of the axiom and the rules, and the rewriting of the axiom by them.

use nodarium_utils::geometry::{path_id, path_random};
use std::collections::HashMap;

/// More symbols than this make the node stop with a message instead of filling the memory.
pub const MAX_MODULES: usize = 250_000;

/// Generations past this rewrite nothing new in any grammar that stays under `MAX_MODULES`.
pub const MAX_GENERATIONS: usize = 64;

/// A symbol with its number in brackets, `F(0.5)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Module {
    pub symbol: char,
    pub arg: Option<f32>,
    /// Stays the same for a module when other parts of the string change, so a stochastic rule
    /// picks the same successor for it.
    pub id: i32,
    /// How far a module that the last, partial generation made has grown, from 0 to 1. A step
    /// that replaces a step starts where the steps it replaces add up to its length.
    pub share: f32,
}

pub fn draws(symbol: char) -> bool {
    matches!(symbol, 'F' | 'G')
}

#[derive(Debug)]
pub struct Successor {
    weight: f32,
    modules: Vec<Module>,
    /// The steps outside of brackets, in steps of length 1.
    main_steps: f32,
}

#[derive(Default, Debug)]
pub struct Rules {
    rules: HashMap<char, Vec<Successor>>,
}

/// Reads `F(0.5)+[-F]X` into modules. Whitespace is left out, `#` starts a comment.
pub fn parse_modules(text: &str) -> Result<Vec<Module>, String> {
    let mut modules = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c.is_whitespace() {
            continue;
        }
        if c == '#' {
            for c in chars.by_ref() {
                if c == '\n' {
                    break;
                }
            }
            continue;
        }
        if c == '(' || c == ')' {
            return Err(format!("`{c}` without a symbol before it, write a number in brackets after a symbol, like F(0.5)"));
        }
        let mut arg = None;
        if chars.peek() == Some(&'(') {
            chars.next();
            let mut number = String::new();
            loop {
                match chars.next() {
                    Some(')') => break,
                    Some(c) => number.push(c),
                    None => return Err(format!("`{c}({number}` has no closing bracket")),
                }
            }
            let value: f32 = number
                .trim()
                .parse()
                .map_err(|_| format!("`{c}({number})`: `{}` is not a number", number.trim()))?;
            arg = Some(value);
        }
        let id = path_id(0x4c53, -1, modules.len() as i32);
        modules.push(Module { symbol: c, arg, id, share: 1.0 });
    }
    Ok(modules)
}

/// Splits `X = F[+X] : 2` into the predecessor, the successor and the weight.
fn parse_rule(rule: &str) -> Result<(char, Successor), String> {
    let (before, after) = ["->", "→", "="]
        .iter()
        .find_map(|arrow| rule.split_once(arrow))
        .ok_or_else(|| "write a rule as `symbol = what it becomes`, like `X = F[+X]F`".to_string())?;
    let predecessor: Vec<char> = before.chars().filter(|c| !c.is_whitespace()).collect();
    let symbol = match predecessor.as_slice() {
        [symbol] => *symbol,
        [] => return Err("the rule has no symbol before the `=`".into()),
        _ => {
            return Err(format!(
                "`{}` before the `=` is more than one symbol, a rule replaces a single symbol",
                before.trim()
            ))
        }
    };
    let (successor, weight) = match after.rsplit_once(':') {
        Some((successor, weight)) => {
            let weight: f32 = weight
                .trim()
                .parse()
                .map_err(|_| format!("the weight `{}` after the `:` is not a number", weight.trim()))?;
            (successor, weight.max(0.0))
        }
        None => (after, 1.0),
    };
    let modules = parse_modules(successor)?;
    let mut depth = 0;
    let mut main_steps = 0.0;
    for module in &modules {
        match module.symbol {
            '[' => depth += 1,
            ']' => depth -= 1,
            s if draws(s) && depth == 0 => main_steps += module.arg.unwrap_or(1.0),
            _ => {}
        }
    }
    Ok((symbol, Successor { weight, modules, main_steps }))
}

/// One rule per line or between `;`, `#` starts a comment.
pub fn parse_rules(text: &str) -> Result<Rules, String> {
    let mut rules = Rules::default();
    for (line_number, line) in text.lines().enumerate() {
        let line = line.split('#').next().unwrap_or("");
        for rule in line.split(';') {
            if rule.trim().is_empty() {
                continue;
            }
            let (symbol, successor) =
                parse_rule(rule).map_err(|e| format!("rule `{}` in line {}: {e}", rule.trim(), line_number + 1))?;
            rules.rules.entry(symbol).or_default().push(successor);
        }
    }
    Ok(rules)
}

impl Rules {
    /// The successor of `module` in `generation`, drawn by the weights of the rules of its
    /// symbol. `None` when it has no rule, or only rules of weight 0.
    fn pick(&self, module: &Module, seed: i32, generation: usize) -> Option<&Successor> {
        let options = self.rules.get(&module.symbol)?;
        let total: f32 = options.iter().map(|s| s.weight).sum();
        if total <= 0.0 {
            return None;
        }
        let mut r = path_random(seed, module.id, generation as i32) * total;
        for option in options {
            if r < option.weight {
                return Some(option);
            }
            r -= option.weight;
        }
        options.iter().rev().find(|s| s.weight > 0.0)
    }
}

/// Rewrites `axiom` `generations` times. A fraction rewrites once more, and what that generation
/// makes has grown by the fraction (`Module::share`).
pub fn rewrite(axiom: &[Module], rules: &Rules, generations: f32, seed: i32) -> Result<Vec<Module>, String> {
    let generations = generations.clamp(0.0, MAX_GENERATIONS as f32);
    let count = generations.ceil() as usize;
    let fraction = generations - (count as f32 - 1.0);
    let mut current = axiom.to_vec();
    for generation in 1..=count {
        let last = generation == count;
        let mut next = Vec::with_capacity(current.len() * 2);
        for module in &current {
            let Some(successor) = rules.pick(module, seed, generation) else {
                next.push(*module);
                continue;
            };
            let replaced_steps = if draws(module.symbol) { module.arg.unwrap_or(1.0) } else { 0.0 };
            let mut depth = 0;
            for (k, template) in successor.modules.iter().enumerate() {
                let share = if !last {
                    1.0
                } else if depth == 0 && draws(template.symbol) && replaced_steps > 0.0 && successor.main_steps > 0.0 {
                    // the steps that replace a step stretch from its length to theirs
                    let start = (replaced_steps / successor.main_steps).clamp(0.0, 4.0);
                    start + (1.0 - start) * fraction
                } else {
                    fraction
                };
                match template.symbol {
                    '[' => depth += 1,
                    ']' => depth -= 1,
                    _ => {}
                }
                next.push(Module { id: path_id(module.id, k as i32, 0x4c53), share, ..*template });
            }
            if next.len() > MAX_MODULES {
                return Err(format!(
                    "generation {generation} grows past {MAX_MODULES} symbols (generation {} has {}). Use fewer generations, or rules that add fewer symbols",
                    generation - 1,
                    current.len()
                ));
            }
        }
        current = next;
    }
    Ok(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(modules: &[Module]) -> String {
        modules
            .iter()
            .map(|m| match m.arg {
                Some(a) => format!("{}({a})", m.symbol),
                None => m.symbol.to_string(),
            })
            .collect()
    }

    fn grow(axiom: &str, rules: &str, generations: f32) -> Vec<Module> {
        rewrite(&parse_modules(axiom).unwrap(), &parse_rules(rules).unwrap(), generations, 1).unwrap()
    }

    #[test]
    fn reads_symbols_numbers_and_comments() {
        let modules = parse_modules("F(0.5) +[ -F ] # a comment\n X").unwrap();
        assert_eq!(text(&modules), "F(0.5)+[-F]X");
        assert!(parse_modules("F(0.5").unwrap_err().contains("no closing bracket"));
        assert!(parse_modules("F(a)").unwrap_err().contains("not a number"));
    }

    #[test]
    fn reads_rules_with_every_arrow_and_separator() {
        let rules = parse_rules("X = F[+X]\nF -> FF # doubles\nA → B; B = A").unwrap();
        assert_eq!(rules.rules.len(), 4);
        assert!(parse_rules("XY = F").unwrap_err().contains("line 1"));
        assert!(parse_rules("\nF[+F]").unwrap_err().contains("line 2"));
        assert!(parse_rules("F = F : x").unwrap_err().contains("weight"));
    }

    #[test]
    fn rewrites_every_symbol_with_its_rule_at_once() {
        assert_eq!(text(&grow("A", "A = AB; B = A", 4.0)), "ABAABABA");
        assert_eq!(text(&grow("X", "X = F[+X]-X", 0.0)), "X");
        // a rule with an empty successor removes the symbol
        assert_eq!(text(&grow("FXF", "X =", 1.0)), "FF");
    }

    #[test]
    fn a_fraction_grows_the_last_generation() {
        let modules = grow("F", "F = F[+F]F", 0.25);
        let shares: Vec<f32> = modules.iter().map(|m| m.share).collect();
        // the two steps that replace the step start at half of their length each
        assert_eq!(shares, vec![0.625, 0.25, 0.25, 0.25, 0.25, 0.625]);
        assert!(grow("F", "F = F[+F]F", 2.0).iter().all(|m| m.share == 1.0));
    }

    #[test]
    fn weights_pick_a_successor_per_symbol_that_stays_with_it() {
        let rules = "A = FA : 1; A = GA : 1";
        let a = grow("[A][A][A][A][A][A][A][A]", rules, 1.0);
        let picks = text(&a);
        assert!(picks.contains('F') && picks.contains('G'), "{picks}");
        assert_eq!(text(&grow("[A][A][A][A][A][A][A][A]", rules, 1.0)), picks);
        // a rule of weight 0 is never picked
        assert_eq!(text(&grow("AAAA", "A = F : 0; A = G", 1.0)), "GGGG");
    }

    #[test]
    fn stops_with_a_message_before_it_blows_up() {
        let error = rewrite(&parse_modules("F").unwrap(), &parse_rules("F = FFF").unwrap(), 40.0, 0).unwrap_err();
        assert!(error.contains("grows past"), "{error}");
    }
}
