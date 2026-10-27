//! Region model, the null region and the loaded set of regions.
//!
//! Java: `gameserver/geoengine/geodata/regions/Region.java`, `regions/NullRegion.java`,
//! `IRegion.java`, and the region table / file loading in `geoengine/GeoEngine.java`.
//!
//! A region is 256 x 256 blocks of 8 x 8 cells (2048 x 2048 cells). Blocks are stored in the file
//! in the order `bx * 256 + by`. All query coordinates are **global** geo cell coordinates, like in
//! Java (`regionX * 2048 + local`); a [`Region`] only looks at the part inside its own region.
//!
//! Memory layout (a region of the converted set must stay under ~3x its file size):
//! - one `u32` slot per block: the type in the top 2 bits and a payload in the low 30 bits
//!   (flat: the `i16` height; complex: index into `complex`; multilayer: index of the block);
//! - complex blocks: 64 `u16` words each in one shared vector;
//! - multilayer blocks: their raw bytes (count byte + words per cell, like Java's `_data`) in one
//!   shared byte buffer, plus a `u16` offset per cell so a query does not rescan the block.
//!
//! # Runtime edits and copy-on-write (L-07)
//! Java edits a region in place (`setNearestNswe`, `unsetNearestNswe`, `reloadRegion`) while other
//! threads read it. Here the path workers read the geodata without locks (D-14), so a region is
//! never changed while somebody else can see it: [`GeoSet`] keeps every region behind an
//! [`Arc`], and [`GeoSet::region_mut`] goes through [`Arc::make_mut`]. A region nobody else holds
//! is edited in place; a region a worker's snapshot still holds is copied first (only that one
//! region, the others stay shared). Cloning a [`GeoSet`] (and so a `GeoEngine`) is cheap: one
//! `Arc` per region.

use std::path::Path;
use std::sync::Arc;

use crate::block::{BLOCK_CELLS, BlockRef, BlockType, MAX_LAYERS, cell_index};
use crate::cell::{NSWE_ALL, cell_nswe};
use crate::error::GeoError;

/// Blocks on one side of a region (`IRegion.REGION_BLOCKS_X` / `_Y`).
pub const REGION_BLOCKS_SIDE: usize = 256;
/// Blocks in a region (`IRegion.REGION_BLOCKS`).
pub const REGION_BLOCKS: usize = REGION_BLOCKS_SIDE * REGION_BLOCKS_SIDE;
/// Cells on one side of a region (`IRegion.REGION_CELLS_X` / `_Y`).
pub const REGION_CELLS_SIDE: i32 = 2048;
/// Regions on one side of the region table (`GeoEngine.GEO_REGIONS_X` / `_Y`).
pub const GEO_REGIONS_SIDE: usize = 32;

const SLOT_TYPE_SHIFT: u32 = 30;
const SLOT_PAYLOAD_MASK: u32 = (1 << SLOT_TYPE_SHIFT) - 1;

/// `Region.getBlock`'s index: `((geoX / 8) % 256) * 256 + (geoY / 8) % 256`.
#[inline]
fn block_index(geo_x: i32, geo_y: i32) -> usize {
    let bx = ((geo_x >> 3) & 0xFF) as usize;
    let by = ((geo_y >> 3) & 0xFF) as usize;
    bx * REGION_BLOCKS_SIDE + by
}

/// Number of blocks of each type in a region.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BlockCounts {
    pub flat: u32,
    pub complex: u32,
    pub multilayer: u32,
}

/// One parsed `.l2j` region.
#[derive(Debug, Clone)]
pub struct Region {
    slots: Box<[u32]>,
    complex: Vec<u16>,
    multilayer_data: Vec<u8>,
    /// Start of each multilayer block inside `multilayer_data`.
    multilayer_starts: Vec<u32>,
    /// 64 entries per multilayer block: offset of each cell's count byte, relative to the block start.
    multilayer_cells: Vec<u16>,
}

/// Bounds-checked little-endian reader over the file bytes.
struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, len: usize) -> Result<&'a [u8], GeoError> {
        let remaining = self.bytes.len() - self.pos;
        if len > remaining {
            return Err(GeoError::UnexpectedEof {
                offset: self.pos,
                needed: len - remaining,
            });
        }
        let end = self.pos + len;
        let out = &self.bytes[self.pos..end];
        self.pos = end;
        Ok(out)
    }

    fn u8(&mut self) -> Result<u8, GeoError> {
        Ok(self.take(1)?[0])
    }
}

