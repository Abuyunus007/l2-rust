//! The control of one setting in the config editor: which one fits the key
//! ([`choose_control`], pure and tested) and how it is drawn ([`draw_entry`]) (card G-07).
//!
//! allowed words, list keys, colours) and the type of the value in the file ([`ValueKind`]).
//!
//! | value | control |
//! |---|---|
//! | secret (`Password`, `Token`, ...) | masked text field |
//! | `True` / `False` | on/off switch |
//! | enum-like key, current value among the listed ones | drop-down |
//! | enum-like key, current value *not* listed | text field with a warning |
//! | number with a range in the table | slider + number field |
//! | number without a range | drag value with `-` / `+` |
//! | hex colour key | text field + colour swatch |
//! | everything else (lists, times, words) | text field |
//!
//! Every control writes into the same text buffer as the plain text field would, so the save
//! path is the one of G-05 and a value outside the slider range can still be typed.

use l2_config::ini_edit::{IniEntry, mask_value};
use l2_config::ranges::{
    EnumOption, RangeSpec, enum_options, is_color_key, is_list_key, parse_color_bgr, range_for,
};

use super::{FileState, ValueKind, parse_float, server_word, values_equal};

/// How a setting is edited.
#[derive(Clone, Debug, PartialEq)]
pub enum Control {
    Secret,
    Switch,
    Slider(&'static RangeSpec),
    Drag,
    Dropdown(Vec<EnumOption>),
    Color,
    Text,
}

/// A [`Control`] and, when the value does not fit the control it would normally get, why the
/// plain text field is shown instead.
#[derive(Clone, Debug, PartialEq)]
pub struct Choice {
    pub control: Control,
    pub warning: Option<String>,
}

impl Choice {
    fn of(control: Control) -> Self {
        Self {
            control,
            warning: None,
        }
    }
}

/// What [`choose_control`] looks at. `file_value` is the value as it stands in the file (not the
/// text being edited), so the control does not change while the owner types.
pub struct ControlInput<'a> {
    pub rel: &'a str,
    pub key: &'a str,
    pub kind: ValueKind,
    pub file_value: &'a str,
    pub help: &'a [String],
    pub secret: bool,
}

fn same_option(option: &str, value: &str) -> bool {
    option.eq_ignore_ascii_case(value.trim()) || values_equal(option, value)
}

pub fn choose_control(input: &ControlInput<'_>) -> Choice {
    if input.secret {
        return Choice::of(Control::Secret);
    }
    if input.kind == ValueKind::Bool {
        return Choice::of(Control::Switch);
    }
    if is_list_key(input.key) {
        return Choice::of(Control::Text);
    }
    if let Some(options) = enum_options(input.key, input.help) {
        if options.iter().any(|o| same_option(&o.value, input.file_value)) {
            return Choice::of(Control::Dropdown(options));
        }
        let listed: Vec<&str> = options.iter().map(|o| o.value.as_str()).collect();
        return Choice {
            control: Control::Text,
            warning: Some(format!(
                "The value is not one of the listed ones ({}); the server may refuse it.",
                listed.join(", ")
            )),
        };
    }
    match input.kind {
        ValueKind::Int | ValueKind::Float => Choice::of(match range_for(input.rel, input.key) {
            Some(range) => Control::Slider(range),
            None => Control::Drag,
        }),
        _ if is_color_key(input.key) && parse_color_bgr(input.file_value).is_some() => {
            Choice::of(Control::Color)
        }
        _ => Choice::of(Control::Text),
    }
}

/// The text a slider writes for `v`: whole numbers for integer keys, the decimals of the step
/// for the others.
pub fn format_slider_value(range: &RangeSpec, kind: ValueKind, v: f64) -> String {
    if kind == ValueKind::Int || range.decimals() == 0 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v:.*}", range.decimals())
    }
}

