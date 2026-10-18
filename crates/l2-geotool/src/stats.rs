//! `l2-geotool stats`: block counts and average layers per region, plus the region file helpers
//! the other subcommands share.
//!
//! Java: none. The block counts are `Region::block_counts`. The layer counts follow
//! `tools/analysis/geocmp.py` (see `l2_geo::view`).

use std::path::{Path, PathBuf};

use anyhow::Context;
use l2_geo::Region;
use l2_geo::view::{CELLS, walk_cells};

/// `12_24.l2j` -> `12_24`; a plain name is returned unchanged.
pub(crate) fn region_stem(arg: &str) -> &str {
    arg.strip_suffix(".l2j").unwrap_or(arg)
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

/// Header of the stats CSV.
pub(crate) const STATS_HEADER: &str = "region,flat,complex,multilayer,avg_layers";

/// One CSV row of the stats table.
pub(crate) fn stats_row(name: &str, region: &Region) -> String {
    let counts = region.block_counts();
    let mut layer_total = 0u64;
    walk_cells(Some(region), None, |_, layers, _| {
        layer_total += layers.len() as u64
    });
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

    use l2_geo::REGION_BLOCKS;

    use super::*;

    /// File bytes of an all-flat region at height `base`.
    fn flat_region_bytes(base: i16) -> Vec<u8> {
        let mut out = Vec::new();
        for _ in 0..REGION_BLOCKS {
            out.push(0);
            out.extend_from_slice(&base.to_le_bytes());
        }
        out
    }

    #[test]
    fn flat_region_row() {
        let region = Region::parse(&flat_region_bytes(-64)).unwrap();
        assert_eq!(stats_row("0_0", &region), "0_0,65536,0,0,1.000");
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
