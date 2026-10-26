//! Block views and their queries.
//!
//! Java: `gameserver/geoengine/geodata/IBlock.java`, `blocks/FlatBlock.java`,
//! `blocks/ComplexBlock.java`, `blocks/MultilayerBlock.java`.
//!
//! A block is 8 x 8 cells. The data lives inside [`crate::Region`] in compact arrays; a
//! [`BlockRef`] is a cheap borrowed view that implements the per-block queries with Java semantics.
//! All coordinates are **global** geo cell coordinates, like in Java; only their low 3 bits
//! (the cell inside the block) are used here.

use crate::cell::{NSWE_ALL, NSWE_EAST, NSWE_NORTH, NSWE_SOUTH, NSWE_WEST, cell_height, cell_nswe};

/// Cells on one side of a block (`IBlock.BLOCK_CELLS_X` / `BLOCK_CELLS_Y`).
pub const BLOCK_CELLS_SIDE: usize = 8;
/// Cells in a block (`IBlock.BLOCK_CELLS`).
pub const BLOCK_CELLS: usize = BLOCK_CELLS_SIDE * BLOCK_CELLS_SIDE;
/// Largest layer count per cell Java accepts when loading.
pub const MAX_LAYERS: u8 = 125;

/// Block type byte in the file (`IBlock.TYPE_*`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum BlockType {
    /// `TYPE_FLAT`: one height, all directions open.
    Flat = 0,
    /// `TYPE_COMPLEX`: 64 cells, one layer each.
    Complex = 1,
    /// `TYPE_MULTILAYER`: 64 cells, 1..=125 layers each.
    Multilayer = 2,
}

impl BlockType {
    /// Decodes the type byte, `None` for anything but 0, 1, 2.
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Flat),
            1 => Some(Self::Complex),
            2 => Some(Self::Multilayer),
            _ => None,
        }
    }
}

/// Index of a cell inside its block: `(geoX % 8) * 8 + (geoY % 8)`.
#[inline]
pub(crate) fn cell_index(geo_x: i32, geo_y: i32) -> usize {
    (((geo_x & 7) << 3) | (geo_y & 7)) as usize
}

/// A borrowed view of one block.
#[derive(Clone, Copy, Debug)]
pub enum BlockRef<'a> {
    /// `FlatBlock`: the stored height is used as is (it is not in the cell-word encoding).
    Flat { height: i16 },
    /// `ComplexBlock`: 64 cell words, index `cx * 8 + cy`.
    Complex { cells: &'a [u16] },
    /// `MultilayerBlock`: `data` is the block's bytes (`u8 n`, then `n` LE words, 64 times, no
    /// type byte) and `cell_offsets[i]` is the offset of cell `i`'s count byte inside `data`.
    /// Java rescans from the start for every query; the offsets are the precomputed result.
    Multilayer { data: &'a [u8], cell_offsets: &'a [u16] },
}

