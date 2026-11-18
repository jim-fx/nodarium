use super::NO_PATH;
use crate::{decode_float, encode_float, NO_COLOR};
use glam::Vec3;
use std::borrow::Cow;

/// The type word of a geometry, after its opening bracket
pub const GEOMETRY_TAG: i32 = 1;

/// The words before the faces of an encoded geometry: the opening bracket and the header
/// `[1, v, f, path id, flags]`
pub const GEOMETRY_HEAD: usize = 7;

/// The closing bracket after the last section
const TAIL: usize = 2;

/// Bits of the flags word in the header of a geometry
pub mod geometry_flags {
    /// One RGBA8 color per vertex follows the normals
    pub const COLORS: i32 = 1;
    /// The groups are followed by how many there are and, for every group, the graph node that
    /// made it, see [`super::Mesh::creators`]. Only with [`GROUPS`].
    pub const CREATORS: i32 = 2;
    /// One group per vertex follows the colors. Without it the geometry is one group
    pub const GROUPS: i32 = 4;
    /// The tables are followed by how many paths the geometry was made from and the link of each
    /// of them, [`super::PATH_LINK_WORDS`] words, see [`super::Mesh::paths`]
    pub const PATHS: i32 = 8;
}

use geometry_flags::{COLORS, CREATORS, GROUPS, PATHS};

/// The words of the link of a path in the table of paths of a geometry: its id, parent id, parent
/// alpha, creator, index and grown, as they are in the header of the path
pub const PATH_LINK_WORDS: usize = 6;

/// The creator of a group that no graph node is known for
pub const NO_CREATOR: i32 = i32::MIN;

/// A geometry with a material group and optionally a color per vertex, decoded from its words,
/// see [`crate::geometry`] for the format. It is encoded without a path id.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesh {
    pub faces: Vec<u32>,
    pub positions: Vec<f32>,
    pub normals: Vec<f32>,
    /// Empty or one color per vertex, [`NO_COLOR`] where the vertex has none of its own
    pub colors: Vec<u32>,
    /// One group per vertex, other programs can give each group its own material
    pub groups: Vec<u32>,
    /// Empty, or for every group the creator id of the graph node that made it ([`NO_CREATOR`]
    /// for none), so the viewer can lead from a part of the geometry to its node
    pub creators: Vec<i32>,
    /// Empty, or the links of the paths the geometry was made from, [`PATH_LINK_WORDS`] words
    /// each, so the viewer can lead from what sits on a path to the path and its node
    pub paths: Vec<i32>,
}

/// Packs a color with components in [0, 1] into one RGBA8 word, red in the lowest byte
pub fn pack_rgba(r: f32, g: f32, b: f32, a: f32) -> u32 {
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
    byte(r) | byte(g) << 8 | byte(b) << 16 | byte(a) << 24
}

/// The words of a geometry up to the end of its groups, the table of its creators and the
/// closing bracket follow them
fn sections_len(v: usize, f: usize, flags: i32) -> usize {
    let per_vertex = 6 + usize::from(flags & COLORS != 0) + usize::from(flags & GROUPS != 0);
    GEOMETRY_HEAD + f * 3 + v * per_vertex
}

/// The words of the table of `creators`: how many there are, then each of them. No creators, no
/// table.
fn table_len(creators: usize) -> usize {
    if creators == 0 { 0 } else { 1 + creators }
}

/// The words of the table of paths with the `words` of their links
fn paths_len(words: usize) -> usize {
    if words == 0 { 0 } else { 1 + words }
}

/// The groups a geometry with these groups numbers: one more than the highest
fn group_count(groups: impl Iterator<Item = u32>) -> usize {
    groups.max().map_or(0, |g| g as usize + 1)
}

/// The tables of an encoded geometry whose sections are `base` long: its creators and the links
/// of its paths, each empty without its flag. `None` when the words after the sections aren't the
/// tables and the closing bracket.
fn read_tables(words: &[i32], base: usize, flags: i32) -> Option<(&[i32], &[i32])> {
    let end = words.len().checked_sub(TAIL)?;
    let (creators, at) = read_table(words, base, end, flags & CREATORS != 0, 1)?;
    let (paths, at) = read_table(words, at, end, flags & PATHS != 0, PATH_LINK_WORDS)?;
    (at == end).then_some((creators, paths))
}

/// The entries of `size` words of the table at `at`, before `end`, and where the next one starts
fn read_table(words: &[i32], at: usize, end: usize, present: bool, size: usize) -> Option<(&[i32], usize)> {
    if !present {
        return Some((&words[..0], at));
    }
    let n = usize::try_from(*words.get(at)?).ok()?;
    let stop = (at + 1).checked_add(n.checked_mul(size)?)?;
    (stop <= end).then(|| (&words[at + 1..stop], stop))
}

/// The opening bracket and the header of a geometry of `len` words, and its closing bracket
fn put_frame(words: &mut [i32], v: usize, f: usize, path_id: i32, flags: i32) {
    let len = words.len();
    words[..GEOMETRY_HEAD].copy_from_slice(&[0, len as i32 - 3, GEOMETRY_TAG, v as i32, f as i32, path_id, flags]);
    words[len - TAIL..].copy_from_slice(&[1, 1]);
}

impl Mesh {
    pub fn vertex_count(&self) -> usize {
        self.positions.len() / 3
    }

    pub fn face_count(&self) -> usize {
        self.faces.len() / 3
    }

    /// Room for exactly `vertices` and `faces` more, without colors. A mesh that grows by
    /// doubling holds up to twice what it needs, and the old and the new buffer while it grows.
    pub fn reserve_exact(&mut self, vertices: usize, faces: usize) {
        self.positions.reserve_exact(vertices * 3);
        self.normals.reserve_exact(vertices * 3);
        self.groups.reserve_exact(vertices);
        self.faces.reserve_exact(faces * 3);
    }

