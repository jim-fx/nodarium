//! Expansion of a primordium into the mature fruit body. A fruit body grows by
//! inflating the hyphae it already has, so this moves and thickens segments
//! and never adds any, which keeps it cheap enough to run on every change of
//! the time.
//!
//! Every segment is its parent's new end plus its own offset, transformed by
//! its tissue. Parents come first, so one pass moves everything and a tissue
//! carries whatever grows from it: the stipe carries the cap, the cap carries
//! the gills and the veil.
//!
//! - Stipe: offsets stretch along the axis, more in the upper half
//! - Cap: offsets that point down further than the open angle turn up towards
//!   it, which unrolls the margin, and the cap grows wider
//! - Gills: grow wider with the cap and a bit deeper
//! - Veil: stretches between cap margin and stipe until it tears, then the
//!   stipe side falls down into the ring and the cap side hangs from the margin
//!
//! Stages read their progress from schedules over `t`, so other species can
//! expand in a different order or not at all.

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use glam::{Quat, Vec3};

use crate::growth::Schedule;
use crate::hyphae::{flags, Hyphae, Tissue};

/// The torn veil is fully collapsed this far past its tear stretch, relative to it
const COLLAPSE_RANGE: f32 = 0.2;
/// How far the ring flares out from the stipe while it hangs down
const RING_FLARE: f32 = 0.35;
/// Strands tear at slightly different stretches and places, by these shares
const TEAR_JITTER: f32 = 0.2;
const POSITION_JITTER: f32 = 0.15;

#[derive(Clone, Debug, PartialEq)]
pub struct ExpandParams {
    pub seed: u32,
    /// Time from 0 to 1
    pub t: f32,
    /// Progress of the stipe elongation over `t`
    pub elongation: Schedule,
    /// Progress of the cap opening over `t`, the gills follow it
    pub opening: Schedule,
    /// How much longer the stipe gets, 2 makes it three times as long
    pub stipe_stretch: f32,
    /// 0 stretches the stipe evenly, 1 only towards the top
    pub apical_stretch: f32,
    /// How much wider the cap and gills get, 1 doubles them
    pub cap_spread: f32,
    /// Angle to the vertical the cap opens to, 0 is up, 0.5 sideways, 1 down
    pub open_angle: f32,
    /// How much deeper the gills get
    pub gill_deepening: f32,
    /// How much thicker the hyphae of stipe, cap and gills get
    pub inflation: f32,
    /// The veil tears once the gap it spans is this many times as wide as at the start
    pub veil_extensibility: f32,
    /// Where the veil tears, 0 at the stipe, 1 at the cap margin
    pub tear_position: f32,
}

impl Default for ExpandParams {
    fn default() -> Self {
        ExpandParams {
            seed: 0,
            t: 1.0,
            elongation: Schedule(vec![0.0, 0.0, 0.05, 0.0, 0.3, 0.5, 0.6, 1.0, 1.0, 1.0]),
            opening: Schedule(vec![0.0, 0.0, 0.3, 0.0, 0.65, 0.55, 0.95, 1.0, 1.0, 1.0]),
            stipe_stretch: 2.0,
            apical_stretch: 0.6,
            cap_spread: 1.0,
            open_angle: 0.6,
            gill_deepening: 0.3,
            inflation: 0.3,
            veil_extensibility: 1.6,
            tear_position: 0.8,
        }
    }
}

fn horizontal(v: Vec3) -> Vec3 {
    Vec3::new(v.x, 0.0, v.z)
}

/// Outwards from the axis at `p`, along `fallback` on the axis
fn radial_direction(p: Vec3, fallback: Vec3) -> Option<Vec3> {
    horizontal(p).try_normalize().or_else(|| horizontal(fallback).try_normalize())
}

/// A uniform value in [0, 1) per seed and index
fn hash(seed: u32, index: u32) -> f32 {
    let mut h = seed ^ index.wrapping_mul(0x9e37_79b9);
    h ^= h >> 16;
    h = h.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 13;
    h = h.wrapping_mul(0xc2b2_ae35);
    h ^= h >> 16;
    (h >> 8) as f32 / (1u32 << 24) as f32
}

