//! Pathfinding on the geodata cells (`PathFinding = 2`) and the worker pool that runs it.
//!
//! Java: `gameserver/geoengine/pathfinding/{PathFinding,NodeBuffer,GeoLocation}.java` and
//! `config/GeoEngineConfig.java` (buffers, weights).
//!
//! * [`PathFinder`] is `PathFinding.findPath` for one thread. Java keeps the `NodeBuffer`s in a
//!   `ThreadLocal`; here every [`PathFinder`] owns its buffers and takes `&mut self`, so the
//!   world loop or a worker simply owns one. The engine is passed in (`&GeoEngine`, immutable,
//!   shared), together with the door / fence view of the moment (D-14).
//! * [`PathPool`] runs requests on plain threads and hands the results to a [`PathSink`].
//!
//! # What is not ported
//! `PathFinding.getNodePos` / `getNodeBlock` / `getRegionX` / `getRegionY` / `getRegionOffset` /
//! `calculateWorldX` / `calculateWorldY`: helpers of the old path node files (`PathFinding = 1`),
//! unused by the cell based search, and nothing here reads path node files.
//!
//! The `PathFinding > 0` check lives in the callers in Java (`Creature`, `SummonAI`, `Summon`),
//! not in `findPath`, and is the caller's job here too (see [`crate::GeoEngineOptions::pathfinding`]).

mod node_buffer;
mod pool;

use thiserror::Error;

use crate::cell::{NSWE_ALL, NSWE_EAST, NSWE_NORTH, NSWE_SOUTH, NSWE_WEST};
use crate::coords::{Location, geo_x, geo_y, world_x, world_y};
use crate::engine::GeoEngine;
use crate::obstacles::DynamicObstacles;
use node_buffer::{NodeBuffer, NodeWeights};
pub use pool::{PathPool, PathPoolConfig, PathPoolError, PathRequest, PathResult, PathSink, PoolStats};

/// Java `GeoLocation`: a path node. Created from a geo cell and a Z, it remembers the open
/// directions of the nearest layer and that layer's height.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GeoLocation {
    x: i32,
    y: i32,
    nswe: u8,
    geo_height: i32,
}

impl GeoLocation {
    /// `new GeoLocation(x, y, z)` / `set(x, y, z)`: `x`, `y` are geo cell coordinates, `z` a world
    /// Z used to pick the layer.
    pub fn new(engine: &GeoEngine, x: i32, y: i32, z: i32) -> GeoLocation {
        let mut nswe = 0;
        // Plain `checkNearestNswe` in the order of the Java code.
        for direction in [NSWE_NORTH, NSWE_EAST, NSWE_SOUTH, NSWE_WEST] {
            if engine.check_nearest_nswe(x, y, z, direction) {
                nswe |= direction;
            }
        }
        GeoLocation {
            x,
            y,
            nswe,
            geo_height: engine.nearest_z(x, y, z),
        }
    }

    pub fn can_go_north(&self) -> bool {
        self.nswe & NSWE_NORTH != 0
    }

    pub fn can_go_east(&self) -> bool {
        self.nswe & NSWE_EAST != 0
    }

    pub fn can_go_south(&self) -> bool {
        self.nswe & NSWE_SOUTH != 0
    }

    pub fn can_go_west(&self) -> bool {
        self.nswe & NSWE_WEST != 0
    }

    pub fn can_go_none(&self) -> bool {
        self.nswe == 0
    }

    pub fn can_go_all(&self) -> bool {
        self.nswe == NSWE_ALL
    }

    /// The packed open directions (`Cell.NSWE_*` bits).
    pub fn nswe(&self) -> u8 {
        self.nswe
    }

    /// `getX()`: world X of the middle of the node's cell.
    pub fn x(&self) -> i32 {
        world_x(self.x)
    }

    /// `getY()`: world Y of the middle of the node's cell.
    pub fn y(&self) -> i32 {
        world_y(self.y)
    }

