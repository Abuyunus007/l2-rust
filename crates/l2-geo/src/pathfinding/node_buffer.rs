//! The A* search on the geodata cells.
//!
//! Java: `gameserver/geoengine/pathfinding/NodeBuffer.java`.
//!
//! Cell state lives in parallel arrays indexed by `(aY * mapSize) + aX`; the open list is an
//! indexed binary min-heap with decrease-key; the closed / open sets are bit flags. `free` only
//! resets the cells that were touched. A buffer is owned by one thread (Java: `ThreadLocal` in
//! `PathFinding`, here: one [`super::PathFinder`] per worker), so there is no locking.
//!
//! All costs are integers scaled by 1000 (`(int) (weight * 1000)`), exactly like Java, so ties
//! break the same way and the paths are identical.

use super::{GeoLocation, PathFailure, PathOptions};
use crate::cell::{NSWE_ALL, NSWE_EAST, NSWE_NORTH, NSWE_SOUTH, NSWE_WEST};
use crate::engine::GeoEngine;

const MAX_ITERATIONS: i32 = 7000;
const COST_SCALE: f32 = 1000.0;
const Z_TOLERANCE: i32 = 64;
const Z_STEP_LIMIT: i32 = 16;

const FLAG_TOUCHED: u8 = 0x1;
const FLAG_OPEN: u8 = 0x2;
const FLAG_CLOSED: u8 = 0x4;

/// The weights of `NodeBuffer`'s constructor: `(int) (WEIGHT * COST_SCALE)`.
///
/// NOTE(java-quirk): the product is a `float`, so `0.6f * 1000` is `600.0` only by luck of the
/// rounding; the cast truncates (`0.707f * 1000` gives 707 but a value just below an integer
/// would lose one). Rust's `f32` multiply and `as i32` behave the same way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct NodeWeights {
    pub(super) low: i32,
    pub(super) medium: i32,
    pub(super) high: i32,
    pub(super) diagonal: i32,
    /// Octile straight step cost (`_hStraight`).
    pub(super) h_straight: i32,
    /// Octile diagonal step extra cost (`_hDiagonalExcess`).
    pub(super) h_diagonal_excess: i32,
    diagonal_strategy: bool,
}

impl NodeWeights {
    pub(crate) fn new(options: &PathOptions) -> NodeWeights {
        let low = (options.low_weight * COST_SCALE) as i32;
        let diagonal = (options.diagonal_weight * COST_SCALE) as i32;
        NodeWeights {
            low,
            medium: (options.medium_weight * COST_SCALE) as i32,
            high: (options.high_weight * COST_SCALE) as i32,
            diagonal,
            h_straight: low,
            h_diagonal_excess: diagonal.wrapping_sub(low),
            diagonal_strategy: options.advanced_diagonal_strategy,
        }
    }
}

/// `NodeBuffer`: one reusable A* workspace of `size * size` cells.
pub(crate) struct NodeBuffer {
    map_size: i32,
    weights: NodeWeights,

    // Cell state (indexed by cell index).
    parent: Vec<i32>,
    g_cost: Vec<i32>,
    h_cost: Vec<i32>,
    f_cost: Vec<i32>,
    flags: Vec<u8>,
    nswe: Vec<u8>,
    z: Vec<i16>,

    // Touched-cell ledger so `free` runs in O(touched).
    touched: Vec<i32>,

    // Indexed binary min-heap.
    heap: Vec<i32>,
    heap_pos: Vec<i32>,
    heap_size: usize,

    base_x: i32,
    base_y: i32,
    target_x: i32,
    target_y: i32,
    target_z: i32,
}

impl NodeBuffer {
    pub(crate) fn new(size: usize, weights: NodeWeights) -> NodeBuffer {
        let capacity = size * size;
        NodeBuffer {
            map_size: size as i32,
            weights,
            parent: vec![0; capacity],
            g_cost: vec![0; capacity],
            h_cost: vec![0; capacity],
            f_cost: vec![0; capacity],
            flags: vec![0; capacity],
            nswe: vec![0; capacity],
            z: vec![0; capacity],
            touched: Vec::new(),
            heap: vec![0; capacity],
            heap_pos: vec![0; capacity],
            heap_size: 0,
            base_x: 0,
            base_y: 0,
            target_x: 0,
            target_y: 0,
            target_z: 0,
        }
    }

    pub(crate) fn map_size(&self) -> usize {
        self.map_size as usize
    }

