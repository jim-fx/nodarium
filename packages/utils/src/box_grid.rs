use glam::{IVec3, Vec3};

/// A box that would go into more cells than this is kept aside and compared to every other box,
/// so one long stem among thousands of short strand segments doesn't fill the grid
const MAX_CELLS_PER_BOX: usize = 512;
/// The grid has at most this many cells per box, larger cells are used when the boxes are spread
/// far apart
const MAX_CELLS_PER_ENTRY: usize = 8;

#[derive(Clone, Copy)]
struct Entry {
    min: Vec3,
    max: Vec3,
    index: u32,
    /// the first cell the box is in
    first: IVec3,
}

/// A fixed set of boxes in a dense grid, to find the pairs that overlap without testing all of
/// them against each other. Nothing can be added after it is built, so the buckets sit in one
/// array, and the cells can be longer along one axis than along another, like the segments of a
/// bundle of strands.
pub struct BoxGrid {
    origin: Vec3,
    cell: Vec3,
    size: IVec3,
    /// where the boxes of every cell start in `entries`, one more than there are cells
    starts: Vec<u32>,
    /// the boxes of every cell, copied in, so the boxes of a cell are read one after another
    entries: Vec<Entry>,
    /// every box in the grid once
    small: Vec<Entry>,
    /// boxes that span too many cells to go into the grid
    large: Vec<Entry>,
    count: usize,
}

fn overlaps(a_min: Vec3, a_max: Vec3, b_min: Vec3, b_max: Vec3) -> bool {
    (a_min.cmple(b_max) & b_min.cmple(a_max)).all()
}

fn finite(min: Vec3, max: Vec3) -> bool {
    min.is_finite() && max.is_finite()
}

impl BoxGrid {
    /// `cell` is the size of a cell along each axis, about the size of the typical box. Boxes are
    /// given as `(min, max)`, a box with a coordinate that is not finite is in no pair.
    pub fn new(cell: Vec3, boxes: &[(Vec3, Vec3)]) -> BoxGrid {
        let (lo, hi) = boxes
            .iter()
            .filter(|(min, max)| finite(*min, *max))
            .fold((Vec3::MAX, Vec3::MIN), |(lo, hi), (min, max)| (lo.min(*min), hi.max(*max)));
        let any = lo.cmple(hi).all();
        let extent = if any { hi - lo } else { Vec3::ZERO };

        let usable = |c: f32| c.is_finite() && c > 0.0;
        let fallback = extent.max_element().max(1.0);
        let mut cell = Vec3::from_array(cell.to_array().map(|c| if usable(c) { c } else { fallback }));
        let budget = (boxes.len() * MAX_CELLS_PER_ENTRY).max(1) as f32;
        let total = |cell: Vec3| ((extent / cell).floor() + 1.0).to_array().iter().product::<f32>();
        while total(cell) > budget && cell.is_finite() {
            cell *= 1.5;
        }
        let size = ((extent / cell).floor() + 1.0).as_ivec3().max(IVec3::ONE);

        let mut grid = BoxGrid {
            origin: if any { lo } else { Vec3::ZERO },
            cell,
            size,
            starts: vec![0; (size.x * size.y * size.z) as usize + 1],
            entries: Vec::new(),
            small: Vec::new(),
            large: Vec::new(),
            count: boxes.len(),
        };

        // counted first, then filled, so the boxes of a cell sit next to each other
        let mut in_grid = vec![false; boxes.len()];
        for (index, &(min, max)) in boxes.iter().enumerate() {
            if !finite(min, max) {
                continue;
            }
            let (lo, hi) = (grid.key(min), grid.key(max));
            if cell_count(lo, hi) > MAX_CELLS_PER_BOX {
                grid.large.push(Entry { min, max, index: index as u32, first: lo });
                continue;
            }
            in_grid[index] = true;
            grid.small.push(Entry { min, max, index: index as u32, first: lo });
            let starts = &mut grid.starts;
            cells(grid.size, lo, hi, |c| starts[c + 1] += 1);
        }
        for c in 1..grid.starts.len() {
            grid.starts[c] += grid.starts[c - 1];
        }
        let mut fill = grid.starts.clone();
        let empty = Entry { min: Vec3::ZERO, max: Vec3::ZERO, index: 0, first: IVec3::ZERO };
        grid.entries = vec![empty; *grid.starts.last().unwrap() as usize];
        for (index, &(min, max)) in boxes.iter().enumerate() {
            if !in_grid[index] {
                continue;
            }
            let (lo, hi) = (grid.key(min), grid.key(max));
            let entries = &mut grid.entries;
            cells(grid.size, lo, hi, |c| {
                entries[fill[c] as usize] = Entry { min, max, index: index as u32, first: lo };
                fill[c] += 1;
            });
        }
        // sorted along x, so a box only has to be compared to the ones that start before it ends
        for c in 0..grid.starts.len() - 1 {
            let (start, end) = (grid.starts[c] as usize, grid.starts[c + 1] as usize);
            grid.entries[start..end].sort_unstable_by(|a, b| a.min.x.total_cmp(&b.min.x));
        }
        grid
    }

    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    fn key(&self, p: Vec3) -> IVec3 {
        ((p - self.origin) / self.cell).floor().as_ivec3().clamp(IVec3::ZERO, self.size - 1)
    }

