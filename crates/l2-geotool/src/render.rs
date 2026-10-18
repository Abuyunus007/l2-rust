//! `l2-geotool render`: PNG of one region, one pixel per geo cell, optionally A | B | difference.
//!
//! Java: none. The colours are a debugging aid for comparing geodata sets, not game data. The
//! pixel rules live in `l2_geo::view`, shared with the admin GUI's geodata viewer.

use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use anyhow::Context;
use l2_geo::Region;
use l2_geo::view::{self, Panel};

use crate::stats::{region_path, region_stem};

/// What a panel shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Mode {
    /// Top layer's Z over the min..max of the rendered regions.
    Height,
    /// Blocked directions of the top layer.
    Nswe,
    /// Layer count per cell.
    Layers,
}

impl From<Mode> for view::Mode {
    fn from(mode: Mode) -> Self {
        match mode {
            Mode::Height => view::Mode::Height,
            Mode::Nswe => view::Mode::Nswe,
            Mode::Layers => view::Mode::Layers,
        }
    }
}

/// Renders `a` alone, or `a | b | difference` side by side, and writes it as an RGB PNG of
/// `size * panels` by `size` pixels.
pub(crate) fn render_png(
    a: &Region,
    b: Option<&Region>,
    mode: Mode,
    size: usize,
    out: &Path,
) -> anyhow::Result<()> {
    let kinds: &[Panel] = if b.is_some() {
        &[Panel::A, Panel::B, Panel::Diff]
    } else {
        &[Panel::A]
    };
    let panels = view::render_panels(Some(a), b, kinds, mode.into(), size);

    let width = size * panels.len();
    let mut rgb = Vec::with_capacity(width * size * 3);
    for row in 0..size {
        for panel in &panels {
            rgb.extend_from_slice(&view::rgb_bytes(&panel[row * size..(row + 1) * size]));
        }
    }
    write_png(out, width, size, &rgb)
}

fn write_png(path: &Path, width: usize, height: usize, rgb: &[u8]) -> anyhow::Result<()> {
    let file = File::create(path).with_context(|| format!("cannot create {}", path.display()))?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), width as u32, height as u32);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(rgb)?;
    writer.finish()?;
    Ok(())
}

/// `l2-geotool render` entry point: loads the region from `dir` (and from `side_by_side`) and
/// writes the PNG.
pub fn run(
    dir: &Path,
    region: &str,
    out: &Path,
    mode: Mode,
    side_by_side: Option<&Path>,
    size: u32,
) -> anyhow::Result<()> {
    let name = region_stem(region);
    let a = Region::load_file(&region_path(dir, name)).with_context(|| format!("region {name}"))?;
    let b = side_by_side
        .map(|dir_b| Region::load_file(&region_path(dir_b, name)).with_context(|| format!("region {name}")))
        .transpose()?;
    render_png(&a, b.as_ref(), mode, size as usize, out)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use l2_geo::REGION_BLOCKS;
    use l2_geo::view::CELLS_SIDE;

    use super::*;

    /// All-flat region at height `base`.
    fn flat_region(base: i16) -> Region {
        let mut bytes = Vec::with_capacity(REGION_BLOCKS * 3);
        for _ in 0..REGION_BLOCKS {
            bytes.push(0);
            bytes.extend_from_slice(&base.to_le_bytes());
        }
        Region::parse(&bytes).unwrap()
    }

    /// Size of a PNG file, read from its header.
    fn png_size(path: &Path) -> (u32, u32) {
        let decoder = png::Decoder::new(std::io::BufReader::new(File::open(path).unwrap()));
        let reader = decoder.read_info().unwrap();
        (reader.info().width, reader.info().height)
    }

    #[test]
    fn single_panel_is_2048_square_by_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("single.png");
        render_png(&flat_region(-64), None, Mode::Height, CELLS_SIDE, &path).unwrap();
        assert_eq!(png_size(&path), (2048, 2048));
    }

    #[test]
    fn side_by_side_has_three_panels() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pair.png");
        let (a, b) = (flat_region(0), flat_region(100));
        for mode in [Mode::Height, Mode::Nswe, Mode::Layers] {
            render_png(&a, Some(&b), mode, 128, &path).unwrap();
            assert_eq!(png_size(&path), (384, 128), "{mode:?}");
        }
    }
}