/// `v` as short text without float noise (`1.1 + 0.1` is `1.2`, not `1.2000000000000002`).
pub fn format_float(v: f64) -> String {
    format!("{}", (v * 1e6).round() / 1e6)
}

fn bool_text(on: bool, like: &str) -> &'static str {
    // Keep the file's spelling (`True` / `true`).
    let capital = like.chars().next().is_none_or(char::is_uppercase);
    match (on, capital) {
        (true, true) => "True",
        (true, false) => "true",
        (false, true) => "False",
        (false, false) => "false",
    }
}

/// An on/off switch. Returns the response; `changed()` is set when it was clicked.
pub fn toggle_switch(ui: &mut egui::Ui, on: &mut bool) -> egui::Response {
    let size = ui.spacing().interact_size.y * egui::vec2(2.0, 1.0);
    let (rect, mut response) = ui.allocate_exact_size(size, egui::Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    response.widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, ui.is_enabled(), *on, ""));
    if ui.is_rect_visible(rect) {
        let how_on = ui.ctx().animate_bool_responsive(response.id, *on);
        let visuals = ui.style().interact_selectable(&response, *on);
        let rect = rect.expand(visuals.expansion);
        let radius = 0.5 * rect.height();
        let fill = if *on {
            ui.visuals().selection.bg_fill
        } else {
            visuals.bg_fill
        };
        ui.painter()
            .rect(rect, radius, fill, visuals.bg_stroke, egui::StrokeKind::Inside);
        let x = egui::lerp((rect.left() + radius)..=(rect.right() - radius), how_on);
        ui.painter().circle(
            egui::pos2(x, rect.center().y),
            0.75 * radius,
            visuals.fg_stroke.color,
            egui::Stroke::NONE,
        );
    }
    response
}

const BAD: egui::Color32 = egui::Color32::from_rgb(0xd9, 0x4f, 0x4f);

fn text_field(ui: &mut egui::Ui, buffer: &mut String, width: f32, bad: bool, editable: bool) {
    // In a narrow column (two-column layout) the field gives way instead of running over.
    let width = width.min(ui.available_width() - 40.0).max(70.0);
    let mut edit = egui::TextEdit::singleline(buffer).desired_width(width);
    if bad {
        edit = edit.text_color(BAD);
    }
    ui.add_enabled(editable, edit);
}

fn draw_slider(
    ui: &mut egui::Ui,
    range: &RangeSpec,
    kind: ValueKind,
    buffer: &mut String,
    editable: bool,
    bad: bool,
) {
    let parsed = parse_float(buffer);
    let mut v = parsed.unwrap_or(range.min);
    ui.scope(|ui| {
        ui.spacing_mut().slider_width = 180.0;
        // `Never`: a value outside the range stays as it is (the handle just rests at the end);
        // only a drag of the handle writes.
        let slider = egui::Slider::new(&mut v, range.min..=range.max)
            .step_by(range.step)
            .show_value(false)
            .clamping(egui::SliderClamping::Never);
        if ui.add_enabled(editable && parsed.is_some(), slider).changed() {
            *buffer = format_slider_value(range, kind, v);
        }
    });
    text_field(ui, buffer, 90.0, bad, editable);
    if let Some(unit) = range.unit() {
        ui.weak(unit);
    }
}

fn draw_drag(ui: &mut egui::Ui, kind: ValueKind, buffer: &mut String, editable: bool, bad: bool) {
    let int = kind == ValueKind::Int;
    let parsed = if int {
        buffer.parse::<i64>().ok().map(|v| v as f64)
    } else {
        parse_float(buffer)
    };
    let Some(current) = parsed else {
        // Not a number (yet): a plain field, the problem line says why.
        text_field(ui, buffer, 140.0, bad, editable);
        return;
    };
    let unit = if int { 1.0 } else { 0.1 };
    let mut next = current;
    ui.add_enabled_ui(editable, |ui| {
        if ui.small_button("\u{2212}").clicked() {
            next -= unit;
        }
        let speed = (current.abs() * 0.005).max(if int { 1.0 } else { 0.01 });
        let mut drag = egui::DragValue::new(&mut next).speed(speed);
        if int {
            drag = drag.max_decimals(0);
        } else {
            drag = drag.max_decimals(6);
        }
        ui.add(drag);
        if ui.small_button("+").clicked() {
            next += unit;
        }
    });
    if next != current {
        *buffer = if int {
            format!("{}", next.round() as i64)
        } else {
            format_float(next)
        };
    }
}