    /// The mesh with all of its groups made by the graph node `creator`, as a node that makes
    /// its mesh alone sends it. A mesh without vertices stays without creators.
    pub fn made_by(mut self, creator: i32) -> Mesh {
        self.creators = vec![creator; group_count(self.groups.iter().copied())];
        self
    }

    pub fn encode(&self) -> Vec<i32> {
        let (v, f) = (self.vertex_count(), self.face_count());
        assert!(self.colors.is_empty() || self.colors.len() == v, "{} colors for {} vertices", self.colors.len(), v);
        assert_eq!(self.groups.len(), v, "every vertex needs a group");
        let colors = if self.colors.is_empty() { 0 } else { COLORS };
        let creators = if self.creators.is_empty() { 0 } else { CREATORS };
        let paths = if self.paths.is_empty() { 0 } else { PATHS };
        assert_eq!(self.paths.len() % PATH_LINK_WORDS, 0, "{} words for the links of paths", self.paths.len());
        let flags = GROUPS | colors | creators | paths;
        let len = sections_len(v, f, flags) + table_len(self.creators.len()) + paths_len(self.paths.len()) + TAIL;
        let mut words = Vec::with_capacity(len);
        words.extend([0, len as i32 - 3, GEOMETRY_TAG, v as i32, f as i32, NO_PATH, flags]);
        words.extend(self.faces.iter().map(|&i| i as i32));
        words.extend(self.positions.iter().map(|&p| encode_float(p)));
        words.extend(self.normals.iter().map(|&n| encode_float(n)));
        words.extend(self.colors.iter().map(|&c| c as i32));
        words.extend(self.groups.iter().map(|&g| g as i32));
        if !self.creators.is_empty() {
            words.push(self.creators.len() as i32);
            words.extend(&self.creators);
        }
        if !self.paths.is_empty() {
            words.push((self.paths.len() / PATH_LINK_WORDS) as i32);
            words.extend(&self.paths);
        }
        words.extend([1, 1]);
        words
    }

    /// Adds another mesh to this one, its groups numbered from `first_group` on. When only one of
    /// them has colors, the vertices of the other get [`NO_COLOR`].
    pub fn append(&mut self, other: &Mesh, first_group: u32) {
        let offset = self.vertex_count() as u32;
        if !self.creators.is_empty() || !other.creators.is_empty() {
            let own = self.creators.len().max(first_group as usize);
            self.creators.resize(own, NO_CREATOR);
            let groups = group_count(other.groups.iter().copied());
            self.creators.extend((0..groups).map(|g| other.creators.get(g).copied().unwrap_or(NO_CREATOR)));
        }
        if !self.colors.is_empty() || !other.colors.is_empty() {
            self.colors.resize(self.vertex_count(), NO_COLOR);
            if other.colors.is_empty() {
                self.colors.extend(std::iter::repeat_n(NO_COLOR, other.vertex_count()));
            } else {
                self.colors.extend(&other.colors);
            }
        }
        self.faces.extend(other.faces.iter().map(|&i| i + offset));
        self.positions.extend(&other.positions);
        self.normals.extend(&other.normals);
        self.groups.extend(other.groups.iter().map(|&g| g + first_group));
        self.paths.extend(&other.paths);
    }

    /// The geometry of `words`, a geometry without groups is one group 0. Panics on anything
    /// else, see [`Mesh::try_decode`].
    pub fn decode(words: &[i32]) -> Mesh {
        Self::try_decode(words).unwrap_or_else(|| {
            panic!(
                "expected a geometry [0, n, 1, v, f, path id, flags, ...] whose sections fill its {} words, got one starting with {:?}",
                words.len(),
                &words[..words.len().min(GEOMETRY_HEAD)]
            )
        })
    }

    /// The geometry when `words` are one, `None` for anything else, see [`MeshWords::read`]
    pub fn try_decode(words: &[i32]) -> Option<Mesh> {
        MeshWords::read(words).map(|mesh| mesh.to_mesh())
    }
}

/// The geometry an input brings: a geometry, or the geometries of a list, like the one of `join`,
/// joined into one, see [`MeshWords::join`]. `None` when it brings none.
pub fn geometry_input(arg: &[i32]) -> Option<Cow<'_, [i32]>> {
    if MeshWords::read(arg).is_some() {
        return Some(Cow::Borrowed(arg));
    }
    if arg.len() < 4 || arg[0] != 0 {
        return None;
    }
    let items: Vec<MeshWords> = super::split_items(arg).into_iter().filter_map(MeshWords::read).collect();
    match items.as_slice() {
        [] => None,
        [one] => Some(Cow::Borrowed(one.words)),
        _ => Some(Cow::Owned(MeshWords::join(&items))),
    }
}

/// Where a builder adds its vertices and faces: a [`Mesh`], or a [`MeshWriter`] that writes them
/// straight into the encoded words
pub trait MeshBuild {
    fn vertex_count(&self) -> usize;
    /// Adds a vertex with a zero normal, see `smooth_normals` of `myzel_core`
    fn push_vertex(&mut self, position: Vec3, group: u32);
    fn push_face(&mut self, face: [u32; 3]);
    /// Drops the vertices from `vertices` on, before a face uses them
    fn truncate_vertices(&mut self, vertices: usize);
}

impl MeshBuild for Mesh {
    fn vertex_count(&self) -> usize {
        Mesh::vertex_count(self)
    }

    fn push_vertex(&mut self, position: Vec3, group: u32) {
        self.positions.extend(position.to_array());
        self.normals.extend([0.0; 3]);
        self.groups.push(group);
    }

    fn push_face(&mut self, face: [u32; 3]) {
        self.faces.extend(face);
    }

