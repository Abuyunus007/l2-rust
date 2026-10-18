//! `l2-geotool stats`: block counts and average layers per region, plus the per-cell walk that
//! the other subcommands share.
//!
//! Java: none. The block counts are `Region::block_counts`. The layer counts follow
//! `tools/analysis/geocmp.py`: one layer per flat or complex cell, `n` per multilayer cell.

use std::path::{Path, PathBuf};

use anyhow::Context;
use l2_geo::block::{BLOCK_CELLS, BLOCK_CELLS_SIDE, BlockRef};
use l2_geo::cell::{NSWE_ALL, cell_height, cell_nswe};
use l2_geo::region::{REGION_BLOCKS_SIDE, REGION_CELLS_SIDE, Region};

/// Cells on one side of a region (2048).
pub(crate) const CELLS_SIDE: usize = REGION_CELLS_SIDE as usize;
/// Cells in a region (2048 x 2048 = 4194304).
pub(crate) const CELLS: usize = CELLS_SIDE * CELLS_SIDE;

/// One layer of a cell: its world Z and its NSWE bits (a flat block is open in all directions).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Layer {
    pub(crate) z: i32,
    pub(crate) nswe: u8,
}

impl Layer {
    fn from_word(word: u16) -> Self {
        Layer {
            z: cell_height(word),
            nswe: cell_nswe(word),
        }
    }
}

/// The top layer of a cell: the highest Z. On a tie the first one in the file wins, like
/// `max()` in `geocmp.py`. `None` only for an empty slice.
pub(crate) fn top(layers: &[Layer]) -> Option<Layer> {
    let mut iter = layers.iter().copied();
    let mut best = iter.next()?;
    for layer in iter {
        if layer.z > best.z {
            best = layer;
        }
    }
    Some(best)
}

/// Fills `out` with the layers of one cell (`cell` = `cx * 8 + cy` inside the block).
fn fill_layers(block: BlockRef<'_>, cell: usize, out: &mut Vec<Layer>) {
    out.clear();
    match block {
        BlockRef::Flat { height } => out.push(Layer {
            z: i32::from(height),
            nswe: NSWE_ALL,
        }),
        BlockRef::Complex { cells } => out.extend(cells.get(cell).map(|&w| Layer::from_word(w))),
        BlockRef::Multilayer { data, cell_offsets } => {
            let Some(&offset) = cell_offsets.get(cell) else {
                return;
            };
            let offset = usize::from(offset);
            let count = data.get(offset).map_or(0, |&n| usize::from(n));
            let words = data.get(offset + 1..offset + 1 + 2 * count).unwrap_or(&[]);
            out.extend(
                words
                    .chunks_exact(2)
                    .map(|w| Layer::from_word(u16::from_le_bytes([w[0], w[1]]))),
            );
        }
    }
}

/// Calls `f(index, layers_a, layers_b)` for every cell of `a` exactly once. `index` is
/// `x * 2048 + y` (x and y inside the region); the walk itself goes block by block, so indices
/// are not visited in order. With `other`, `layers_b` holds the same cell of `other`; both
/// regions have the same block grid, so the walk runs in lockstep. Without it `layers_b` is empty.
pub(crate) fn walk_cells(a: &Region, other: Option<&Region>, mut f: impl FnMut(usize, &[Layer], &[Layer])) {
    let mut layers_a = Vec::with_capacity(8);
    let mut layers_b = Vec::with_capacity(8);
    for bx in 0..REGION_BLOCKS_SIDE {
        for by in 0..REGION_BLOCKS_SIDE {
            let (gx, gy) = ((bx * BLOCK_CELLS_SIDE) as i32, (by * BLOCK_CELLS_SIDE) as i32);
            let block_a = a.block(gx, gy);
            let block_b = other.map(|r| r.block(gx, gy));
            for cell in 0..BLOCK_CELLS {
                let x = bx * BLOCK_CELLS_SIDE + cell / BLOCK_CELLS_SIDE;
                let y = by * BLOCK_CELLS_SIDE + cell % BLOCK_CELLS_SIDE;
                let index = x * CELLS_SIDE + y;
                fill_layers(block_a, cell, &mut layers_a);
                match block_b {
                    Some(block) => {
                        fill_layers(block, cell, &mut layers_b);
                        f(index, &layers_a, &layers_b);
                    }
                    None => f(index, &layers_a, &[]),
                }
            }
        }
    }
}