/// Turns an offset that points down further than `angle` towards it and widens it.
/// Angles are measured from up over outwards, so an incurved margin, pointing
/// down and back to the axis, lies beyond down and unrolls through it.
fn open_offset(offset: Vec3, start: Vec3, angle: f32, open: f32, spread: f32) -> Vec3 {
    let mut opened = offset;
    if let Some(out) = radial_direction(start, offset) {
        let (a, b) = (offset.dot(out), offset.y);
        let mut current = a.atan2(b);
        if current < -FRAC_PI_2 {
            current += TAU;
        }
        if open > 0.0 && current > angle {
            let turned = current + (angle - current) * open;
            let length = a.hypot(b);
            opened += out * (length * turned.sin() - a) + Vec3::Y * (length * turned.cos() - b);
        }
    }
    let widen = spread * open;
    opened + horizontal(opened) * widen - offset
}

/// One veil strand, the stipe half from its anchor to the middle, then the cap half back to the margin
struct Strand {
    stipe_anchor: usize,
    cap_anchor: usize,
    stipe_half: Vec<usize>,
    cap_half: Vec<usize>,
}

/// Linear veil chains, each with the non-veil segment it hangs from
fn veil_chains(hyphae: &Hyphae) -> Vec<(usize, Vec<usize>)> {
    let mut chain_of = vec![usize::MAX; hyphae.len()];
    let mut chains: Vec<(usize, Vec<usize>)> = Vec::new();
    for (i, s) in hyphae.segments.iter().enumerate() {
        let Some(parent) = s.parent.map(|p| p as usize) else { continue };
        if s.tissue != Tissue::Veil {
            continue;
        }
        if hyphae.segments[parent].tissue == Tissue::Veil && chain_of[parent] != usize::MAX {
            chain_of[i] = chain_of[parent];
            chains[chain_of[i]].1.push(i);
        } else {
            chain_of[i] = chains.len();
            chains.push((parent, vec![i]));
        }
    }
    chains
}

/// Pairs every cap half with the nearest free stipe half by where their tips meet
fn strands(hyphae: &Hyphae) -> Vec<Strand> {
    let tip = |chain: &(usize, Vec<usize>)| hyphae.segments[*chain.1.last().unwrap()].end;
    let side = |chain: &(usize, Vec<usize>)| hyphae.segments[chain.1[0]].flags & flags::STIPE_SIDE != 0;
    let (stipe, cap): (Vec<_>, Vec<_>) = veil_chains(hyphae).into_iter().partition(side);
    let mut used = vec![false; stipe.len()];
    let mut strands = Vec::with_capacity(cap.len());
    for c in cap {
        let nearest = (0..stipe.len())
            .filter(|&j| !used[j])
            .min_by(|&a, &b| tip(&stipe[a]).distance_squared(tip(&c)).total_cmp(&tip(&stipe[b]).distance_squared(tip(&c))));
        let Some(j) = nearest else { break };
        used[j] = true;
        strands.push(Strand {
            stipe_anchor: stipe[j].0,
            cap_anchor: c.0,
            stipe_half: stipe[j].1.clone(),
            cap_half: c.1,
        });
    }
    strands
}

fn hang(from: Vec3, to: Vec3, amount: f32) -> Quat {
    match (from.try_normalize(), to.try_normalize()) {
        (Some(from), Some(to)) => Quat::IDENTITY.slerp(Quat::from_rotation_arc(from, to), amount),
        _ => Quat::IDENTITY,
    }
}