    fn truncate_vertices(&mut self, vertices: usize) {
        self.positions.truncate(vertices * 3);
        self.normals.truncate(vertices * 3);
        self.groups.truncate(vertices);
    }
}

/// A geometry with groups and without colors written straight into its encoded words, by a
/// builder that counts its vertices and faces before it adds them. A [`Mesh`] that is encoded is
/// in memory twice, as the mesh and as its words.
///
/// The writer has room for the counted vertices and faces, a builder that adds more panics. One
/// that adds fewer leaves a gap after the faces and after the vertices, which [`MeshWriter::finish`]
/// closes.
pub struct MeshWriter<'a> {
    words: &'a mut [i32],
    /// What there is room for
    vertices: usize,
    faces: usize,
    /// What was added
    v: usize,
    f: usize,
}

impl<'a> MeshWriter<'a> {
    /// The words of a geometry of `vertices` and `faces` with groups and without colors
    pub fn len(vertices: usize, faces: usize) -> usize {
        sections_len(vertices, faces, GROUPS) + TAIL
    }

    /// A writer for up to `vertices` and `faces` into `words`, which are zero and at least
    /// [`MeshWriter::len`] long
    pub fn new(words: &'a mut [i32], vertices: usize, faces: usize) -> Self {
        assert!(words.len() >= Self::len(vertices, faces), "{} words for {} vertices and {} faces", words.len(), vertices, faces);
        MeshWriter { words, vertices, faces, v: 0, f: 0 }
    }

    /// The words a geometry of `vertices` and `faces` needs with the table of `creators` after
    /// it, see [`MeshWriter::finish_with`]
    pub fn len_with(vertices: usize, faces: usize, creators: usize) -> usize {
        Self::len(vertices, faces) + table_len(creators)
    }

    /// Closes the gaps of what wasn't added and writes the header, returns the words of the
    /// geometry. The normals are zero.
    pub fn finish(self) -> usize {
        self.finish_with(&[])
    }

    /// [`MeshWriter::finish`] with the creator of every group after the groups. The words have
    /// room for them, see [`MeshWriter::len_with`]. A geometry without vertices gets no table.
    pub fn finish_with(self, creators: &[i32]) -> usize {
        let MeshWriter { words, vertices, faces, v, f } = self;
        let head = GEOMETRY_HEAD;
        let (positions, groups) = (head + faces * 3, head + faces * 3 + vertices * 6);
        // every section only moves towards the front, past the end of the one before it
        words.copy_within(positions..positions + v * 3, head + f * 3);
        words[head + f * 3 + v * 3..head + f * 3 + v * 6].fill(0);
        words.copy_within(groups..groups + v, head + f * 3 + v * 6);
        let base = sections_len(v, f, GROUPS);
        let creators = if v == 0 { &[] } else { creators };
        let flags = GROUPS | if creators.is_empty() { 0 } else { CREATORS };
        if !creators.is_empty() {
            words[base] = creators.len() as i32;
            words[base + 1..base + 1 + creators.len()].copy_from_slice(creators);
        }
        let len = base + table_len(creators.len()) + TAIL;
        put_frame(&mut words[..len], v, f, NO_PATH, flags);
        len
    }
}

impl MeshBuild for MeshWriter<'_> {
    fn vertex_count(&self) -> usize {
        self.v
    }

    fn push_vertex(&mut self, position: Vec3, group: u32) {
        assert!(self.v < self.vertices, "more than the {} vertices counted", self.vertices);
        let at = GEOMETRY_HEAD + self.faces * 3 + self.v * 3;
        self.words[at..at + 3].copy_from_slice(&position.to_array().map(encode_float));
        self.words[GEOMETRY_HEAD + self.faces * 3 + self.vertices * 6 + self.v] = group as i32;
        self.v += 1;
    }

    fn push_face(&mut self, face: [u32; 3]) {
        assert!(self.f < self.faces, "more than the {} faces counted", self.faces);
        let at = GEOMETRY_HEAD + self.f * 3;
        self.words[at..at + 3].copy_from_slice(&face.map(|i| i as i32));
        self.f += 1;
    }

    fn truncate_vertices(&mut self, vertices: usize) {
        self.v = self.v.min(vertices);
    }
}

/// The faces, positions and normals of an encoded geometry, the normals to change
pub struct Sections<'a> {
    pub faces: &'a [u32],
    pub positions: &'a [f32],
    pub normals: &'a mut [f32],
}

/// The sections of the words of a geometry, see [`MeshWords::read`] for what they must be
pub fn geometry_sections(words: &mut [i32]) -> Sections<'_> {
    let (v, f) = (words[3] as usize, words[4] as usize);
    let (faces, rest) = words[GEOMETRY_HEAD..].split_at_mut(f * 3);
    let (positions, rest) = rest.split_at_mut(v * 3);
    // SAFETY: u32 and f32 are as large and aligned as i32, and any of their bits are a value
    unsafe {
        Sections {
            faces: std::slice::from_raw_parts(faces.as_ptr().cast(), faces.len()),
            positions: std::slice::from_raw_parts(positions.as_ptr().cast(), positions.len()),
            normals: std::slice::from_raw_parts_mut(rest.as_mut_ptr().cast(), v * 3),
        }
    }
}

/// Writes words one after the other into a buffer that is exactly as long as all of them
struct Words<'a> {
    words: &'a mut [i32],
    at: usize,
}

impl<'a> Words<'a> {
    fn new(words: &'a mut [i32]) -> Self {
        Words { words, at: 0 }
    }

    fn put(&mut self, words: &[i32]) {
        self.words[self.at..self.at + words.len()].copy_from_slice(words);
        self.at += words.len();
    }

    fn fill(&mut self, n: usize, word: i32) {
        self.words[self.at..self.at + n].fill(word);
        self.at += n;
    }