fn draw_dropdown(ui: &mut egui::Ui, salt: &str, options: &[EnumOption], buffer: &mut String, editable: bool) {
    let current = options.iter().find(|o| same_option(&o.value, buffer));
    let shown = current.map_or(buffer.as_str(), |o| o.label.as_str()).to_owned();
    ui.add_enabled_ui(editable, |ui| {
        egui::ComboBox::from_id_salt(salt)
            .selected_text(shown)
            .width(260.0)
            .show_ui(ui, |ui| {
                for option in options {
                    let selected = current.is_some_and(|c| c.value == option.value);
                    if ui.selectable_label(selected, &option.label).clicked() && !selected {
                        buffer.clone_from(&option.value);
                    }
                }
            });
    });
}

fn draw_color(ui: &mut egui::Ui, buffer: &mut String, editable: bool, bad: bool) {
    text_field(ui, buffer, 80.0, bad, editable);
    let (rect, response) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), egui::Sense::hover());
    match parse_color_bgr(buffer) {
        Some((r, g, b)) => {
            ui.painter()
                .rect_filled(rect, 3.0, egui::Color32::from_rgb(r, g, b));
            ui.painter().rect_stroke(
                rect,
                3.0,
                ui.visuals().widgets.noninteractive.fg_stroke,
                egui::StrokeKind::Inside,
            );
            response.on_hover_text("Colour as the game client shows it (the digits are BBGGRR)");
        }
        None => {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "?",
                egui::FontId::proportional(14.0),
                BAD,
            );
            response.on_hover_text("Six hex digits expected, like 808080");
        }
    }
}