impl Region {
    /// Parses a whole `.l2j` file.
    ///
    /// Fails on an unknown block type, a layer count outside 1..=125, truncated data, or bytes left
    /// over after the last block. (Java skips the last check.)
    pub fn parse(bytes: &[u8]) -> Result<Region, GeoError> {
        let mut r = Reader { bytes, pos: 0 };
        let mut region = Region {
            slots: vec![0u32; REGION_BLOCKS].into_boxed_slice(),
            complex: Vec::new(),
            multilayer_data: Vec::new(),
            multilayer_starts: Vec::new(),
            multilayer_cells: Vec::new(),
        };

        for block in 0..REGION_BLOCKS {
            let type_offset = r.pos;
            let type_byte = r.u8()?;
            let block_type = BlockType::from_u8(type_byte).ok_or(GeoError::InvalidBlockType {
                block,
                value: type_byte,
                offset: type_offset,
            })?;
            let payload = match block_type {
                BlockType::Flat => {
                    let b = r.take(2)?;
                    u32::from(u16::from_le_bytes([b[0], b[1]]))
                }
                BlockType::Complex => {
                    let index = (region.complex.len() / BLOCK_CELLS) as u32;
                    let raw = r.take(BLOCK_CELLS * 2)?;
                    region
                        .complex
                        .extend(raw.chunks_exact(2).map(|w| u16::from_le_bytes([w[0], w[1]])));
                    index
                }
                BlockType::Multilayer => {
                    let index = region.multilayer_starts.len() as u32;
                    let start = r.pos;
                    let mut cell_offsets = [0u16; BLOCK_CELLS];
                    for (cell, slot) in cell_offsets.iter_mut().enumerate() {
                        let count_offset = r.pos;
                        let count = r.u8()?;
                        // Java reads the count as a signed byte and rejects <= 0 or > 125, which
                        // also rejects 128..=255.
                        if count == 0 || count > MAX_LAYERS {
                            return Err(GeoError::InvalidLayerCount {
                                block,
                                cell,
                                count,
                                offset: count_offset,
                            });
                        }
                        // At most 63 * (1 + 250) < 65536, so the offset fits in u16.
                        *slot = (count_offset - start) as u16;
                        r.take(usize::from(count) * 2)?;
                    }
                    region.multilayer_starts.push(region.multilayer_data.len() as u32);
                    region.multilayer_data.extend_from_slice(&bytes[start..r.pos]);
                    region.multilayer_cells.extend_from_slice(&cell_offsets);
                    index
                }
            };
            region.slots[block] = ((block_type as u32) << SLOT_TYPE_SHIFT) | payload;
        }

        if r.pos != bytes.len() {
            return Err(GeoError::TrailingBytes {
                count: bytes.len() - r.pos,
                offset: r.pos,
            });
        }
        region.complex.shrink_to_fit();
        region.multilayer_data.shrink_to_fit();
        region.multilayer_starts.shrink_to_fit();
        region.multilayer_cells.shrink_to_fit();
        Ok(region)
    }

    /// Reads and parses one file.
    pub fn load_file(path: &Path) -> Result<Region, GeoError> {
        let bytes = std::fs::read(path).map_err(|source| GeoError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        Region::parse(&bytes).map_err(|source| GeoError::File {
            path: path.to_path_buf(),
            source: Box::new(source),
        })
    }

    /// `Region.getBlock`: the block containing the cell, as a view.
    ///
    /// Uses `((geoX / 8) % 256) * 256 + (geoY / 8) % 256`, i.e. only the position inside this region.
    pub fn block(&self, geo_x: i32, geo_y: i32) -> BlockRef<'_> {
        self.block_at(block_index(geo_x, geo_y))
    }

