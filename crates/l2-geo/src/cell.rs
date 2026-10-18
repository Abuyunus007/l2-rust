//! Cell constants and cell word decoding.
//!
//! Java: `gameserver/geoengine/geodata/Cell.java` and the cell helpers in `ComplexBlock` /
//! `MultilayerBlock`.

/// East NSWE flag (`Cell.NSWE_EAST`).
pub const NSWE_EAST: u8 = 1 << 0;
/// West NSWE flag.
pub const NSWE_WEST: u8 = 1 << 1;
/// South NSWE flag.
pub const NSWE_SOUTH: u8 = 1 << 2;
/// North NSWE flag.
pub const NSWE_NORTH: u8 = 1 << 3;
/// North-East flags.
pub const NSWE_NORTH_EAST: u8 = NSWE_NORTH | NSWE_EAST;
/// North-West flags.
pub const NSWE_NORTH_WEST: u8 = NSWE_NORTH | NSWE_WEST;
/// South-East flags.
pub const NSWE_SOUTH_EAST: u8 = NSWE_SOUTH | NSWE_EAST;
/// South-West flags.
pub const NSWE_SOUTH_WEST: u8 = NSWE_SOUTH | NSWE_WEST;
/// All four directions (`Cell.NSWE_ALL`).
pub const NSWE_ALL: u8 = NSWE_EAST | NSWE_WEST | NSWE_SOUTH | NSWE_NORTH;

/// Height of an encoded cell word: `(short) (v & 0xFFF0) >> 1`.
///
/// The word keeps the height in bits 4..15 (already doubled), so the shift is an arithmetic shift
/// on the *signed* 16-bit value: negative heights stay negative.
#[inline]
pub fn cell_height(raw: u16) -> i32 {
    i32::from(((raw & 0xFFF0) as i16) >> 1)
}

/// NSWE bits of an encoded cell word: `v & 0x000F`.
#[inline]
pub fn cell_nswe(raw: u16) -> u8 {
    (raw & 0x000F) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn height_is_signed_and_nswe_is_low_nibble() {
        // 96 * 2 = 192 = 0x00C0, plus NSWE 0b0101.
        assert_eq!(cell_height(0x00C0 | 0b0101), 96);
        assert_eq!(cell_nswe(0x00C0 | 0b0101), 0b0101);
        // -96 * 2 = -192 = 0xFF40 as u16.
        assert_eq!(cell_height((-192i16) as u16 | 0x0F), -96);
        assert_eq!(cell_nswe((-192i16) as u16 | 0x0F), 0x0F);
        // Extremes: 0x7FF0 -> 16376, 0x8000 -> -16384.
        assert_eq!(cell_height(0x7FF0), 16376);
        assert_eq!(cell_height(0x8000), -16384);
        assert_eq!(cell_height(0), 0);
    }
}
