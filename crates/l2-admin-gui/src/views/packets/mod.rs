//! Packet inspector: runs the decrypting proxy in-process (D-35), lists the packets live, and
//! shows the selected one as decoded fields plus a hex dump. Captures can be saved to and
//! opened from JSON-lines files (the format of `l2_proxy::capture`).
//!
//! ```text
//!   proxy_control.rs   start/stop, the runtime the proxy runs on
//!   list.rs            Store (records + filter), Filter, the virtual list
//!   detail.rs          field tree, hex dump
//! ```
//!
//! Two stores: `live` collects what the proxy sees (also while another view is shown, as long as
//! this view was drawn once since the packets arrived: records are taken in when the view is
//! drawn), `file` holds an opened capture. While a file is open the list shows it read-only and
//! the live store keeps collecting in the background; "Back to live" returns to it.

mod detail;
mod list;
mod proxy_control;

use std::path::Path;

use anyhow::Context as _;
use l2_proxy::capture::{CaptureReader, CaptureWriter};
use l2_proxy::{Channel, Dir, Record};

use crate::app::AppCtx;
use crate::views::View;

use list::{CompiledFilter, Filter, MAX_LIVE_RECORDS, Store};
use proxy_control::{ControlAction, ProxyController, RunState};

/// A capture opened from disk.
struct OpenFile {
    path: String,
    store: Store,
}

/// A message under the toolbar.
struct Notice {
    error: bool,
    text: String,
}

impl Notice {
    fn info(text: impl Into<String>) -> Self {
        Self {
            error: false,
            text: text.into(),
        }
    }

    fn error(text: impl Into<String>) -> Self {
        Self {
            error: true,
            text: text.into(),
        }
    }
}

pub struct PacketsView {
    controller: ProxyController,
    live: Store,
    file: Option<OpenFile>,
    filter: Filter,
    compiled: CompiledFilter,
    /// Bumped whenever `filter` changes; the stores re-filter when they see a new number.
    generation: u64,
    /// The selected row of the store on screen.
    selected: Option<u64>,
    /// Why the proxy could not be started from the typed settings.
    settings_error: Option<String>,
    notice: Option<Notice>,
}

impl Default for PacketsView {
    fn default() -> Self {
        let filter = Filter::default();
        Self {
            controller: ProxyController::default(),
            live: Store::new(MAX_LIVE_RECORDS),
            file: None,
            compiled: CompiledFilter::new(&filter),
            filter,
            generation: 1,
            selected: None,
            settings_error: None,
            notice: None,
        }
    }
}

impl PacketsView {
    fn shown_store(&self) -> &Store {
        self.file.as_ref().map_or(&self.live, |f| &f.store)
    }

    fn proxy_row(&mut self, ui: &mut egui::Ui, cx: &mut AppCtx<'_>) {
        let state = self.controller.state();
        match proxy_control::controls_ui(ui, &mut cx.settings.packets, state) {
            ControlAction::Start => match proxy_control::parse_config(&cx.settings.packets) {
                Ok(config) => {
                    self.settings_error = None;
                    self.controller.start(config, Some(ui.ctx().clone()));
                }
                Err(e) => self.settings_error = Some(e),
            },
            ControlAction::Stop => self.controller.stop(Some(ui.ctx().clone())),
            ControlAction::None => {}
        }

        let error = self.settings_error.as_deref().or_else(|| self.controller.error());
        ui.horizontal_wrapped(|ui| {
            if let Some(error) = error {
                ui.colored_label(ui.visuals().error_fg_color, error);
            } else {
                ui.label(self.proxy_status(cx));
            }
        });
    }

    fn proxy_status(&self, cx: &AppCtx<'_>) -> String {
        match self.controller.state() {
            RunState::Stopped => format!(
                "Proxy stopped. Start it, then point the game client's login address at {}.",
                cx.settings.packets.listen.trim()
            ),
            RunState::Starting => "Starting the proxy (generating keys)...".to_string(),
            RunState::Stopping => "Stopping the proxy...".to_string(),
            RunState::Running { local_addr } => {
                let relays = self.controller.game_relays();
                let relays = if relays.is_empty() {
                    String::new()
                } else {
                    let ports: Vec<String> = relays
                        .iter()
                        .map(|(id, port, real)| format!("server {id}: port {port} -> {real}"))
                        .collect();
                    format!(", game relays: {}", ports.join("; "))
                };
                format!(
                    "Proxy running on {local_addr} -> {}. {} connections, {} packets{relays}",
                    cx.settings.packets.upstream.trim(),
                    self.live.connections(),
                    self.live.total()
                )
            }
        }
    }

