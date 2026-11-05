use glam::{Vec2, Vec3};
use nodarium_utils::decode_float;

use crate::field::{opcode, Field};
use crate::noise::value_noise3;

/// Deepest nesting of distances and transforms a program may use
const MAX_DEPTH: usize = 64;

#[derive(Clone, Copy, Debug)]
enum Op {
    Sphere(f32),
    Capsule(Vec3, Vec3, f32),
    Revolve { start: usize, end: usize },
    Union,
    SmoothUnion(f32),
    SmoothSubtract(f32),
    Translate(Vec3),
    Bend(f32),
    Warp { amplitude: f32, frequency: f32, seed: u32 },
    End,
}

/// Field ops decoded and checked once, so they can be evaluated at many points
#[derive(Clone, Debug)]
pub struct Program {
    ops: Vec<Op>,
    profiles: Vec<Vec2>,
}

impl Program {
    pub fn parse(words: &[i32]) -> Program {
        let mut ops = Vec::new();
        let mut profiles = Vec::new();
        let mut i = 0;
        // distances on the stack, and the stack height at each open transform
        let mut depth = 0usize;
        let mut scopes: Vec<usize> = Vec::new();

        let operands = |i: usize, count: usize| -> Vec<f32> {
            assert!(
                i + count <= words.len(),
                "field op {} at word {} needs {} operands, only {} words left",
                words[i - 1],
                i - 1,
                count,
                words.len() - i
            );
            words[i..i + count].iter().map(|&w| decode_float(w)).collect()
        };

        while i < words.len() {
            let code = words[i];
            i += 1;
            let op = match code {
                opcode::SPHERE => {
                    let o = operands(i, 1);
                    i += 1;
                    Op::Sphere(o[0])
                }
                opcode::CAPSULE => {
                    let o = operands(i, 7);
                    i += 7;
                    Op::Capsule(Vec3::new(o[0], o[1], o[2]), Vec3::new(o[3], o[4], o[5]), o[6])
                }
                opcode::REVOLVE => {
                    let n = operands(i, 1)[0] as usize;
                    i += 1;
                    assert!(n >= 3, "revolve profile needs at least 3 points, got {}", n);
                    let o = operands(i, n * 2);
                    i += n * 2;
                    let start = profiles.len();
                    profiles.extend(o.chunks_exact(2).map(|c| Vec2::new(c[0], c[1])));
                    Op::Revolve { start, end: profiles.len() }
                }
                opcode::UNION => Op::Union,
                opcode::SMOOTH_UNION | opcode::SMOOTH_SUBTRACT => {
                    let k = operands(i, 1)[0];
                    i += 1;
                    if code == opcode::SMOOTH_UNION {
                        Op::SmoothUnion(k)
                    } else {
                        Op::SmoothSubtract(k)
                    }
                }
                opcode::TRANSLATE => {
                    let o = operands(i, 3);
                    i += 3;
                    Op::Translate(Vec3::new(o[0], o[1], o[2]))
                }
                opcode::BEND => {
                    let k = operands(i, 1)[0];
                    i += 1;
                    Op::Bend(k)
                }
                opcode::WARP => {
                    let o = operands(i, 3);
                    let seed = words[i + 2] as u32;
                    i += 3;
                    Op::Warp { amplitude: o[0], frequency: o[1], seed }
                }
                opcode::END => Op::End,
                _ => panic!("unknown field opcode {} at word {}", code, i - 1),
            };

            match op {
                Op::Sphere(_) | Op::Capsule(..) | Op::Revolve { .. } => depth += 1,
                Op::Union | Op::SmoothUnion(_) | Op::SmoothSubtract(_) => {
                    let floor = scopes.last().copied().unwrap_or(0);
                    assert!(depth >= floor + 2, "field combines two shapes at word {} but has fewer", i);
                    depth -= 1;
                }
                Op::Translate(_) | Op::Bend(_) | Op::Warp { .. } => scopes.push(depth),
                Op::End => {
                    let start = scopes.pop().expect("field has an END without a transform to close");
                    assert!(depth == start + 1, "a field transform must wrap exactly one shape");
                }
            }
            assert!(depth <= MAX_DEPTH && scopes.len() < MAX_DEPTH, "field is nested deeper than {}", MAX_DEPTH);
            ops.push(op);
        }

        assert!(scopes.is_empty(), "field has a transform without END");
        assert!(depth <= 1, "field leaves {} shapes that are never combined", depth);
        Program { ops, profiles }
    }

