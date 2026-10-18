//! `l2-geotool render`: PNG of one region, one pixel per geo cell, optionally A | B | difference.
//!
//! Java: none. The colours are a debugging aid for comparing geodata sets, not game data.

use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use anyhow::Context;
use l2_geo::Region;
use l2_geo::cell::{NSWE_EAST, NSWE_NORTH, NSWE_SOUTH, NSWE_WEST};

use crate::stats::{CELLS, CELLS_SIDE, Layer, region_path, region_stem, top, walk_cells};

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

type Rgb = [u8; 3];

/// Background for empty cells and for the minimum of a colour ramp.
const BASE: Rgb = [30, 30, 30];

/// Colour stops (position 0..=1, colour) of the height ramp: deep water blue up to white.
const TERRAIN: &[(f32, Rgb)] = &[
    (0.0, [20, 30, 110]),
    (0.25, [30, 130, 90]),
    (0.5, [110, 170, 60]),
    (0.75, [190, 150, 70]),
    (1.0, [250, 250, 245]),
];

/// Colour stops of the layer-count heatmap: 1 layer up to 8 or more.
const LAYERS: &[(f32, Rgb)] = &[
    (0.0, [20, 20, 80]),
    (0.33, [30, 150, 90]),
    (0.66, [240, 200, 40]),
    (1.0, [230, 40, 40]),
];

/// Colour stops of the difference panels: black (equal) to white (largest difference).
const HEAT: &[(f32, Rgb)] = &[
    (0.0, [0, 0, 0]),
    (0.33, [140, 20, 20]),
    (0.66, [250, 130, 20]),
    (1.0, [255, 255, 220]),
];

/// Colour of each blocked direction in `nswe` mode. Several blocked directions combine by taking
/// the per-channel maximum, so all four blocked is white.
const NSWE_COLOURS: [(u8, Rgb); 4] = [
    (NSWE_NORTH, [255, 64, 64]),
    (NSWE_EAST, [64, 255, 64]),
    (NSWE_SOUTH, [64, 128, 255]),
    (NSWE_WEST, [255, 255, 64]),
];

/// Height range of the rendered regions (shared by A and B so the colours compare).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Scale {
    min: i32,
    max: i32,
}

/// Panels of a side-by-side image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Panel {
    A,
    B,
    Diff,
}

/// Linear interpolation over colour stops; `t` is clamped to 0..=1.
fn ramp(stops: &[(f32, Rgb)], t: f32) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    for stop in stops.windows(2) {
        let ((t0, c0), (t1, c1)) = (stop[0], stop[1]);
        if t <= t1 {
            let f = if t1 > t0 { (t - t0) / (t1 - t0) } else { 0.0 };
            let mix = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * f).round() as u8;
            return [mix(c0[0], c1[0]), mix(c0[1], c1[1]), mix(c0[2], c1[2])];
        }
    }
    stops.last().map_or(BASE, |stop| stop.1)
}

fn max_channels(a: Rgb, b: Rgb) -> Rgb {
    [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])]
}

/// Colour of one cell of one region.
fn cell_colour(mode: Mode, scale: Scale, layers: &[Layer]) -> Rgb {
    match mode {
        Mode::Layers => ramp(LAYERS, (layers.len() as f32 - 1.0) / 7.0),
        Mode::Height => match top(layers) {
            Some(top) => {
                let span = (scale.max - scale.min).max(1);
                ramp(TERRAIN, (top.z - scale.min) as f32 / span as f32)
            }
            None => BASE,
        },
        Mode::Nswe => match top(layers) {
            Some(top) => {
                let blocked = !top.nswe & 0x0F;
                NSWE_COLOURS
                    .iter()
                    .filter(|(bit, _)| blocked & bit != 0)
                    .fold(BASE, |acc, (_, colour)| max_channels(acc, *colour))
            }
            None => BASE,
        },
    }
}

/// Colour of one cell in the difference panel: how far A and B differ in the rendered quantity.
fn diff_colour(mode: Mode, layers_a: &[Layer], layers_b: &[Layer]) -> Rgb {
    let t = match (mode, top(layers_a), top(layers_b)) {
        (Mode::Height, Some(a), Some(b)) => a.z.abs_diff(b.z).min(256) as f32 / 256.0,
        (Mode::Nswe, Some(a), Some(b)) => ((a.nswe ^ b.nswe) & 0x0F).count_ones() as f32 / 4.0,
        (Mode::Layers, _, _) => layers_a.len().abs_diff(layers_b.len()).min(8) as f32 / 8.0,
        _ => 0.0,
    };
    ramp(HEAT, t)
}

