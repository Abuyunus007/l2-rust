//! Geodata viewer: one region of geodata set A and set B, drawn side by side, with the world
//! position and height under the pointer (card M3-GUI-03). Read-only.
//!
//! Java: none (tooling). The pictures and the layer rules are `l2_geo::view`, shared with
//! `l2-geotool render`; the coordinates are `l2_geo::coords`, so nothing here re-derives the
//! world <-> geo math.
//!
//! [`RegionPos`], [`cell_in_panel`], [`panel_sets`] and [`describe_cell`] are the view model and
//! have no egui in them, so they are tested directly. [`load_and_render`] runs on a background
//! thread (it reads and parses one file per set). The view only draws the answer.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

use l2_geo::Region;
use l2_geo::cell::{NSWE_ALL, NSWE_EAST, NSWE_NORTH, NSWE_SOUTH, NSWE_WEST};
use l2_geo::coords;
use l2_geo::engine::{GeoOverride, TILE_X_MAX, TILE_X_MIN, TILE_Y_MAX, TILE_Y_MIN};
use l2_geo::region::REGION_CELLS_SIDE;
use l2_geo::view::{self, CELLS_SIDE, Mode, Panel, Rgb, top_layer};

use crate::app::AppCtx;
use crate::settings::Settings;
use crate::views::View;

/// Screen pixels per geo cell, in the zoom steps the view offers.
pub const ZOOMS: [f32; 4] = [0.25, 0.5, 1.0, 2.0];
/// Index into [`ZOOMS`] at first open (0.5: a side-by-side pair fits a normal window).
const DEFAULT_ZOOM: usize = 1;
/// Region (20, 18) holds the world origin (`coords` module), so it is the first one shown.
const DEFAULT_REGION: (u32, u32) = (20, 18);

/// A region of the world: the file `X_Y.l2j`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RegionPos {
    pub x: u32,
    pub y: u32,
}

impl RegionPos {
    /// Global geo cell of local cell (0, 0) of the region (`regionX * 2048`).
    pub fn geo_origin(self) -> (i32, i32) {
        (
            self.x as i32 * REGION_CELLS_SIDE,
            self.y as i32 * REGION_CELLS_SIDE,
        )
    }

    /// Global geo cell of a local cell `(x, y)` of this region.
    pub fn geo_of(self, x: usize, y: usize) -> (i32, i32) {
        let (ox, oy) = self.geo_origin();
        (ox + x as i32, oy + y as i32)
    }

    /// World X and Y of the middle of local cell `(x, y)`, through `coords::world_x` / `world_y`.
    pub fn world_of(self, x: usize, y: usize) -> (i32, i32) {
        let (gx, gy) = self.geo_of(x, y);
        (coords::world_x(gx), coords::world_y(gy))
    }
}

/// The region and the local cell that contain world `(wx, wy)`, through `coords::geo_x` /
/// `geo_y`. `None` when the point lies outside the regions the geodata has (the tile range).
pub fn locate_world(wx: i32, wy: i32) -> Option<(RegionPos, usize, usize)> {
    let (gx, gy) = (coords::geo_x(wx), coords::geo_y(wy));
    let x = u32::try_from(gx.div_euclid(REGION_CELLS_SIDE)).ok()?;
    let y = u32::try_from(gy.div_euclid(REGION_CELLS_SIDE)).ok()?;
    if !(TILE_X_MIN..=TILE_X_MAX).contains(&x) || !(TILE_Y_MIN..=TILE_Y_MAX).contains(&y) {
        return None;
    }
    let lx = gx.rem_euclid(REGION_CELLS_SIDE) as usize;
    let ly = gy.rem_euclid(REGION_CELLS_SIDE) as usize;
    Some((RegionPos { x, y }, lx, ly))
}

/// The cell under the pointer: which panel (in panel order) and the local cell in it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellPos {
    pub panel: usize,
    pub x: usize,
    pub y: usize,
}

