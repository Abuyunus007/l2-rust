//! World <-> geo coordinate conversion.
//!
//! Java: `gameserver/geoengine/GeoEngine.java` (`getGeoX`, `getGeoY`, `getWorldX`, `getWorldY` and
//! the constants above them).
//!
//! A geo cell is 16 x 16 world units. Geo cell `(0, 0)` starts at world `(-655360, -589824)`,
//! i.e. at the corner of region 0_0 (the world origin sits at region 20_18, 20 * 32768 = 655360).
//! All arithmetic wraps like Java `int` (no panics in debug builds for absurd inputs).

/// `GeoEngine.WORLD_MIN_X`: world X of geo cell 0.
pub const WORLD_MIN_X: i32 = -655_360;
/// `GeoEngine.WORLD_MIN_Y`: world Y of geo cell 0.
pub const WORLD_MIN_Y: i32 = -589_824;
/// `GeoEngine.COORDINATE_SCALE`: world units per geo cell.
pub const COORDINATE_SCALE: i32 = 16;
/// `GeoEngine.COORDINATE_OFFSET`: `world_x` returns the middle of the cell.
pub const COORDINATE_OFFSET: i32 = 8;

/// A world position (`Location` in Java, without heading and instance).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct Location {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl Location {
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Location { x, y, z }
    }
}

/// `GeoEngine.getGeoX`: world X to geo cell X.
///
/// NOTE(java-quirk): Java `int` division truncates toward zero, so a world X just left of
/// [`WORLD_MIN_X`] (-1 to -15 units) still gives cell 0, not -1. Rust's `/` on `i32` behaves the same.
pub fn geo_x(world_x: i32) -> i32 {
    world_x.wrapping_sub(WORLD_MIN_X) / COORDINATE_SCALE
}

/// `GeoEngine.getGeoY`: world Y to geo cell Y (same truncation as [`geo_x`]).
pub fn geo_y(world_y: i32) -> i32 {
    world_y.wrapping_sub(WORLD_MIN_Y) / COORDINATE_SCALE
}

/// `GeoEngine.getWorldX`: geo cell X to the world X of the cell's middle.
pub fn world_x(geo_x: i32) -> i32 {
    geo_x
        .wrapping_mul(COORDINATE_SCALE)
        .wrapping_add(WORLD_MIN_X)
        .wrapping_add(COORDINATE_OFFSET)
}

/// `GeoEngine.getWorldY`: geo cell Y to the world Y of the cell's middle.
pub fn world_y(geo_y: i32) -> i32 {
    geo_y
        .wrapping_mul(COORDINATE_SCALE)
        .wrapping_add(WORLD_MIN_Y)
        .wrapping_add(COORDINATE_OFFSET)
}

/// `GeoUtils.computeNswe`: the NSWE flags of a step from `(last_x, last_y)` to `(x, y)`.
///
/// Y grows to the south (`y > last_y` is south), X grows to the east. No step gives 0.
pub fn compute_nswe(last_x: i32, last_y: i32, x: i32, y: i32) -> u8 {
    use crate::cell::{
        NSWE_EAST, NSWE_NORTH, NSWE_NORTH_EAST, NSWE_NORTH_WEST, NSWE_SOUTH, NSWE_SOUTH_EAST,
        NSWE_SOUTH_WEST, NSWE_WEST,
    };
    if x > last_x {
        if y > last_y {
            NSWE_SOUTH_EAST
        } else if y < last_y {
            NSWE_NORTH_EAST
        } else {
            NSWE_EAST
        }
    } else if x < last_x {
        if y > last_y {
            NSWE_SOUTH_WEST
        } else if y < last_y {
            NSWE_NORTH_WEST
        } else {
            NSWE_WEST
        }
    } else if y > last_y {
        NSWE_SOUTH
    } else if y < last_y {
        NSWE_NORTH
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::{NSWE_EAST, NSWE_NORTH, NSWE_NORTH_WEST, NSWE_SOUTH_EAST};

    #[test]
    fn world_origin_is_region_20_18() {
        // World (0, 0) is cell (40960, 36864) = region 20_18 at its corner.
        assert_eq!(geo_x(0), 20 * 2048);
        assert_eq!(geo_y(0), 18 * 2048);
        assert_eq!(world_x(20 * 2048), 8);
        assert_eq!(world_y(18 * 2048), 8);
    }

    #[test]
    fn cell_edges() {
        assert_eq!(geo_x(WORLD_MIN_X), 0);
        assert_eq!(geo_x(WORLD_MIN_X + 15), 0);
        assert_eq!(geo_x(WORLD_MIN_X + 16), 1);
        assert_eq!(geo_y(WORLD_MIN_Y + 31), 1);
        // The middle of a cell converts back to the same cell.
        for cell in [0, 1, 2047, 2048, 55_295] {
            assert_eq!(geo_x(world_x(cell)), cell);
            assert_eq!(geo_y(world_y(cell)), cell);
        }
    }

    #[test]
    fn negative_coordinates_round_toward_zero_like_java() {
        // NOTE(java-quirk): (-1) / 16 == 0 in Java, so cell 0 also covers the 15 units before it.
        assert_eq!(geo_x(WORLD_MIN_X - 1), 0);
        assert_eq!(geo_x(WORLD_MIN_X - 15), 0);
        assert_eq!(geo_x(WORLD_MIN_X - 16), -1);
        assert_eq!(geo_x(WORLD_MIN_X - 17), -1);
        assert_eq!(geo_y(WORLD_MIN_Y - 1), 0);
        assert_eq!(geo_y(WORLD_MIN_Y - 31), -1);
    }

    #[test]
    fn arithmetic_wraps_like_java_int() {
        // Integer.MAX_VALUE - (-655360) overflows to a negative int in Java.
        assert_eq!(geo_x(i32::MAX), (i32::MAX.wrapping_add(655_360)) / 16);
        assert!(geo_x(i32::MAX) < 0);
        assert_eq!(
            world_x(i32::MAX),
            i32::MAX.wrapping_mul(16).wrapping_add(-655_360 + 8)
        );
    }

    #[test]
    fn nswe_of_a_step() {
        assert_eq!(compute_nswe(5, 5, 6, 5), NSWE_EAST);
        assert_eq!(compute_nswe(5, 5, 5, 4), NSWE_NORTH);
        assert_eq!(compute_nswe(5, 5, 4, 4), NSWE_NORTH_WEST);
        assert_eq!(compute_nswe(5, 5, 6, 9), NSWE_SOUTH_EAST);
        assert_eq!(compute_nswe(5, 5, 5, 5), 0);
    }
}