/// Height range over the top layers of `a` (and `b`). Falls back to 0..=0 for no data.
fn height_scale(a: &Region, b: Option<&Region>) -> Scale {
    let mut scale = Scale {
        min: i32::MAX,
        max: i32::MIN,
    };
    walk_cells(a, b, |_, layers_a, layers_b| {
        for layers in [layers_a, layers_b] {
            if let Some(top) = top(layers) {
                scale.min = scale.min.min(top.z);
                scale.max = scale.max.max(top.z);
            }
        }
    });
    if scale.min > scale.max {
        Scale { min: 0, max: 0 }
    } else {
        scale
    }
}

/// One full-resolution panel (`CELLS_SIDE` x `CELLS_SIDE`, row-major, row = cell Y).
fn full_panel(a: &Region, b: Option<&Region>, panel: Panel, mode: Mode, scale: Scale) -> Vec<Rgb> {
    let mut pixels = vec![BASE; CELLS];
    walk_cells(a, b, |index, layers_a, layers_b| {
        let (x, y) = (index / CELLS_SIDE, index % CELLS_SIDE);
        let colour = match panel {
            Panel::A => cell_colour(mode, scale, layers_a),
            Panel::B => cell_colour(mode, scale, layers_b),
            Panel::Diff => diff_colour(mode, layers_a, layers_b),
        };
        pixels[y * CELLS_SIDE + x] = colour;
    });
    pixels
}

/// Box-averages a full-resolution panel down to `size` x `size`.
fn downscale(full: &[Rgb], size: usize) -> Vec<Rgb> {
    if size == CELLS_SIDE {
        return full.to_vec();
    }
    let bounds = |i: usize| (i * CELLS_SIDE / size, (i + 1) * CELLS_SIDE / size);
    let mut out = Vec::with_capacity(size * size);
    for oy in 0..size {
        let (y0, y1) = bounds(oy);
        for ox in 0..size {
            let (x0, x1) = bounds(ox);
            let mut sum = [0u32; 3];
            let mut count = 0u32;
            for y in y0..y1 {
                for pixel in &full[y * CELLS_SIDE + x0..y * CELLS_SIDE + x1] {
                    for (s, &v) in sum.iter_mut().zip(pixel) {
                        *s += u32::from(v);
                    }
                    count += 1;
                }
            }
            out.push(sum.map(|s| (s / count.max(1)) as u8));
        }
    }
    out
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
    let scale = height_scale(a, b);
    let kinds: &[Panel] = if b.is_some() {
        &[Panel::A, Panel::B, Panel::Diff]
    } else {
        &[Panel::A]
    };
    let panels: Vec<Vec<Rgb>> = kinds
        .iter()
        .map(|&kind| downscale(&full_panel(a, b, kind, mode, scale), size))
        .collect();

    let width = size * panels.len();
    let mut rgb = Vec::with_capacity(width * size * 3);
    for row in 0..size {
        for panel in &panels {
            for pixel in &panel[row * size..(row + 1) * size] {
                rgb.extend_from_slice(pixel);
            }
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

    #[test]
    fn height_ramp_spans_the_range() {
        let a = flat_region(-100);
        let b = flat_region(300);
        let scale = height_scale(&a, Some(&b));
        assert_eq!(scale, Scale { min: -100, max: 300 });
        assert_eq!(ramp(TERRAIN, 0.0), [20, 30, 110]);
        assert_eq!(ramp(TERRAIN, 1.0), [250, 250, 245]);
        // Flat regions: one value, so the range is empty and the colour is the bottom of the ramp.
        assert_eq!(height_scale(&a, None), Scale { min: -100, max: -100 });
    }

    #[test]
    fn nswe_colour_shows_blocked_directions() {
        let open_all = [Layer { z: 0, nswe: 0x0F }];
        let blocked_north = [Layer {
            z: 0,
            nswe: 0x0F & !NSWE_NORTH,
        }];
        let blocked_all = [Layer { z: 0, nswe: 0 }];
        let scale = Scale { min: 0, max: 0 };
        assert_eq!(cell_colour(Mode::Nswe, scale, &open_all), BASE);
        assert_eq!(cell_colour(Mode::Nswe, scale, &blocked_north), [255, 64, 64]);
        assert_eq!(cell_colour(Mode::Nswe, scale, &blocked_all), [255, 255, 255]);
    }

    #[test]
    fn downscale_averages_boxes() {
        // Top half white, bottom half black: a 2 x 2 result is white on top, black below.
        let mut full = vec![[0u8; 3]; CELLS];
        for pixel in &mut full[..CELLS / 2] {
            *pixel = [255, 255, 255];
        }
        assert_eq!(
            downscale(&full, 2),
            [[255, 255, 255], [255, 255, 255], [0, 0, 0], [0, 0, 0]]
        );
        // A uniform panel stays uniform at any size.
        let flat = vec![[10u8, 20, 30]; CELLS];
        assert!(downscale(&flat, 3).iter().all(|p| *p == [10, 20, 30]));
    }
}