    fn extend(&mut self, words: impl ExactSizeIterator<Item = i32>) {
        let n = words.len();
        for (to, word) in self.words[self.at..self.at + n].iter_mut().zip(words) {
            *to = word;
        }
        self.at += n;
    }

    fn done(self) {
        assert_eq!(self.at, self.words.len(), "{} words written into {}", self.at, self.words.len());
    }
}

/// A geometry read in place from its words. Nodes that only pass geometry on build their output
/// from these, a decoded [`Mesh`] would be one more copy of every geometry in the node's memory.
#[derive(Clone, Copy, Debug)]
pub struct MeshWords<'a> {
    words: &'a [i32],
    v: usize,
    f: usize,
    flags: i32,
    /// The creator of every group, empty when the geometry has no table of them
    creators: &'a [i32],
    /// The links of its paths, empty when the geometry has no table of them
    paths: &'a [i32],
}

impl<'a> MeshWords<'a> {
    /// The geometry when `words` are one, brackets included, `None` for anything else: counts
    /// that don't match the length, unknown flags or a face with a vertex that isn't there
    pub fn read(words: &'a [i32]) -> Option<Self> {
        let n = words.len();
        if n < GEOMETRY_HEAD + TAIL || words[0] != 0 || words[2] != GEOMETRY_TAG || words[n - TAIL] != 1 {
            return None;
        }
        if words[1] as i64 != n as i64 - 3 {
            return None;
        }
        let (v, f, flags) = (usize::try_from(words[3]).ok()?, usize::try_from(words[4]).ok()?, words[6]);
        if flags & !(COLORS | CREATORS | GROUPS | PATHS) != 0 || (flags & CREATORS != 0 && flags & GROUPS == 0) {
            return None;
        }
        // in u64, so counts up to i32::MAX can't overflow on wasm32
        let per_vertex = 6 + u64::from(flags & COLORS != 0) + u64::from(flags & GROUPS != 0);
        let base = GEOMETRY_HEAD as u64 + f as u64 * 3 + v as u64 * per_vertex;
        if base + TAIL as u64 > n as u64 {
            return None;
        }
        let (creators, paths) = read_tables(words, base as usize, flags)?;
        if words[GEOMETRY_HEAD..GEOMETRY_HEAD + f * 3].iter().any(|&i| i < 0 || i as usize >= v) {
            return None;
        }
        Some(MeshWords { words, v, f, flags, creators, paths })
    }