    /// `getZ()`: the layer height (`_geoHeight`).
    pub fn z(&self) -> i32 {
        self.geo_height
    }

    /// `getNodeX()`: the geo cell X.
    pub fn node_x(&self) -> i32 {
        self.x
    }

    /// `getNodeY()`: the geo cell Y.
    pub fn node_y(&self) -> i32 {
        self.y
    }

    /// The node as a world position (middle of the cell, layer height).
    pub fn location(&self) -> Location {
        Location::new(self.x(), self.y(), self.z())
    }
}

/// Why [`PathFinder::find_path_detailed`] found no path. Java returns `null` for all of them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathFailure {
    /// No geodata under the start or the target (`hasGeo`).
    NoGeo,
    /// `64 + 2 * max(|dx|, |dy|)` cells exceed the largest configured buffer (`alloc` gives null).
    BufferTooSmall { needed: usize, largest: usize },
    /// The open list ran empty: the target cannot be reached inside the buffer area.
    NoPath,
    /// 7000 nodes were expanded without reaching the target.
    IterationLimit,
}

/// Errors of the pathfinding setup.
#[derive(Debug, Error)]
pub enum PathError {
    /// `PathFindBuffers` has an entry that is not `<size>x<count>` with a positive integer size
    /// (Java: `Invalid buffer definition` / `NumberFormatException`, `PathFinding` aborts the load).
    #[error("invalid path buffer definition {0:?}")]
    InvalidBuffers(String),
}

/// The `GeoEngineConfig` values the path search reads.
#[derive(Clone, Debug, PartialEq)]
pub struct PathOptions {
    /// Map sizes (cells per side) of the buffers, ascending. From `PathFindBuffers`.
    ///
    /// NOTE(java-quirk): Java parses only the part before the `x` of `100x6`. The count (`6`) is
    /// never used: buffers are allocated lazily, one per size class per thread.
    pub buffer_sizes: Vec<usize>,
    /// `LowWeight`: cost of a step on open, flat ground.
    pub low_weight: f32,
    /// `MediumWeight`: cost of a step next to an obstacle.
    pub medium_weight: f32,
    /// `HighWeight`: cost of a step onto a blocked cell or up / down a cliff.
    pub high_weight: f32,
    /// `DiagonalWeight`: cost of a diagonal step (only with `AdvancedDiagonalStrategy`).
    pub diagonal_weight: f32,
    /// `AdvancedDiagonalStrategy`.
    pub advanced_diagonal_strategy: bool,
    /// `MaxPostfilterPasses`: line of sight passes over the found path, 0 disables them.
    pub max_postfilter_passes: i32,
}

impl PathOptions {
    /// `PathFindBuffers` as in the `PathFinding` constructor: entries split at `;`, size = the
    /// integer before the `x`, sorted ascending. An empty list is an error (Java would
    /// have no size class and never find a path); so is a size of 0 or less.
    pub fn parse_buffers(text: &str) -> Result<Vec<usize>, PathError> {
        // Java `String.split(";")` drops trailing empty strings ("100x6;" is fine), keeps empty
        // ones in the middle and gives one empty entry for the empty string.
        let mut entries: Vec<&str> = text.split(';').collect();
        if !text.is_empty() {
            while entries.last() == Some(&"") {
                entries.pop();
            }
        }
        if entries.is_empty() {
            return Err(PathError::InvalidBuffers(text.to_string()));
        }
        let mut sizes = Vec::new();
        for entry in entries {
            let size = entry
                .split('x')
                .next()
                .and_then(|s| s.parse::<usize>().ok())
                .filter(|s| *s > 0)
                .ok_or_else(|| PathError::InvalidBuffers(entry.to_string()))?;
            sizes.push(size);
        }
        sizes.sort_unstable();
        Ok(sizes)
    }