    /// The block with the given index (`bx * 256 + by`, the file order).
    fn block_at(&self, index: usize) -> BlockRef<'_> {
        let slot = self.slots[index];
        let payload = (slot & SLOT_PAYLOAD_MASK) as usize;
        match slot >> SLOT_TYPE_SHIFT {
            0 => BlockRef::Flat {
                height: payload as u16 as i16,
            },
            1 => BlockRef::Complex {
                cells: &self.complex[payload * BLOCK_CELLS..(payload + 1) * BLOCK_CELLS],
            },
            _ => {
                let start = self.multilayer_starts[payload] as usize;
                let end = self
                    .multilayer_starts
                    .get(payload + 1)
                    .map_or(self.multilayer_data.len(), |&e| e as usize);
                BlockRef::Multilayer {
                    data: &self.multilayer_data[start..end],
                    cell_offsets: &self.multilayer_cells[payload * BLOCK_CELLS..(payload + 1) * BLOCK_CELLS],
                }
            }
        }
    }

    /// Block type (0 flat, 1 complex, 2 multilayer) of the block containing the cell.
    pub fn block_type(&self, geo_x: i32, geo_y: i32) -> BlockType {
        self.block(geo_x, geo_y).block_type()
    }

    /// `IRegion.getNearestZ`.
    pub fn nearest_z(&self, geo_x: i32, geo_y: i32, world_z: i32) -> i32 {
        self.block(geo_x, geo_y).nearest_z(geo_x, geo_y, world_z)
    }

    /// `IRegion.getNextLowerZ`.
    pub fn next_lower_z(&self, geo_x: i32, geo_y: i32, world_z: i32) -> i32 {
        self.block(geo_x, geo_y).next_lower_z(geo_x, geo_y, world_z)
    }

    /// `IRegion.getNextHigherZ`.
    pub fn next_higher_z(&self, geo_x: i32, geo_y: i32, world_z: i32) -> i32 {
        self.block(geo_x, geo_y).next_higher_z(geo_x, geo_y, world_z)
    }

    /// `IRegion.checkNearestNswe`: are all bits of `nswe` open on the cell (nearest layer)?
    pub fn check_nearest_nswe(&self, geo_x: i32, geo_y: i32, world_z: i32, nswe: u8) -> bool {
        self.block(geo_x, geo_y)
            .check_nearest_nswe(geo_x, geo_y, world_z, nswe)
    }

    /// `IBlock.getNearestNswe` of the containing block: the open directions of the cell.
    pub fn nearest_nswe(&self, geo_x: i32, geo_y: i32, world_z: i32) -> u8 {
        self.block(geo_x, geo_y).nearest_nswe(geo_x, geo_y, world_z)
    }

    /// `Region.setNearestNswe`: opens the directions `nswe` of the cell (nearest layer to
    /// `world_z` in a multilayer block).
    ///
    /// NOTE(java-quirk): `ComplexBlock` / `MultilayerBlock.setNearestNswe` only write when *none*
    /// of the bits of `nswe` is open yet (`(current & nswe) == 0`); asking for two directions of
    /// which one is open already changes nothing.
    pub fn set_nearest_nswe(&mut self, geo_x: i32, geo_y: i32, world_z: i32, nswe: u8) {
        // Flat block cells are enabled by default on all directions.
        // NOTE(java-quirk): Java has the `convertFlatToComplex` call commented out here, so a flat
        // block stays flat.
        if self.block(geo_x, geo_y).block_type() == BlockType::Flat {
            return;
        }
        self.edit_nearest_nswe(geo_x, geo_y, world_z, |current| {
            ((current & nswe) == 0).then_some(current | nswe)
        });
    }

    /// `Region.unsetNearestNswe`: closes the directions `nswe` of the cell. A flat block is turned
    /// into a complex block first (`convertFlatToComplex`).
    ///
    /// Writes when *any* bit of `nswe` is open (`(current & nswe) != 0`) and clears all of them.
    pub fn unset_nearest_nswe(&mut self, geo_x: i32, geo_y: i32, world_z: i32, nswe: u8) {
        let index = block_index(geo_x, geo_y);
        if let BlockRef::Flat { height } = self.block_at(index) {
            self.convert_flat_to_complex(index, height);
        }
        self.edit_nearest_nswe(geo_x, geo_y, world_z, |current| {
            ((current & nswe) != 0).then_some(current & !nswe)
        });
    }

    /// `Region.convertFlatToComplex`: 64 cells of the flat height, all directions open.
    ///
    /// NOTE(java-quirk): the height is shifted into the cell word without the `0xFFF0` mask and
    /// without a range check (`(short) (height << 1) | NSWE_ALL`), so a height that is not a
    /// multiple of 8 is rounded down to one (100 becomes 96, -7 becomes -8) and a height outside
    /// -16384..16383 wraps (20000 becomes -12768).
    fn convert_flat_to_complex(&mut self, index: usize, height: i16) {
        let word = ((i32::from(height) << 1) as u16) | u16::from(NSWE_ALL);
        let complex_index = (self.complex.len() / BLOCK_CELLS) as u32;
        self.complex.reserve_exact(BLOCK_CELLS);
        self.complex.extend(std::iter::repeat_n(word, BLOCK_CELLS));
        self.slots[index] = ((BlockType::Complex as u32) << SLOT_TYPE_SHIFT) | complex_index;
    }

    /// The shared body of `ComplexBlock` / `MultilayerBlock.setNearestNswe` and
    /// `unsetNearestNswe`: `change` gets the NSWE bits of the cell (or of the layer nearest to
    /// `world_z`) and returns the new bits, or `None` to leave the cell alone. The height bits stay
    /// as they are (Java re-encodes `(height << 1) | nswe`, which gives the same word).
    fn edit_nearest_nswe(&mut self, geo_x: i32, geo_y: i32, world_z: i32, change: impl Fn(u8) -> Option<u8>) {
        let index = block_index(geo_x, geo_y);
        let slot = self.slots[index];
        let payload = (slot & SLOT_PAYLOAD_MASK) as usize;
        let cell = cell_index(geo_x, geo_y);
        match BlockType::from_u8((slot >> SLOT_TYPE_SHIFT) as u8) {
            Some(BlockType::Complex) => {
                let word = &mut self.complex[payload * BLOCK_CELLS + cell];
                if let Some(nswe) = change(cell_nswe(*word)) {
                    *word = (*word & 0xFFF0) | u16::from(nswe & 0x0F);
                }
            }
            Some(BlockType::Multilayer) => {
                let Some(layer) = self.block_at(index).nearest_layer_index(geo_x, geo_y, world_z) else {
                    return;
                };
                let at = self.multilayer_starts[payload] as usize
                    + usize::from(self.multilayer_cells[payload * BLOCK_CELLS + cell])
                    + 1
                    + 2 * layer;
                let word = u16::from_le_bytes([self.multilayer_data[at], self.multilayer_data[at + 1]]);
                if let Some(nswe) = change(cell_nswe(word)) {
                    let word = (word & 0xFFF0) | u16::from(nswe & 0x0F);
                    self.multilayer_data[at..at + 2].copy_from_slice(&word.to_le_bytes());
                }
            }
            // Flat blocks have no cell words (`FlatBlock.setNearestNswe` throws; the region never
            // calls it).
            _ => {}
        }
    }

    /// The region in the `.l2j` layout `Region.saveToFile` writes: per block the type byte, then
    /// the flat height, the 64 complex words (both little-endian) or the multilayer bytes as
    /// loaded. An unedited region gives back the bytes it was parsed from.
    pub fn to_bytes(&self) -> Vec<u8> {
        let counts = self.block_counts();
        let mut out = Vec::with_capacity(
            counts.flat as usize * 3
                + counts.complex as usize * (1 + 2 * BLOCK_CELLS)
                + counts.multilayer as usize
                + self.multilayer_data.len(),
        );
        for index in 0..REGION_BLOCKS {
            let block = self.block_at(index);
            out.push(block.block_type() as u8);
            match block {
                BlockRef::Flat { height } => out.extend_from_slice(&height.to_le_bytes()),
                BlockRef::Complex { cells } => {
                    for word in cells {
                        out.extend_from_slice(&word.to_le_bytes());
                    }
                }
                BlockRef::Multilayer { data, .. } => out.extend_from_slice(data),
            }
        }
        out
    }

    /// `Region.saveToFile`: writes [`to_bytes`](Self::to_bytes) to `path`, deleting an existing
    /// file first like Java. The caller picks the path (Java: `GeoEditPath` + file name).
    pub fn save_to_file(&self, path: &Path) -> Result<(), GeoError> {
        let io_err = |source| GeoError::Io {
            path: path.to_path_buf(),
            source,
        };
        if path.exists() {
            std::fs::remove_file(path).map_err(io_err)?;
        }
        std::fs::write(path, self.to_bytes()).map_err(io_err)
    }

    /// Always true for a real region (`Region.hasGeo`).
    pub fn has_geo(&self) -> bool {
        true
    }

    /// How many blocks of each type the region has.
    pub fn block_counts(&self) -> BlockCounts {
        let multilayer = self.multilayer_starts.len() as u32;
        let complex = (self.complex.len() / BLOCK_CELLS) as u32;
        BlockCounts {
            flat: REGION_BLOCKS as u32 - complex - multilayer,
            complex,
            multilayer,
        }
    }

    /// Bytes of heap memory used by the region data (excluding the struct itself).
    pub fn heap_bytes(&self) -> usize {
        std::mem::size_of_val(&*self.slots)
            + self.complex.capacity() * 2
            + self.multilayer_data.capacity()
            + self.multilayer_starts.capacity() * 4
            + self.multilayer_cells.capacity() * 2
    }
}