    fn filter_row(&mut self, ui: &mut egui::Ui, cx: &mut AppCtx<'_>) {
        let packets = &mut cx.settings.packets;
        let mut dir = self.filter.dir;
        let mut channel = self.filter.channel;
        let mut text = self.filter.text.clone();
        ui.horizontal_wrapped(|ui| {
            ui.label("Direction");
            egui::ComboBox::from_id_salt("packets_dir")
                .selected_text(match dir {
                    None => "All",
                    Some(Dir::C2s) => "→ client to server",
                    Some(Dir::S2c) => "← server to client",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut dir, None, "All");
                    ui.selectable_value(&mut dir, Some(Dir::C2s), "→ client to server");
                    ui.selectable_value(&mut dir, Some(Dir::S2c), "← server to client");
                });
            ui.label("Channel");
            egui::ComboBox::from_id_salt("packets_channel")
                .selected_text(match channel {
                    None => "All",
                    Some(c) => list::channel_label(c),
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut channel, None, "All");
                    ui.selectable_value(&mut channel, Some(Channel::Login), "login");
                    ui.selectable_value(&mut channel, Some(Channel::Game), "game");
                });
            ui.add(
                egui::TextEdit::singleline(&mut text)
                    .hint_text("Opcode (0x59) or name")
                    .desired_width(170.0),
            );
            ui.checkbox(&mut packets.hide_noisy, "Hide noisy");
            ui.add_enabled(
                packets.hide_noisy,
                egui::TextEdit::singleline(&mut packets.noisy)
                    .hint_text("Names or 0x opcodes, separated by commas")
                    .desired_width(280.0),
            )
            .on_hover_text(
                "Packets hidden while \"Hide noisy\" is on: whole names (MOVE_TO_LOCATION, \
                 MoveToLocation) or opcodes with 0x (0x59, 0x00e8 for an ex opcode)",
            );
        });

        let current = Filter {
            dir,
            channel,
            text,
            hide_noisy: packets.hide_noisy,
            noisy: packets.noisy.clone(),
        };
        if current != self.filter {
            self.compiled = CompiledFilter::new(&current);
            self.filter = current;
            self.generation += 1;
        }
    }

    fn file_row(&mut self, ui: &mut egui::Ui, cx: &mut AppCtx<'_>) {
        let mut save = false;
        let mut open = false;
        let mut back_to_live = false;
        let mut clear = false;
        ui.horizontal_wrapped(|ui| {
            ui.label("Capture file");
            ui.add(egui::TextEdit::singleline(&mut cx.settings.packets.capture_path).desired_width(260.0));
            save = ui
                .add_enabled(self.shown_store().len() > 0, egui::Button::new("Save"))
                .on_hover_text("Write every packet of the list (not only the filtered ones) to this file")
                .clicked();
            open = ui
                .button("Open")
                .on_hover_text("Read a capture file (read-only; no proxy needed)")
                .clicked();
            if self.file.is_some() {
                back_to_live = ui.button("Back to live").clicked();
            } else {
                clear = ui
                    .add_enabled(self.live.len() > 0, egui::Button::new("Clear"))
                    .on_hover_text("Forget the live packets")
                    .clicked();
            }
        });

        let path = cx.settings.packets.capture_path.trim().to_string();
        if save {
            self.notice = Some(match save_capture(&path, self.shown_store().records()) {
                Ok(count) => Notice::info(format!("Saved {count} packets to {path}")),
                Err(e) => Notice::error(format!("Cannot save {path}: {e:#}")),
            });
        }
        if open {
            match load_capture(&path) {
                Ok(store) => {
                    self.notice = Some(Notice::info(format!(
                        "Opened {path}: {} packets (read-only)",
                        store.len()
                    )));
                    self.file = Some(OpenFile { path, store });
                    self.selected = None;
                }
                Err(e) => self.notice = Some(Notice::error(format!("Cannot open {path}: {e:#}"))),
            }
        }
        if back_to_live {
            self.file = None;
            self.selected = None;
            self.notice = None;
        }
        if clear {
            self.live.clear();
            self.selected = None;
            self.notice = None;
        }
    }

    fn status_row(&self, ui: &mut egui::Ui) {
        let store = self.shown_store();
        ui.horizontal_wrapped(|ui| {
            match &self.file {
                Some(file) => ui.label(format!("File {} (read-only)", file.path)),
                None => ui.label("Live"),
            };
            ui.label(format!(
                "{} of {} packets shown, times in UTC",
                store.visible_len(),
                store.len()
            ));
            if let Some(notice) = &self.notice {
                if notice.error {
                    ui.colored_label(ui.visuals().error_fg_color, &notice.text);
                } else {
                    ui.label(&notice.text);
                }
            }
        });
    }
}