impl BlockRef<'_> {
    /// The block's type.
    pub fn block_type(&self) -> BlockType {
        match self {
            Self::Flat { .. } => BlockType::Flat,
            Self::Complex { .. } => BlockType::Complex,
            Self::Multilayer { .. } => BlockType::Multilayer,
        }
    }

    /// `IBlock.checkNearestNswe`: are all bits of `nswe` set on the cell (or, for multilayer, on
    /// the layer nearest to `world_z`)?
    pub fn check_nearest_nswe(&self, geo_x: i32, geo_y: i32, world_z: i32, nswe: u8) -> bool {
        match self {
            Self::Flat { .. } => true,
            Self::Complex { cells } => (cell_nswe(cells[cell_index(geo_x, geo_y)]) & nswe) == nswe,
            Self::Multilayer { .. } => {
                let layer = self.nearest_layer(geo_x, geo_y, world_z);
                (cell_nswe(layer) & nswe) == nswe
            }
        }
    }

    /// `IBlock.getNearestNswe`: the NSWE bits of the cell / nearest layer.
    ///
    /// Java builds it from four `checkNearestNswe` calls; for a cell that equals the low nibble.
    pub fn nearest_nswe(&self, geo_x: i32, geo_y: i32, world_z: i32) -> u8 {
        match self {
            Self::Flat { .. } => NSWE_ALL,
            _ => [NSWE_NORTH, NSWE_EAST, NSWE_SOUTH, NSWE_WEST]
                .into_iter()
                .filter(|&dir| self.check_nearest_nswe(geo_x, geo_y, world_z, dir))
                .fold(0, |acc, dir| acc | dir),
        }
    }

    /// `IBlock.getNearestZ`.
    pub fn nearest_z(&self, geo_x: i32, geo_y: i32, world_z: i32) -> i32 {
        match self {
            Self::Flat { height } => i32::from(*height),
            Self::Complex { cells } => cell_height(cells[cell_index(geo_x, geo_y)]),
            Self::Multilayer { .. } => {
                let nearest = cell_height(self.nearest_layer(geo_x, geo_y, world_z));
                // NOTE(java-quirk): a nearest layer more than 1000 above world_z is treated as
                // "a different floor" and replaced by the next lower layer, so for multilayer
                // cells this is not always the closest layer.
                if nearest - world_z > 1000 {
                    return self.next_lower_z(geo_x, geo_y, world_z);
                }
                nearest
            }
        }
    }

    /// `IBlock.getNextLowerZ`: the highest layer at or below `world_z`, else `world_z`.
    pub fn next_lower_z(&self, geo_x: i32, geo_y: i32, world_z: i32) -> i32 {
        match self {
            Self::Flat { height } => {
                let height = i32::from(*height);
                if height <= world_z { height } else { world_z }
            }
            Self::Complex { cells } => {
                let height = cell_height(cells[cell_index(geo_x, geo_y)]);
                if height <= world_z { height } else { world_z }
            }
            Self::Multilayer { .. } => {
                let mut lower = i32::MIN;
                for layer in self.layers(geo_x, geo_y) {
                    let z = cell_height(layer);
                    if z == world_z {
                        return z;
                    }
                    if z < world_z && z > lower {
                        lower = z;
                    }
                }
                if lower == i32::MIN { world_z } else { lower }
            }
        }
    }

    /// `IBlock.getNextHigherZ`: the lowest layer at or above `world_z`, else `world_z`.
    pub fn next_higher_z(&self, geo_x: i32, geo_y: i32, world_z: i32) -> i32 {
        match self {
            Self::Flat { height } => {
                let height = i32::from(*height);
                if height >= world_z { height } else { world_z }
            }
            Self::Complex { cells } => {
                let height = cell_height(cells[cell_index(geo_x, geo_y)]);
                if height >= world_z { height } else { world_z }
            }
            Self::Multilayer { .. } => {
                let mut higher = i32::MAX;
                for layer in self.layers(geo_x, geo_y) {
                    let z = cell_height(layer);
                    if z == world_z {
                        return z;
                    }
                    if z > world_z && z < higher {
                        higher = z;
                    }
                }
                if higher == i32::MAX { world_z } else { higher }
            }
        }
    }

    /// The raw layer words of a multilayer cell, in file order. Empty for other block types.
    fn layers(&self, geo_x: i32, geo_y: i32) -> impl Iterator<Item = u16> + '_ {
        let (data, offset) = match self {
            Self::Multilayer { data, cell_offsets } => {
                (*data, usize::from(cell_offsets[cell_index(geo_x, geo_y)]))
            }
            _ => (&[][..], 0),
        };
        let count = data.get(offset).map_or(0, |&n| usize::from(n));
        // The parser guarantees `offset + 1 + 2 * count <= data.len()`.
        data.get(offset + 1..offset + 1 + 2 * count)
            .unwrap_or(&[])
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
    }

    /// `MultilayerBlock.getNearestLayer`: the layer word whose height is closest to `world_z`.
    ///
    /// An exact match wins immediately; on equal distance the layer that comes first in the file
    /// wins (strict `<`). Only valid for multilayer blocks (returns 0 otherwise).
    fn nearest_layer(&self, geo_x: i32, geo_y: i32, world_z: i32) -> u16 {
        self.nearest_layer_index(geo_x, geo_y, world_z)
            .and_then(|i| self.layers(geo_x, geo_y).nth(i))
            .unwrap_or(0)
    }

    /// Which layer of the cell [`nearest_layer`](Self::nearest_layer) picks (0 = first in the
    /// file). `None` for a block that is not multilayer.
    ///
    /// Also the layer `MultilayerBlock.setNearestNswe` / `unsetNearestNswe` edit: their search loop
    /// is the same (exact match first, then the first of the nearest).
    pub(crate) fn nearest_layer_index(&self, geo_x: i32, geo_y: i32, world_z: i32) -> Option<usize> {
        if !matches!(self, Self::Multilayer { .. }) {
            return None;
        }
        let mut nearest_dz = 0;
        let mut nearest = None;
        for (i, layer) in self.layers(geo_x, geo_y).enumerate() {
            let z = cell_height(layer);
            if z == world_z {
                return Some(i);
            }
            let dz = (z - world_z).abs();
            if i == 0 || dz < nearest_dz {
                nearest_dz = dz;
                nearest = Some(i);
            }
        }
        nearest
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Encodes a layer word: height doubled in bits 4..15, plus NSWE. Heights must be multiples
    /// of 8 (the real data's granularity), otherwise the low bits collide with the NSWE nibble.
    fn word(height: i32, nswe: u8) -> u16 {
        (((height * 2) as i16) as u16 & 0xFFF0) | u16::from(nswe)
    }

    /// A multilayer block where every cell has the same layers. Returns (data, offsets).
    fn uniform_multilayer(layers: &[u16]) -> (Vec<u8>, Vec<u16>) {
        let mut data = Vec::new();
        let mut offsets = Vec::new();
        for _ in 0..BLOCK_CELLS {
            offsets.push(data.len() as u16);
            data.push(layers.len() as u8);
            for l in layers {
                data.extend_from_slice(&l.to_le_bytes());
            }
        }
        (data, offsets)
    }

    #[test]
    fn flat_block() {
        let b = BlockRef::Flat { height: -120 };
        assert_eq!(b.block_type(), BlockType::Flat);
        assert!(b.check_nearest_nswe(0, 0, 5000, NSWE_ALL));
        assert_eq!(b.nearest_nswe(3, 3, 0), NSWE_ALL);
        assert_eq!(b.nearest_z(0, 0, 999), -120);
        assert_eq!(b.next_lower_z(0, 0, -100), -120);
        assert_eq!(b.next_lower_z(0, 0, -200), -200);
        assert_eq!(b.next_higher_z(0, 0, -200), -120);
        assert_eq!(b.next_higher_z(0, 0, -100), -100);
    }

    #[test]
    fn complex_block_uses_cell_index_x_major() {
        let mut cells = [word(0, NSWE_ALL); BLOCK_CELLS];
        // Cell (cx 2, cy 5) -> index 21. Global coordinates 8 * k + (2, 5) hit the same cell.
        cells[21] = word(-296, NSWE_EAST | NSWE_NORTH);
        let b = BlockRef::Complex { cells: &cells };
        assert_eq!(b.nearest_z(8 * 77 + 2, 8 * 13 + 5, 0), -296);
        assert_eq!(b.nearest_z(8 * 77 + 5, 8 * 13 + 2, 0), 0);
        assert!(b.check_nearest_nswe(2, 5, 0, NSWE_EAST));
        assert!(b.check_nearest_nswe(2, 5, 0, NSWE_EAST | NSWE_NORTH));
        assert!(!b.check_nearest_nswe(2, 5, 0, NSWE_WEST));
        assert!(!b.check_nearest_nswe(2, 5, 0, NSWE_EAST | NSWE_WEST));
        assert_eq!(b.nearest_nswe(2, 5, 0), NSWE_EAST | NSWE_NORTH);
        assert_eq!(b.next_lower_z(2, 5, -100), -296);
        assert_eq!(b.next_lower_z(2, 5, -400), -400);
        assert_eq!(b.next_higher_z(2, 5, -400), -296);
        assert_eq!(b.next_higher_z(2, 5, 100), 100);
    }

    #[test]
    fn multilayer_nearest_exact_tie_and_first_wins() {
        let (data, offsets) = uniform_multilayer(&[
            word(0, 0b0001),
            word(96, 0b0010),
            word(200, 0b0100),
            word(-96, 0b1000),
        ]);
        let b = BlockRef::Multilayer {
            data: &data,
            cell_offsets: &offsets,
        };
        // Exact match.
        assert_eq!(b.nearest_z(0, 0, 96), 96);
        assert!(b.check_nearest_nswe(0, 0, 96, 0b0010));
        assert!(!b.check_nearest_nswe(0, 0, 96, 0b0001));
        // Closest layer.
        assert_eq!(b.nearest_z(0, 0, 160), 200);
        assert_eq!(b.nearest_nswe(0, 0, 160), 0b0100);
        // Tie between 0 and 96 at 48: layer 0 comes first in the file and wins.
        assert_eq!(b.nearest_z(0, 0, 48), 0);
        assert_eq!(b.nearest_nswe(0, 0, 48), 0b0001);
        // Tie between 0 and -96 at -48: layer 0 comes first (-96 is last in the file).
        assert_eq!(b.nearest_z(0, 0, -48), 0);
        assert_eq!(b.nearest_nswe(0, 0, -48), 0b0001);
    }

    #[test]
    fn multilayer_next_lower_and_higher() {
        let (data, offsets) = uniform_multilayer(&[word(0, 15), word(96, 15), word(200, 15), word(-96, 15)]);
        let b = BlockRef::Multilayer {
            data: &data,
            cell_offsets: &offsets,
        };
        assert_eq!(b.next_lower_z(1, 1, 150), 96);
        assert_eq!(b.next_lower_z(1, 1, 96), 96);
        assert_eq!(b.next_lower_z(1, 1, -500), -500);
        assert_eq!(b.next_higher_z(1, 1, 150), 200);
        assert_eq!(b.next_higher_z(1, 1, 200), 200);
        assert_eq!(b.next_higher_z(1, 1, 500), 500);
        assert_eq!(b.next_higher_z(1, 1, -150), -96);
    }

    #[test]
    fn multilayer_nearest_z_falls_back_when_more_than_1000_above() {
        // Layers at 0 and 3000. From z = 2100 the nearest is 3000, 900 above: kept. From z = 1900
        // the nearest is still 3000 but 1100 above, so the next lower layer (0) is returned.
        let (data, offsets) = uniform_multilayer(&[word(0, 15), word(3000, 15)]);
        let b = BlockRef::Multilayer {
            data: &data,
            cell_offsets: &offsets,
        };
        assert_eq!(b.nearest_z(0, 0, 2100), 3000);
        assert_eq!(b.nearest_z(0, 0, 1900), 0);
        // Exactly 1000 above is not "more than 1000".
        assert_eq!(b.nearest_z(0, 0, 2000), 3000);
        // No lower layer at all: next_lower_z returns world_z itself.
        let (data, offsets) = uniform_multilayer(&[word(3000, 15)]);
        let b = BlockRef::Multilayer {
            data: &data,
            cell_offsets: &offsets,
        };
        assert_eq!(b.nearest_z(0, 0, 1000), 1000);
    }

    #[test]
    fn multilayer_cells_have_independent_layers() {
        let mut data = Vec::new();
        let mut offsets = Vec::new();
        for cell in 0..BLOCK_CELLS {
            offsets.push(data.len() as u16);
            let layers: Vec<u16> = (0..=(cell % 3))
                .map(|i| word((cell * 8 + i * 40) as i32, 15))
                .collect();
            data.push(layers.len() as u8);
            for l in layers {
                data.extend_from_slice(&l.to_le_bytes());
            }
        }
        let b = BlockRef::Multilayer {
            data: &data,
            cell_offsets: &offsets,
        };
        // Cell 63 (cx 7, cy 7): 1 layer at 504. Cell 62: 3 layers 496, 536, 576.
        assert_eq!(b.nearest_z(7, 7, 0), 504);
        assert_eq!(b.next_higher_z(7, 6, 520), 536);
        assert_eq!(b.next_lower_z(7, 6, 520), 496);
    }

    #[test]
    fn block_type_from_u8() {
        assert_eq!(BlockType::from_u8(0), Some(BlockType::Flat));
        assert_eq!(BlockType::from_u8(1), Some(BlockType::Complex));
        assert_eq!(BlockType::from_u8(2), Some(BlockType::Multilayer));
        assert_eq!(BlockType::from_u8(3), None);
    }
}