    /// The defaults of `GeoEngineConfig.load()` when a key is missing.
    fn java_defaults() -> PathOptions {
        PathOptions {
            buffer_sizes: vec![100, 128, 192, 256, 320, 384, 500],
            low_weight: 0.5,
            medium_weight: 2.0,
            high_weight: 3.0,
            diagonal_weight: 0.707,
            advanced_diagonal_strategy: true,
            max_postfilter_passes: 3,
        }
    }

    /// The values of the shipped `dist/game/config/GeoEngine.ini`. Differs from [`Default`] in
    /// `AdvancedDiagonalStrategy = False`.
    pub fn shipped() -> PathOptions {
        PathOptions {
            advanced_diagonal_strategy: false,
            ..PathOptions::java_defaults()
        }
    }
}

impl Default for PathOptions {
    /// The defaults in the Java code when a key is missing (not the shipped ini, see
    /// [`PathOptions::shipped`]).
    fn default() -> Self {
        PathOptions::java_defaults()
    }
}

impl TryFrom<&l2_config::game::GeoEngineConfig> for PathOptions {
    type Error = PathError;

    fn try_from(config: &l2_config::game::GeoEngineConfig) -> Result<Self, PathError> {
        Ok(PathOptions {
            buffer_sizes: PathOptions::parse_buffers(&config.pathfind_buffers)?,
            low_weight: config.low_weight,
            medium_weight: config.medium_weight,
            high_weight: config.high_weight,
            diagonal_weight: config.diagonal_weight,
            advanced_diagonal_strategy: config.advanced_diagonal_strategy,
            max_postfilter_passes: config.max_postfilter_passes,
        })
    }
}

/// `PathFinding` for one thread: the lazily allocated [`NodeBuffer`]s and `findPath`.
pub struct PathFinder {
    options: PathOptions,
    weights: NodeWeights,
    /// One slot per size class, allocated on first use (Java: the `ThreadLocal` array).
    buffers: Vec<Option<NodeBuffer>>,
}

impl std::fmt::Debug for PathFinder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PathFinder")
            .field("options", &self.options)
            .field(
                "buffers_allocated",
                &self.buffers.iter().filter(|b| b.is_some()).count(),
            )
            .finish()
    }
}

impl PathFinder {
    /// Fails when `buffer_sizes` is empty or has a size of 0.
    pub fn new(mut options: PathOptions) -> Result<PathFinder, PathError> {
        if options.buffer_sizes.is_empty() || options.buffer_sizes.contains(&0) {
            return Err(PathError::InvalidBuffers(format!("{:?}", options.buffer_sizes)));
        }
        options.buffer_sizes.sort_unstable();
        let weights = NodeWeights::new(&options);
        let buffers = options.buffer_sizes.iter().map(|_| None).collect();
        Ok(PathFinder {
            options,
            weights,
            buffers,
        })
    }

    pub fn options(&self) -> &PathOptions {
        &self.options
    }

    /// `findPath(x, y, z, tx, ty, tz, instanceId, playable)` with Java's `null` as `None`.
    ///
    /// `obstacles` answers the door / fence questions of the line of sight postfilter (Java:
    /// `DoorData` / `FenceData` inside `GeoEngine.canMoveToTarget`); the search itself does not
    /// look at doors. An empty path (`Some(vec![])`) means start and target are the same cell.
    pub fn find_path(
        &mut self,
        engine: &GeoEngine,
        obstacles: &dyn DynamicObstacles,
        from: Location,
        to: Location,
        instance_id: i32,
        playable: bool,
    ) -> Option<Vec<GeoLocation>> {
        self.find_path_detailed(engine, obstacles, from, to, instance_id, playable)
            .ok()
    }