/// `NullRegion`: what a missing region answers. No geodata, everything passable, Z unchanged.
#[derive(Clone, Copy, Debug, Default)]
pub struct NullRegion;

impl NullRegion {
    /// `NullRegion.checkNearestNswe`: always true.
    pub fn check_nearest_nswe(&self, _geo_x: i32, _geo_y: i32, _world_z: i32, _nswe: u8) -> bool {
        true
    }

    /// Open in all directions.
    pub fn nearest_nswe(&self, _geo_x: i32, _geo_y: i32, _world_z: i32) -> u8 {
        crate::cell::NSWE_ALL
    }

    /// `NullRegion.getNearestZ`: returns `world_z`.
    pub fn nearest_z(&self, _geo_x: i32, _geo_y: i32, world_z: i32) -> i32 {
        world_z
    }

    /// `NullRegion.getNextLowerZ`: returns `world_z`.
    pub fn next_lower_z(&self, _geo_x: i32, _geo_y: i32, world_z: i32) -> i32 {
        world_z
    }

    /// `NullRegion.getNextHigherZ`: returns `world_z`.
    pub fn next_higher_z(&self, _geo_x: i32, _geo_y: i32, world_z: i32) -> i32 {
        world_z
    }

    /// Always false (`NullRegion.hasGeo`).
    pub fn has_geo(&self) -> bool {
        false
    }
}