impl View for PacketsView {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut AppCtx<'_>) {
        let records = self.controller.poll();
        if !records.is_empty() {
            self.live.push_all(records);
        }

        self.proxy_row(ui, cx);
        ui.separator();
        self.filter_row(ui, cx);
        self.file_row(ui, cx);

        let (compiled, generation) = (&self.compiled, self.generation);
        match self.file.as_mut() {
            Some(file) => file.store.refresh(compiled, generation),
            None => self.live.refresh(compiled, generation),
        }
        self.status_row(ui);
        ui.add_space(2.0);

        egui::Panel::bottom("packet_detail")
            .resizable(true)
            .default_size(260.0)
            .min_size(120.0)
            .show(ui, |ui| {
                let store = self.shown_store();
                let record = self.selected.and_then(|id| store.get(id)).map(|row| &row.record);
                detail::detail_ui(ui, record);
            });
        egui::CentralPanel::default().show(ui, |ui| {
            let clicked = list::list_ui(ui, self.shown_store(), self.selected);
            if let Some(id) = clicked {
                // Clicking the selected row again deselects it.
                self.selected = if self.selected == Some(id) { None } else { Some(id) };
            }
        });
    }
}

/// Serializes `records` into the capture format and writes them to `path` (parent folders are
/// created). Returns how many were written. The records of a live proxy are already free of
/// credentials, so nothing is filtered here.
fn save_capture<'a>(path: &str, records: impl Iterator<Item = &'a Record>) -> anyhow::Result<usize> {
    anyhow::ensure!(!path.is_empty(), "no file name given");
    let mut writer = CaptureWriter::new(Vec::new())?;
    let mut count = 0;
    for record in records {
        writer.write_record(record)?;
        count += 1;
    }
    if let Some(parent) = Path::new(path).parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("cannot create the folder {}", parent.display()))?;
    }
    std::fs::write(path, writer.into_inner())?;
    Ok(count)
}