/// The local cell under a point given in screen pixels from the panel's top-left corner, at
/// `zoom` screen pixels per cell. `None` outside the panel.
pub fn cell_in_panel(local: (f32, f32), zoom: f32) -> Option<(usize, usize)> {
    let (x, y) = (local.0 / zoom, local.1 / zoom);
    // Written so that NaN also fails the test.
    if !(x >= 0.0 && y >= 0.0) {
        return None;
    }
    let (x, y) = (x as usize, y as usize);
    (x < CELLS_SIDE && y < CELLS_SIDE).then_some((x, y))
}

/// What the picture shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Source {
    /// Set A and set B side by side, with one height scale.
    #[default]
    Both,
    A,
    B,
    /// Each region comes from set B when listed in the override list, otherwise from set A (the
    /// per-region override of D-26, as `GeoEngine` resolves it).
    Override,
}

impl Source {
    pub const ALL: [Source; 4] = [Source::Both, Source::A, Source::B, Source::Override];

    pub fn label(self) -> &'static str {
        match self {
            Source::Both => "A | B",
            Source::A => "Set A",
            Source::B => "Set B",
            Source::Override => "Override",
        }
    }
}

/// One of the two geodata sets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SetId {
    A,
    B,
}

impl SetId {
    pub fn label(self) -> &'static str {
        match self {
            SetId::A => "Set A",
            SetId::B => "Set B",
        }
    }
}

/// The sets drawn, in panel order, for a source choice at region `pos`. `overrides` are the
/// regions taken from set B under [`Source::Override`].
pub fn panel_sets(source: Source, pos: RegionPos, overrides: &[(u32, u32)]) -> Vec<SetId> {
    match source {
        Source::Both => vec![SetId::A, SetId::B],
        Source::A => vec![SetId::A],
        Source::B => vec![SetId::B],
        Source::Override if overrides.contains(&(pos.x, pos.y)) => vec![SetId::B],
        Source::Override => vec![SetId::A],
    }
}

/// Label of a render mode, for the mode picker.
pub fn mode_label(mode: Mode) -> &'static str {
    match mode {
        Mode::Height => "Height",
        Mode::Nswe => "NSWE",
        Mode::Layers => "Layers",
    }
}

/// The open directions of a cell as letters (`N E S W`), or `all` / `none`.
pub fn nswe_text(nswe: u8) -> String {
    if nswe & NSWE_ALL == NSWE_ALL {
        return "all open".to_string();
    }
    let open: Vec<&str> = [
        (NSWE_NORTH, "N"),
        (NSWE_EAST, "E"),
        (NSWE_SOUTH, "S"),
        (NSWE_WEST, "W"),
    ]
    .iter()
    .filter(|(bit, _)| nswe & bit != 0)
    .map(|(_, letter)| *letter)
    .collect();
    if open.is_empty() {
        "none open".to_string()
    } else {
        format!("open {}", open.join(" "))
    }
}

/// One line for the readout under the pictures: world position, geo cell and the height of the
/// top layer (with every layer when the cell has more than one).
pub fn describe_cell(set: SetId, region: Option<&Region>, pos: RegionPos, x: usize, y: usize) -> String {
    let (wx, wy) = pos.world_of(x, y);
    let (gx, gy) = pos.geo_of(x, y);
    let head = format!("{}: world {wx}, {wy} (geo {gx}, {gy})", set.label());
    if region.is_none() {
        return format!("{head}: no geodata here");
    }
    let layers = view::cell_layers(region, gx, gy);
    match top_layer(&layers) {
        None => format!("{head}: no layers"),
        Some(top) => {
            let stack = if layers.len() > 1 {
                let zs: Vec<String> = layers.iter().map(|l| l.z.to_string()).collect();
                format!("; all {} layers at {}", layers.len(), zs.join(", "))
            } else {
                String::new()
            };
            format!("{head}: height {} ({}){stack}", top.z, nswe_text(top.nswe))
        }
    }
}

