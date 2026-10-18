//! The world region grid: which objects are near which.
// Java: org/l2jmobius/gameserver/entity/World.java (constants, init, getRegion, calculateRegionZ)
// Java: org/l2jmobius/gameserver/entity/WorldRegion.java (visible objects, activation, surrounding)
//
// Java allocates every region up front (257 x 273 x 16 = 1.1 million objects with their
// surrounding arrays). Here a region exists only while it holds objects or activation state;
// an absent region reads as empty and inactive (or active with `GridsAlwaysOn`). The surrounding
// regions are computed on the fly in Java's order instead of being stored.

use std::collections::{BTreeSet, HashMap};

use l2_util::ObjectId;

use crate::timer::TimerId;

/// `World.SHIFT_BY`: a region is 2048 x 2048 units.
pub const SHIFT_BY: u32 = 11;
pub const TILE_SIZE: i32 = 32768;
pub const TILE_X_MIN: i32 = 11;
pub const TILE_Y_MIN: i32 = 10;
pub const TILE_X_MAX: i32 = 26;
pub const TILE_Y_MAX: i32 = 26;
pub const TILE_ZERO_COORD_X: i32 = 20;
pub const TILE_ZERO_COORD_Y: i32 = 18;
pub const WORLD_X_MIN: i32 = (TILE_X_MIN - TILE_ZERO_COORD_X) * TILE_SIZE;
pub const WORLD_Y_MIN: i32 = (TILE_Y_MIN - TILE_ZERO_COORD_Y) * TILE_SIZE;
pub const WORLD_X_MAX: i32 = ((TILE_X_MAX - TILE_ZERO_COORD_X) + 1) * TILE_SIZE;
pub const WORLD_Y_MAX: i32 = ((TILE_Y_MAX - TILE_ZERO_COORD_Y) + 1) * TILE_SIZE;
pub const WORLD_Z_MIN: i32 = -16000;
pub const WORLD_Z_MAX: i32 = 16000;
pub const Z_REGION_SIZE: i32 = 2000;
/// `World.OFFSET_X`: so that the top left region is 0,0.
pub const OFFSET_X: i32 = (WORLD_X_MIN >> SHIFT_BY).abs();
pub const OFFSET_Y: i32 = (WORLD_Y_MIN >> SHIFT_BY).abs();
/// `World.REGIONS_X`: the array is `REGIONS_X + 1` wide.
pub const REGIONS_X: i32 = (WORLD_X_MAX >> SHIFT_BY) + OFFSET_X;
pub const REGIONS_Y: i32 = (WORLD_Y_MAX >> SHIFT_BY) + OFFSET_Y;
/// `World.REGIONS_Z`: the array is exactly `REGIONS_Z` high.
pub const REGIONS_Z: i32 = (WORLD_Z_MIN.abs() + WORLD_Z_MAX.abs()) / Z_REGION_SIZE;

/// One cell of the grid, by its Java array indexes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RegionId {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl RegionId {
    /// Java `WorldRegion.isSurroundingRegion`: within one cell on every axis (a coordinate check,
    /// not the stored surrounding list).
    pub fn is_surrounding(self, other: RegionId) -> bool {
        (self.x - other.x).abs() <= 1 && (self.y - other.y).abs() <= 1 && (self.z - other.z).abs() <= 1
    }

    /// Whether `other` is in this region's [`surrounding`](Self::surrounding) list, without
    /// building the list: within one cell on every axis and inside the bounds `World.init` used
    /// (so never the last column or row, see the quirk below).
    pub fn lists(self, other: RegionId) -> bool {
        self.is_surrounding(other)
            && (0..REGIONS_X).contains(&other.x)
            && (0..REGIONS_Y).contains(&other.y)
            && (0..REGIONS_Z).contains(&other.z)
    }

    /// Java `getSurroundingRegions()`: the neighbours built in `World.init`, in that order, with
    /// this region swapped to the front.
    ///
    /// NOTE(java-quirk): `World.init` only takes neighbours with `sx < REGIONS_X` and
    /// `sy < REGIONS_Y` although the array has `REGIONS_X + 1` columns, so the last column and row
    /// are in nobody's list, not even their own.
    pub fn surrounding(self) -> Vec<RegionId> {
        let mut list = Vec::with_capacity(27);
        for sx in self.x - 1..=self.x + 1 {
            for sy in self.y - 1..=self.y + 1 {
                for sz in self.z - 1..=self.z + 1 {
                    if (0..REGIONS_X).contains(&sx)
                        && (0..REGIONS_Y).contains(&sy)
                        && (0..REGIONS_Z).contains(&sz)
                    {
                        list.push(RegionId { x: sx, y: sy, z: sz });
                    }
                }
            }
        }
        // WorldRegion.setSurroundingRegions: "Make sure that this region is always the first".
        if let Some(i) = list.iter().position(|r| *r == self) {
            list.swap(0, i);
        }
        list
    }
}

