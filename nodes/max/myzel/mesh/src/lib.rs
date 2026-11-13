use myzel_core::{mesh_flags, opaque, read_int, smooth_normals_in_words, MESH_TAG};
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::{
    abi::{self, Output},
    encode_float,
};

nodarium_definition_file!("src/definition.json");

/// The substrates in the order of the options of the `substrate` input, baked by
/// `bake/bake_substrates.py`, see `bake/SOURCES.md`
const SUBSTRATES: [&[u8]; 4] = [
    include_bytes!("../substrates/stump.bin"),
    include_bytes!("../substrates/moss.bin"),
    include_bytes!("../substrates/needles.bin"),
    include_bytes!("../substrates/leaves.bin"),
];

/// A baked substrate: quantized positions, sRGB colors and faces, see `bake/bake_substrates.py`
struct Baked<'a> {
    vertices: usize,
    faces: usize,
    min: [f32; 3],
    step: [f32; 3],
    data: &'a [u8],
}

impl<'a> Baked<'a> {
    const HEADER: usize = 32;

    fn read(data: &'a [u8]) -> Self {
        let u32_at = |at: usize| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        let f32_at = |at: usize| f32::from_bits(u32_at(at));
        let (vertices, faces) = (u32_at(0) as usize, u32_at(4) as usize);
        assert_eq!(data.len(), Self::HEADER + vertices * 9 + faces * 6, "a baked substrate of the wrong size");
        Baked {
            vertices,
            faces,
            min: [f32_at(8), f32_at(12), f32_at(16)],
            step: [f32_at(20), f32_at(24), f32_at(28)],
            data,
        }
    }

    fn u16_at(&self, at: usize) -> u16 {
        u16::from_le_bytes([self.data[at], self.data[at + 1]])
    }

    fn position(&self, v: usize, axis: usize) -> f32 {
        let q = self.u16_at(Self::HEADER + (v * 3 + axis) * 2);
        self.min[axis] + q as f32 * self.step[axis]
    }

    fn color(&self, v: usize) -> [u8; 3] {
        let at = Self::HEADER + self.vertices * 6 + v * 3;
        [self.data[at], self.data[at + 1], self.data[at + 2]]
    }

    fn corner(&self, i: usize) -> u16 {
        self.u16_at(Self::HEADER + self.vertices * 9 + i * 2)
    }

    /// The words of the mesh, see `myzel_core`
    fn len(&self, colored: bool) -> usize {
        4 + self.faces * 3 + self.vertices * (7 + colored as usize)
    }

    fn write(&self, colored: bool, words: &mut [i32]) {
        let (v, f) = (self.vertices, self.faces);
        let flags = if colored { mesh_flags::COLORS } else { 0 };
        words[..4].copy_from_slice(&[MESH_TAG, v as i32, f as i32, flags]);
        for i in 0..f * 3 {
            words[4 + i] = self.corner(i) as i32;
        }
        let positions = 4 + f * 3;
        for i in 0..v * 3 {
            words[positions + i] = encode_float(self.position(i / 3, i % 3));
        }
        if colored {
            let colors = positions + v * 6;
            for i in 0..v {
                words[colors + i] = opaque(self.color(i)) as i32;
            }
        }
        // the groups after the colors stay 0, the whole substrate is one group
        smooth_normals_in_words(words);
    }
}

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Output {
    let substrate = read_int(args[0]).clamp(0, SUBSTRATES.len() as i32 - 1) as usize;
    let colored = read_int(args[1]) != 0;
    let baked = Baked::read(SUBSTRATES[substrate]);
    abi::write_result(baked.len(colored), |words| {
        baked.write(colored, words);
        words.len()
    })
    .into()
}

#[cfg(test)]
mod tests {
    use super::{__nodarium_execute_inner, SUBSTRATES};
    use myzel_core::Mesh;

    fn run(substrate: i32, colored: bool) -> Mesh {
        Mesh::decode(unsafe { __nodarium_execute_inner(&[&[substrate], &[colored as i32]]).words() })
    }

    #[test]
    fn every_substrate_decodes_into_a_low_poly_mesh() {
        for substrate in 0..SUBSTRATES.len() as i32 {
            let mesh = run(substrate, true);
            assert!((1000..=6000).contains(&mesh.face_count()), "{} faces", mesh.face_count());
            assert!(mesh.faces.iter().all(|&i| (i as usize) < mesh.vertex_count()));
            assert!(mesh.positions.iter().all(|p| p.is_finite()));
            assert!(mesh.groups.iter().all(|&g| g == 0));
            for n in mesh.normals.chunks_exact(3) {
                assert!((n[0] * n[0] + n[1] * n[1] + n[2] * n[2] - 1.0).abs() < 1e-4);
            }
            assert_eq!(mesh.colors.len(), mesh.vertex_count());
            assert!(mesh.colors.iter().all(|&c| c >> 24 == 0xff), "opaque colors");
        }
    }

    #[test]
    fn the_substrates_have_their_real_size_around_the_axis() {
        // stump, moss, needles, leaves: how far they reach across, in metres
        for (substrate, (lo, hi)) in [(0.45, 0.65), (0.18, 0.28), (0.24, 0.34), (0.24, 0.34)].into_iter().enumerate() {
            let mesh = run(substrate as i32, true);
            let axis = |a: usize| mesh.positions.iter().skip(a).step_by(3).copied();
            let width = axis(0).fold(f32::MIN, f32::max) - axis(0).fold(f32::MAX, f32::min);
            assert!((lo..hi).contains(&width), "substrate {substrate} is {width} m across");
            let centre = axis(0).sum::<f32>() / mesh.vertex_count() as f32;
            assert!(centre.abs() < 0.05, "substrate {substrate} centred at x {centre}");
            // most of the surface faces up
            let up = mesh.normals.iter().skip(1).step_by(3).filter(|&&y| y > 0.0).count();
            assert!(up * 10 > mesh.vertex_count() * 7);
        }
    }

    #[test]
    fn colored_off_leaves_the_colors_out_and_nothing_else() {
        for substrate in 0..SUBSTRATES.len() as i32 {
            let colored = run(substrate, true);
            let plain = run(substrate, false);
            assert!(plain.colors.is_empty());
            assert_eq!(plain, Mesh { colors: Vec::new(), ..colored });
        }
    }
}
