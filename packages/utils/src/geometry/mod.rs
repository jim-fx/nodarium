//! Paths, geometry and instances: the data the Plantarium and Myzel nodes pass between them.
//!
//! # Geometry `[0, n, 1, v, f, path id, flags, ...faces, ...positions, ...normals, ...colors?, ...groups?, k?, ...creators?, m?, ...paths?, 1, 1]`
//!
//! The one surface type, in its brackets like every item of a list. `faces` are `3f` vertex
//! indices (counter-clockwise seen from outside), `positions` and `normals` are `3v` f32. The
//! `path id` is the path the geometry was made from, [`NO_PATH`] for none. The `flags`
//! ([`geometry_flags`]) say which of the optional sections follow:
//!
//! - `COLORS`: one sRGB RGBA8 word per vertex, red in the lowest byte. Alpha is how much of the
//!   color shows over the viewer's material: 255 the color alone, 0 ([`crate::NO_COLOR`]) the
//!   material, as for a vertex without a color.
//! - `GROUPS`: one material group per vertex, a small integer. Other programs can give each
//!   group its own material. A geometry without them is one group, the one of its path.
//! - `CREATORS` (only with `GROUPS`): the number `k` of groups and, for every group, the creator
//!   id of the graph node that made it ([`NO_CREATOR`] for none), so the viewer can lead from a
//!   part of the geometry to its node.
//! - `PATHS`: the number `m` of paths the geometry was made from and the link of each of them,
//!   [`PATH_LINK_WORDS`] words: id, parent id, parent alpha, creator, index and grown, as in the
//!   header of the path. `to_geometry` writes them for the paths it turns into tubes, so the
//!   viewer can still lead from an instance on a path to the path and its node.
//!
//! [`MeshWords`] reads a geometry in place, [`Mesh`] is one decoded, [`MeshWriter`] writes one
//! straight into its words. `join` joins the geometries with groups of its inputs into one, the
//! groups of every geometry numbered after those of the ones before it, see [`MeshWords::join`].

mod bend;
mod calculate_normals;
mod extrude_path;
mod fill_gaps;
mod follow;
mod geometry_data;
mod instance_data;
mod math;
mod mesh;
mod path_data;
mod transform;

pub use bend::*;
pub use calculate_normals::*;
pub use extrude_path::*;
pub use fill_gaps::*;
pub use follow::*;
pub use geometry_data::*;
pub use instance_data::*;
pub use math::*;
pub use mesh::*;
pub use path_data::*;
pub use transform::*;