/// Java `World.calculateRegionZ`.
pub fn region_z(z: i32) -> i32 {
    if z < WORLD_Z_MIN {
        return 0;
    }
    if z > WORLD_Z_MAX {
        return REGIONS_Z - 1;
    }
    (z - WORLD_Z_MIN) / Z_REGION_SIZE
}

/// Java `World.getRegion(x, y, z)`. `None` where Java's array access throws
/// (`ArrayIndexOutOfBoundsException`): outside the world, and
/// NOTE(java-quirk): at exactly `z == WORLD_Z_MAX`, whose index is `REGIONS_Z`.
pub fn region_of(x: i32, y: i32, z: i32) -> Option<RegionId> {
    let rx = (x >> SHIFT_BY) + OFFSET_X;
    let ry = (y >> SHIFT_BY) + OFFSET_Y;
    let rz = region_z(z);
    let inside =
        (0..=REGIONS_X).contains(&rx) && (0..=REGIONS_Y).contains(&ry) && (0..REGIONS_Z).contains(&rz);
    inside.then_some(RegionId { x: rx, y: ry, z: rz })
}

/// What a region holds. Absent from the grid = empty, inactive (or always on), no timer.
#[derive(Debug, Default)]
pub(crate) struct Region {
    /// Java `_visibleObjects`. A set ordered by object id, so every walk over a region is
    /// deterministic (D-13).
    // NOTE: Java's ConcurrentHashMap key set has no defined order; packets to a player entering a
    // crowded area may come in a different order than on the Java server.
    pub objects: BTreeSet<ObjectId>,
    /// Java `_doors`: the doors in this region **and** in every region around it (Java adds a
    /// door to the lists of all its surrounding regions), so the geodata checks need only the
    /// region of the start point.
    pub doors: BTreeSet<ObjectId>,
    /// Java `_fences`, kept the same way as `doors`.
    pub fences: BTreeSet<ObjectId>,
    pub active: bool,
    /// Java `_activeNeighbors`.
    pub active_neighbors: i32,
    /// Java `_neighborsTask`: the pending activation or deactivation of the neighbours.
    pub neighbors_task: Option<TimerId>,
}

/// The sparse grid.
#[derive(Debug, Default)]
pub(crate) struct RegionGrid {
    regions: HashMap<RegionId, Region>,
    /// `GeneralConfig.GRIDS_ALWAYS_ON`: the initial value of every region's `_active`.
    pub always_on: bool,
}

impl RegionGrid {
    pub fn new(always_on: bool) -> Self {
        Self {
            regions: HashMap::new(),
            always_on,
        }
    }

    pub fn get(&self, id: RegionId) -> Option<&Region> {
        self.regions.get(&id)
    }

    pub fn get_mut(&mut self, id: RegionId) -> &mut Region {
        let always_on = self.always_on;
        self.regions.entry(id).or_insert_with(|| Region {
            active: always_on,
            ..Region::default()
        })
    }

    pub fn is_active(&self, id: RegionId) -> bool {
        self.regions.get(&id).map_or(self.always_on, |r| r.active)
    }

    /// Java `areNeighborsActive`.
    pub fn are_neighbors_active(&self, id: RegionId) -> bool {
        self.always_on || self.regions.get(&id).is_some_and(|r| r.active_neighbors > 0)
    }