/// Draws one setting: name, markers, control, default, problem and help. `show_file` adds the
/// file name in small print (the topic view mixes files; the by-file view does not need it).
pub fn draw_entry(ui: &mut egui::Ui, file: &mut FileState, entry: &IniEntry, show_file: bool) {
    let default = file.default_of(entry);
    let kind = file.kind_of(entry);
    let secret = file.is_secret(&entry.key);
    let differs = file.differs_from_default(entry);
    let problem = file.problem(entry);
    let dirty = file.is_dirty(entry);
    let choice = choose_control(&ControlInput {
        rel: &file.rel,
        key: &entry.key,
        kind,
        file_value: &entry.value,
        help: &entry.help,
        secret,
    });
    let salt = format!("{}/{}/{}", server_word(file.side), file.rel, entry.key);
    ui.push_id(&salt, |ui| {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let mut name = egui::RichText::new(&entry.key).strong();
            if dirty {
                name = name.italics();
            }
            ui.label(name);
            if dirty {
                ui.label("*").on_hover_text("Changed, not saved yet");
            }
            if differs {
                let shown = match (&default, secret) {
                    (Some(d), false) => format!("Differs from the default ({d})"),
                    _ => "Differs from the default".to_owned(),
                };
                ui.colored_label(egui::Color32::from_rgb(0xe0, 0xa0, 0x30), "\u{25cf}")
                    .on_hover_text(shown);
            }
            if show_file {
                ui.weak(egui::RichText::new(format!("{} / {}", server_word(file.side), file.rel)).small());
            }
            if default.is_some() && differs && ui.small_button("Reset to default").clicked() {
                file.reset_to_default(entry);
            }
        });
        let Some(buffer) = file.buffers.get_mut(&entry.key) else {
            return;
        };
        let editable = entry.editable;
        let bad = problem.is_some();
        ui.horizontal_wrapped(|ui| {
            match &choice.control {
                Control::Secret => {
                    ui.add_enabled(
                        editable,
                        egui::TextEdit::singleline(buffer)
                            .password(true)
                            .desired_width(320.0),
                    )
                    .on_hover_text(format!(
                        "Hidden ({}). Only written when changed.",
                        mask_value(&entry.value)
                    ));
                }
                Control::Switch => {
                    let mut on = buffer.trim().eq_ignore_ascii_case("true");
                    let response = ui.add_enabled_ui(editable, |ui| toggle_switch(ui, &mut on)).inner;
                    if response.changed() {
                        *buffer = bool_text(on, &entry.value).to_owned();
                    }
                    ui.label(buffer.as_str());
                }
                Control::Slider(range) => draw_slider(ui, range, kind, buffer, editable, bad),
                Control::Drag => draw_drag(ui, kind, buffer, editable, bad),
                Control::Dropdown(options) => draw_dropdown(ui, &salt, options, buffer, editable),
                Control::Color => draw_color(ui, buffer, editable, bad),
                Control::Text => text_field(ui, buffer, 420.0, bad, editable),
            }
            if let (Some(d), false) = (&default, secret) {
                ui.weak(format!("default: {d}"));
            }
        });
        if let Some(problem) = problem {
            ui.colored_label(ui.visuals().error_fg_color, problem);
        }
        if let Some(warning) = &choice.warning {
            ui.colored_label(ui.visuals().warn_fg_color, warning.as_str());
        }
        for line in &entry.help {
            ui.add(egui::Label::new(egui::RichText::new(line).weak().small()).wrap());
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pick(rel: &str, key: &str, kind: ValueKind, value: &str, help: &[&str]) -> Choice {
        let help: Vec<String> = help.iter().map(|s| (*s).to_owned()).collect();
        choose_control(&ControlInput {
            rel,
            key,
            kind,
            file_value: value,
            help: &help,
            secret: false,
        })
    }

    #[test]
    fn booleans_get_a_switch() {
        assert_eq!(
            pick(
                "Rates.ini",
                "UseQuestRewardMultipliers",
                ValueKind::Bool,
                "False",
                &[]
            )
            .control,
            Control::Switch
        );
    }

    #[test]
    fn secrets_are_masked_before_anything_else() {
        let c = choose_control(&ControlInput {
            rel: "Database.ini",
            key: "Password",
            kind: ValueKind::Text,
            file_value: "x",
            help: &[],
            secret: true,
        });
        assert_eq!(c.control, Control::Secret);
    }

    #[test]
    fn numbers_with_a_range_get_a_slider_and_the_others_a_drag_value() {
        let c = pick("Rates.ini", "RateXp", ValueKind::Float, "1", &[]);
        let Control::Slider(r) = c.control else {
            panic!("{c:?}")
        };
        assert_eq!((r.min, r.max), (0.0, 100.0));
        let c = pick("Server.ini", "LoginserverPort", ValueKind::Int, "2106", &[]);
        assert!(matches!(c.control, Control::Slider(r) if r.max == 65535.0));
        // No entry in the table: a drag value.
        assert_eq!(
            pick(
                "General.ini",
                "MultisellAmountLimit",
                ValueKind::Int,
                "10000",
                &[]
            )
            .control,
            Control::Drag
        );
        assert_eq!(
            pick("Player.ini", "MaxAdena", ValueKind::Int, "3000000000000000", &[]).control,
            Control::Drag
        );
    }

    #[test]
    fn enum_keys_get_a_drop_down_when_the_value_is_listed() {
        let help = [
            "Pathfinding options:",
            "0 = Disabled",
            "1 = Enabled using path node files.",
            "2 = Enabled at runtime.",
        ];
        let c = pick("GeoEngine.ini", "PathFinding", ValueKind::Int, "2", &help);
        let Control::Dropdown(options) = c.control else {
            panic!("{c:?}")
        };
        assert_eq!(options.len(), 3);
        assert!(c.warning.is_none());
        // Words match without regard to case.
        let help = ["Available Options: bow, cast, all"];
        assert!(matches!(
            pick("Player.ini", "AltGameCancelByHit", ValueKind::Text, "CAST", &help).control,
            Control::Dropdown(_)
        ));
    }

    #[test]
    fn a_value_that_is_not_listed_gets_a_text_field_with_a_warning() {
        let help = ["Available Options: bow, cast, all"];
        let c = pick(
            "Player.ini",
            "AltGameCancelByHit",
            ValueKind::Text,
            "sometimes",
            &help,
        );
        assert_eq!(c.control, Control::Text);
        assert!(c.warning.is_some_and(|w| w.contains("bow, cast, all")));
        let c = pick(
            "GeoEngine.ini",
            "PathFinding",
            ValueKind::Int,
            "7",
            &["0 = a", "1 = b"],
        );
        assert_eq!(c.control, Control::Text);
        assert!(c.warning.is_some());
    }

    #[test]
    fn lists_times_and_words_are_text_fields() {
        for (key, value) in [
            ("LogItemsIdsList", "4356"),
            ("ServerRestartDays", "4"),
            ("ListOfProtectedItems", "0"),
        ] {
            assert_eq!(
                pick("General.ini", key, ValueKind::Text, value, &[]).control,
                Control::Text,
                "{key}"
            );
        }
        assert_eq!(
            pick("Rates.ini", "BossDropList", ValueKind::Text, "4356,1,2,100;", &[]).control,
            Control::Text
        );
        assert_eq!(
            pick("Olympiad.ini", "OlympiadPeriod", ValueKind::Text, "MONTH", &[]).control,
            Control::Text
        );
        assert_eq!(
            pick(
                "Server.ini",
                "ServerRestartSchedule",
                ValueKind::Text,
                "08:00",
                &[]
            )
            .control,
            Control::Text
        );
    }

    #[test]
    fn colour_keys_get_a_swatch_only_for_hex_colours() {
        assert_eq!(
            pick(
                "Custom/FactionSystem.ini",
                "GoodNameColor",
                ValueKind::Text,
                "00FF00",
                &[]
            )
            .control,
            Control::Color
        );
        // A value that is not six hex digits stays a plain field.
        assert_eq!(
            pick(
                "Custom/FactionSystem.ini",
                "GoodNameColor",
                ValueKind::Text,
                "green",
                &[]
            )
            .control,
            Control::Text
        );
    }

    #[test]
    fn a_range_does_not_apply_to_a_text_value() {
        // `RateXp` empty or garbage: the kind is Text, so no slider.
        assert_eq!(
            pick("Rates.ini", "RateXp", ValueKind::Text, "fast", &[]).control,
            Control::Text
        );
    }

    #[test]
    fn slider_values_are_written_cleanly() {
        let int = RangeSpec {
            file: "",
            pattern: "x",
            min: 0.0,
            max: 100.0,
            step: 1.0,
            unit: "",
        };
        assert_eq!(format_slider_value(&int, ValueKind::Int, 42.0), "42");
        assert_eq!(format_slider_value(&int, ValueKind::Float, 42.0), "42");
        let dec = RangeSpec { step: 0.05, ..int };
        assert_eq!(format_slider_value(&dec, ValueKind::Float, 0.35), "0.35");
        assert_eq!(format_slider_value(&dec, ValueKind::Int, 0.6), "1");
        assert_eq!(format_float(1.1 + 0.1), "1.2");
        assert_eq!(format_float(3.0), "3");
        assert_eq!(format_float(-0.5), "-0.5");
    }

    #[test]
    fn the_switch_keeps_the_spelling_of_the_file() {
        assert_eq!(bool_text(true, "False"), "True");
        assert_eq!(bool_text(false, "true"), "false");
        assert_eq!(bool_text(true, ""), "True");
    }
}