    /// Calls `visit(a, b)` once for every pair of boxes that overlap, touching counts, with
    /// `a < b`. The indices are the places of the boxes in the list the grid was built from. The
    /// pairs come cell by cell, not in order.
    pub fn for_each_pair(&self, mut visit: impl FnMut(u32, u32)) {
        for (k, large) in self.large.iter().enumerate() {
            let others = self.large[k + 1..].iter().chain(self.small.iter());
            for other in others {
                if overlaps(large.min, large.max, other.min, other.max) {
                    visit(large.index.min(other.index), large.index.max(other.index));
                }
            }
        }

        for z in 0..self.size.z {
            for y in 0..self.size.y {
                for x in 0..self.size.x {
                    let c = ((z * self.size.y + y) * self.size.x + x) as usize;
                    let here = IVec3::new(x, y, z);
                    let cell = &self.entries[self.starts[c] as usize..self.starts[c + 1] as usize];
                    for (k, a) in cell.iter().enumerate() {
                        for b in &cell[k + 1..] {
                            if b.min.x > a.max.x {
                                break;
                            }
                            // sorted along x, so they overlap along x, and a pair that shares
                            // several cells is only reported in the first one
                            let overlap = (b.min.y <= a.max.y)
                                & (a.min.y <= b.max.y)
                                & (b.min.z <= a.max.z)
                                & (a.min.z <= b.max.z);
                            if overlap & (a.first.max(b.first) == here) {
                                visit(a.index.min(b.index), a.index.max(b.index));
                            }
                        }
                    }
                }
            }
        }
    }
}

fn cell_count(lo: IVec3, hi: IVec3) -> usize {
    let size = (hi - lo + 1).max(IVec3::ZERO);
    (size.x * size.y * size.z) as usize
}

fn cells(size: IVec3, lo: IVec3, hi: IVec3, mut visit: impl FnMut(usize)) {
    for z in lo.z..=hi.z {
        for y in lo.y..=hi.y {
            let row = ((z * size.y + y) * size.x) as usize;
            for x in lo.x..=hi.x {
                visit(row + x as usize);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Rng(u32);
    impl Rng {
        fn next(&mut self) -> f32 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 17;
            self.0 ^= self.0 << 5;
            self.0 as f32 / u32::MAX as f32 * 2.0 - 1.0
        }
        fn vec(&mut self) -> Vec3 {
            Vec3::new(self.next(), self.next(), self.next())
        }
    }

    fn brute_force(boxes: &[(Vec3, Vec3)]) -> Vec<(u32, u32)> {
        let mut pairs = Vec::new();
        for (a, &(a_min, a_max)) in boxes.iter().enumerate() {
            for (b, &(b_min, b_max)) in boxes.iter().enumerate().skip(a + 1) {
                if finite(a_min, a_max) && finite(b_min, b_max) && overlaps(a_min, a_max, b_min, b_max) {
                    pairs.push((a as u32, b as u32));
                }
            }
        }
        pairs
    }

    fn pairs(cell: Vec3, boxes: &[(Vec3, Vec3)]) -> Vec<(u32, u32)> {
        let mut pairs = Vec::new();
        BoxGrid::new(cell, boxes).for_each_pair(|a, b| pairs.push((a, b)));
        pairs.sort();
        pairs
    }

    #[test]
    fn finds_the_same_pairs_as_brute_force() {
        let mut rng = Rng(7);
        let boxes: Vec<(Vec3, Vec3)> = (0..3000)
            .map(|i| {
                let a = rng.vec() * 0.05;
                // a few boxes are far too large for the grid
                let size = if i % 500 == 0 { 0.2 } else { 0.004 * (rng.next() + 1.0) };
                (a, a + rng.vec().abs() * size)
            })
            .collect();
        let expected = brute_force(&boxes);
        assert!(expected.len() > 1000);
        // cells longer along y, like the segments of upright strands, cells smaller and larger
        // than the boxes
        for cell in [Vec3::new(0.003, 0.006, 0.003), Vec3::splat(0.001), Vec3::splat(0.02)] {
            assert_eq!(pairs(cell, &boxes), expected);
        }
    }

    #[test]
    fn boxes_that_only_touch_are_a_pair() {
        let boxes = [(Vec3::ZERO, Vec3::ONE), (Vec3::ONE, Vec3::splat(2.0)), (Vec3::splat(2.5), Vec3::splat(3.0))];
        assert_eq!(pairs(Vec3::splat(0.5), &boxes), vec![(0, 1)]);
    }

    #[test]
    fn far_apart_boxes_keep_the_grid_small() {
        let boxes = [(Vec3::ZERO, Vec3::splat(0.001)), (Vec3::splat(1000.0), Vec3::splat(1000.001))];
        let grid = BoxGrid::new(Vec3::splat(0.001), &boxes);
        assert!(grid.starts.len() <= 2 * MAX_CELLS_PER_ENTRY + 1);
        assert!(pairs(Vec3::splat(0.001), &boxes).is_empty());
    }

    #[test]
    fn an_empty_grid_has_no_pairs() {
        let grid = BoxGrid::new(Vec3::ONE, &[]);
        assert!(grid.is_empty());
        assert!(pairs(Vec3::ONE, &[]).is_empty());
    }

    #[test]
    fn boxes_at_the_ends_of_the_float_range_are_a_pair() {
        let boxes = [(Vec3::splat(-3e38), Vec3::splat(-3e38)), (Vec3::splat(-3e38), Vec3::splat(3e38))];
        assert_eq!(pairs(Vec3::splat(0.5), &boxes), vec![(0, 1)]);
    }

    #[test]
    fn boxes_that_are_not_finite_are_left_out() {
        let boxes = [(Vec3::ZERO, Vec3::ONE), (Vec3::NAN, Vec3::ONE), (Vec3::ZERO, Vec3::INFINITY), (Vec3::ZERO, Vec3::ONE)];
        assert_eq!(pairs(Vec3::splat(0.5), &boxes), vec![(0, 3)]);
    }
}