    /// The objects of one region, in id order.
    pub fn objects(&self, id: RegionId) -> impl Iterator<Item = ObjectId> + '_ {
        self.regions
            .get(&id)
            .into_iter()
            .flat_map(|r| r.objects.iter().copied())
    }

    /// Java `WorldRegion.getDoors()`, in id order.
    pub fn doors(&self, id: RegionId) -> impl Iterator<Item = ObjectId> + '_ {
        self.regions
            .get(&id)
            .into_iter()
            .flat_map(|r| r.doors.iter().copied())
    }

    /// Java `WorldRegion.getFences()`, in id order.
    pub fn fences(&self, id: RegionId) -> impl Iterator<Item = ObjectId> + '_ {
        self.regions
            .get(&id)
            .into_iter()
            .flat_map(|r| r.fences.iter().copied())
    }

    /// Drops regions that hold nothing worth keeping, so the map stays small.
    pub fn compact(&mut self, id: RegionId) {
        let always_on = self.always_on;
        if let Some(r) = self.regions.get(&id)
            && r.objects.is_empty()
            && r.doors.is_empty()
            && r.fences.is_empty()
            && r.active == always_on
            && r.active_neighbors == 0
            && r.neighbors_task.is_none()
        {
            self.regions.remove(&id);
        }
    }

    /// Regions with any state (tests, admin).
    pub fn len(&self) -> usize {
        self.regions.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_match_java() {
        assert_eq!(WORLD_X_MIN, -294_912);
        assert_eq!(WORLD_X_MAX, 229_376);
        assert_eq!(WORLD_Y_MIN, -262_144);
        assert_eq!(WORLD_Y_MAX, 294_912);
        assert_eq!((OFFSET_X, OFFSET_Y), (144, 128));
        assert_eq!((REGIONS_X, REGIONS_Y, REGIONS_Z), (256, 272, 16));
    }

    #[test]
    fn regions_of_coordinates() {
        // Talking Island village area.
        assert_eq!(
            region_of(-84_318, 244_579, -3_730),
            Some(RegionId {
                x: (-84_318 >> 11) + 144,
                y: (244_579 >> 11) + 128,
                z: (-3_730 + 16_000) / 2_000
            })
        );
        // Arithmetic shift rounds towards minus infinity, like Java's `>>`.
        assert_eq!(region_of(-1, 0, 0).map(|r| r.x), Some(143));
        assert_eq!(region_of(0, 0, 0).map(|r| r.x), Some(144));
        // Z outside the world clamps; exactly WORLD_Z_MAX is Java's out-of-bounds index.
        assert_eq!(region_of(0, 0, -20_000).map(|r| r.z), Some(0));
        assert_eq!(region_of(0, 0, 20_000).map(|r| r.z), Some(15));
        assert_eq!(region_of(0, 0, WORLD_Z_MAX), None);
        assert_eq!(region_of(0, 0, WORLD_Z_MAX - 1).map(|r| r.z), Some(15));
        // Outside the world.
        assert_eq!(region_of(WORLD_X_MIN - 2_048, 0, 0), None);
        assert_eq!(region_of(0, WORLD_Y_MAX + 4_096, 0), None);
        // The last column exists (REGIONS_X + 1 wide).
        assert_eq!(region_of(WORLD_X_MAX, 0, 0).map(|r| r.x), Some(REGIONS_X));
    }

    #[test]
    fn surrounding_regions_have_self_first_and_java_order() {
        let r = RegionId { x: 10, y: 20, z: 5 };
        let s = r.surrounding();
        assert_eq!(s.len(), 27);
        assert_eq!(s[0], r);
        // The first neighbour Java built took the slot this region had (index 13).
        assert_eq!(s[13], RegionId { x: 9, y: 19, z: 4 });
        assert!(s.iter().all(|n| r.is_surrounding(*n)));
        // Corner of the grid: 2 x 2 x 2.
        assert_eq!(RegionId { x: 0, y: 0, z: 0 }.surrounding().len(), 8);
    }

    #[test]
    fn the_last_column_is_in_nobodys_list() {
        let edge = RegionId {
            x: REGIONS_X,
            y: 100,
            z: 5,
        };
        let s = edge.surrounding();
        assert!(!s.contains(&edge), "Java quirk: not even its own list");
        assert_eq!(s.len(), 9, "only the x - 1 column");
        let inner = RegionId {
            x: REGIONS_X - 1,
            y: 100,
            z: 5,
        };
        assert!(!inner.surrounding().contains(&edge));
    }

    #[test]
    fn lists_matches_the_surrounding_list() {
        for (x, y, z) in [
            (10, 20, 5),
            (0, 0, 0),
            (REGIONS_X, 100, 5),
            (REGIONS_X - 1, REGIONS_Y, 15),
        ] {
            let r = RegionId { x, y, z };
            let list = r.surrounding();
            for dx in -2..=2 {
                for dy in -2..=2 {
                    for dz in -2..=2 {
                        let o = RegionId {
                            x: x + dx,
                            y: y + dy,
                            z: z + dz,
                        };
                        assert_eq!(r.lists(o), list.contains(&o), "{r:?} lists {o:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn sparse_grid_defaults() {
        let mut grid = RegionGrid::new(false);
        let id = RegionId { x: 1, y: 1, z: 1 };
        assert!(!grid.is_active(id));
        grid.get_mut(id).objects.insert(ObjectId(5));
        assert_eq!(grid.objects(id).collect::<Vec<_>>(), [ObjectId(5)]);
        grid.get_mut(id).objects.clear();
        grid.compact(id);
        assert_eq!(grid.len(), 0);
        assert!(RegionGrid::new(true).is_active(id));
    }
}