/// The 32 x 32 table of regions (`GeoEngine.REGIONS`), `None` where no file exists.
///
/// Region `(x, y)` covers geo cells `x * 2048 .. (x + 1) * 2048` on X and likewise on Y.
///
/// Every region sits behind an [`Arc`]: cloning the set shares the regions, and
/// [`region_mut`](Self::region_mut) copies a region only when a clone still holds it
/// (copy-on-write, see the module docs).
#[derive(Debug, Clone)]
pub struct GeoSet {
    regions: Vec<Option<Arc<Region>>>,
}

impl Default for GeoSet {
    fn default() -> Self {
        Self::new()
    }
}

impl GeoSet {
    /// An empty set: every query answers like [`NullRegion`].
    pub fn new() -> Self {
        GeoSet {
            regions: vec![None; GEO_REGIONS_SIDE * GEO_REGIONS_SIDE],
        }
    }

    /// Loads every `X_Y.l2j` file in `dir` (`GeoEngine.FILE_NAME_FORMAT`). Other names are ignored.
    ///
    /// A file that fails to parse fails the whole load (Java logs a warning and skips the file;
    /// the caller of a tool or server start should decide, so the error is returned).
    pub fn load_dir(dir: &Path) -> Result<GeoSet, GeoError> {
        let io_err = |source| GeoError::Io {
            path: dir.to_path_buf(),
            source,
        };
        let mut set = GeoSet::new();
        for entry in std::fs::read_dir(dir).map_err(io_err)? {
            let entry = entry.map_err(io_err)?;
            let path = entry.path();
            let Some((x, y)) = path
                .file_name()
                .and_then(|n| n.to_str())
                .and_then(parse_region_file_name)
            else {
                continue;
            };
            if x as usize >= GEO_REGIONS_SIDE || y as usize >= GEO_REGIONS_SIDE {
                return Err(GeoError::RegionOutOfRange { path, x, y });
            }
            let region = Region::load_file(&path)?;
            tracing::debug!("loaded geodata region {x}_{y}");
            set.regions[x as usize * GEO_REGIONS_SIDE + y as usize] = Some(Arc::new(region));
        }
        Ok(set)
    }

    /// Puts a region into the table. Returns false when `(x, y)` is outside the 32 x 32 grid.
    pub fn insert(&mut self, x: u32, y: u32, region: Region) -> bool {
        if x as usize >= GEO_REGIONS_SIDE || y as usize >= GEO_REGIONS_SIDE {
            return false;
        }
        self.regions[x as usize * GEO_REGIONS_SIDE + y as usize] = Some(Arc::new(region));
        true
    }

    /// The region with the given region coordinates, if loaded.
    pub fn region(&self, x: u32, y: u32) -> Option<&Region> {
        self.region_arc(x, y).map(|r| &**r)
    }

    /// The shared handle of a region: lets a caller keep a snapshot of one region (to save it
    /// later) or check whether two sets still share it ([`Arc::ptr_eq`]).
    pub fn region_arc(&self, x: u32, y: u32) -> Option<&Arc<Region>> {
        if x as usize >= GEO_REGIONS_SIDE || y as usize >= GEO_REGIONS_SIDE {
            return None;
        }
        self.regions[x as usize * GEO_REGIONS_SIDE + y as usize].as_ref()
    }

    /// The region to edit. Copies it first when another set (an engine snapshot a path worker
    /// still uses) shares it, so readers of the old set never see the change.
    pub fn region_mut(&mut self, x: u32, y: u32) -> Option<&mut Region> {
        if x as usize >= GEO_REGIONS_SIDE || y as usize >= GEO_REGIONS_SIDE {
            return None;
        }
        self.regions[x as usize * GEO_REGIONS_SIDE + y as usize]
            .as_mut()
            .map(Arc::make_mut)
    }

    /// The region containing a global geo cell (`GeoEngine.getRegion`), if loaded.
    ///
    /// Cells outside the table (negative or beyond 32 regions) have no region; Java would index
    /// out of bounds there.
    pub fn region_at(&self, geo_x: i32, geo_y: i32) -> Option<&Region> {
        let x = geo_x.div_euclid(REGION_CELLS_SIDE);
        let y = geo_y.div_euclid(REGION_CELLS_SIDE);
        if x < 0 || y < 0 {
            return None;
        }
        self.region(x as u32, y as u32)
    }

    /// Number of loaded regions.
    pub fn loaded_count(&self) -> usize {
        self.regions.iter().flatten().count()
    }

    /// `GeoEngine.hasGeoPos`.
    pub fn has_geo(&self, geo_x: i32, geo_y: i32) -> bool {
        self.region_at(geo_x, geo_y).is_some()
    }