    /// Like [`find_path`](Self::find_path) but says why there is no path.
    pub fn find_path_detailed(
        &mut self,
        engine: &GeoEngine,
        obstacles: &dyn DynamicObstacles,
        from: Location,
        to: Location,
        instance_id: i32,
        playable: bool,
    ) -> Result<Vec<GeoLocation>, PathFailure> {
        if !engine.has_geo(from.x, from.y) || !engine.has_geo(to.x, to.y) {
            return Err(PathFailure::NoGeo);
        }

        let gx = geo_x(from.x);
        let gy = geo_y(from.y);
        let gtx = geo_x(to.x);
        let gty = geo_y(to.y);
        let needed = 64 + 2 * (gx - gtx).abs().max((gy - gty).abs()) as usize;
        let slot = self
            .options
            .buffer_sizes
            .iter()
            .position(|size| *size >= needed)
            .ok_or(PathFailure::BufferTooSmall {
                needed,
                largest: self.options.buffer_sizes.last().copied().unwrap_or(0),
            })?;
        let weights = self.weights;
        let size = self.options.buffer_sizes[slot];
        let buffer = self.buffers[slot].get_or_insert_with(|| NodeBuffer::new(size, weights));
        debug_assert_eq!(buffer.map_size(), size);

        let gz = engine.height(from.x, from.y, from.z);
        let gtz = engine.height(to.x, to.y, to.z);
        let result = buffer.find_path(engine, (gx, gy, gz), (gtx, gty, gtz));
        buffer.free();
        let path = result?;

        if path.len() < 3 || self.options.max_postfilter_passes <= 0 {
            return Ok(path);
        }

        Ok(self.apply_post_filtering(engine, obstacles, path, from, instance_id, playable))
    }