    /// Signed distance at `p`, negative inside, infinite for an empty field
    pub fn eval(&self, p: Vec3) -> f32 {
        let mut distances = [0.0f32; MAX_DEPTH];
        let mut dn = 0;
        let mut points = [Vec3::ZERO; MAX_DEPTH];
        points[0] = p;
        let mut pn = 1;

        for op in &self.ops {
            let q = points[pn - 1];
            match *op {
                Op::Sphere(r) => {
                    distances[dn] = q.length() - r;
                    dn += 1;
                }
                Op::Capsule(a, b, r) => {
                    distances[dn] = capsule(q, a, b, r);
                    dn += 1;
                }
                Op::Revolve { start, end } => {
                    let flat = Vec2::new(Vec2::new(q.x, q.z).length(), q.y);
                    distances[dn] = polygon(flat, &self.profiles[start..end]);
                    dn += 1;
                }
                Op::Union | Op::SmoothUnion(_) | Op::SmoothSubtract(_) => {
                    let (a, b) = (distances[dn - 2], distances[dn - 1]);
                    dn -= 1;
                    distances[dn - 1] = match *op {
                        Op::Union => a.min(b),
                        Op::SmoothUnion(k) => smooth_min(a, b, k),
                        Op::SmoothSubtract(k) => -smooth_min(-a, b, k),
                        _ => unreachable!(),
                    };
                }
                Op::Translate(offset) => {
                    points[pn] = q - offset;
                    pn += 1;
                }
                Op::Bend(k) => {
                    let (s, c) = (k * q.y).sin_cos();
                    points[pn] = Vec3::new(c * q.x - s * q.y, s * q.x + c * q.y, q.z);
                    pn += 1;
                }
                Op::Warp { amplitude, frequency, seed } => {
                    points[pn] = q + value_noise3(q * frequency, seed) * amplitude;
                    pn += 1;
                }
                Op::End => pn -= 1,
            }
        }

        if dn == 0 {
            f32::INFINITY
        } else {
            distances[0]
        }
    }
}

/// Signed distance of a field (`[10, n, bbox, ...ops]`) at `p`. Parses the
/// program on every call, use [`Program`] to evaluate many points.
pub fn eval(field: &[i32], p: Vec3) -> f32 {
    Field::decode(field).program().eval(p)
}

fn smooth_min(a: f32, b: f32, k: f32) -> f32 {
    if k <= 0.0 {
        return a.min(b);
    }
    let h = (k - (a - b).abs()).max(0.0) / k;
    a.min(b) - h * h * k * 0.25
}

fn capsule(p: Vec3, a: Vec3, b: Vec3, r: f32) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let len = ba.length_squared();
    let h = if len > 0.0 { (pa.dot(ba) / len).clamp(0.0, 1.0) } else { 0.0 };
    (pa - ba * h).length() - r
}

