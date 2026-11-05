//! The veins as smooth lines for the mesh. The model grows them out of
//! straight segments that meet at kinks, so every strand is resampled and
//! smoothed here. A strand starts on the smoothed strand it branches off and
//! keeps its end on the margin, so the veins stay connected.

use glam::Vec2;

use crate::sim::Strand;

pub struct VeinLine {
    pub points: Vec<Vec2>,
    /// Number of CPs supplied through each point.
    pub flux: Vec<f32>,
    /// Indices of the points other lines branch off at.
    pub anchors: Vec<usize>,
}

impl VeinLine {
    pub fn length(&self) -> f32 {
        self.points.windows(2).map(|w| w[0].distance(w[1])).sum()
    }

    fn mirrored(&self) -> VeinLine {
        VeinLine {
            points: self.points.iter().map(|p| Vec2::new(-p.x, p.y)).collect(),
            flux: self.flux.clone(),
            anchors: self.anchors.clone(),
        }
    }
}

/// Lines of both halves from the strands of the right half, sampled every
/// `step` and smoothed `passes` times. Every line comes after the one it
/// branches off.
pub fn lines(strands: &[Strand], step: f32, passes: usize) -> Vec<VeinLine> {
    let mut lines: Vec<VeinLine> = Vec::with_capacity(strands.len() * 2);
    // where every point of a strand ended up in its line
    let mut samples: Vec<Vec<usize>> = Vec::with_capacity(strands.len());
    for strand in strands {
        let (mut points, flux, index) = resample(strand, step);
        if let Some((parent, k)) = strand.parent {
            let at = samples[parent][k];
            points[0] = lines[parent].points[at];
            lines[parent].anchors.push(at);
        }
        smooth(&mut points, passes);
        samples.push(index);
        lines.push(VeinLine {
            points,
            flux,
            anchors: Vec::new(),
        });
    }

    // The midvein lies on the axis, everything else gets a mirror image.
    let right = lines.len();
    for i in 0..right {
        if lines[i].points.iter().any(|p| p.x.abs() > 1e-4 * step) {
            let mirrored = lines[i].mirrored();
            lines.push(mirrored);
        }
    }
    lines
}

/// Splits every segment into pieces of at most `step`. Returns the points, the
/// flux at each of them and the index of every point of the strand.
fn resample(strand: &Strand, step: f32) -> (Vec<Vec2>, Vec<f32>, Vec<usize>) {
    let mut points = vec![strand.points[0]];
    let mut flux = vec![strand.flux[0]];
    let mut index = vec![0];
    for (w, &f) in strand.points.windows(2).zip(&strand.flux[1..]) {
        let pieces = (w[0].distance(w[1]) / step).ceil().max(1.0) as usize;
        for j in 1..=pieces {
            points.push(w[0].lerp(w[1], j as f32 / pieces as f32));
            flux.push(f);
        }
        index.push(points.len() - 1);
    }
    (points, flux, index)
}

/// Laplacian smoothing with both ends held in place.
fn smooth(points: &mut [Vec2], passes: usize) {
    let n = points.len();
    if n < 3 {
        return;
    }
    let mut previous = points.to_vec();
    for _ in 0..passes {
        previous.copy_from_slice(points);
        for i in 1..n - 1 {
            points[i] = previous[i] * 0.5 + (previous[i - 1] + previous[i + 1]) * 0.25;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Angle between the segments meeting at `points[i]`, in degrees.
    fn turn(points: &[Vec2], i: usize) -> f32 {
        (points[i] - points[i - 1]).angle_to(points[i + 1] - points[i]).abs().to_degrees()
    }

    #[test]
    fn kinks_get_rounded_and_branches_stay_attached() {
        // A vein with a kink of 45 degrees, and one branching off at the kink.
        let strands = [
            Strand {
                points: vec![Vec2::new(10.0, 0.0), Vec2::new(10.0, 20.0), Vec2::new(30.0, 40.0)],
                flux: vec![2.0, 2.0, 1.0],
                parent: None,
            },
            Strand {
                points: vec![Vec2::new(10.0, 20.0), Vec2::new(-10.0, 30.0)],
                flux: vec![1.0, 1.0],
                parent: Some((0, 1)),
            },
        ];
        let lines = lines(&strands, 0.5, 20);
        let (main, branch) = (&lines[0], &lines[1]);

        // both ends stay where they were
        assert_eq!(main.points[0], strands[0].points[0]);
        assert_eq!(*main.points.last().unwrap(), strands[0].points[2]);
        assert_eq!(*branch.points.last().unwrap(), strands[1].points[1]);
        // the branch starts on the smoothed vein, where the kink was
        assert_eq!(main.anchors.len(), 1);
        assert_eq!(branch.points[0], main.points[main.anchors[0]]);
        assert!(branch.points[0].distance(strands[0].points[1]) > 0.1);
        // no sharp turn is left along the vein
        let sharpest = (1..main.points.len() - 1).map(|i| turn(&main.points, i)).fold(0.0, f32::max);
        assert!(sharpest < 10.0, "turns by {sharpest} degrees");
        // every strand off the axis gets a mirror image
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[2].points[5], lines[0].points[5] * Vec2::new(-1.0, 1.0));
    }
}