    /// Removes waypoints that can be skipped because a straight walk to the next one works. Up to
    /// `MaxPostfilterPasses` passes for playable characters, one for everything else.
    fn apply_post_filtering(
        &self,
        engine: &GeoEngine,
        obstacles: &dyn DynamicObstacles,
        initial_path: Vec<GeoLocation>,
        start: Location,
        instance_id: i32,
        playable: bool,
    ) -> Vec<GeoLocation> {
        let mut path = initial_path;
        let mut pass = 0;
        loop {
            pass += 1;
            let mut changed = false;
            let mut current = start;

            let mut optimized = Vec::new();
            for pair in path.windows(2) {
                let (waypoint, next) = (&pair[0], &pair[1]);
                // Can we walk straight to the next waypoint?
                if engine.can_move_to_target_with(obstacles, current, next.location(), instance_id) {
                    // Skip this waypoint.
                    changed = true;
                } else {
                    // Keep it.
                    optimized.push(*waypoint);
                    current = waypoint.location();
                }
            }

            // Always add the final destination.
            if let Some(last) = path.last() {
                optimized.push(*last);
            }

            path = optimized;
            if !(playable && changed && path.len() > 2 && pass < self.options.max_postfilter_passes) {
                break;
            }
        }
        path
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    //! A small synthetic world for the unit tests: one flat region (20_18) with optional walls.

    use std::sync::Arc;

    use crate::block::BLOCK_CELLS;
    use crate::cell::NSWE_ALL;
    use crate::coords::{Location, world_x, world_y};
    use crate::engine::{GeoEngine, GeoEngineOptions};
    use crate::region::{GeoSet, REGION_BLOCKS, Region};

    /// Region 20_18 starts at geo (40960, 36864).
    pub const RX: i32 = 20 * 2048;
    pub const RY: i32 = 18 * 2048;

    /// A flat region at height 0 with complex blocks at `(bx, by)`; `wall(cell_x, cell_y)` is true
    /// for a cell that is closed in all directions (global geo coordinates).
    pub fn region_with_walls(wall: impl Fn(i32, i32) -> bool, blocks: &[(i32, i32)]) -> Vec<u8> {
        let mut out = Vec::new();
        for block in 0..REGION_BLOCKS {
            let bx = (block / 256) as i32;
            let by = (block % 256) as i32;
            if blocks.contains(&(bx, by)) {
                out.push(1);
                for cell in 0..(BLOCK_CELLS as i32) {
                    // Cells of a complex block are stored as x * 8 + y.
                    let cx = RX + bx * 8 + cell / 8;
                    let cy = RY + by * 8 + cell % 8;
                    let nswe = if wall(cx, cy) { 0 } else { NSWE_ALL };
                    // Height 0, so only the NSWE bits are set.
                    out.extend_from_slice(&u16::from(nswe).to_le_bytes());
                }
            } else {
                out.push(0);
                out.extend_from_slice(&0i16.to_le_bytes());
            }
        }
        out
    }

    pub fn engine(wall: impl Fn(i32, i32) -> bool, blocks: &[(i32, i32)]) -> GeoEngine {
        let bytes = region_with_walls(wall, blocks);
        let mut set = GeoSet::new();
        assert!(set.insert(20, 18, Region::parse(&bytes).unwrap()));
        GeoEngine::new(set, GeoEngineOptions::shipped())
    }

    /// An engine with a vertical wall of cells at `x = RX + 1000` for `y` in `RY+960..RY+1040`
    /// (blocks covering it are complex), everything else open.
    pub fn engine_with_wall() -> GeoEngine {
        let wall_x = RX + 1000;
        let blocks: Vec<(i32, i32)> = (960 / 8..=1040 / 8).map(|by| (1000 / 8, by)).collect();
        engine(
            move |x, y| x == wall_x && (RY + 960..RY + 1040).contains(&y),
            &blocks,
        )
    }

    pub fn arc_engine() -> Arc<GeoEngine> {
        Arc::new(engine_with_wall())
    }

    /// World position of the middle of a geo cell.
    pub fn at(geo_x: i32, geo_y: i32) -> Location {
        Location::new(world_x(geo_x), world_y(geo_y), 0)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::test_support::{RX, RY, at, engine_with_wall};
    use super::*;
    use crate::obstacles::NoObstacles;

    fn finder(options: PathOptions) -> PathFinder {
        PathFinder::new(options).unwrap()
    }

    #[test]
    fn parses_buffer_definitions_like_java() {
        assert_eq!(
            PathOptions::parse_buffers("500x2;100x6;128x6").unwrap(),
            vec![100, 128, 500]
        );
        // The count after the x is ignored, and may even be missing.
        assert_eq!(PathOptions::parse_buffers("100").unwrap(), vec![100]);
        assert_eq!(PathOptions::parse_buffers("64xabc").unwrap(), vec![64]);
        assert_eq!(PathOptions::parse_buffers("100x6;").unwrap(), vec![100]);
        for bad in ["", ";", "x6", "abc", "0x1", "-5x1", "100x6;;200x1"] {
            assert!(
                PathOptions::parse_buffers(bad).is_err(),
                "{bad:?} should be rejected"
            );
        }
    }

    #[test]
    fn options_from_config_match_the_shipped_ini() {
        let ini = l2_config::IniFile::parse(
            "GeoEngine.ini",
            "PathFinding = 2\nAdvancedDiagonalStrategy = False\n",
        )
        .unwrap();
        let config = l2_config::game::GeoEngineConfig::from_ini(&ini);
        let options = PathOptions::try_from(&config).unwrap();
        assert_eq!(options, PathOptions::shipped());
        assert_ne!(PathOptions::default(), PathOptions::shipped());
    }

    #[test]
    fn weights_are_scaled_like_java() {
        let w = NodeWeights::new(&PathOptions::shipped());
        assert_eq!((w.low, w.medium, w.high, w.diagonal), (500, 2000, 3000, 707));
        assert_eq!((w.h_straight, w.h_diagonal_excess), (500, 207));
    }

    #[test]
    fn straight_walk_is_one_waypoint() {
        let engine = engine_with_wall();
        let mut f = finder(PathOptions::shipped());
        let from = at(RX + 10, RY + 10);
        let to = at(RX + 40, RY + 10);
        let path = f.find_path(&engine, &NoObstacles, from, to, 0, true).unwrap();
        assert_eq!(path.len(), 1);
        assert_eq!(
            (path[0].node_x(), path[0].node_y(), path[0].z()),
            (RX + 40, RY + 10, 0)
        );
        assert!(path[0].can_go_all());
    }

    #[test]
    fn same_cell_gives_an_empty_path() {
        let engine = engine_with_wall();
        let mut f = finder(PathOptions::shipped());
        let p = at(RX + 10, RY + 10);
        assert_eq!(
            f.find_path(&engine, &NoObstacles, p, p, 0, true),
            Some(Vec::new())
        );
    }

    #[test]
    fn walks_around_a_wall() {
        let engine = engine_with_wall();
        let from = at(RX + 990, RY + 1000);
        let to = at(RX + 1010, RY + 1000);
        // The direct line is blocked.
        assert!(!engine.can_move_to_target(from, to, 0));
        for options in [PathOptions::shipped(), PathOptions::default()] {
            let mut f = finder(options);
            let path = f.find_path(&engine, &NoObstacles, from, to, 0, true).unwrap();
            assert!(path.len() >= 2, "{path:?}");
            let last = path.last().unwrap();
            assert_eq!((last.node_x(), last.node_y()), (RX + 1010, RY + 1000));
            // The detour goes round one end of the wall (y < 960 or y >= 1040).
            assert!(
                path.iter()
                    .any(|n| n.node_y() < RY + 960 || n.node_y() >= RY + 1040),
                "{path:?}"
            );
            // Every leg is walkable.
            let mut at_ = from;
            for node in &path {
                assert!(
                    engine.can_move_to_target(at_, node.location(), 0),
                    "{at_:?} -> {node:?}"
                );
                at_ = node.location();
            }
        }
    }

    #[test]
    fn reports_why_there_is_no_path() {
        let engine = engine_with_wall();
        let mut f = finder(PathOptions {
            buffer_sizes: vec![100],
            ..PathOptions::shipped()
        });
        let p = at(RX + 10, RY + 10);
        // Outside the loaded region: no geodata.
        let nowhere = Location::new(-600_000, -500_000, 0);
        assert_eq!(
            f.find_path_detailed(&engine, &NoObstacles, p, nowhere, 0, true),
            Err(PathFailure::NoGeo)
        );
        // 64 + 2 * 40 = 144 > 100.
        assert_eq!(
            f.find_path_detailed(&engine, &NoObstacles, p, at(RX + 50, RY + 10), 0, true),
            Err(PathFailure::BufferTooSmall {
                needed: 144,
                largest: 100
            })
        );
        // 64 + 2 * 18 = 100 fits exactly.
        assert!(
            f.find_path_detailed(&engine, &NoObstacles, p, at(RX + 28, RY + 10), 0, true)
                .is_ok()
        );
    }

    #[test]
    fn buffer_is_reusable_after_a_search() {
        let engine = engine_with_wall();
        let mut f = finder(PathOptions::shipped());
        let from = at(RX + 990, RY + 1000);
        let to = at(RX + 1010, RY + 1000);
        let first = f.find_path(&engine, &NoObstacles, from, to, 0, true);
        let second = f.find_path(&engine, &NoObstacles, from, to, 0, true);
        assert!(first.is_some());
        assert_eq!(first, second);
    }

    #[test]
    fn postfilter_uses_the_obstacles_of_the_call() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct Counting(AtomicUsize);
        impl DynamicObstacles for Counting {
            fn doors_between(&self, _: Location, _: Location, _: i32, _: bool) -> bool {
                self.0.fetch_add(1, Ordering::Relaxed);
                false
            }
            fn fence_between(&self, _: Location, _: Location, _: i32) -> bool {
                false
            }
        }

        let engine = engine_with_wall();
        let mut f = finder(PathOptions::shipped());
        let obstacles = Counting(AtomicUsize::new(0));
        let from = at(RX + 990, RY + 1000);
        let to = at(RX + 1010, RY + 1000);
        let path = f.find_path(&engine, &obstacles, from, to, 0, true).unwrap();
        assert!(!path.is_empty());
        assert!(
            obstacles.0.load(Ordering::Relaxed) > 0,
            "postfilter never asked the doors"
        );
    }
}
