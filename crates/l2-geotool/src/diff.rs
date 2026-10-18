//! `l2-geotool diff`: the columns of `tools/analysis/geocmp.py` plus the surface-containment
//! percentages of `tools/analysis/geolayers.py`.
//!
//! Java: none. "Within 16" is the Z tolerance the Python prototypes use; heights are world Z.

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{Context, anyhow};
use l2_geo::Region;

use crate::stats::{CELLS, region_names, region_path, region_stem, top, walk_cells};

/// Header of the diff CSV.
pub(crate) const DIFF_HEADER: &str = "region,a_flat,a_cplx,a_multi,b_flat,b_cplx,b_multi,a_avg_layers,b_avg_layers,\
pct_layercount_diff,mean_abs_dz,pct_dz_gt16,pct_nswe_diff,pct_a_top_in_b,pct_b_top_in_a";

/// Z tolerance: `pct_dz_gt16` counts cells above it, and the containment percentages accept a
/// layer within it (geolayers.py).
const Z_TOLERANCE: i32 = 16;

/// Per-cell sums over one region pair.
#[derive(Default)]
struct Totals {
    layers_a: u64,
    layers_b: u64,
    count_differs: u64,
    dz_sum: u64,
    dz_over_tolerance: u64,
    nswe_differs: u64,
    a_top_in_b: u64,
    b_top_in_a: u64,
}

/// One CSV row of the diff table for a region pair.
pub(crate) fn diff_row(name: &str, a: &Region, b: &Region) -> String {
    let counts_a = a.block_counts();
    let counts_b = b.block_counts();
    let mut t = Totals::default();
    walk_cells(a, Some(b), |_, layers_a, layers_b| {
        t.layers_a += layers_a.len() as u64;
        t.layers_b += layers_b.len() as u64;
        if layers_a.len() != layers_b.len() {
            t.count_differs += 1;
        }
        let (Some(top_a), Some(top_b)) = (top(layers_a), top(layers_b)) else {
            return;
        };
        let dz = top_a.z.abs_diff(top_b.z);
        t.dz_sum += u64::from(dz);
        if dz > Z_TOLERANCE.unsigned_abs() {
            t.dz_over_tolerance += 1;
        }
        if top_a.nswe != top_b.nswe {
            t.nswe_differs += 1;
        }
        if layers_b.iter().any(|l| (top_a.z - l.z).abs() <= Z_TOLERANCE) {
            t.a_top_in_b += 1;
        }
        if layers_a.iter().any(|l| (top_b.z - l.z).abs() <= Z_TOLERANCE) {
            t.b_top_in_a += 1;
        }
    });

    let total = CELLS as f64;
    let pct = |n: u64| 100.0 * n as f64 / total;
    format!(
        "{name},{},{},{},{},{},{},{:.3},{:.3},{:.2},{:.1},{:.2},{:.2},{:.2},{:.2}",
        counts_a.flat,
        counts_a.complex,
        counts_a.multilayer,
        counts_b.flat,
        counts_b.complex,
        counts_b.multilayer,
        t.layers_a as f64 / total,
        t.layers_b as f64 / total,
        pct(t.count_differs),
        t.dz_sum as f64 / total,
        pct(t.dz_over_tolerance),
        pct(t.nswe_differs),
        pct(t.a_top_in_b),
        pct(t.b_top_in_a),
    )
}

/// The diff CSV lines (header first) for the given regions, or for every region of `dir_a` when
/// `regions` is empty. Regions are loaded and compared in parallel; output keeps the input order.
pub fn run(dir_a: &Path, dir_b: &Path, regions: &[String]) -> anyhow::Result<Vec<String>> {
    let names: Vec<String> = if regions.is_empty() {
        region_names(dir_a)?
    } else {
        regions.iter().map(|r| region_stem(r).to_owned()).collect()
    };
    let rows = par_map(names.as_slice(), |name: &String| -> anyhow::Result<String> {
        let a = Region::load_file(&region_path(dir_a, name))?;
        let b = Region::load_file(&region_path(dir_b, name))?;
        Ok(diff_row(name, &a, &b))
    });
    let mut lines = vec![DIFF_HEADER.to_owned()];
    for (name, row) in names.iter().zip(rows) {
        lines.push(row.with_context(|| format!("region {name}"))?);
    }
    Ok(lines)
}

