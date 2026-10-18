//! Cell layers and debug pictures of a region: what `l2-geotool render` draws and what the admin
//! GUI geodata viewer shows. The rules live here once, so both tools colour the same way.
//!
//! Java: none (tooling). The layer count follows `tools/analysis/geocmp.py`: one layer per flat or
//! complex cell, `n` per multilayer cell. Game logic does not use this module; it only reads
//! [`Region`]s.
//!
//! Regions are optional everywhere: `None` is a missing file (a null region, no geodata), which
//! draws as the background colour.

use crate::block::{BLOCK_CELLS, BLOCK_CELLS_SIDE, BlockRef, cell_index};
use crate::cell::{NSWE_ALL, NSWE_EAST, NSWE_NORTH, NSWE_SOUTH, NSWE_WEST, cell_height, cell_nswe};
use crate::region::{REGION_BLOCKS_SIDE, REGION_CELLS_SIDE, Region};

/// Cells on one side of a region (2048).
pub const CELLS_SIDE: usize = REGION_CELLS_SIDE as usize;
/// Cells in a region (2048 x 2048 = 4194304).
pub const CELLS: usize = CELLS_SIDE * CELLS_SIDE;

/// An RGB colour.
pub type Rgb = [u8; 3];

/// What a panel shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Top layer's Z over the min..max of the rendered regions.
    Height,
    /// Blocked directions of the top layer.
    Nswe,
    /// Layer count per cell.
    Layers,
}

/// Panels of a picture: the first region, the second region, or the difference of both.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Panel {
    A,
    B,
    Diff,
}

/// One layer of a cell: its world Z and its NSWE bits (a flat block is open in all directions).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layer {
    pub z: i32,
    pub nswe: u8,
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
pub fn top_layer(layers: &[Layer]) -> Option<Layer> {
    let mut iter = layers.iter().copied();
    let mut best = iter.next()?;
    for layer in iter {
        if layer.z > best.z {
            best = layer;
        }
    }
    Some(best)
}