/// The region names (file stems) of every `.l2j` file in `dir`, in name order.
pub(crate) fn region_names(dir: &Path) -> anyhow::Result<Vec<String>> {
    let entries = std::fs::read_dir(dir).with_context(|| format!("cannot read {}", dir.display()))?;
    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .filter_map(|e| e.file_name().into_string().ok())
        .filter_map(|n| n.strip_suffix(".l2j").map(str::to_owned))
        .collect();
    names.sort();
    Ok(names)
}

/// Path of region `name` (`12_24`) in `dir`.
pub(crate) fn region_path(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("{name}.l2j"))
}

/// `12_24.l2j` -> `12_24`; a plain name is returned unchanged.
pub(crate) fn region_stem(arg: &str) -> &str {
    arg.strip_suffix(".l2j").unwrap_or(arg)
}

/// Header of the stats CSV.
pub(crate) const STATS_HEADER: &str = "region,flat,complex,multilayer,avg_layers";

/// One CSV row of the stats table.
pub(crate) fn stats_row(name: &str, region: &Region) -> String {
    let counts = region.block_counts();
    let mut layer_total = 0u64;
    walk_cells(region, None, |_, layers, _| layer_total += layers.len() as u64);
    let avg_layers = layer_total as f64 / CELLS as f64;
    format!(
        "{name},{},{},{},{avg_layers:.3}",
        counts.flat, counts.complex, counts.multilayer
    )
}

/// The whole stats CSV for a directory (header plus one row per region).
pub fn run(dir: &Path) -> anyhow::Result<String> {
    let names = region_names(dir)?;
    let rows = crate::diff::par_map(names.as_slice(), |name: &String| -> anyhow::Result<String> {
        let region = Region::load_file(&region_path(dir, name))?;
        Ok(stats_row(name, &region))
    });
    let mut out = String::from(STATS_HEADER);
    out.push('\n');
    for (name, row) in names.iter().zip(rows) {
        out.push_str(&row.with_context(|| format!("region {name}"))?);
        out.push('\n');
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    /// File bytes of an all-flat region at height `base`.
    fn flat_region_bytes(base: i16) -> Vec<u8> {
        let mut out = Vec::new();
        for _ in 0..REGION_BLOCKS_SIDE * REGION_BLOCKS_SIDE {
            out.push(0);
            out.extend_from_slice(&base.to_le_bytes());
        }
        out
    }

    #[test]
    fn flat_region_has_one_layer_per_cell() {
        let region = Region::parse(&flat_region_bytes(-64)).unwrap();
        assert_eq!(stats_row("0_0", &region), "0_0,65536,0,0,1.000");
        let mut visited = vec![false; CELLS];
        walk_cells(&region, None, |index, layers, other| {
            assert!(!visited[index], "cell {index} visited twice");
            visited[index] = true;
            assert_eq!(
                layers,
                &[Layer {
                    z: -64,
                    nswe: NSWE_ALL
                }]
            );
            assert!(other.is_empty());
        });
        assert!(visited.iter().all(|&v| v));
    }

    #[test]
    fn region_names_are_sorted_and_stripped() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["20_18.l2j", "11_23.l2j", "geo_index.txt", "12_24.l2j.bak"] {
            std::fs::write(dir.path().join(name), b"x").unwrap();
        }
        assert_eq!(region_names(dir.path()).unwrap(), ["11_23", "20_18"]);
        assert_eq!(region_stem("12_24.l2j"), "12_24");
        assert_eq!(region_stem("12_24"), "12_24");
    }
}