    /// `findPath(x, y, z, tx, ty, tz)`: A* from `start` to `target` (geo x, geo y, world z each). The path is in
    /// forward order, start exclusive, target inclusive, colinear runs collapsed to their end
    /// points. Call [`free`](Self::free) before the buffer is used again.
    pub(crate) fn find_path(
        &mut self,
        engine: &GeoEngine,
        (x, y, z): (i32, i32, i32),
        (tx, ty, tz): (i32, i32, i32),
    ) -> Result<Vec<GeoLocation>, PathFailure> {
        self.base_x = x + (tx - x - self.map_size) / 2;
        self.base_y = y + (ty - y - self.map_size) / 2;
        self.target_x = tx;
        self.target_y = ty;
        self.target_z = tz;

        let Some(start_idx) = self.cell_idx(x, y) else {
            return Err(PathFailure::NoPath);
        };

        self.touch(engine, start_idx, x, y, z);
        let start = start_idx as usize;
        self.g_cost[start] = 0;
        self.f_cost[start] = self.h_cost[start];
        self.heap_push(start_idx);

        for _ in 0..MAX_ITERATIONS {
            if self.heap_size == 0 {
                return Err(PathFailure::NoPath);
            }

            let curr_idx = self.heap_poll();
            let curr = curr_idx as usize;
            self.flags[curr] &= !FLAG_OPEN;
            self.flags[curr] |= FLAG_CLOSED;

            let curr_x = self.idx_to_x(curr_idx);
            let curr_y = self.idx_to_y(curr_idx);
            let curr_z = i32::from(self.z[curr]);

            if curr_x == self.target_x
                && curr_y == self.target_y
                && (curr_z - self.target_z).abs() < Z_TOLERANCE
            {
                return Ok(self.construct_path(engine, curr_idx));
            }

            self.expand_neighbors(engine, curr_idx, curr_x, curr_y, curr_z);
        }

        Err(PathFailure::IterationLimit)
    }

    /// `free()`: resets the touched cells and the heap so the buffer can be reused.
    pub(crate) fn free(&mut self) {
        for &idx in &self.touched {
            self.flags[idx as usize] = 0;
        }
        self.touched.clear();
        self.heap_size = 0;
    }

    fn cell_idx(&self, gx: i32, gy: i32) -> Option<i32> {
        let a_x = gx - self.base_x;
        if a_x < 0 || a_x >= self.map_size {
            return None;
        }
        let a_y = gy - self.base_y;
        if a_y < 0 || a_y >= self.map_size {
            return None;
        }
        Some(a_y * self.map_size + a_x)
    }

    fn idx_to_x(&self, idx: i32) -> i32 {
        self.base_x + idx % self.map_size
    }

    fn idx_to_y(&self, idx: i32) -> i32 {
        self.base_y + idx / self.map_size
    }

    /// Lazily initialises a cell on first contact: the NSWE bitmask and the nearest Z are asked
    /// from the engine exactly once per cell per search.
    fn touch(&mut self, engine: &GeoEngine, idx: i32, gx: i32, gy: i32, ref_z: i32) {
        let i = idx as usize;
        if self.flags[i] & FLAG_TOUCHED != 0 {
            return;
        }

        let mut nswe = 0;
        // Same order as Java (N, E, S, W); plain `checkNearestNswe`, no anti corner cut here.
        for direction in [NSWE_NORTH, NSWE_EAST, NSWE_SOUTH, NSWE_WEST] {
            if engine.check_nearest_nswe(gx, gy, ref_z, direction) {
                nswe |= direction;
            }
        }

        self.nswe[i] = nswe;
        // NOTE(java-quirk): `(short) getNearestZ(...)` truncates to 16 bits.
        self.z[i] = engine.nearest_z(gx, gy, ref_z) as i16;
        self.flags[i] = FLAG_TOUCHED;
        self.parent[i] = -1;
        self.g_cost[i] = i32::MAX;
        self.h_cost[i] = self.compute_h(gx, gy);
        self.f_cost[i] = i32::MAX;
        self.heap_pos[i] = -1;

        self.touched.push(idx);
    }

    /// Octile heuristic in pre-scaled integer arithmetic. Z is not part of it (it is folded into
    /// the neighbour weighting instead).
    fn compute_h(&self, gx: i32, gy: i32) -> i32 {
        let dx = (gx - self.target_x).abs();
        let dy = (gy - self.target_y).abs();
        let max_d = dx.max(dy);
        let min_d = dx.min(dy);
        max_d
            .wrapping_mul(self.weights.h_straight)
            .wrapping_add(min_d.wrapping_mul(self.weights.h_diagonal_excess))
    }

