//! Geodata tool: compare two geodata sets numerically and visually (D-26).
//!
//! Java: none (uses `l2-geo`). The metrics and CSV columns follow `tools/analysis/geocmp.py`, and
//! the surface-containment percentages follow `tools/analysis/geolayers.py`.

mod diff;
mod render;
mod stats;

use std::io::Write;
use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// Geodata tool for comparing `.l2j` sets (`X_Y.l2j`, one file per region).
#[derive(Parser)]
#[command(name = "l2-geotool", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Per-region block counts and average layers per cell, as CSV on stdout.
    ///
    /// Columns: region, flat, complex, multilayer (block counts), avg_layers (mean layer count
    /// over all 2048 x 2048 cells). Regions are listed in name order.
    Stats {
        /// Directory with the `X_Y.l2j` files.
        dir: PathBuf,
    },
    /// Compare two sets region by region, as CSV on stdout.
    ///
    /// The first 13 columns are the geocmp.py columns. pct_a_top_in_b and pct_b_top_in_a are the
    /// geolayers.py metrics: the share of cells whose top layer (in A, resp. B) has a layer of the
    /// other set within 16 units. Heights are world Z. Percentages are of all 4194304 cells.
    Diff {
        /// First geodata set (the "A" columns, e.g. the downloaded set).
        dir_a: PathBuf,
        /// Second geodata set (the "B" columns, e.g. the converted set).
        dir_b: PathBuf,
        /// Regions to compare, e.g. `12_24` or `12_24.l2j`. Default: every `.l2j` in `dir_a`.
        regions: Vec<String>,
    },
    /// Render one region to a PNG, one pixel per geo cell (2048 x 2048).
    ///
    /// Modes: `height` colours the top layer's Z over the region's min..max (with
    /// `--side-by-side`, A and B share one scale so their colours compare); `nswe` colours the
    /// blocked directions of the top layer (red N, green E, blue S, yellow W, white all four);
    /// `layers` is a heatmap of the layer count per cell (1 to 8 or more).
    ///
    /// With `--side-by-side DIR_B` the image is three panels, A | B | difference, where the
    /// difference panel is a heat map of |dz| (height), of differing NSWE bits (nswe) or of the
    /// layer-count difference (layers). Pixel x is the cell's X inside the region, pixel y its Y.
    Render {
        /// Directory with the `X_Y.l2j` files (set A).
        dir: PathBuf,
        /// Region to render, e.g. `12_24` or `12_24.l2j`.
        region: String,
        /// Output PNG file.
        #[arg(long)]
        out: PathBuf,
        /// What to draw.
        #[arg(long, value_enum, default_value = "height")]
        mode: render::Mode,
        /// Render this directory (set B) and a difference panel next to set A.
        #[arg(long, value_name = "DIR_B")]
        side_by_side: Option<PathBuf>,
        /// Width and height of one panel in pixels, 1 to 2048 (2048 = one pixel per cell). Smaller
        /// sizes are box-averaged.
        #[arg(long, default_value_t = 2048, value_parser = clap::value_parser!(u32).range(1..=2048))]
        size: u32,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let mut out = std::io::stdout().lock();
    match cli.command {
        Command::Stats { dir } => {
            out.write_all(stats::run(&dir)?.as_bytes())?;
        }
        Command::Diff {
            dir_a,
            dir_b,
            regions,
        } => {
            for line in diff::run(&dir_a, &dir_b, &regions)? {
                writeln!(out, "{line}")?;
            }
        }
        Command::Render {
            dir,
            region,
            out: file,
            mode,
            side_by_side,
            size,
        } => {
            render::run(&dir, &region, &file, mode, side_by_side.as_deref(), size)?;
            writeln!(out, "wrote {}", file.display())?;
        }
    }
    Ok(())
}