/// Places the veil between its moved anchors, or tears it
fn move_veil(hyphae: &Hyphae, out: &mut Hyphae, moved: &[Vec3], params: &ExpandParams) {
    for (k, strand) in strands(hyphae).iter().enumerate() {
        let (b0, a0) = (hyphae.segments[strand.stipe_anchor].end, hyphae.segments[strand.cap_anchor].end);
        let (db, da) = (moved[strand.stipe_anchor], moved[strand.cap_anchor]);
        let path: Vec<usize> = strand.stipe_half.iter().chain(strand.cap_half.iter().rev()).copied().collect();
        let mut along = Vec::with_capacity(path.len());
        let mut previous = b0;
        let mut length = 0.0;
        for &i in &path {
            length += previous.distance(hyphae.segments[i].end);
            along.push(length);
            previous = hyphae.segments[i].end;
        }
        length += previous.distance(a0);
        if length <= 0.0 {
            continue;
        }
        for w in &mut along {
            *w /= length;
        }

        let gap = a0.distance(b0);
        let stretch = if gap > 0.0 { (a0 + da).distance(b0 + db) / gap } else { 1.0 };
        let threshold = params.veil_extensibility.max(1.0) * (1.0 + TEAR_JITTER * (hash(params.seed, 2 * k as u32) - 0.5));
        if stretch <= threshold {
            for (&i, &w) in path.iter().zip(&along) {
                out.segments[i].end = hyphae.segments[i].end + db.lerp(da, w);
            }
            continue;
        }

        let tear_at = (params.tear_position + POSITION_JITTER * (hash(params.seed, 2 * k as u32 + 1) - 0.5)).clamp(0.02, 0.98);
        let mut on_ring: Vec<bool> = along.iter().map(|&w| w < tear_at).collect();
        // The segment that spans the tear is torn off its parent. The cap half
        // runs backwards along the path (a cap segment's parent comes after it),
        // so there it is the last one on the ring. A tear between the two tips
        // spans no segment: the cap tip has no children, so it moves to the ring
        // as a loose point.
        let halfway = strand.stipe_half.len();
        let first_off_ring = on_ring.iter().position(|&r| !r).unwrap_or(path.len());
        let broken = if first_off_ring < halfway {
            first_off_ring
        } else if first_off_ring > halfway {
            first_off_ring - 1
        } else {
            on_ring[halfway] = true;
            halfway
        };
        out.segments[path[broken]].flags |= flags::BROKEN;

        let collapse = ((stretch / threshold - 1.0) / COLLAPSE_RANGE).clamp(0.0, 1.0);
        let collapse = collapse * collapse * (3.0 - 2.0 * collapse);
        let out_dir = radial_direction(b0, a0 - b0).unwrap_or(Vec3::X);
        let ring = hang(a0 - b0, out_dir * RING_FLARE - Vec3::Y, collapse);
        let fringe = hang(b0 - a0, Vec3::NEG_Y, collapse);
        for (j, &i) in path.iter().enumerate() {
            let p = hyphae.segments[i].end;
            out.segments[i].end = if on_ring[j] { b0 + db + ring * (p - b0) } else { a0 + da + fringe * (p - a0) };
        }
    }
}

/// Moves stipe, cap, gills and veil to where they are at `params.t`, adds no segments
pub fn expand(hyphae: &Hyphae, params: &ExpandParams) -> Hyphae {
    let t = params.t.clamp(0.0, 1.0);
    let elongation = params.elongation.at(t);
    let opening = params.opening.at(t);
    let stretch = params.stipe_stretch.max(0.0) * elongation;
    let apical = params.apical_stretch.clamp(0.0, 1.0);
    let open_angle = params.open_angle.clamp(0.0, 1.0) * PI;
    let spread = params.cap_spread.max(0.0);
    let deepen = params.gill_deepening.max(0.0) * opening;

    let top = hyphae.segments.iter().filter(|s| s.tissue == Tissue::Stipe).map(|s| s.end.y).fold(0.0, f32::max);
    let mut out = hyphae.clone();
    let mut moved = vec![Vec3::ZERO; hyphae.len()];
    for (i, s) in hyphae.segments.iter().enumerate() {
        let Some(parent) = s.parent.map(|p| p as usize) else { continue };
        let start = hyphae.segments[parent].end;
        let offset = s.end - start;
        let (own, inflate) = match s.tissue {
            Tissue::Stipe => {
                let height = if top > 0.0 { (start.y / top).clamp(0.0, 1.0) } else { 0.0 };
                let weight = 1.0 + apical * (2.0 * height - 1.0);
                (Vec3::Y * offset.y * stretch * weight, elongation)
            }
            Tissue::Cap | Tissue::CapCuticle => (open_offset(offset, start, open_angle, opening, spread), opening),
            Tissue::Gill => {
                let widen = horizontal(offset) * spread * opening;
                (widen + Vec3::Y * offset.y * deepen, opening)
            }
            Tissue::Veil | Tissue::Mycelium => (Vec3::ZERO, 0.0),
        };
        moved[i] = moved[parent] + own;
        out.segments[i].end = s.end + moved[i];
        out.segments[i].radius = s.radius * (1.0 + params.inflation.max(0.0) * inflate);
    }
    move_veil(hyphae, &mut out, &moved, params);
    out
}