/// Runs `f` on every item on scoped threads (`available_parallelism - 2`, at least one) and returns
/// the results in input order. A worker that panics turns its unfinished items into errors.
pub(crate) fn par_map<T, R, F>(items: &[T], f: F) -> Vec<anyhow::Result<R>>
where
    T: Sync,
    R: Send,
    F: Fn(&T) -> anyhow::Result<R> + Sync,
{
    let cores = std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get);
    let threads = cores.saturating_sub(2).clamp(1, items.len().max(1));
    let next = AtomicUsize::new(0);
    let parts: Vec<Vec<(usize, anyhow::Result<R>)>> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..threads)
            .map(|_| {
                scope.spawn(|| {
                    let mut done = Vec::new();
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        let Some(item) = items.get(index) else {
                            break;
                        };
                        done.push((index, f(item)));
                    }
                    done
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap_or_default())
            .collect()
    });
    let mut slots: Vec<Option<anyhow::Result<R>>> = items.iter().map(|_| None).collect();
    for (index, result) in parts.into_iter().flatten() {
        if let Some(slot) = slots.get_mut(index) {
            *slot = Some(result);
        }
    }
    slots
        .into_iter()
        .enumerate()
        .map(|(index, slot)| slot.unwrap_or_else(|| Err(anyhow!("worker thread panicked (item {index})"))))
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use l2_geo::REGION_BLOCKS;

    use super::*;
    use crate::stats::stats_row;

    /// Real geodata folders (not in CI). Tests that need them skip with a message when missing.
    const DOWNLOAD: &str = r"C:\l2-rust\geodata-download";
    const CONVERTED: &str = r"C:\l2-rust\geodata-converted";

    /// Rows of `tools/analysis/geocmp.py` (`docs/analysis/05-geodata-comparison.csv`) with the
    /// geolayers.py percentages appended (two decimals, from the same run).
    const EXPECTED: [(&str, &str); 4] = [
        (
            "12_24",
            "12_24,16591,36437,12508,43518,84,21934,1.172,1.765,30.68,937.8,91.75,29.66,8.25,8.43",
        ),
        (
            "14_23",
            "14_23,20434,35467,9635,7330,4841,53365,1.158,4.881,80.41,14.0,17.96,4.14,82.04,82.04",
        ),
        (
            "16_10",
            "16_10,64291,905,340,10692,1020,53824,1.005,2.629,81.55,10553.7,100.00,0.98,78.28,0.00",
        ),
        (
            "17_10",
            "17_10,64860,0,676,62251,511,2774,1.008,1.125,3.69,338.0,100.00,0.70,0.02,0.00",
        ),
    ];

    /// A region at height `base` where block 0 is complex (1 layer per cell), block 1 is
    /// multilayer (2 layers per cell) and every other block is flat, so all block types take part.
    fn sample_region(base: i16) -> Region {
        let word = |height: i16, nswe: u8| ((height * 2) as u16 & 0xFFF0) | u16::from(nswe);
        let mut out = Vec::new();
        for block in 0..REGION_BLOCKS {
            match block {
                0 => {
                    out.push(1);
                    for _ in 0..64 {
                        out.extend_from_slice(&word(base, 0x0F).to_le_bytes());
                    }
                }
                1 => {
                    out.push(2);
                    for _ in 0..64 {
                        out.push(2);
                        out.extend_from_slice(&word(base, 0x0F).to_le_bytes());
                        out.extend_from_slice(&word(base + 200, 0x08).to_le_bytes());
                    }
                }
                _ => {
                    out.push(0);
                    out.extend_from_slice(&base.to_le_bytes());
                }
            }
        }
        Region::parse(&out).unwrap()
    }

    #[test]
    fn identical_regions_have_no_differences() {
        let a = sample_region(16);
        // 65534 flat blocks, one complex and one multilayer block: 4194368 layers in 4194304 cells.
        assert_eq!(
            diff_row("1_1", &a, &a),
            "1_1,65534,1,1,65534,1,1,1.000,1.000,0.00,0.0,0.00,0.00,100.00,100.00"
        );
    }

    #[test]
    fn diff_matches_python_csv_for_real_regions() {
        let (a_dir, b_dir) = (Path::new(DOWNLOAD), Path::new(CONVERTED));
        if !a_dir.is_dir() || !b_dir.is_dir() {
            eprintln!("skipping diff_matches_python_csv_for_real_regions: geodata folders not found");
            return;
        }
        for (name, expected) in EXPECTED {
            let a = Region::load_file(&region_path(a_dir, name)).unwrap();
            let b = Region::load_file(&region_path(b_dir, name)).unwrap();
            assert_eq!(diff_row(name, &a, &b), expected, "region {name}");
        }
    }

    #[test]
    fn stats_matches_python_csv_for_a_real_region() {
        let a_dir = Path::new(DOWNLOAD);
        if !a_dir.is_dir() {
            eprintln!("skipping stats_matches_python_csv_for_a_real_region: geodata folder not found");
            return;
        }
        let a = Region::load_file(&region_path(a_dir, "12_24")).unwrap();
        assert_eq!(stats_row("12_24", &a), "12_24,16591,36437,12508,1.172");
    }

    #[test]
    fn par_map_keeps_order_and_reports_errors() {
        let items: Vec<u32> = (0..100).collect();
        let out = par_map(&items, |&n| {
            if n == 42 {
                Err(anyhow!("bad item"))
            } else {
                Ok(n * 2)
            }
        });
        assert_eq!(out.len(), 100);
        for (i, result) in out.iter().enumerate() {
            match i {
                42 => assert!(result.is_err()),
                _ => assert_eq!(*result.as_ref().unwrap(), i as u32 * 2),
            }
        }
    }
}