    /// The four orthogonal neighbours always, plus the four diagonals with
    /// `AdvancedDiagonalStrategy` when both flanking cells permit the diagonal.
    fn expand_neighbors(&mut self, engine: &GeoEngine, curr_idx: i32, x: i32, y: i32, z: i32) {
        let nswe = self.nswe[curr_idx as usize];
        if nswe == 0 {
            return;
        }

        let mut idx_e = -1;
        let mut idx_s = -1;
        let mut idx_w = -1;
        let mut idx_n = -1;

        if nswe & NSWE_EAST != 0 {
            idx_e = self.consider_neighbor(engine, curr_idx, x + 1, y, z, false);
        }
        if nswe & NSWE_SOUTH != 0 {
            idx_s = self.consider_neighbor(engine, curr_idx, x, y + 1, z, false);
        }
        if nswe & NSWE_WEST != 0 {
            idx_w = self.consider_neighbor(engine, curr_idx, x - 1, y, z, false);
        }
        if nswe & NSWE_NORTH != 0 {
            idx_n = self.consider_neighbor(engine, curr_idx, x, y - 1, z, false);
        }

        if !self.weights.diagonal_strategy {
            return;
        }

        if idx_e >= 0
            && idx_s >= 0
            && self.nswe[idx_e as usize] & NSWE_SOUTH != 0
            && self.nswe[idx_s as usize] & NSWE_EAST != 0
        {
            self.consider_neighbor(engine, curr_idx, x + 1, y + 1, z, true);
        }
        if idx_s >= 0
            && idx_w >= 0
            && self.nswe[idx_w as usize] & NSWE_SOUTH != 0
            && self.nswe[idx_s as usize] & NSWE_WEST != 0
        {
            self.consider_neighbor(engine, curr_idx, x - 1, y + 1, z, true);
        }
        if idx_n >= 0
            && idx_e >= 0
            && self.nswe[idx_e as usize] & NSWE_NORTH != 0
            && self.nswe[idx_n as usize] & NSWE_EAST != 0
        {
            self.consider_neighbor(engine, curr_idx, x + 1, y - 1, z, true);
        }
        if idx_n >= 0
            && idx_w >= 0
            && self.nswe[idx_w as usize] & NSWE_NORTH != 0
            && self.nswe[idx_n as usize] & NSWE_WEST != 0
        {
            self.consider_neighbor(engine, curr_idx, x - 1, y - 1, z, true);
        }
    }

    /// Relaxes a neighbour against the current node: picks a movement weight from the terrain
    /// (high for blocked / cliff, medium for next to blocked, low or diagonal otherwise), then
    /// pushes it or sifts it up in the open heap when a better path is found. Returns the cell
    /// index of the neighbour, or -1 outside the buffer.
    fn consider_neighbor(
        &mut self,
        engine: &GeoEngine,
        parent_idx: i32,
        nx: i32,
        ny: i32,
        parent_z: i32,
        diagonal: bool,
    ) -> i32 {
        let Some(idx) = self.cell_idx(nx, ny) else {
            return -1;
        };

        self.touch(engine, idx, nx, ny, parent_z);
        let i = idx as usize;

        if self.flags[i] & FLAG_CLOSED != 0 {
            return idx;
        }

        let neighbor_z = i32::from(self.z[i]);
        let step_z = (neighbor_z - parent_z).abs();

        let weight = if (self.nswe[i] & NSWE_ALL) != NSWE_ALL || step_z > Z_STEP_LIMIT {
            self.weights.high
        } else if self.is_high_weight(engine, nx + 1, ny, neighbor_z)
            || self.is_high_weight(engine, nx - 1, ny, neighbor_z)
            || self.is_high_weight(engine, nx, ny + 1, neighbor_z)
            || self.is_high_weight(engine, nx, ny - 1, neighbor_z)
        {
            self.weights.medium
        } else if diagonal {
            self.weights.diagonal
        } else {
            self.weights.low
        };

        let new_g_cost = self.g_cost[parent_idx as usize].wrapping_add(weight);
        let in_open = self.flags[i] & FLAG_OPEN != 0;
        if in_open && new_g_cost >= self.g_cost[i] {
            return idx;
        }

        self.parent[i] = parent_idx;
        self.g_cost[i] = new_g_cost;
        self.f_cost[i] = new_g_cost.wrapping_add(self.h_cost[i]);
        if in_open {
            self.heap_sift_up(self.heap_pos[i] as usize);
        } else {
            self.heap_push(idx);
        }

        idx
    }

    fn is_high_weight(&mut self, engine: &GeoEngine, gx: i32, gy: i32, ref_z: i32) -> bool {
        let Some(idx) = self.cell_idx(gx, gy) else {
            return true;
        };

        self.touch(engine, idx, gx, gy, ref_z);
        let i = idx as usize;
        (self.nswe[i] & NSWE_ALL) != NSWE_ALL || (i32::from(self.z[i]) - ref_z).abs() > Z_STEP_LIMIT
    }

