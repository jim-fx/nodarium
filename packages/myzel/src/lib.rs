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
//! material. `groups` holds one small integer per vertex that tells which
//! material group it belongs to: for meshes made from paths the graph node that
//! made the path, numbered by [`group_numbers`], 0 for meshes made from a field.
//! Meshes of grown hyphae have [`mesh_flags::TISSUE_GROUPS`] set, their groups
//! are the [`Tissue`]s.
//!
//! # Paths
//!
//! Myzel meshes Plantarium paths, see `nodarium_utils::geometry`. Grown hyphae
//! become paths before meshing, see [`Hyphae::to_paths`].
//!
//! # Hyphae `[11, n, ...segments]`
//!
//! Grown hyphae, 8 words per segment, parents before children, see [`Hyphae`].
//!
//! # Lines `[12, n, ...n × (x0, y0, z0, x1, y1, z1, group)]`
//!
//! Paths as line segments for the viewer, in the groups of their mesh, see [`path_lines`].
//!
//! # Settings `[13, setting, value, ...creators]`
//!
//! A value of `hyphae_to_mesh` for every path of the graph nodes `creators`: `setting` 0 is the
//! blend, 1 the fibres, 2 the fill, `value` is an f32. The `set_blend`, `set_fibres` and
//! `set_fill` nodes put one into a path list next to the paths that pass through them, see
//! [`with_setting`], and the mesh nodes give it to the paths, see [`apply_settings`]. A later
//! setting replaces an earlier one, paths without one keep the value of the mesh node.
//!
//! Inputs of Myzel nodes are plain values, see [`read_float`] and friends.

mod cap;
mod cap_shape;
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
mod paths;
mod polygonize;
mod settings;
mod spatial;

pub use cap::{closing_turn, curl_turn, margin_points, pose_caps, pose_caps_of, testing, Cap, CapFrame, CapRay, Step};
pub use cap_shape::{cap_alphas, grow_cap, CapShape};
pub use density::{mesh_segments, surface, surface_in, Frame, Meshing, Segment, SegmentSurface, TissueField};
pub use eval::{eval, Program};
pub use expand::{expand, ExpandParams};
pub use field::{opcode, Bbox, Field, FIELD_TAG};
pub use gills::{grow_gills, GillParams, MAX_GILLS, MAX_LAMELLULAE};
pub use growth::{grow, GrowthParams, Schedule, MAX_ITERATIONS, MAX_SEGMENTS};
pub use hyphae::{flags, HyphaSegment, Hyphae, Tissue, HYPHAE_TAG};
pub use hyphae_mesh::{hyphae_mesh, paths_mesh, HyphaeMeshParams};
pub use paths::{
    group_numbers, path_lines, path_segments, read_mesh_paths, read_paths, retain_points, MeshPath, PathGroup,
    PathStyle, LINES_TAG, TISSUE_COUNT,
};
pub use inputs::{read_field, read_float, read_floats, read_hyphae, read_int, read_mesh, read_vec3};
pub use mesh::{mesh_flags, pack_rgba, Mesh, MESH_TAG};
pub use noise::value_noise;
pub use polygonize::polygonize;
pub use settings::{apply_settings, read_settings, with_setting, PathSettings, Setting, SettingItem, SETTING_TAG};
pub use spatial::{distance_sq_to_segment, SpatialHash};
