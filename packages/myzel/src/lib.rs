//! Shared core of the Myzel nodes: meshes, paths and the shapes of caps and gills.
//!
//! # Meshes
//!
//! A Myzel node makes its surface as a geometry with groups and the creators of its groups, see
//! `nodarium_utils::geometry`: [`Mesh`], [`MeshWords`] and [`MeshWriter`] live there and are
//! re-exported here. For meshes made from paths a group is the graph node that made the path,
//! numbered by [`group_numbers`], 0 for meshes made from a pattern. Thin closed sheets like gill
//! plates and veils, which stay sharp at any mesh resolution, are built with [`add_sheet`]. The
//! gills of `lamellae` and `ridges` come from [`gills`], the tubes of `pores` from [`pores`], the
//! veil of `membrane` and the skins of `skirt` from [`veil`].
//!
//! # Paths
//!
//! Myzel meshes Plantarium paths, see `nodarium_utils::geometry`. What a path carries at its
//! points shapes the surface around it: its colors (Plantarium's `set_color`) color it, its
//! fibres (`set_fibres`) and its fill (`set_fill`) take the place of those of `hyphae_to_mesh`,
//! see `point_values` in `nodarium_utils::geometry` and [`paths_mesh`].
//!
//! Inputs of Myzel nodes are plain values, see `read_float` and friends in `nodarium_utils`, a
//! color input is read once, without a path and with alpha 0, see `read_color`.

mod cap;
mod cap_shape;
mod density;
mod gills;
mod hyphae_mesh;
mod inputs;
mod noise;
mod paths;
mod pores;
mod sheet;
mod spatial;
pub mod veil;

pub use cap::{
    closing_turn, curl_spread, curl_turn, margin_points, pose_caps, pose_caps_of, testing, AlongRay, Cap, CapFrame, CapRay, Step,
};
pub use cap_shape::{cap_alphas, grow_cap, ray_angles, CapShape};
pub use density::{mesh_segments, surface, surface_and_inside_in, surface_in, Frame, Inside, Meshing, Segment, SegmentSurface, TissueField};
pub use gills::{gills, write_gills, GillInputs, GillProfile, Gills};
pub use hyphae_mesh::{paths_mesh, HyphaeMeshParams};
pub use pores::{pores, write_pores, PoreInputs};
pub use paths::{
    assign_bodies, group_creators, group_numbers, path_segments, read_paths, with_values_along, MeshPath, PathGroup, PathStyle,
};
pub use inputs::read_mesh;
pub use nodarium_utils::{encode_color, linear_rgba, mix_colors, opaque, pack_linear, read_color, read_float, read_floats, read_int, NO_COLOR};
pub use nodarium_utils::geometry::{geometry_flags, geometry_input, pack_rgba, Mesh, MeshBuild, MeshWords, MeshWriter, NO_CREATOR};
pub use noise::value_noise;
pub use sheet::{add_sheet, sheet_checks, sheet_size, smooth_normals, smooth_normals_in_words, write_mesh, SheetGrid};
pub use spatial::{distance_sq_to_segment, SpatialHash};