/// A folder typed in a settings field: surrounding spaces and quotes (Explorer's "Copy as path"
/// adds them) are removed. `None` when nothing is set.
pub fn folder_of(text: &str) -> Option<PathBuf> {
    let trimmed = text.trim().trim_matches('"').trim();
    (!trimmed.is_empty()).then(|| PathBuf::from(trimmed))
}

/// Everything a load needs. Two requests that are equal show the same picture, so the view
/// compares requests to know when to load again.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    pub pos: RegionPos,
    pub source: Source,
    pub mode: Mode,
    pub folder_a: Option<PathBuf>,
    pub folder_b: Option<PathBuf>,
    /// The override list as typed. Empty unless the source is [`Source::Override`], so typing
    /// in the list does not reload for the other sources.
    pub overrides: String,
}

/// The answer of a load: the regions (for the readout) and one picture per panel.
pub struct Loaded {
    pub sets: Vec<SetId>,
    pub a: Option<Region>,
    pub b: Option<Region>,
    /// One square picture per panel, [`CELLS_SIDE`] pixels on each side.
    pub pictures: Vec<egui::ColorImage>,
    /// Things the user should know (a folder not set, a missing file, an ignored list entry).
    pub notes: Vec<String>,
}

impl Loaded {
    fn region(&self, set: SetId) -> Option<&Region> {
        match set {
            SetId::A => self.a.as_ref(),
            SetId::B => self.b.as_ref(),
        }
    }
}

/// Reads the region file of one set. A missing file is no geodata here (not an error); a folder
/// that is not there is.
fn load_set(
    folder: Option<&Path>,
    set: SetId,
    pos: RegionPos,
    notes: &mut Vec<String>,
) -> Result<Option<Region>, String> {
    let Some(folder) = folder else {
        notes.push(format!("{} folder is not set.", set.label()));
        return Ok(None);
    };
    if !folder.is_dir() {
        return Err(format!("{} is not a folder: {}", set.label(), folder.display()));
    }
    let path = folder.join(format!("{}_{}.l2j", pos.x, pos.y));
    if !path.is_file() {
        notes.push(format!(
            "{}: no {}_{}.l2j, so no geodata here.",
            set.label(),
            pos.x,
            pos.y
        ));
        return Ok(None);
    }
    Region::load_file(&path)
        .map(Some)
        .map_err(|e| format!("{}: {e}", path.display()))
}

/// Loads the region file of each shown set and renders the pictures. Runs on a background
/// thread; only the files of the selected region are read.
pub fn load_and_render(request: Request) -> Result<Loaded, String> {
    let mut notes = Vec::new();
    let overrides = if request.source == Source::Override {
        let (regions, ignored) = GeoOverride::parse_regions(&request.overrides);
        for entry in ignored {
            notes.push(format!("ignored override entry '{entry}'."));
        }
        regions
    } else {
        Vec::new()
    };
    let sets = panel_sets(request.source, request.pos, &overrides);
    let needs = |set: SetId| sets.contains(&set);
    let a = if needs(SetId::A) {
        load_set(request.folder_a.as_deref(), SetId::A, request.pos, &mut notes)?
    } else {
        None
    };
    let b = if needs(SetId::B) {
        load_set(request.folder_b.as_deref(), SetId::B, request.pos, &mut notes)?
    } else {
        None
    };

    // Two panels share one height scale (`render_panels` takes both regions); one panel is drawn
    // from its own region alone.
    let (first, second) = match sets.as_slice() {
        [SetId::A, SetId::B] => (a.as_ref(), b.as_ref()),
        [SetId::B] => (b.as_ref(), None),
        _ => (a.as_ref(), None),
    };
    let panels: Vec<Panel> = if sets.len() == 2 {
        vec![Panel::A, Panel::B]
    } else {
        vec![Panel::A]
    };
    let pictures = view::render_panels(first, second, &panels, request.mode, CELLS_SIDE)
        .iter()
        .map(|pixels: &Vec<Rgb>| {
            egui::ColorImage::from_rgb([CELLS_SIDE, CELLS_SIDE], &view::rgb_bytes(pixels))
        })
        .collect();
    Ok(Loaded {
        sets,
        a,
        b,
        pictures,
        notes,
    })
}

