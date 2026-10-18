//! Geodata (`.l2j`) parsing and the block-level height / NSWE queries.
//!
//! Java: `gameserver/geoengine/geodata/**`. M1 covers the file parser, the in-memory region model
//! and the queries `getNearestZ`, `getNextLowerZ`, `getNextHigherZ`, `checkNearestNswe`. World to
//! geo coordinate conversion, line of sight, movement, pathfinding, the per-region override (D-26)
//! and runtime NSWE edits belong to M3.
//!
//! All coordinates in this crate are **global geo cell coordinates** (`regionX * 2048 + local`),
//! exactly what `GeoEngine` passes to `Region`. Z values are world Z.

pub mod block;
pub mod cell;
pub mod error;
pub mod region;

pub use block::{BLOCK_CELLS, BlockRef, BlockType};
pub use error::GeoError;
pub use region::{BlockCounts, GEO_REGIONS_SIDE, GeoSet, NullRegion, REGION_BLOCKS, Region};