/// Signed distance to a closed polygon. Edges on the axis (x = 0) end up inside
/// the revolved solid, so they count for the sign but not for the distance.
fn polygon(p: Vec2, points: &[Vec2]) -> f32 {
    let mut d = f32::INFINITY;
    let mut inside = false;
    let mut j = points.len() - 1;
    for i in 0..points.len() {
        let (vi, vj) = (points[i], points[j]);
        let e = vj - vi;
        let w = p - vi;
        let len = e.length_squared();
        if !(vi.x == 0.0 && vj.x == 0.0) {
            let t = if len > 0.0 { (w.dot(e) / len).clamp(0.0, 1.0) } else { 0.0 };
            d = d.min((w - e * t).length_squared());
        }
        if (vi.y > p.y) != (vj.y > p.y) && p.x < vi.x + (p.y - vi.y) / (vj.y - vi.y) * e.x {
            inside = !inside;
        }
        j = i;
    }
    if inside {
        -d.sqrt()
    } else {
        d.sqrt()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) {
        assert!((a - b).abs() < 1e-5, "{} != {}", a, b);
    }

    fn at(field: &Field, x: f32, y: f32, z: f32) -> f32 {
        field.program().eval(Vec3::new(x, y, z))
    }

    #[test]
    fn sphere() {
        let f = Field::sphere(1.0);
        close(at(&f, 2.0, 0.0, 0.0), 1.0);
        close(at(&f, 0.0, 0.0, 0.0), -1.0);
        close(at(&f, 0.0, 0.6, 0.8), 0.0);
    }

    #[test]
    fn capsule_distance() {
        let f = Field::capsule(Vec3::ZERO, Vec3::new(0.0, 2.0, 0.0), 0.5);
        close(at(&f, 1.0, 1.0, 0.0), 0.5);
        close(at(&f, 0.0, 3.0, 0.0), 0.5);
        close(at(&f, 0.0, -1.0, 0.0), 0.5);
        close(at(&f, 0.0, 1.0, 0.0), -0.5);
    }

    #[test]
    fn revolve_square_is_a_cylinder() {
        let profile = [Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0), Vec2::new(1.0, 2.0), Vec2::new(0.0, 2.0)];
        let f = Field::revolve(&profile);
        close(at(&f, 2.0, 1.0, 0.0), 1.0);
        close(at(&f, 0.0, 1.0, -3.0), 2.0);
        close(at(&f, 0.0, 3.0, 0.0), 1.0);
        // on the axis inside, the axis edge must not count as surface
        close(at(&f, 0.0, 1.0, 0.0), -1.0);
        close(at(&f, 0.9, 1.0, 0.0), -0.1);
    }

    #[test]
    fn union_and_smooth_union() {
        let a = || Field::sphere(1.0).translate(Vec3::new(-1.0, 0.0, 0.0));
        let b = || Field::sphere(1.0).translate(Vec3::new(1.0, 0.0, 0.0));
        let p = Vec3::new(0.0, 1.0, 0.0);
        let hard = a().union(b()).program().eval(p);
        close(hard, 2f32.sqrt() - 1.0);
        // both spheres are equally far, so the blend lowers it by exactly k / 4
        let smooth = a().smooth_union(b(), 0.4).program().eval(p);
        close(smooth, 2f32.sqrt() - 1.0 - 0.1);
        // far from the seam the blend changes nothing
        close(a().smooth_union(b(), 0.4).program().eval(Vec3::new(-3.0, 0.0, 0.0)), 1.0);
    }

    #[test]
    fn smooth_subtract_cuts() {
        let f = Field::sphere(1.0).smooth_subtract(Field::sphere(0.5), 0.0);
        close(at(&f, 0.0, 0.0, 0.0), 0.5);
        close(at(&f, 0.75, 0.0, 0.0), -0.25);
    }

    #[test]
    fn translate_only_applies_to_its_field() {
        let f = Field::sphere(1.0).translate(Vec3::new(5.0, 0.0, 0.0)).union(Field::sphere(1.0));
        close(at(&f, 0.0, 0.0, 0.0), -1.0);
        close(at(&f, 5.0, 0.0, 0.0), -1.0);
    }

    #[test]
    fn bend_moves_the_top_sideways() {
        let stem = || Field::capsule(Vec3::ZERO, Vec3::new(0.0, 2.0, 0.0), 0.1);
        let bent = stem().bend(0.3);
        let top = Vec3::new(0.0, 2.0, 0.0);
        assert!(stem().program().eval(top) < 0.0);
        assert!(bent.program().eval(top) > 0.0);
        assert!(bent.program().eval(Vec3::ZERO) < 0.0);
    }

    #[test]
    fn warp_is_deterministic_and_seeded() {
        let p = Vec3::new(0.9, 0.2, 0.1);
        let a = Field::sphere(1.0).warp(0.2, 3.0, 1).program();
        let b = Field::sphere(1.0).warp(0.2, 3.0, 2).program();
        assert_eq!(a.eval(p), a.eval(p));
        assert_ne!(a.eval(p), b.eval(p));
    }

    #[test]
    fn warp_keeps_large_seeds() {
        // both would round to the same f32
        let p = Vec3::new(0.9, 0.2, 0.1);
        let a = Field::sphere(1.0).warp(0.2, 3.0, 0xDEAD_BEEF).program();
        let b = Field::sphere(1.0).warp(0.2, 3.0, 0xDEAD_BEEE).program();
        assert_ne!(a.eval(p), b.eval(p));
    }

    #[test]
    fn eval_reads_encoded_fields() {
        let words = Field::sphere(2.0).encode();
        close(eval(&words, Vec3::new(3.0, 0.0, 0.0)), 1.0);
    }

    #[test]
    fn empty_field_is_infinitely_far() {
        assert_eq!(Program::parse(&[]).eval(Vec3::ZERO), f32::INFINITY);
    }

    #[test]
    #[should_panic(expected = "unknown field opcode")]
    fn rejects_unknown_ops() {
        Program::parse(&[99]);
    }

    #[test]
    #[should_panic(expected = "fewer")]
    fn rejects_unbalanced_programs() {
        let mut ops = Field::sphere(1.0).ops;
        ops.push(opcode::UNION);
        Program::parse(&ops);
    }
}