    /// `GeoEngine.getNearestZ` on the table: [`NullRegion`] behaviour where no region is loaded.
    pub fn nearest_z(&self, geo_x: i32, geo_y: i32, world_z: i32) -> i32 {
        match self.region_at(geo_x, geo_y) {
            Some(r) => r.nearest_z(geo_x, geo_y, world_z),
            None => NullRegion.nearest_z(geo_x, geo_y, world_z),
        }
    }

    /// `GeoEngine.getNextLowerZ`.
    pub fn next_lower_z(&self, geo_x: i32, geo_y: i32, world_z: i32) -> i32 {
        match self.region_at(geo_x, geo_y) {
            Some(r) => r.next_lower_z(geo_x, geo_y, world_z),
            None => NullRegion.next_lower_z(geo_x, geo_y, world_z),
        }
    }

    /// `GeoEngine.getNextHigherZ`.
    pub fn next_higher_z(&self, geo_x: i32, geo_y: i32, world_z: i32) -> i32 {
        match self.region_at(geo_x, geo_y) {
            Some(r) => r.next_higher_z(geo_x, geo_y, world_z),
            None => NullRegion.next_higher_z(geo_x, geo_y, world_z),
        }
    }

    /// `GeoEngine.checkNearestNswe`.
    pub fn check_nearest_nswe(&self, geo_x: i32, geo_y: i32, world_z: i32, nswe: u8) -> bool {
        match self.region_at(geo_x, geo_y) {
            Some(r) => r.check_nearest_nswe(geo_x, geo_y, world_z, nswe),
            None => NullRegion.check_nearest_nswe(geo_x, geo_y, world_z, nswe),
        }
    }

    /// The open directions of the cell; all four where no region is loaded.
    pub fn nearest_nswe(&self, geo_x: i32, geo_y: i32, world_z: i32) -> u8 {
        match self.region_at(geo_x, geo_y) {
            Some(r) => r.nearest_nswe(geo_x, geo_y, world_z),
            None => NullRegion.nearest_nswe(geo_x, geo_y, world_z),
        }
    }
}

