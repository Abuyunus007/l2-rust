//! Errors of the geodata parser and loader.

use std::path::PathBuf;

/// Everything that can go wrong while parsing a `.l2j` region or loading a directory of them.
///
/// Java throws a `RuntimeException` (bad block type, bad layer count) or a buffer exception
/// (truncated file) and `GeoEngine` logs a warning per file. Trailing bytes are not an error in
/// Java; this port rejects them on purpose (the task card asks for an exact length).
#[derive(Debug, thiserror::Error)]
pub enum GeoError {
    /// `Region.load`: "Invalid block type N!".
    #[error("invalid block type {value} for block {block} at byte offset {offset}")]
    InvalidBlockType { block: usize, value: u8, offset: usize },
    /// `MultilayerBlock`: "Geo file corrupted! Invalid layers count!" (valid is 1..=125).
    #[error("invalid layer count {count} in block {block}, cell {cell} at byte offset {offset}")]
    InvalidLayerCount {
        block: usize,
        cell: usize,
        count: u8,
        offset: usize,
    },
    /// The data ends in the middle of a block.
    #[error("unexpected end of data at byte offset {offset} (needed {needed} more bytes)")]
    UnexpectedEof { offset: usize, needed: usize },
    /// All 65536 blocks were read but bytes remain.
    #[error("{count} trailing bytes after the last block (starting at byte offset {offset})")]
    TrailingBytes { count: usize, offset: usize },
    /// Reading a file or directory failed.
    #[error("cannot read {}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// A region file name is `X_Y.l2j` but X or Y is outside the 32 x 32 region grid.
    #[error("region {x}_{y} in {} is outside the 32 x 32 region grid", path.display())]
    RegionOutOfRange { path: PathBuf, x: u32, y: u32 },
    /// A region file failed to parse.
    #[error("{}: {source}", path.display())]
    File {
        path: PathBuf,
        #[source]
        source: Box<GeoError>,
    },
}