    fn heap_push(&mut self, idx: i32) {
        let pos = self.heap_size;
        self.heap_size += 1;
        self.heap[pos] = idx;
        self.heap_pos[idx as usize] = pos as i32;
        self.flags[idx as usize] |= FLAG_OPEN;
        self.heap_sift_up(pos);
    }

    fn heap_poll(&mut self) -> i32 {
        let root = self.heap[0];
        self.heap_pos[root as usize] = -1;
        self.heap_size -= 1;
        let last_pos = self.heap_size;
        if last_pos > 0 {
            let moved = self.heap[last_pos];
            self.heap[0] = moved;
            self.heap_pos[moved as usize] = 0;
            self.heap_sift_down(0);
        }
        root
    }

    /// Order: lower `f`, then lower `h`.
    fn heap_sift_up(&mut self, start_pos: usize) {
        let mut pos = start_pos;
        let idx = self.heap[pos];
        let f_cost = self.f_cost[idx as usize];
        let h_cost = self.h_cost[idx as usize];
        while pos > 0 {
            let parent_pos = (pos - 1) >> 1;
            let parent_idx = self.heap[parent_pos];
            let parent_f = self.f_cost[parent_idx as usize];
            if f_cost > parent_f || (f_cost == parent_f && h_cost >= self.h_cost[parent_idx as usize]) {
                break;
            }

            self.heap[pos] = parent_idx;
            self.heap_pos[parent_idx as usize] = pos as i32;
            pos = parent_pos;
        }

        self.heap[pos] = idx;
        self.heap_pos[idx as usize] = pos as i32;
    }

    fn heap_sift_down(&mut self, start_pos: usize) {
        let mut pos = start_pos;
        let idx = self.heap[pos];
        let f_cost = self.f_cost[idx as usize];
        let h_cost = self.h_cost[idx as usize];
        let half = self.heap_size >> 1;
        while pos < half {
            let mut child_pos = (pos << 1) + 1;
            let mut child_idx = self.heap[child_pos];
            let mut child_f = self.f_cost[child_idx as usize];
            let right_pos = child_pos + 1;
            if right_pos < self.heap_size {
                let right_idx = self.heap[right_pos];
                let right_f = self.f_cost[right_idx as usize];
                if right_f < child_f
                    || (right_f == child_f
                        && self.h_cost[right_idx as usize] < self.h_cost[child_idx as usize])
                {
                    child_pos = right_pos;
                    child_idx = right_idx;
                    child_f = right_f;
                }
            }

            if f_cost < child_f || (f_cost == child_f && h_cost <= self.h_cost[child_idx as usize]) {
                break;
            }

            self.heap[pos] = child_idx;
            self.heap_pos[child_idx as usize] = pos as i32;
            pos = child_pos;
        }

        self.heap[pos] = idx;
        self.heap_pos[idx as usize] = pos as i32;
    }

    /// Walks the parent chain from the result cell backwards and emits a waypoint only when the
    /// movement direction changes.
    fn construct_path(&self, engine: &GeoEngine, end_idx: i32) -> Vec<GeoLocation> {
        let mut reversed = Vec::new();
        let mut prev_dir_x = i32::MIN;
        let mut prev_dir_y = i32::MIN;
        let mut curr = end_idx;

        while self.parent[curr as usize] != -1 {
            let parent = self.parent[curr as usize];
            let grand = self.parent[parent as usize];
            let curr_x = self.idx_to_x(curr);
            let curr_y = self.idx_to_y(curr);
            let parent_x = self.idx_to_x(parent);
            let parent_y = self.idx_to_y(parent);

            let (dir_x, dir_y) = if !self.weights.diagonal_strategy && grand != -1 {
                let tmp_x = curr_x - self.idx_to_x(grand);
                let tmp_y = curr_y - self.idx_to_y(grand);
                if tmp_x.abs() == tmp_y.abs() {
                    (tmp_x, tmp_y)
                } else {
                    (curr_x - parent_x, curr_y - parent_y)
                }
            } else {
                (curr_x - parent_x, curr_y - parent_y)
            };

            if dir_x != prev_dir_x || dir_y != prev_dir_y {
                prev_dir_x = dir_x;
                prev_dir_y = dir_y;
                reversed.push(GeoLocation::new(
                    engine,
                    curr_x,
                    curr_y,
                    i32::from(self.z[curr as usize]),
                ));
            }

            curr = parent;
        }

        reversed.reverse();
        reversed
    }
}