pub struct GeodataView {
    pos: RegionPos,
    source: Source,
    mode: Mode,
    /// The override list, as typed (only used by [`Source::Override`]).
    overrides: String,
    /// Index into [`ZOOMS`].
    zoom: usize,
    /// The picture on screen (the last load that succeeded).
    shown: Option<Loaded>,
    /// One texture per panel of `shown`.
    textures: Vec<egui::TextureHandle>,
    /// The loader thread's answer, while a load is running.
    loading: Option<Receiver<Result<Loaded, String>>>,
    /// The request the view last started (also when it failed), so a failure is not retried
    /// every frame; a change of any setting starts a new one.
    last_started: Option<Request>,
    /// Why the last load failed.
    error: Option<String>,
    /// The "Go to world" fields, as typed.
    goto_x: String,
    goto_y: String,
    /// Why the last "Go to world" did not move the region.
    goto_note: Option<String>,
}

impl Default for GeodataView {
    fn default() -> Self {
        Self {
            pos: RegionPos {
                x: DEFAULT_REGION.0,
                y: DEFAULT_REGION.1,
            },
            source: Source::default(),
            mode: Mode::Height,
            overrides: String::new(),
            zoom: DEFAULT_ZOOM,
            shown: None,
            textures: Vec::new(),
            loading: None,
            last_started: None,
            error: None,
            goto_x: String::new(),
            goto_y: String::new(),
            goto_note: None,
        }
    }
}

impl GeodataView {
    /// The request for the current choices. `None` when neither folder is set (nothing to read).
    fn request(&self, settings: &Settings) -> Option<Request> {
        let folder_a = folder_of(&settings.geodata_a_path);
        let folder_b = folder_of(&settings.geodata_b_path);
        if folder_a.is_none() && folder_b.is_none() {
            return None;
        }
        Some(Request {
            pos: self.pos,
            source: self.source,
            mode: self.mode,
            folder_a,
            folder_b,
            overrides: if self.source == Source::Override {
                self.overrides.clone()
            } else {
                String::new()
            },
        })
    }

    /// Starts the loader thread for `request`. The thread wakes the UI when it is done.
    fn start(&mut self, request: Request, ctx: &egui::Context) {
        let (tx, rx) = mpsc::channel();
        let repaint = ctx.clone();
        let job = request.clone();
        let spawned = thread::Builder::new().name("geodata-load".into()).spawn(move || {
            let result = load_and_render(job);
            // The view may be gone when the app closes; then nobody needs the answer.
            let _ = tx.send(result);
            repaint.request_repaint();
        });
        self.last_started = Some(request);
        match spawned {
            // The handle is dropped: the thread is detached and ends after one load.
            Ok(_handle) => {
                self.loading = Some(rx);
                self.error = None;
            }
            Err(e) => self.error = Some(format!("cannot start the loader thread: {e}")),
        }
    }

    /// Takes the loader's answer if it has arrived.
    fn poll_loader(&mut self, ctx: &egui::Context) {
        let Some(rx) = self.loading.as_ref() else {
            return;
        };
        let result = match rx.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => Err("the loader thread stopped without an answer".to_string()),
        };
        self.loading = None;
        match result {
            Ok(mut loaded) => {
                self.textures = loaded
                    .pictures
                    .drain(..)
                    .enumerate()
                    .map(|(i, picture)| {
                        ctx.load_texture(
                            format!("geodata_panel_{i}"),
                            picture,
                            egui::TextureOptions::NEAREST,
                        )
                    })
                    .collect();
                self.shown = Some(loaded);
                self.error = None;
            }
            // A failed load drops the old picture so it never contradicts the error.
            Err(message) => {
                self.shown = None;
                self.textures.clear();
                self.error = Some(message);
            }
        }
    }

    /// The readout for the cell under the pointer.
    fn readout(loaded: &Loaded, pos: RegionPos, cell: CellPos) -> String {
        let set = loaded.sets[cell.panel];
        describe_cell(set, loaded.region(set), pos, cell.x, cell.y)
    }
}

