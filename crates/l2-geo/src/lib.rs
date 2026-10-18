//! Geodata (`.l2j`) parsing, the region model and the `GeoEngine` queries.
//!
//! Java: `gameserver/geoengine/**`. M1: the file parser, the in-memory region model and the block
//! queries `getNearestZ`, `getNextLowerZ`, `getNextHigherZ`, `checkNearestNswe` ([`region`],
//! [`block`]). M3-GEO-01: [`GeoEngine`] (whole-world loading with the per-region override of D-26,
//! world <-> geo coordinates, `getHeight`, line of sight, movement checks) and the
//! [`DynamicObstacles`] hook for doors and fences. Pathfinding (M3-GEO-02) and runtime NSWE edits
//! for doors are not here yet.
//!
//! All coordinates in the region / block types are **global geo cell coordinates**
//! (`regionX * 2048 + local`), exactly what `GeoEngine` passes to `Region`. Z values are world Z.
//! [`GeoEngine`] also takes world coordinates (see [`coords`]).

pub mod block;
pub mod cell;
pub mod coords;
pub mod engine;
pub mod error;
pub mod line;
pub mod obstacles;
pub mod region;

pub use block::{BLOCK_CELLS, BlockRef, BlockType};
pub use coords::Location;
pub use engine::{GeoEngine, GeoEngineOptions, GeoLoadSpec, GeoOverride, LoadReport};
pub use error::GeoError;
pub use line::{GridLineIterator2D, GridLineIterator3D};
pub use obstacles::{DynamicObstacles, NoObstacles};
pub use region::{BlockCounts, GEO_REGIONS_SIDE, GeoSet, NullRegion, REGION_BLOCKS, Region};