/// Parses `X_Y.l2j` (decimal, no sign, no extra characters).
fn parse_region_file_name(name: &str) -> Option<(u32, u32)> {
    let stem = name.strip_suffix(".l2j")?;
    let (x, y) = stem.split_once('_')?;
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    if !digits(x) || !digits(y) {
        return None;
    }
    Some((x.parse().ok()?, y.parse().ok()?))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::cell::{NSWE_ALL, NSWE_EAST, NSWE_NORTH, NSWE_WEST};

    /// Encodes a cell word (height must be a multiple of 8).
    fn word(height: i32, nswe: u8) -> u16 {
        (((height * 2) as i16) as u16 & 0xFFF0) | u16::from(nswe)
    }

    /// Builds file bytes: every block flat at `base`, except the ones in `special`
    /// (block index -> raw bytes including the type byte).
    fn build(base: i16, special: &[(usize, Vec<u8>)]) -> Vec<u8> {
        let mut out = Vec::new();
        for block in 0..REGION_BLOCKS {
            if let Some((_, raw)) = special.iter().find(|(i, _)| *i == block) {
                out.extend_from_slice(raw);
            } else {
                out.push(0);
                out.extend_from_slice(&base.to_le_bytes());
            }
        }
        out
    }

    fn complex_bytes(cells: &[u16; 64]) -> Vec<u8> {
        let mut v = vec![1];
        for c in cells {
            v.extend_from_slice(&c.to_le_bytes());
        }
        v
    }

    /// Multilayer block where cell `i` has the layers given by `f(i)`.
    fn multilayer_bytes(f: impl Fn(usize) -> Vec<u16>) -> Vec<u8> {
        let mut v = vec![2];
        for i in 0..64 {
            let layers = f(i);
            v.push(layers.len() as u8);
            for l in layers {
                v.extend_from_slice(&l.to_le_bytes());
            }
        }
        v
    }

    #[test]
    fn all_flat_region() {
        let bytes = build(-64, &[]);
        assert_eq!(bytes.len(), 196_608);
        let r = Region::parse(&bytes).unwrap();
        assert_eq!(
            r.block_counts(),
            BlockCounts {
                flat: 65536,
                complex: 0,
                multilayer: 0
            }
        );
        // Global coordinates of region (3, 5): the region offset is ignored.
        let (gx, gy) = (3 * 2048 + 100, 5 * 2048 + 2047);
        assert_eq!(r.block_type(gx, gy), BlockType::Flat);
        assert_eq!(r.nearest_z(gx, gy, 12345), -64);
        assert_eq!(r.next_lower_z(gx, gy, -100), -100);
        assert_eq!(r.next_higher_z(gx, gy, -100), -64);
        assert!(r.check_nearest_nswe(gx, gy, 0, NSWE_ALL));
        assert_eq!(r.nearest_nswe(gx, gy, 0), NSWE_ALL);
        assert!(r.has_geo());
        // All-flat: 3 bytes per block in the file, ~1.3x in memory.
        assert!(r.heap_bytes() < 3 * bytes.len());
    }

    #[test]
    fn block_order_is_bx_times_256_plus_by() {
        let mut cells = [word(8, NSWE_ALL); 64];
        cells[7 * 8 + 1] = word(80, NSWE_EAST);
        // Block bx = 2, by = 3 -> index 2 * 256 + 3. Cells (16..24, 24..32).
        let bytes = build(0, &[(2 * 256 + 3, complex_bytes(&cells))]);
        let r = Region::parse(&bytes).unwrap();
        assert_eq!(r.block_counts().complex, 1);
        assert_eq!(r.block_type(16, 24), BlockType::Complex);
        assert_eq!(r.block_type(24, 16), BlockType::Flat);
        assert_eq!(r.nearest_z(16 + 7, 24 + 1, 0), 80);
        assert_eq!(r.nearest_z(16, 24, 0), 8);
        assert!(r.check_nearest_nswe(16 + 7, 24 + 1, 0, NSWE_EAST));
        assert!(!r.check_nearest_nswe(16 + 7, 24 + 1, 0, NSWE_WEST));
        assert_eq!(r.nearest_nswe(16 + 7, 24 + 1, 0), NSWE_EAST);
        // The same local cell in another region of the global grid hits the same block.
        assert_eq!(r.nearest_z(11 * 2048 + 16 + 7, 4 * 2048 + 24 + 1, 0), 80);
    }

    #[test]
    fn multilayer_blocks_are_addressed_independently() {
        let a = multilayer_bytes(|i| vec![word(i as i32 * 8, 15), word(i as i32 * 8 + 400, NSWE_NORTH)]);
        let b = multilayer_bytes(|_| vec![word(-800, 15)]);
        let bytes = build(
            0,
            &[(0, a), (1, b), (65535, multilayer_bytes(|_| vec![word(8, 3)]))],
        );
        let r = Region::parse(&bytes).unwrap();
        assert_eq!(
            r.block_counts(),
            BlockCounts {
                flat: 65533,
                complex: 0,
                multilayer: 3
            }
        );
        // Block 0 is bx 0, by 0; cell (1, 2) is index 10 -> layers 80 and 480.
        assert_eq!(r.nearest_z(1, 2, 70), 80);
        assert_eq!(r.nearest_z(1, 2, 400), 480);
        assert_eq!(r.nearest_nswe(1, 2, 400), NSWE_NORTH);
        assert_eq!(r.next_lower_z(1, 2, 400), 80);
        assert_eq!(r.next_higher_z(1, 2, 400), 480);
        // Block 1 is bx 0, by 1 (cells y 8..16).
        assert_eq!(r.nearest_z(0, 8, 0), -800);
        assert_eq!(r.block_type(0, 8), BlockType::Multilayer);
        // Last block (bx 255, by 255): the last multilayer block ends exactly at the data end.
        assert_eq!(r.nearest_z(2047, 2047, 0), 8);
        assert_eq!(r.nearest_nswe(2047, 2047, 0), 3);
    }

    #[test]
    fn rejects_bad_block_type() {
        let mut bytes = build(0, &[]);
        bytes[3 * 5] = 3; // type byte of block 5
        assert!(matches!(
            Region::parse(&bytes),
            Err(GeoError::InvalidBlockType {
                block: 5,
                value: 3,
                offset: 15
            })
        ));
    }

    #[test]
    fn rejects_bad_layer_counts() {
        for bad in [0u8, 126, 128, 255] {
            let mut block = multilayer_bytes(|_| vec![word(0, 15)]);
            block[1 + 3 * 10] = bad; // count byte of cell 10
            let bytes = build(0, &[(0, block)]);
            assert!(
                matches!(
                    Region::parse(&bytes),
                    Err(GeoError::InvalidLayerCount { block: 0, cell: 10, count, .. }) if count == bad
                ),
                "count {bad}"
            );
        }
        // 125 and 1 are fine.
        let many = multilayer_bytes(|i| vec![word(0, 15); if i == 0 { 125 } else { 1 }]);
        assert!(Region::parse(&build(0, &[(0, many)])).is_ok());
    }

    #[test]
    fn rejects_truncated_and_trailing_data() {
        let bytes = build(0, &[]);
        assert!(matches!(
            Region::parse(&bytes[..bytes.len() - 1]),
            Err(GeoError::UnexpectedEof { .. })
        ));
        assert!(matches!(Region::parse(&[]), Err(GeoError::UnexpectedEof { .. })));
        let mut long = bytes.clone();
        long.push(0);
        assert!(matches!(
            Region::parse(&long),
            Err(GeoError::TrailingBytes {
                count: 1,
                offset: 196_608
            })
        ));
        // A multilayer block cut off inside its layer data.
        let ml = multilayer_bytes(|_| vec![word(0, 15), word(8, 15)]);
        let mut cut = build(0, &[(65535, ml)]);
        cut.truncate(cut.len() - 1);
        assert!(matches!(Region::parse(&cut), Err(GeoError::UnexpectedEof { .. })));
    }

    #[test]
    fn multilayer_memory_stays_under_three_times_the_file() {
        // Worst realistic case: every block multilayer with one layer per cell.
        let one = multilayer_bytes(|_| vec![word(0, 15)]);
        let mut bytes = Vec::new();
        for _ in 0..REGION_BLOCKS {
            bytes.extend_from_slice(&one);
        }
        let r = Region::parse(&bytes).unwrap();
        assert!(
            r.heap_bytes() < 3 * bytes.len(),
            "{} vs {}",
            r.heap_bytes(),
            bytes.len()
        );
    }

    #[test]
    fn null_region_and_empty_set() {
        let n = NullRegion;
        assert!(n.check_nearest_nswe(1, 2, 3, NSWE_ALL));
        assert_eq!(n.nearest_nswe(1, 2, 3), NSWE_ALL);
        assert_eq!(n.nearest_z(1, 2, 777), 777);
        assert_eq!(n.next_lower_z(1, 2, 777), 777);
        assert_eq!(n.next_higher_z(1, 2, 777), 777);
        assert!(!n.has_geo());

        let set = GeoSet::new();
        assert_eq!(set.loaded_count(), 0);
        assert!(!set.has_geo(100, 100));
        assert_eq!(set.nearest_z(100, 100, -5), -5);
        assert!(set.check_nearest_nswe(100, 100, 0, NSWE_ALL));
        // Outside the table, including negative cells: no panic, null behaviour.
        assert_eq!(set.nearest_z(-1, 5, 9), 9);
        assert_eq!(set.nearest_z(40 * 2048, 5, 9), 9);
    }

    #[test]
    fn geo_set_dispatches_by_region_and_loads_a_directory() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("20_18.l2j"), build(16, &[])).unwrap();
        std::fs::write(dir.path().join("21_18.l2j"), build(-16, &[])).unwrap();
        // Ignored names.
        std::fs::write(dir.path().join("readme.txt"), b"x").unwrap();
        std::fs::write(dir.path().join("20_18.l2j.bak"), b"x").unwrap();
        std::fs::write(dir.path().join("a_b.l2j"), b"x").unwrap();
        std::fs::write(dir.path().join("-1_5.l2j"), b"x").unwrap();

        let set = GeoSet::load_dir(dir.path()).unwrap();
        assert_eq!(set.loaded_count(), 2);
        assert!(set.region(20, 18).is_some());
        assert!(set.region(18, 20).is_none());
        assert_eq!(set.nearest_z(20 * 2048 + 5, 18 * 2048 + 5, 0), 16);
        assert_eq!(set.nearest_z(21 * 2048, 18 * 2048 + 2047, 0), -16);
        assert!(set.has_geo(21 * 2048, 18 * 2048));
        // Neighbouring region without a file.
        assert!(!set.has_geo(22 * 2048, 18 * 2048));
        assert_eq!(set.nearest_z(22 * 2048, 18 * 2048, 33), 33);
    }

    #[test]
    fn load_dir_reports_bad_files_and_out_of_range_regions() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("20_18.l2j"), b"short").unwrap();
        let err = GeoSet::load_dir(dir.path()).unwrap_err();
        assert!(matches!(err, GeoError::File { .. }), "{err}");
        assert!(err.to_string().contains("20_18.l2j"));

        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("32_0.l2j"), build(0, &[])).unwrap();
        assert!(matches!(
            GeoSet::load_dir(dir.path()),
            Err(GeoError::RegionOutOfRange { x: 32, y: 0, .. })
        ));

        let missing = dir.path().join("nope");
        assert!(matches!(GeoSet::load_dir(&missing), Err(GeoError::Io { .. })));
    }

    #[test]
    fn region_file_names() {
        assert_eq!(parse_region_file_name("20_18.l2j"), Some((20, 18)));
        assert_eq!(parse_region_file_name("0_0.l2j"), Some((0, 0)));
        assert_eq!(parse_region_file_name("20_18.L2J"), None);
        assert_eq!(parse_region_file_name("20_18"), None);
        assert_eq!(parse_region_file_name("_18.l2j"), None);
        assert_eq!(parse_region_file_name("+20_18.l2j"), None);
        assert_eq!(parse_region_file_name("20_18_1.l2j"), None);
    }
}