impl View for GeodataView {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut AppCtx<'_>) {
        self.poll_loader(ui.ctx());

        ui.heading("Geodata viewer");
        ui.add_space(8.0);
        for (label, text, hint) in [
            (
                "Set A folder:",
                &mut cx.settings.geodata_a_path,
                "the folder with the X_Y.l2j files of set A",
            ),
            (
                "Set B folder:",
                &mut cx.settings.geodata_b_path,
                "the folder with the X_Y.l2j files of set B",
            ),
        ] {
            ui.horizontal(|ui| {
                ui.label(label);
                ui.add(
                    egui::TextEdit::singleline(text)
                        .hint_text(hint)
                        .desired_width(420.0),
                );
            });
        }
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.label("Region x:");
            ui.add(egui::DragValue::new(&mut self.pos.x).range(TILE_X_MIN..=TILE_X_MAX));
            ui.label("y:");
            ui.add(egui::DragValue::new(&mut self.pos.y).range(TILE_Y_MIN..=TILE_Y_MAX));
            ui.separator();
            egui::ComboBox::from_id_salt("geodata_source")
                .selected_text(self.source.label())
                .show_ui(ui, |ui| {
                    for source in Source::ALL {
                        ui.selectable_value(&mut self.source, source, source.label());
                    }
                });
            egui::ComboBox::from_id_salt("geodata_mode")
                .selected_text(mode_label(self.mode))
                .show_ui(ui, |ui| {
                    for mode in [Mode::Height, Mode::Nswe, Mode::Layers] {
                        ui.selectable_value(&mut self.mode, mode, mode_label(mode));
                    }
                });
            ui.separator();
            ui.label("Zoom:");
            if ui.add_enabled(self.zoom > 0, egui::Button::new("-")).clicked() {
                self.zoom -= 1;
            }
            ui.label(format!("{:.0}%", ZOOMS[self.zoom] * 100.0));
            if ui
                .add_enabled(self.zoom + 1 < ZOOMS.len(), egui::Button::new("+"))
                .clicked()
            {
                self.zoom += 1;
            }
        });
        ui.horizontal(|ui| {
            ui.label("Go to world:");
            ui.add(
                egui::TextEdit::singleline(&mut self.goto_x)
                    .hint_text("x")
                    .desired_width(90.0),
            );
            ui.add(
                egui::TextEdit::singleline(&mut self.goto_y)
                    .hint_text("y")
                    .desired_width(90.0),
            );
            if ui.button("Go").clicked() {
                self.goto_note = match (
                    self.goto_x.trim().parse::<i32>(),
                    self.goto_y.trim().parse::<i32>(),
                ) {
                    (Ok(wx), Ok(wy)) => match locate_world(wx, wy) {
                        Some((pos, _, _)) => {
                            self.pos = pos;
                            None
                        }
                        None => Some("That world position is outside the geodata regions.".to_string()),
                    },
                    _ => Some("Type whole numbers for the world x and y.".to_string()),
                };
            }
        });
        if let Some(note) = &self.goto_note {
            ui.weak(note.as_str());
        }
        if self.source == Source::Override {
            ui.horizontal(|ui| {
                ui.label("Regions from set B:");
                ui.add(
                    egui::TextEdit::singleline(&mut self.overrides)
                        .hint_text("X_Y, comma separated, e.g. 12_24,17_10 (the others come from set A)")
                        .desired_width(420.0),
                );
            });
        }

        match self.request(cx.settings) {
            None => {
                ui.label("Set the set A and/or set B folder to view a region. Each is the folder with the X_Y.l2j files.");
                return;
            }
            Some(want) => {
                if self.loading.is_none() && self.last_started.as_ref() != Some(&want) {
                    self.start(want, ui.ctx());
                }
            }
        }

        if self.loading.is_some() {
            ui.label(format!("Loading region {}_{}...", self.pos.x, self.pos.y));
        }
        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error.as_str());
        }
        let Some(loaded) = &self.shown else {
            return;
        };
        for note in &loaded.notes {
            ui.weak(note.as_str());
        }

        let zoom = ZOOMS[self.zoom];
        let side = CELLS_SIDE as f32 * zoom;
        let mut hover: Option<CellPos> = None;
        egui::ScrollArea::both()
            .id_salt("geodata_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (panel, (set, texture)) in loaded.sets.iter().zip(&self.textures).enumerate() {
                        ui.vertical(|ui| {
                            ui.label(set.label());
                            let size = egui::vec2(side, side);
                            let response = ui.add(
                                egui::Image::from_texture(egui::load::SizedTexture::new(texture.id(), size))
                                    .fit_to_exact_size(size)
                                    .sense(egui::Sense::hover()),
                            );
                            if let Some(pointer) = response.hover_pos() {
                                let local = pointer - response.rect.min;
                                if let Some((x, y)) = cell_in_panel((local.x, local.y), zoom) {
                                    hover = Some(CellPos { panel, x, y });
                                }
                            }
                        });
                    }
                });
            });

        ui.separator();
        match hover {
            Some(cell) => ui.label(Self::readout(loaded, self.pos, cell)),
            None => ui.label("Point at a cell to see its world position and height."),
        };
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use l2_geo::REGION_BLOCKS;

    use super::*;

    const WORLD_ORIGIN_REGION: RegionPos = RegionPos { x: 20, y: 18 };

    /// File bytes of an all-flat region at height `base`.
    fn flat_bytes(base: i16) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(REGION_BLOCKS * 3);
        for _ in 0..REGION_BLOCKS {
            bytes.push(0);
            bytes.extend_from_slice(&base.to_le_bytes());
        }
        bytes
    }

    #[test]
    fn world_origin_is_the_corner_of_region_20_18() {
        // Local cell (0, 0) of region 20_18 holds world (0, 0) to the middle of its cell.
        assert_eq!(WORLD_ORIGIN_REGION.world_of(0, 0), (8, 8));
        assert_eq!(locate_world(8, 8), Some((WORLD_ORIGIN_REGION, 0, 0)));
    }

    #[test]
    fn world_and_local_cells_round_trip() {
        for (x, y) in [(5, 7), (2047, 2047), (1000, 3), (0, 2047)] {
            let (wx, wy) = WORLD_ORIGIN_REGION.world_of(x, y);
            assert_eq!(locate_world(wx, wy), Some((WORLD_ORIGIN_REGION, x, y)));
        }
        // Sixteen world units per cell: five cells to the east is 80 units further.
        assert_eq!(
            WORLD_ORIGIN_REGION.world_of(5, 0).0 - WORLD_ORIGIN_REGION.world_of(0, 0).0,
            80
        );
    }

    #[test]
    fn region_edges_belong_to_the_neighbour() {
        // Region 20 spans world X 0..32768; the cell just outside on either side is in region 21
        // or 19, at its near or far edge.
        assert_eq!(locate_world(32767, 8), Some((WORLD_ORIGIN_REGION, 2047, 0)));
        assert_eq!(locate_world(32768, 8), Some((RegionPos { x: 21, y: 18 }, 0, 0)));
        assert_eq!(locate_world(-1, 8), Some((RegionPos { x: 19, y: 18 }, 2047, 0)));
        // Outside the tile range (regions 11..=26 x 10..=26) there is no region to show.
        assert_eq!(locate_world(-655_361, 8), None);
    }

    #[test]
    fn pointer_pixels_map_to_cells_at_each_zoom() {
        assert_eq!(cell_in_panel((0.0, 0.0), 1.0), Some((0, 0)));
        // At half zoom each cell is half a pixel, so 3.9 px and 0.1 px are cells 7 and 0.
        assert_eq!(cell_in_panel((3.9, 0.1), 0.5), Some((7, 0)));
        assert_eq!(cell_in_panel((3.9, 0.1), 2.0), Some((1, 0)));
        // Outside the panel, or before it, there is no cell.
        assert_eq!(cell_in_panel((CELLS_SIDE as f32 * 0.5, 0.0), 0.5), None);
        assert_eq!(cell_in_panel((-1.0, 0.0), 1.0), None);
        assert_eq!(cell_in_panel((f32::NAN, 0.0), 1.0), None);
    }

    #[test]
    fn panels_follow_the_source() {
        let pos = WORLD_ORIGIN_REGION;
        assert_eq!(panel_sets(Source::Both, pos, &[]), [SetId::A, SetId::B]);
        assert_eq!(panel_sets(Source::A, pos, &[]), [SetId::A]);
        assert_eq!(panel_sets(Source::B, pos, &[]), [SetId::B]);
        // The override list picks set B for its regions only.
        assert_eq!(panel_sets(Source::Override, pos, &[(20, 18)]), [SetId::B]);
        assert_eq!(panel_sets(Source::Override, pos, &[(12, 24)]), [SetId::A]);
    }

    #[test]
    fn open_directions_read_as_letters() {
        assert_eq!(nswe_text(NSWE_ALL), "all open");
        assert_eq!(nswe_text(0), "none open");
        assert_eq!(nswe_text(NSWE_ALL & !NSWE_NORTH), "open E S W");
    }

    #[test]
    fn readout_names_the_set_world_position_and_height() {
        let region = Region::parse(&flat_bytes(40)).unwrap();
        let text = describe_cell(SetId::B, Some(&region), WORLD_ORIGIN_REGION, 0, 0);
        assert_eq!(text, "Set B: world 8, 8 (geo 40960, 36864): height 40 (all open)");
        let missing = describe_cell(SetId::A, None, WORLD_ORIGIN_REGION, 0, 0);
        assert!(missing.starts_with("Set A: world 8, 8") && missing.ends_with("no geodata here"));
    }

    #[test]
    fn folders_are_trimmed_and_unquoted() {
        assert_eq!(
            folder_of("  \"C:\\geo\\set a\"  "),
            Some(PathBuf::from("C:\\geo\\set a"))
        );
        assert_eq!(folder_of("   "), None);
    }

    #[test]
    fn load_reports_a_folder_that_is_not_there() {
        let dir = tempfile::tempdir().unwrap();
        let request = Request {
            pos: WORLD_ORIGIN_REGION,
            source: Source::A,
            mode: Mode::Height,
            folder_a: Some(dir.path().join("missing")),
            folder_b: None,
            overrides: String::new(),
        };
        let error = load_and_render(request)
            .err()
            .expect("missing folder is an error");
        assert!(error.contains("is not a folder"), "{error}");
    }

    #[test]
    fn load_reads_one_region_per_set_and_notes_the_gaps() {
        let set_a = tempfile::tempdir().unwrap();
        std::fs::write(set_a.path().join("20_18.l2j"), flat_bytes(10)).unwrap();
        let set_b = tempfile::tempdir().unwrap(); // no 20_18.l2j here
        let request = Request {
            pos: WORLD_ORIGIN_REGION,
            source: Source::Both,
            mode: Mode::Height,
            folder_a: Some(set_a.path().to_path_buf()),
            folder_b: Some(set_b.path().to_path_buf()),
            overrides: String::new(),
        };
        let loaded = load_and_render(request).unwrap();
        assert_eq!(loaded.sets, [SetId::A, SetId::B]);
        assert!(loaded.a.is_some() && loaded.b.is_none());
        assert_eq!(loaded.pictures.len(), 2);
        assert_eq!(loaded.pictures[0].size, [CELLS_SIDE, CELLS_SIDE]);
        assert!(
            loaded.notes.iter().any(|n| n.contains("no 20_18.l2j")),
            "{:?}",
            loaded.notes
        );
    }
}