    /// The words of the geometry, brackets included
    pub fn words(&self) -> &'a [i32] {
        self.words
    }

    /// The decoded geometry, a geometry without groups is one group 0
    pub fn to_mesh(&self) -> Mesh {
        let floats = |section: &[i32]| section.iter().map(|&w| decode_float(w)).collect();
        Mesh {
            faces: self.faces().iter().map(|&i| i as u32).collect(),
            positions: floats(self.positions()),
            normals: floats(self.normals()),
            colors: self.colors().map_or_else(Vec::new, |c| c.iter().map(|&c| c as u32).collect()),
            groups: self.groups().map_or_else(|| vec![0; self.v], |g| g.iter().map(|&g| g as u32).collect()),
            creators: self.creators.to_vec(),
            paths: self.paths.to_vec(),
        }
    }

    /// The creator of every group, empty when the geometry doesn't know them
    pub fn creators(&self) -> &'a [i32] {
        self.creators
    }

    /// The links of the paths the geometry was made from, [`PATH_LINK_WORDS`] words each, empty
    /// when the geometry doesn't know them
    pub fn paths(&self) -> &'a [i32] {
        self.paths
    }

    /// The id of the path the geometry was made from, [`NO_PATH`] for none
    pub fn path_id(&self) -> i32 {
        self.words[5]
    }

    /// Whether the geometry brings a group per vertex, without them it is one group
    pub fn has_groups(&self) -> bool {
        self.flags & GROUPS != 0
    }

    /// The words of the geometry up to the end of its groups
    fn base_len(&self) -> usize {
        sections_len(self.v, self.f, self.flags)
    }

    /// The tables as they follow the groups: the creators and the paths, each with their number
    fn table(&self) -> &'a [i32] {
        &self.words[self.base_len()..self.words.len() - TAIL]
    }

    pub fn vertex_count(&self) -> usize {
        self.v
    }

    pub fn face_count(&self) -> usize {
        self.f
    }

    /// The words of the brackets, the header and the faces, the vertices follow them
    pub fn faces_end(&self) -> usize {
        GEOMETRY_HEAD + self.f * 3
    }

    fn section(&self, start: usize, len: usize) -> &'a [i32] {
        &self.words[GEOMETRY_HEAD + start..GEOMETRY_HEAD + start + len]
    }

    pub fn faces(&self) -> &'a [i32] {
        self.section(0, self.f * 3)
    }

    /// The bits of the f32 positions, x, y and z per vertex
    pub fn positions(&self) -> &'a [i32] {
        self.section(self.f * 3, self.v * 3)
    }

    /// The bits of the f32 normals, x, y and z per vertex
    pub fn normals(&self) -> &'a [i32] {
        self.section(self.f * 3 + self.v * 3, self.v * 3)
    }

    pub fn colors(&self) -> Option<&'a [i32]> {
        (self.flags & COLORS != 0).then(|| self.section(self.f * 3 + self.v * 6, self.v))
    }

    pub fn groups(&self) -> Option<&'a [i32]> {
        self.has_groups().then(|| &self.words[self.base_len() - self.v..self.base_len()])
    }

    /// The words of [`MeshWords::painted`]
    pub fn painted_len(&self) -> usize {
        let colors = if self.v > 0 { COLORS } else { 0 };
        sections_len(self.v, self.f, (self.flags & !COLORS) | colors) + self.table().len() + TAIL
    }

    /// The encoded geometry with every vertex in `color`
    pub fn painted(&self, color: u32) -> Vec<i32> {
        let mut words = vec![0; self.painted_len()];
        self.paint_into(color, &mut words);
        words
    }

    /// Writes [`MeshWords::painted`] into `words`, which are [`MeshWords::painted_len`] long
    pub fn paint_into(&self, color: u32, words: &mut [i32]) {
        let colors = if self.v > 0 { COLORS } else { 0 };
        put_frame(words, self.v, self.f, self.path_id(), (self.flags & !COLORS) | colors);
        let len = words.len();
        let mut out = Words::new(&mut words[GEOMETRY_HEAD..len - TAIL]);
        out.put(self.faces());
        out.put(self.positions());
        out.put(self.normals());
        out.fill(self.v, color as i32);
        out.put(self.groups().unwrap_or(&[]));
        out.put(self.table());
        out.done();
    }

    /// Paints the geometry that `words` are like [`MeshWords::paint_into`], where it is. A
    /// geometry without colors needs a word more per vertex, `words` has them after it.
    pub fn paint_in_place(words: &mut [i32], color: u32) {
        let (v, f, flags) = (words[3] as usize, words[4] as usize, words[6]);
        let painted = (flags & !COLORS) | if v > 0 { COLORS } else { 0 };
        let colors = GEOMETRY_HEAD + f * 3 + v * 6;
        // the groups, the table of the creators and the closing bracket follow the colors
        let tail = colors + if flags & COLORS != 0 { v } else { 0 };
        let groups = if flags & GROUPS != 0 { v } else { 0 };
        let mut table = if flags & CREATORS != 0 { table_len(words[tail + groups] as usize) } else { 0 };
        if flags & PATHS != 0 {
            table += paths_len(words[tail + groups + table] as usize * PATH_LINK_WORDS);
        }
        let rest = groups + table + TAIL;
        assert_eq!(words.len(), sections_len(v, f, painted) + table + TAIL, "no room to paint a geometry in place");
        if flags & COLORS == 0 {
            words.copy_within(tail..tail + rest, colors + v);
        }
        words[colors..colors + v].fill(color as i32);
        words[1] = words.len() as i32 - 3;
        words[6] = painted;
    }

    /// The words of [`MeshWords::join`]
    pub fn join_len(meshes: &[MeshWords]) -> usize {
        let meshes: Vec<&MeshWords> = meshes.iter().filter(|m| m.v > 0).collect();
        let (v, f, colored) =
            meshes.iter().fold((0, 0, false), |(v, f, colored), m| (v + m.v, f + m.f, colored || m.colors().is_some()));
        let creators =
            if meshes.iter().any(|m| !m.creators.is_empty()) { meshes.iter().map(|m| m.group_count()).sum() } else { 0 };
        let paths = meshes.iter().map(|m| m.paths.len()).sum();
        sections_len(v, f, GROUPS | if colored { COLORS } else { 0 }) + table_len(creators) + paths_len(paths) + TAIL
    }

    /// The groups this geometry numbers, one for a geometry without groups
    pub fn group_count(&self) -> usize {
        match self.groups() {
            Some(groups) => group_count(groups.iter().map(|&g| g as u32)),
            None => usize::from(self.v > 0),
        }
    }

    /// The encoded geometries one after the other as one, the groups of each numbered after
    /// those of the geometries before it, a geometry without groups as one group. Geometries
    /// without vertices are left out. When only some of them have colors, the vertices of the
    /// others get [`NO_COLOR`]. The joined geometry was made from no path.
    pub fn join(meshes: &[MeshWords]) -> Vec<i32> {
        let mut words = vec![0; Self::join_len(meshes)];
        Self::join_into(meshes, &mut words);
        words
    }

    /// Writes [`MeshWords::join`] into `words`, which are [`MeshWords::join_len`] long
    pub fn join_into(meshes: &[MeshWords], words: &mut [i32]) {
        let meshes: Vec<&MeshWords> = meshes.iter().filter(|m| m.v > 0).collect();
        let v = meshes.iter().map(|m| m.v).sum::<usize>();
        let f = meshes.iter().map(|m| m.f).sum::<usize>();
        let colored = meshes.iter().any(|m| m.colors().is_some());
        let with_creators = meshes.iter().any(|m| !m.creators.is_empty());
        let paths = meshes.iter().map(|m| m.paths.len()).sum::<usize>();
        let flags = GROUPS
            | if colored { COLORS } else { 0 }
            | if with_creators { CREATORS } else { 0 }
            | if paths > 0 { PATHS } else { 0 };
        put_frame(words, v, f, NO_PATH, flags);
        let len = words.len();
        let mut out = Words::new(&mut words[GEOMETRY_HEAD..len - TAIL]);
        let mut offset = 0u32;
        for mesh in &meshes {
            out.extend(mesh.faces().iter().map(|&i| (i as u32 + offset) as i32));
            offset += mesh.v as u32;
        }
        for mesh in &meshes {
            out.put(mesh.positions());
        }
        for mesh in &meshes {
            out.put(mesh.normals());
        }
        if colored {
            for mesh in &meshes {
                match mesh.colors() {
                    Some(colors) => out.put(colors),
                    None => out.fill(mesh.v, NO_COLOR as i32),
                }
            }
        }
        let mut next = 0u32;
        let mut counts = Vec::with_capacity(meshes.len());
        for mesh in &meshes {
            match mesh.groups() {
                Some(groups) => out.extend(groups.iter().map(|&g| (g as u32 + next) as i32)),
                None => out.fill(mesh.v, next as i32),
            }
            counts.push(mesh.group_count());
            next += counts[counts.len() - 1] as u32;
        }
        if with_creators {
            // the groups of every geometry keep their creators, behind those of the ones before it
            out.put(&[next as i32]);
            for (mesh, &count) in meshes.iter().zip(&counts) {
                out.extend((0..count).map(|g| mesh.creators.get(g).copied().unwrap_or(NO_CREATOR)));
            }
        }
        if paths > 0 {
            out.put(&[(paths / PATH_LINK_WORDS) as i32]);
            for mesh in &meshes {
                out.put(mesh.paths);
            }
        }
        out.done();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{concat_args, split_args};

    fn triangle() -> Mesh {
        Mesh {
            faces: vec![0, 1, 2],
            positions: vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            normals: vec![0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0],
            colors: vec![pack_rgba(1.0, 0.0, 0.0, 1.0); 3],
            groups: vec![0, 2, 2],
            creators: Vec::new(),
            paths: Vec::new(),
        }
    }

    /// [`triangle`] that knows which graph node made each of its three groups
    fn made() -> Mesh {
        Mesh { creators: vec![40, NO_CREATOR, -7], ..triangle() }
    }

    /// A geometry of a plant node: no groups, made from the path 33
    fn plain(colored: bool) -> Vec<i32> {
        let mut words = vec![0, 0, GEOMETRY_TAG, 3, 1, 33, if colored { COLORS } else { 0 }, 0, 1, 2];
        words.extend([0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0f32].map(encode_float));
        words.extend([0.0, 0.0, 1.0f32].repeat(3).into_iter().map(encode_float));
        if colored {
            words.extend([5, 6, 7]);
        }
        words.extend([1, 1]);
        words[1] = words.len() as i32 - 3;
        words
    }

    #[test]
    fn the_creators_follow_the_groups() {
        let words = made().encode();
        assert_eq!(words[6], COLORS | CREATORS | GROUPS);
        assert_eq!(&words[words.len() - 9..], &[0, 2, 2, 3, 40, NO_CREATOR, -7, 1, 1]);
        assert_eq!(Mesh::decode(&words), made());
        assert_eq!(MeshWords::read(&words).unwrap().creators(), &[40, NO_CREATOR, -7]);
        assert_eq!(MeshWords::read(&triangle().encode()).unwrap().creators(), &[] as &[i32]);
        // a table that is shorter or longer than it says
        let mut short = words.clone();
        short.remove(short.len() - 3);
        short[1] -= 1;
        assert_eq!(Mesh::try_decode(&short), None);
        let mut long = words.clone();
        long.insert(long.len() - 2, 0);
        long[1] += 1;
        assert_eq!(Mesh::try_decode(&long), None);
    }

    /// [`made`] with the links of two paths
    fn with_paths() -> Mesh {
        Mesh { paths: vec![5, -1, 0, 40, 0, 1, 6, 5, 7, 40, 1, 2], ..made() }
    }

    #[test]
    fn the_links_of_the_paths_follow_the_creators() {
        let words = with_paths().encode();
        assert_eq!(words[6], COLORS | CREATORS | GROUPS | PATHS);
        assert_eq!(&words[words.len() - 19..words.len() - 2], &[3, 40, NO_CREATOR, -7, 2, 5, -1, 0, 40, 0, 1, 6, 5, 7, 40, 1, 2]);
        assert_eq!(Mesh::decode(&words), with_paths());
        let read = MeshWords::read(&words).unwrap();
        assert_eq!(read.paths(), &with_paths().paths[..]);
        assert_eq!(read.creators(), &[40, NO_CREATOR, -7]);
        // without creators
        let plain = Mesh { creators: Vec::new(), ..with_paths() };
        assert_eq!(Mesh::decode(&plain.encode()), plain);
        // a table that says it has more links than there are words
        let mut short = words.clone();
        short.remove(short.len() - 3);
        short[1] -= 1;
        assert_eq!(Mesh::try_decode(&short), None);
    }

    #[test]
    fn joining_and_painting_keep_the_links_of_the_paths() {
        let (a, b) = (with_paths().encode(), Mesh { paths: vec![9, 5, 0, 41, 0, 1], ..triangle() }.encode());
        let joined = Mesh::decode(&MeshWords::join(&[MeshWords::read(&a).unwrap(), MeshWords::read(&b).unwrap()]));
        assert_eq!(joined.paths, [with_paths().paths, vec![9, 5, 0, 41, 0, 1]].concat());
        let painted = MeshWords::read(&a).unwrap().painted(3);
        assert_eq!(Mesh::decode(&painted).paths, with_paths().paths);
        let mut uncolored = Mesh { colors: Vec::new(), ..with_paths() }.encode();
        uncolored.resize(painted.len(), -1);
        MeshWords::paint_in_place(&mut uncolored, 3);
        assert_eq!(uncolored, painted);
    }

    #[test]
    fn a_mesh_made_by_one_node_names_it_for_every_group() {
        assert_eq!(triangle().made_by(9).creators, vec![9; 3]);
        assert!(Mesh::default().made_by(9).creators.is_empty());
    }

    #[test]
    fn encodes_a_geometry_with_groups_in_brackets() {
        let mesh = triangle();
        let words = mesh.encode();
        assert_eq!(&words[..GEOMETRY_HEAD], &[0, words.len() as i32 - 3, GEOMETRY_TAG, 3, 1, NO_PATH, COLORS | GROUPS]);
        assert_eq!(&words[words.len() - 5..], &[0, 2, 2, 1, 1]);
        assert_eq!(Mesh::decode(&words), mesh);
        // it is one item of a list, like the geometry of a plant node
        let list = concat_args(vec![&words, &plain(false)]);
        assert_eq!(split_args(&list), vec![words.as_slice(), plain(false).as_slice()]);
    }

    #[test]
    fn a_geometry_without_groups_is_one_group() {
        let words = plain(true);
        let read = MeshWords::read(&words).unwrap();
        assert!(!read.has_groups());
        assert_eq!(read.group_count(), 1);
        assert_eq!(read.path_id(), 33);
        let mesh = Mesh::decode(&words);
        assert_eq!(mesh.groups, vec![0; 3]);
        assert_eq!(mesh.colors, vec![5, 6, 7]);
        assert_eq!(mesh.positions, triangle().positions);
    }

    #[test]
    fn appends_a_mesh_with_its_groups_after_the_others() {
        let mut mesh = Mesh { colors: Vec::new(), ..triangle() };
        mesh.append(&triangle(), 3);
        assert_eq!(mesh.vertex_count(), 6);
        assert_eq!(mesh.faces, vec![0, 1, 2, 3, 4, 5]);
        assert_eq!(mesh.groups, vec![0, 2, 2, 3, 5, 5]);
        // the vertices without colors keep the material
        let red = pack_rgba(1.0, 0.0, 0.0, 1.0);
        assert_eq!(mesh.colors, [[NO_COLOR; 3], [red; 3]].concat());
        let mut plain = Mesh { colors: Vec::new(), ..triangle() };
        plain.append(&Mesh { colors: Vec::new(), ..triangle() }, 0);
        assert!(plain.colors.is_empty());
        let mut empty = Mesh::default();
        empty.append(&triangle(), 0);
        assert_eq!(empty, triangle());
    }

    #[test]
    fn colors_are_optional() {
        let mesh = Mesh { colors: Vec::new(), ..triangle() };
        let words = mesh.encode();
        assert_eq!(&words[2..GEOMETRY_HEAD], &[GEOMETRY_TAG, 3, 1, NO_PATH, GROUPS]);
        assert_eq!(words.len(), GEOMETRY_HEAD + 3 + 3 * 7 + 2);
        assert_eq!(Mesh::decode(&words), mesh);
    }

    #[test]
    fn packs_red_into_the_lowest_byte() {
        assert_eq!(pack_rgba(1.0, 0.0, 0.0, 0.0), 0xff);
        assert_eq!(pack_rgba(0.0, 0.0, 0.0, 1.0), 0xff00_0000);
    }

    #[test]
    fn try_decode_takes_only_whole_geometries() {
        let words = triangle().encode();
        assert_eq!(Mesh::try_decode(&words), Some(triangle()));
        assert_eq!(Mesh::try_decode(&Mesh::default().encode()), Some(Mesh::default()));
        assert_eq!(Mesh::try_decode(&[]), None);
        assert_eq!(Mesh::try_decode(&[0, 1, 1, 1]), None);
        assert_eq!(Mesh::try_decode(&words[..words.len() - 1]), None);
        let mut longer = words.clone();
        longer.insert(GEOMETRY_HEAD, 0);
        assert_eq!(Mesh::try_decode(&longer), None);
        longer[1] += 1;
        assert_eq!(Mesh::try_decode(&longer), None);
        assert_eq!(Mesh::try_decode(&[0, 6, GEOMETRY_TAG, -1, 0, NO_PATH, 0, 1, 1]), None);
        assert_eq!(Mesh::try_decode(&[0, 6, GEOMETRY_TAG, i32::MAX, i32::MAX, NO_PATH, 0, 1, 1]), None);
        let mut unknown_flag = words.clone();
        unknown_flag[6] |= 16;
        assert_eq!(Mesh::try_decode(&unknown_flag), None);
        // creators without groups
        let mut no_groups = plain(false);
        let end = no_groups.len() - 2;
        no_groups.splice(end..end, [1, 5]);
        no_groups[1] += 2;
        no_groups[6] = CREATORS;
        assert_eq!(Mesh::try_decode(&no_groups), None);
        let mut missing_vertex = words.clone();
        missing_vertex[GEOMETRY_HEAD + 2] = 3;
        assert_eq!(Mesh::try_decode(&missing_vertex), None);
    }

    fn other(colored: bool) -> Mesh {
        Mesh {
            faces: vec![0, 1, 2, 2, 1, 3],
            positions: vec![0.5, 0.0, 0.0, 1.5, 0.0, 0.0, 0.5, 1.0, 0.0, 1.5, 1.0, -0.0],
            normals: vec![0.0, 0.0, -1.0, 0.0, f32::NAN, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0],
            colors: if colored { vec![1, 2, 3, NO_COLOR] } else { Vec::new() },
            groups: vec![1, 0, 4, 1],
            creators: Vec::new(),
            paths: Vec::new(),
        }
    }

    #[test]
    fn joins_the_words_like_appending_the_meshes() {
        let plain = Mesh { colors: Vec::new(), ..triangle() };
        let cases = [
            vec![],
            vec![Mesh::default()],
            vec![triangle()],
            vec![plain.clone(), other(false)],
            vec![other(true), Mesh::default(), plain.clone(), triangle()],
            vec![plain.clone(), other(false), other(true)],
            // only some know their creators, one of them fewer than it has groups
            vec![made(), other(false), Mesh { creators: vec![5, 6], ..other(true) }],
            vec![plain, made(), Mesh::default().made_by(3)],
        ];
        for meshes in cases {
            let mut appended = Mesh::default();
            let mut next = 0;
            for mesh in meshes.iter().filter(|m| m.vertex_count() > 0) {
                appended.append(mesh, next);
                next += mesh.groups.iter().max().map_or(0, |g| g + 1);
            }
            let encoded: Vec<Vec<i32>> = meshes.iter().map(Mesh::encode).collect();
            let words: Vec<MeshWords> = encoded.iter().map(|w| MeshWords::read(w).unwrap()).collect();
            assert_eq!(MeshWords::join(&words), appended.encode());
        }
    }

    #[test]
    fn a_geometry_without_groups_joins_as_one_group() {
        let (a, b) = (plain(true), made().encode());
        let words = [MeshWords::read(&a).unwrap(), MeshWords::read(&b).unwrap()];
        let joined = Mesh::decode(&MeshWords::join(&words));
        assert_eq!(joined.groups, vec![0, 0, 0, 1, 3, 3]);
        assert_eq!(joined.creators, vec![NO_CREATOR, 40, NO_CREATOR, -7]);
        assert_eq!(&joined.colors[..3], &[5, 6, 7]);
    }

    #[test]
    fn an_input_brings_a_geometry_or_the_geometries_of_a_list_joined() {
        let (a, b) = (made().encode(), plain(false));
        assert_eq!(geometry_input(&a).as_deref(), Some(a.as_slice()));
        assert_eq!(geometry_input(&concat_args(vec![&a])).as_deref(), Some(a.as_slice()));
        let joined = MeshWords::join(&[MeshWords::read(&a).unwrap(), MeshWords::read(&b).unwrap()]);
        assert_eq!(geometry_input(&concat_args(vec![&a, &b])).as_deref(), Some(joined.as_slice()));
        assert_eq!(geometry_input(&[0, 1, 1, 1]), None);
        assert_eq!(geometry_input(&[]), None);
    }

    #[test]
    fn paints_the_words_like_the_mesh() {
        let color = pack_rgba(0.2, 0.4, 0.6, 1.0);
        for mesh in [Mesh::default(), triangle(), other(false), other(true), made(), other(false).made_by(4)] {
            let words = mesh.encode();
            let painted = Mesh { colors: vec![color; mesh.vertex_count()], ..mesh };
            assert_eq!(MeshWords::read(&words).unwrap().painted(color), painted.encode());
        }
        // a geometry without groups keeps its path and gets none
        let painted = MeshWords::read(&plain(false)).unwrap().painted(9);
        let mut expected = plain(true);
        expected[GEOMETRY_HEAD + 3 + 18..GEOMETRY_HEAD + 3 + 21].fill(9);
        assert_eq!(painted, expected);
    }

    #[test]
    fn paints_the_words_in_place_like_the_mesh() {
        let color = pack_rgba(0.2, 0.4, 0.6, 1.0);
        let colored = Mesh { colors: vec![7; 3], ..triangle() };
        let meshes = [Mesh::default(), triangle(), other(false), other(true), colored, made(), other(false).made_by(4)];
        for mut words in meshes.iter().map(Mesh::encode).chain([plain(false), plain(true)]) {
            let painted = MeshWords::read(&words).unwrap().painted(color);
            // the room for the colors holds what was there before
            words.resize(painted.len(), -1);
            MeshWords::paint_in_place(&mut words, color);
            assert_eq!(words, painted);
        }
    }

    /// What a builder adds: two triangles, and two vertices it takes back
    fn build(mesh: &mut impl MeshBuild) {
        for (i, x) in [0.0, 1.0, 0.0, 1.0].into_iter().enumerate() {
            mesh.push_vertex(Vec3::new(x, (i / 2) as f32, -0.0), i as u32 % 2);
        }
        mesh.push_vertex(Vec3::splat(9.0), 7);
        mesh.push_vertex(Vec3::splat(8.0), 7);
        mesh.truncate_vertices(4);
        mesh.push_face([0, 1, 2]);
        mesh.push_face([2, 1, 3]);
    }

    #[test]
    fn writes_the_words_of_the_mesh_it_builds() {
        let mut mesh = Mesh::default();
        build(&mut mesh);
        let encoded = mesh.encode();
        // as many as it adds, or room for more
        for (vertices, faces) in [(6, 2), (6, 5), (9, 2), (11, 3)] {
            let mut words = vec![0; MeshWriter::len(vertices, faces) + 2];
            let mut writer = MeshWriter::new(&mut words, vertices, faces);
            build(&mut writer);
            assert_eq!(writer.vertex_count(), 4);
            let len = writer.finish();
            assert_eq!(&words[..len], &encoded[..], "room for {vertices} vertices and {faces} faces");
        }
    }

    #[test]
    fn writes_the_creators_after_the_mesh_it_builds() {
        let mut mesh = Mesh::default();
        build(&mut mesh);
        let encoded = Mesh { creators: vec![11, 12], ..mesh }.encode();
        for (vertices, faces) in [(6, 2), (11, 3)] {
            let mut words = vec![0; MeshWriter::len_with(vertices, faces, 2)];
            let mut writer = MeshWriter::new(&mut words, vertices, faces);
            build(&mut writer);
            let len = writer.finish_with(&[11, 12]);
            assert_eq!(&words[..len], &encoded[..], "room for {vertices} vertices and {faces} faces");
        }
        // nothing built, nothing to name a creator for
        let mut words = vec![0; MeshWriter::len_with(3, 1, 2)];
        let len = MeshWriter::new(&mut words, 3, 1).finish_with(&[11, 12]);
        assert_eq!(&words[..len], &Mesh::default().encode()[..]);
    }

    #[test]
    #[should_panic(expected = "more than the 1 faces counted")]
    fn a_writer_takes_no_more_than_it_counted() {
        let mut words = vec![0; MeshWriter::len(6, 1)];
        build(&mut MeshWriter::new(&mut words, 6, 1));
    }

    #[test]
    #[should_panic(expected = "expected a geometry")]
    fn rejects_truncated_geometry() {
        let words = triangle().encode();
        Mesh::decode(&words[..words.len() - 3]);
    }

    #[test]
    #[should_panic(expected = "group")]
    fn every_vertex_needs_a_group() {
        Mesh { groups: Vec::new(), ..triangle() }.encode();
    }
}