#[cfg(test)]
mod tests {
    use std::sync::OnceLock;

    use super::*;
    use crate::gills::{grow_gills, GillParams};
    use crate::growth::{grow, GrowthParams};
    use crate::hyphae::HyphaSegment;

    fn button() -> &'static Hyphae {
        static BUTTON: OnceLock<Hyphae> = OnceLock::new();
        BUTTON.get_or_init(|| grow_gills(&grow(&GrowthParams::default()), &GillParams::default()))
    }

    fn at(t: f32) -> Hyphae {
        expand(button(), &ExpandParams { t, ..ExpandParams::default() })
    }

    fn top(h: &Hyphae, tissue: Tissue) -> f32 {
        h.segments.iter().filter(|s| s.tissue == tissue).map(|s| s.end.y).fold(f32::MIN, f32::max)
    }

    fn width(h: &Hyphae) -> f32 {
        h.segments.iter().filter(|s| s.tissue == Tissue::Cap).map(|s| horizontal(s.end).length()).fold(0.0, f32::max)
    }

    fn broken(h: &Hyphae) -> usize {
        h.segments.iter().filter(|s| s.flags & flags::BROKEN != 0).count()
    }

    #[test]
    fn nothing_changes_at_the_start() {
        assert_eq!(&at(0.0), button());
    }

    #[test]
    fn only_moves_and_inflates() {
        let h = at(1.0);
        assert_eq!(h.len(), button().len());
        for (a, b) in h.segments.iter().zip(&button().segments) {
            assert_eq!((a.parent, a.tissue, a.birth), (b.parent, b.tissue, b.birth));
            assert!(a.radius >= b.radius);
        }
    }

    #[test]
    fn the_stipe_gets_taller_and_carries_the_cap() {
        let steps: Vec<Hyphae> = [0.0, 0.2, 0.4, 0.6, 1.0].iter().map(|&t| at(t)).collect();
        for pair in steps.windows(2) {
            assert!(top(&pair[1], Tissue::Stipe) >= top(&pair[0], Tissue::Stipe));
        }
        let (start, end) = (&steps[0], &steps[4]);
        assert!(top(end, Tissue::Stipe) > 2.5 * top(start, Tissue::Stipe));
        let lift = top(end, Tissue::Stipe) - top(start, Tissue::Stipe);
        assert!(top(end, Tissue::Cap) - top(start, Tissue::Cap) > lift * 0.8);
    }

    #[test]
    fn the_upper_stipe_stretches_more() {
        let params = ExpandParams { t: 1.0, apical_stretch: 1.0, ..ExpandParams::default() };
        let h = expand(button(), &params);
        let height = top(button(), Tissue::Stipe);
        let (mut lower, mut upper) = (0.0, 0.0);
        for (i, s) in button().segments.iter().enumerate().filter(|(_, s)| s.tissue == Tissue::Stipe) {
            let (before, after) = (s.end.y - button().start(i).y, h.segments[i].end.y - h.start(i).y);
            if button().start(i).y < height * 0.4 {
                lower += after - before;
            } else if button().start(i).y > height * 0.6 {
                upper += after - before;
            }
        }
        assert!(upper > lower * 2.0, "upper {} lower {}", upper, lower);
    }

    #[test]
    fn the_cap_opens() {
        let (start, end) = (button(), &at(1.0));
        assert!(width(end) > 1.5 * width(start));
        // the margin turns from pointing down to the open angle
        let pointing_down = |h: &Hyphae| {
            (0..h.len())
                .filter(|&i| h.segments[i].tissue == Tissue::Cap && h.segments[i].parent.is_some())
                .filter(|&i| (h.segments[i].end - h.start(i)).normalize().y < -0.7)
                .count()
        };
        assert!(pointing_down(end) * 10 < pointing_down(start), "{} {}", pointing_down(end), pointing_down(start));
    }

    #[test]
    fn the_veil_holds_until_it_tears() {
        let intact = at(0.3);
        assert_eq!(broken(&intact), 0);
        assert!(top(&intact, Tissue::Stipe) > top(button(), Tissue::Stipe));

        let torn = at(1.0);
        let strands = strands(button());
        assert!(strands.len() > 50);
        assert_eq!(broken(&torn), strands.len());
        for s in torn.segments.iter().filter(|s| s.flags & flags::BROKEN != 0) {
            assert_eq!(s.tissue, Tissue::Veil);
        }
    }

    #[test]
    fn the_torn_veil_becomes_a_ring_and_a_fringe() {
        let torn = at(1.0);
        let stipe_radius = torn
            .segments
            .iter()
            .filter(|s| s.tissue == Tissue::Stipe)
            .map(|s| horizontal(s.end).length())
            .fold(0.0, f32::max);
        for strand in strands(button()) {
            let ring = torn.segments[strand.stipe_anchor].end;
            let margin = torn.segments[strand.cap_anchor].end;
            for &i in &strand.stipe_half[..strand.stipe_half.len() / 2] {
                let p = torn.segments[i].end;
                assert!(p.y <= ring.y + 1e-4, "the ring hangs down from where the veil grew from the stipe");
                assert!(horizontal(p).length() < stipe_radius * 1.5, "the ring stays at the stipe");
            }
            let edge = torn.segments[strand.cap_half[0]].end;
            assert!(edge.distance(margin) < 0.001, "the fringe hangs at the margin");
            assert!(edge.y < margin.y);
        }
    }

    #[test]
    fn no_torn_veil_segment_spans_the_tear() {
        let before = button().to_segments();
        for seed in 0..4 {
            for step in 1..50 {
                let params = ExpandParams { t: 1.0, seed, tear_position: step as f32 / 50.0, ..ExpandParams::default() };
                let h = expand(button(), &params);
                for (i, s) in h.to_segments().iter().enumerate() {
                    if h.segments[i].tissue != Tissue::Veil || h.segments[i].flags & flags::BROKEN != 0 {
                        continue;
                    }
                    let (length, original) = (s.start.distance(s.end), before[i].start.distance(before[i].end));
                    assert!(length <= original * 1.01 + 1e-6, "veil segment {} stretched from {} to {}", i, original, length);
                }
            }
        }
    }

    #[test]
    fn the_same_inputs_give_the_same_mushroom() {
        assert_eq!(at(0.7), at(0.7));
        let other = expand(button(), &ExpandParams { t: 0.7, seed: 3, ..ExpandParams::default() });
        assert_ne!(other, at(0.7));
        assert_eq!(other.segments[..5000], at(0.7).segments[..5000]);
    }

    #[test]
    fn works_without_veil_or_stipe() {
        assert_eq!(expand(&Hyphae::default(), &ExpandParams::default()), Hyphae::default());
        let mut h = Hyphae::default();
        let root =
            h.push(HyphaSegment { end: Vec3::ZERO, radius: 0.1, parent: None, birth: 0.0, tissue: Tissue::Cap, flags: 0 });
        h.push(HyphaSegment { end: Vec3::new(0.0, -1.0, 1.0), parent: Some(root), ..h.segments[0] });
        let e = expand(&h, &ExpandParams::default());
        assert_eq!(e.len(), 2);
        assert!(e.segments[1].end.y > -1.0);
    }
}