/// Fills `out` with the layers of one cell (`cell` = `cx * 8 + cy` inside the block). A missing
/// block (`None`) gives no layers.
fn fill_layers(block: Option<BlockRef<'_>>, cell: usize, out: &mut Vec<Layer>) {
    out.clear();
    match block {
        None => {}
        Some(BlockRef::Flat { height }) => out.push(Layer {
            z: i32::from(height),
            nswe: NSWE_ALL,
        }),
        Some(BlockRef::Complex { cells }) => out.extend(cells.get(cell).map(|&w| Layer::from_word(w))),
        Some(BlockRef::Multilayer { data, cell_offsets }) => {
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

/// The layers of the cell at global geo coordinates `(geo_x, geo_y)`, the coordinates [`Region`]
/// takes. Empty when there is no region.
pub fn cell_layers(region: Option<&Region>, geo_x: i32, geo_y: i32) -> Vec<Layer> {
    let mut out = Vec::new();
    let block = region.map(|r| r.block(geo_x, geo_y));
    fill_layers(block, cell_index(geo_x, geo_y), &mut out);
    out
}

/// Calls `f(index, layers_a, layers_b)` for every cell once. `index` is `x * 2048 + y` (x and y
/// inside the region); the walk goes block by block, so indices are not visited in order. A
/// missing region gives empty layers for its side. Both regions have the same block grid, so the
/// walk runs in lockstep.
pub fn walk_cells(a: Option<&Region>, b: Option<&Region>, mut f: impl FnMut(usize, &[Layer], &[Layer])) {
    let mut layers_a = Vec::with_capacity(8);
    let mut layers_b = Vec::with_capacity(8);
    for bx in 0..REGION_BLOCKS_SIDE {
        for by in 0..REGION_BLOCKS_SIDE {
            let (gx, gy) = ((bx * BLOCK_CELLS_SIDE) as i32, (by * BLOCK_CELLS_SIDE) as i32);
            let block_a = a.map(|r| r.block(gx, gy));
            let block_b = b.map(|r| r.block(gx, gy));
            for cell in 0..BLOCK_CELLS {
                let x = bx * BLOCK_CELLS_SIDE + cell / BLOCK_CELLS_SIDE;
                let y = by * BLOCK_CELLS_SIDE + cell % BLOCK_CELLS_SIDE;
                fill_layers(block_a, cell, &mut layers_a);
                fill_layers(block_b, cell, &mut layers_b);
                f(x * CELLS_SIDE + y, &layers_a, &layers_b);
            }
        }
    }
}

/// Background for empty cells and for the minimum of a colour ramp.
pub const BASE: Rgb = [30, 30, 30];

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

/// Colour of each blocked direction in [`Mode::Nswe`]. Several blocked directions combine by
/// taking the per-channel maximum, so all four blocked is white.
const NSWE_COLOURS: [(u8, Rgb); 4] = [
    (NSWE_NORTH, [255, 64, 64]),
    (NSWE_EAST, [64, 255, 64]),
    (NSWE_SOUTH, [64, 128, 255]),
    (NSWE_WEST, [255, 255, 64]),
];

/// Height range of the rendered regions (shared by A and B so the colours compare).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scale {
    pub min: i32,
    pub max: i32,
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

/// Colour of one cell of one region. A cell without layers (missing region) is [`BASE`].
pub fn cell_colour(mode: Mode, scale: Scale, layers: &[Layer]) -> Rgb {
    match mode {
        Mode::Layers if layers.is_empty() => BASE,
        Mode::Layers => ramp(LAYERS, (layers.len() as f32 - 1.0) / 7.0),
        Mode::Height => match top_layer(layers) {
            Some(top) => {
                let span = (scale.max - scale.min).max(1);
                ramp(TERRAIN, (top.z - scale.min) as f32 / span as f32)
            }
            None => BASE,
        },
        Mode::Nswe => match top_layer(layers) {
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
pub fn diff_colour(mode: Mode, layers_a: &[Layer], layers_b: &[Layer]) -> Rgb {
    let t = match (mode, top_layer(layers_a), top_layer(layers_b)) {
        (Mode::Height, Some(a), Some(b)) => a.z.abs_diff(b.z).min(256) as f32 / 256.0,
        (Mode::Nswe, Some(a), Some(b)) => ((a.nswe ^ b.nswe) & 0x0F).count_ones() as f32 / 4.0,
        (Mode::Layers, _, _) => layers_a.len().abs_diff(layers_b.len()).min(8) as f32 / 8.0,
        _ => 0.0,
    };
    ramp(HEAT, t)
}

/// Height range over the top layers of `a` and `b`. Falls back to 0..=0 for no data.
pub fn height_scale(a: Option<&Region>, b: Option<&Region>) -> Scale {
    let mut scale = Scale {
        min: i32::MAX,
        max: i32::MIN,
    };
    walk_cells(a, b, |_, layers_a, layers_b| {
        for layers in [layers_a, layers_b] {
            if let Some(top) = top_layer(layers) {
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
fn full_panel(a: Option<&Region>, b: Option<&Region>, panel: Panel, mode: Mode, scale: Scale) -> Vec<Rgb> {
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

/// Renders each panel in `panels` as `size` x `size` pixels, row-major (row = cell Y, column =
/// cell X). `size` below 2048 box-averages the cells. A and B share one height scale, so their
/// colours compare; a missing region is drawn as [`BASE`].
pub fn render_panels(
    a: Option<&Region>,
    b: Option<&Region>,
    panels: &[Panel],
    mode: Mode,
    size: usize,
) -> Vec<Vec<Rgb>> {
    let scale = height_scale(a, b);
    panels
        .iter()
        .map(|&panel| downscale(&full_panel(a, b, panel, mode, scale), size))
        .collect()
}

/// Flattens pixels to `R, G, B` bytes: the layout of a PNG row or an egui image.
pub fn rgb_bytes(pixels: &[Rgb]) -> Vec<u8> {
    pixels.iter().flatten().copied().collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::REGION_BLOCKS;

    /// File bytes of an all-flat region at height `base`.
    fn flat_bytes(base: i16) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(REGION_BLOCKS * 3);
        for _ in 0..REGION_BLOCKS {
            bytes.push(0);
            bytes.extend_from_slice(&base.to_le_bytes());
        }
        bytes
    }

    fn flat_region(base: i16) -> Region {
        Region::parse(&flat_bytes(base)).unwrap()
    }

    /// File bytes with the given blocks (block index `bx * 256 + by`) as complex blocks with
    /// varied cell words; every other block is flat at height 0.
    fn bytes_with_complex(complex: &[usize]) -> Vec<u8> {
        let mut out = Vec::new();
        for index in 0..REGION_BLOCKS {
            match complex.iter().position(|&c| c == index) {
                Some(k) => {
                    out.push(1);
                    for cell in 0..BLOCK_CELLS {
                        let level = cell as u16 + 1 + 64 * k as u16;
                        let word = (level << 4) | (cell as u16 & 0x0F);
                        out.extend_from_slice(&word.to_le_bytes());
                    }
                }
                None => {
                    out.push(0);
                    out.extend_from_slice(&0i16.to_le_bytes());
                }
            }
        }
        out
    }

    #[test]
    fn flat_region_has_one_layer_per_cell() {
        let region = flat_region(-64);
        let mut visited = vec![false; CELLS];
        walk_cells(Some(&region), None, |index, layers, other| {
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
    fn cell_layers_agree_with_the_walk() {
        // Two complex blocks: block (0, 0) and block (1, 2), so the cell index mapping is tested
        // away from the origin too. Only the cells of those blocks are compared.
        let region = Region::parse(&bytes_with_complex(&[0, 258])).unwrap();
        walk_cells(Some(&region), None, |index, layers, _| {
            let (x, y) = (index / CELLS_SIDE, index % CELLS_SIDE);
            let in_first = x < 8 && y < 8;
            let in_second = (8..16).contains(&x) && (16..24).contains(&y);
            if in_first || in_second {
                assert_eq!(
                    cell_layers(Some(&region), x as i32, y as i32),
                    layers,
                    "cell {x},{y}"
                );
            }
        });
        assert_eq!(cell_layers(Some(&region), 9, 17).len(), 1);
        assert!(cell_layers(None, 9, 17).is_empty());
    }

    #[test]
    fn ramp_spans_the_range() {
        assert_eq!(ramp(TERRAIN, 0.0), [20, 30, 110]);
        assert_eq!(ramp(TERRAIN, 1.0), [250, 250, 245]);
        assert_eq!(ramp(TERRAIN, -3.0), [20, 30, 110], "clamped below");
        assert_eq!(ramp(TERRAIN, 7.0), [250, 250, 245], "clamped above");
    }

    #[test]
    fn height_scale_covers_both_sets() {
        let a = flat_region(-100);
        let b = flat_region(300);
        assert_eq!(height_scale(Some(&a), Some(&b)), Scale { min: -100, max: 300 });
        // Flat regions: one value, so the range is empty and the colour is the bottom of the ramp.
        assert_eq!(height_scale(Some(&a), None), Scale { min: -100, max: -100 });
        assert_eq!(height_scale(None, None), Scale { min: 0, max: 0 });
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
    fn missing_region_is_background() {
        let scale = Scale { min: 0, max: 0 };
        assert_eq!(cell_colour(Mode::Layers, scale, &[]), BASE);
        assert_eq!(cell_colour(Mode::Height, scale, &[]), BASE);
        let panels = render_panels(None, None, &[Panel::A], Mode::Layers, 4);
        assert!(panels[0].iter().all(|&p| p == BASE));
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

    #[test]
    fn render_panels_gives_one_square_per_panel() {
        let (a, b) = (flat_region(0), flat_region(100));
        let panels = render_panels(
            Some(&a),
            Some(&b),
            &[Panel::A, Panel::B, Panel::Diff],
            Mode::Height,
            16,
        );
        assert_eq!(panels.len(), 3);
        assert!(panels.iter().all(|p| p.len() == 16 * 16));
        // A is at the bottom of the shared scale, B at the top.
        assert_eq!(panels[0][0], ramp(TERRAIN, 0.0));
        assert_eq!(panels[1][0], ramp(TERRAIN, 1.0));
        assert_eq!(rgb_bytes(&panels[0][..2]).len(), 6);
    }
}