/// Reads a capture file into a store that keeps every record.
fn load_capture(path: &str) -> anyhow::Result<Store> {
    anyhow::ensure!(!path.is_empty(), "no file name given");
    let records = CaptureReader::open(path)?.read_all()?;
    Ok(Store::from_records(records))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::bridge::{Bridge, ConnState};
    use crate::settings::Settings;

    fn record(n: u8) -> Record {
        let mut r = Record::new(
            1,
            if n % 2 == 0 { Dir::C2s } else { Dir::S2c },
            Channel::Game,
            n,
            format!("PACKET_{n}"),
            Some(json!({ "n": n })),
            &[n, 1, 2],
        );
        r.state = Some("IN_GAME".into());
        r
    }

    #[test]
    fn save_then_open_round_trips_and_creates_folders() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("captures").join("one.jsonl");
        let path = path.to_str().unwrap();
        let records: Vec<Record> = (0..5).map(record).collect();

        assert_eq!(save_capture(path, records.iter()).unwrap(), 5);
        let store = load_capture(path).unwrap();
        assert_eq!(store.len(), 5);
        assert_eq!(store.records().cloned().collect::<Vec<_>>(), records);
    }

    #[test]
    fn a_saved_file_is_a_proxy_capture() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.jsonl");
        let record = record(3);
        save_capture(path.to_str().unwrap(), std::iter::once(&record)).unwrap();
        let reader = CaptureReader::open(&path).unwrap();
        assert_eq!(reader.header().format, "l2-capture");
        assert_eq!(reader.read_all().unwrap(), vec![record]);
    }

    #[test]
    fn open_reports_missing_and_foreign_files() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("nope.jsonl");
        assert!(load_capture(missing.to_str().unwrap()).is_err());
        assert!(load_capture("").is_err());

        let foreign = dir.path().join("foreign.jsonl");
        std::fs::write(
            &foreign,
            "{\"format\":\"other\",\"version\":1,\"tool\":\"x\",\"created_ms\":1}
",
        )
        .unwrap();
        let error = format!("{:#}", load_capture(foreign.to_str().unwrap()).unwrap_err());
        assert!(error.contains("not an l2-capture file"), "{error}");
    }

    #[test]
    fn saving_without_a_name_fails() {
        assert!(save_capture("", std::iter::empty()).is_err());
    }

    /// Draws the view for a few frames in a headless egui context (no window, no GPU), so
    /// layout mistakes that panic (nested panels, bad ids) show up in tests.
    fn run_frames(view: &mut PacketsView, settings: &mut Settings, frames: usize) {
        let bridge = Bridge::spawn(None).unwrap();
        let conn = ConnState::default();
        let launch = crate::launch::LaunchHandle::detached();
        let ctx = egui::Context::default();
        for _ in 0..frames {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1100.0, 760.0),
                )),
                ..egui::RawInput::default()
            };
            let mut output = ctx.run_ui(input, |ui| {
                let mut cx = AppCtx {
                    settings,
                    conn: &conn,
                    bridge: &bridge,
                    launch: &launch,
                };
                view.ui(ui, &mut cx);
            });
            // Nothing renders these; egui panics if they are dropped unhandled.
            output.textures_delta.clear();
        }
    }

    fn many_records(count: u32) -> Vec<Record> {
        (0..count)
            .map(|n| {
                let mut r = record(u8::try_from(n % 200).unwrap());
                r.fields =
                    (n % 3 != 0).then(|| json!({ "id": n, "list": [1, 2, { "deep": "x" }], "name": "abc" }));
                r
            })
            .collect()
    }

    #[test]
    fn the_view_draws_empty_with_records_selected_and_with_a_file() {
        let mut settings = Settings::default();
        let mut view = PacketsView::default();
        run_frames(&mut view, &mut settings, 2);

        view.live.push_all(many_records(5_000));
        run_frames(&mut view, &mut settings, 2);
        assert_eq!(view.live.visible_len(), 5_000);

        // Select a packet with fields and one without, and draw the detail pane.
        for id in [1, 3, 4_999] {
            view.selected = Some(id);
            run_frames(&mut view, &mut settings, 2);
        }

        // Filters apply on the next frame.
        settings.packets.hide_noisy = true;
        settings.packets.noisy = "PACKET_1, PACKET_2".into();
        view.filter.text = "packet".into();
        run_frames(&mut view, &mut settings, 2);
        assert!(view.live.visible_len() < 5_000);

        // A file replaces the list; "back to live" is the only way out, and live data stays.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.jsonl");
        save_capture(path.to_str().unwrap(), many_records(10).iter()).unwrap();
        view.file = Some(OpenFile {
            path: path.display().to_string(),
            store: load_capture(path.to_str().unwrap()).unwrap(),
        });
        view.selected = None;
        run_frames(&mut view, &mut settings, 2);
        assert_eq!(view.file.as_ref().unwrap().store.len(), 10);
        assert_eq!(view.live.len(), 5_000);
    }

    #[test]
    fn bad_addresses_do_not_start_the_proxy_and_the_view_still_draws() {
        let mut settings = Settings::default();
        settings.packets.listen = "not an address".into();
        let mut view = PacketsView::default();
        run_frames(&mut view, &mut settings, 1);
        assert_eq!(view.controller.state(), RunState::Stopped);
        assert!(proxy_control::parse_config(&settings.packets).is_err());
    }
}
