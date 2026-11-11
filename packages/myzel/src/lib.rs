//! Shared core of the Myzel nodes: meshes, paths, colors and the settings that shape them.
//!
//! # Mesh `[14, v, f, flags, ...faces, ...positions, ...normals, ...colors, ...groups]`
//!
//! `faces` are `3f` vertex indices (counter-clockwise seen from outside),
//! `positions` and `normals` are `3v` f32. `colors` is only there when `flags`
//! has [`mesh_flags::COLORS`] set and then holds one sRGB RGBA8 word per vertex, red
//! in the lowest byte. Meshes without colors are shown with the viewer's
//! material, and so are vertices whose alpha is 0 ([`NO_COLOR`]); in between
//! the alpha is how much of the vertex color shows over the material. `groups` holds one small integer per vertex that tells which
//! material group it belongs to: for meshes made from paths the graph node that
//! made the path, numbered by [`group_numbers`], 0 for meshes made from a pattern.
//! Thin closed sheets like gill plates and veils, which stay
//! sharp at any mesh resolution, are built with [`add_sheet`]. The gills of `lamellae` and
//! `ridges` come from [`gills`].
//!
//! # Paths
//!
//! Myzel meshes Plantarium paths, see `nodarium_utils::geometry`.
//!
//! # Lines `[12, n, ...n × (x0, y0, z0, x1, y1, z1, group)]`
//!
//! Paths as line segments for the viewer, in the groups of their mesh, see [`path_lines`].
//!
//! # Settings `[13, setting, value, ...creators]`
//!
//! A value of `hyphae_to_mesh` for every path of the graph nodes `creators`: `setting` 1 is the
//! fibres, 2 the fill, 3 the color, `value` is an f32 (the color as
//! `r | g << 8 | b << 16`, see [`color_value`]). The `set_fibres`, `set_fill` and
//! `set_color` nodes put one into a path list next to the paths that pass through them, see
//! [`with_setting`], and the mesh nodes give it to the paths, see [`apply_settings`]. A later
//! setting replaces an earlier one, paths without one keep the value of the mesh node.
//!
//! # Colors `[0, 4, r, g, b, 1, 1]`
//!
//! A color input holds the sRGB bytes, the way the host sends a color set on a node. Color nodes
//! like `mix_color` send theirs the same way, see [`read_color`].
//!
//! Inputs of Myzel nodes are plain values, see [`read_float`] and friends.

mod cap;
mod color;
mod cap_shape;
mod density;
mod gills;
mod hyphae_mesh;
mod inputs;
mod mesh;
mod noise;
mod paths;
mod settings;
mod sheet;
mod spatial;

pub use cap::{closing_turn, curl_turn, margin_points, pose_caps, pose_caps_of, testing, Cap, CapFrame, CapRay, Step};
pub use cap_shape::{cap_alphas, grow_cap, ray_angles, CapShape};
pub use density::{mesh_segments, surface, surface_in, Frame, Meshing, Segment, SegmentSurface, TissueField};
pub use color::{
    encode_color, linear_rgba, mix_colors, opaque, pack_linear, paint_mesh, read_color, NO_COLOR,
};
pub use gills::{gills, GillInputs, GillProfile, Gills};
pub use hyphae_mesh::{paths_mesh, HyphaeMeshParams};
pub use paths::{
    group_numbers, path_lines, path_segments, read_paths, MeshPath, PathGroup,
    PathStyle, LINES_TAG,
};
pub use inputs::{read_float, read_floats, read_int, read_mesh};
pub use mesh::{mesh_flags, pack_rgba, Mesh, MESH_TAG};
pub use noise::value_noise;
pub use settings::{
    apply_settings, color_value, read_settings, with_setting, PathSettings, Setting, SettingItem, SETTING_TAG,
};
pub use sheet::{add_sheet, sheet_checks, smooth_normals, SheetGrid};
pub use spatial::{distance_sq_to_segment, SpatialHash};
