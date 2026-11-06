//! Shared core of the Myzel nodes: a field is a small stack program describing a
//! shape as a signed distance function, it only becomes a mesh at the very end.
//!
//! # Field `[10, n, min_x, min_y, min_z, max_x, max_y, max_z, ...ops]`
//!
//! `n` is the number of op words after the 8 header words, the bounding box is
//! stored as f32 and contains the whole surface. Each op is an integer opcode
//! followed by its f32 operands (see [`opcode`]), only the seed of `WARP` is a
//! raw integer word. Primitives push a distance, combining ops pop two and push
//! one. Transform ops push a transformed point
//! that the following ops are evaluated at, until the matching `END` pops it
//! again, so a transform only applies to the field it wraps:
//!
//! ```text
//! TRANSLATE 0 1 0  SPHERE 0.5  END  CAPSULE 0 0 0 0 1 0 0.1  SMOOTH_UNION 0.2
//! ```
//!
//! # Mesh `[14, v, f, flags, ...faces, ...positions, ...normals, ...colors, ...groups]`
//!
//! `faces` are `3f` vertex indices (counter-clockwise seen from outside),
//! `positions` and `normals` are `3v` f32. `colors` is only there when `flags`
//! has [`mesh_flags::COLORS`] set and then holds one RGBA8 word per vertex, red
//! in the lowest byte. Meshes without colors are shown with the viewer's
//! material. `groups` holds one integer per vertex that tells which material
//! group it belongs to: the [`Tissue`] for meshes made from hyphae, the
//! graph node that made the path for meshes made from paths, 0 for meshes
//! made from a field.
//!
//! # Hyphae `[11, n, ...segments]`
//!
//! Grown hyphae, 8 words per segment, parents before children, see [`Hyphae`].
//!
//! Inputs of Myzel nodes are plain values, see [`read_float`] and friends.

mod density;
mod eval;
mod expand;
mod field;
mod gills;
mod growth;
mod hyphae;
mod hyphae_mesh;
mod inputs;
mod mesh;
mod noise;
mod polygonize;
mod spatial;

pub use density::{mesh_segments, surface, Segment, SegmentSurface};
pub use eval::{eval, Program};
pub use expand::{expand, ExpandParams};
pub use field::{opcode, Bbox, Field, FIELD_TAG};
pub use gills::{grow_gills, GillParams, MAX_GILLS, MAX_LAMELLULAE};
pub use growth::{grow, GrowthParams, Schedule, MAX_ITERATIONS, MAX_SEGMENTS};
pub use hyphae::{flags, HyphaSegment, Hyphae, Tissue, HYPHAE_TAG};
pub use hyphae_mesh::{hyphae_mesh, path_segments, paths_mesh, HyphaeMeshParams, MeshPath};
pub use inputs::{read_field, read_float, read_floats, read_hyphae, read_int, read_mesh, read_vec3};
pub use mesh::{mesh_flags, pack_rgba, Mesh, MESH_TAG};
pub use noise::value_noise;
pub use polygonize::polygonize;
pub use spatial::{distance_sq_to_segment, SpatialHash};
