//! Small egui widgets shared by the XML forms: the on/off switch, colour swatches, a slider with
//! a number field, and the error text next to a field.

use egui::{Color32, Response, Ui, Vec2};

use super::model::{Issue, Row, messages_for};

/// An on/off switch (a toggle, not a check box).
pub fn switch(ui: &mut Ui, on: &mut bool) -> Response {
    let size = Vec2::new(34.0, 18.0);
    let (rect, mut response) = ui.allocate_exact_size(size, egui::Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    if ui.is_rect_visible(rect) {
        let t = ui.ctx().animate_bool_responsive(response.id, *on);
        let radius = rect.height() / 2.0;
        let off = Color32::from_gray(90);
        let on_color = Color32::from_rgb(0x2e, 0x9e, 0x5a);
        let fill = Color32::from_rgb(
            lerp(off.r(), on_color.r(), t),
            lerp(off.g(), on_color.g(), t),
            lerp(off.b(), on_color.b(), t),
        );
        ui.painter().rect_filled(rect, radius, fill);
        let x = egui::lerp((rect.left() + radius)..=(rect.right() - radius), t);
        ui.painter()
            .circle_filled(egui::pos2(x, rect.center().y), radius - 2.0, Color32::WHITE);
    }
    response.on_hover_text(if *on { "On" } else { "Off" })
}

fn lerp(a: u8, b: u8, t: f32) -> u8 {
    let v = f32::from(a) + (f32::from(b) - f32::from(a)) * t.clamp(0.0, 1.0);
    // 0..=255 by the clamp above
    v.round().clamp(0.0, 255.0) as u8
}

/// L2 stores colours as `BBGGRR`, so the hex digits of the file are read the other way round to
/// show what the client draws. Up to 6 hex digits; shorter text is zero-padded on the left.
pub fn l2_color(hex: &str) -> Option<Color32> {
    let hex = hex.trim();
    if hex.is_empty() || hex.len() > 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let value = u32::from_str_radix(hex, 16).ok()?;
    let (blue, green, red) = ((value >> 16) & 0xff, (value >> 8) & 0xff, value & 0xff);
    let byte = |v: u32| u8::try_from(v).unwrap_or(0);
    Some(Color32::from_rgb(byte(red), byte(green), byte(blue)))
}

/// The `BBGGRR` text of a colour (upper case, 6 digits).
pub fn l2_hex(color: Color32) -> String {
    format!("{:02X}{:02X}{:02X}", color.b(), color.g(), color.r())
}

/// A colour swatch: a filled square (or a crossed one when the text is no colour).
pub fn swatch(ui: &mut Ui, hex: &str) {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(18.0), egui::Sense::hover());
    match l2_color(hex) {
        Some(color) => {
            ui.painter().rect_filled(rect, 3.0, color);
            ui.painter().rect_stroke(
                rect,
                3.0,
                egui::Stroke::new(1.0, Color32::from_gray(120)),
                egui::StrokeKind::Inside,
            );
            response.on_hover_text("As the L2 client shows it (the digits are BBGGRR)");
        }
        None => {
            ui.painter().rect_stroke(
                rect,
                3.0,
                egui::Stroke::new(1.0, Color32::from_rgb(0xd9, 0x4f, 0x4f)),
                egui::StrokeKind::Inside,
            );
            ui.painter().line_segment(
                [rect.left_top(), rect.right_bottom()],
                egui::Stroke::new(1.0, Color32::from_rgb(0xd9, 0x4f, 0x4f)),
            );
        }
    }
}

/// Swatch, hex text field and a colour picker button for a `BBGGRR` value. True when changed.
pub fn color_cell(ui: &mut Ui, hex: &mut String, problem: bool) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        swatch(ui, hex);
        changed |= text_cell(ui, hex, 60.0, problem).changed();
        if let Some(color) = l2_color(hex) {
            let mut rgb = [color.r(), color.g(), color.b()];
            if ui.color_edit_button_srgb(&mut rgb).changed() {
                *hex = l2_hex(Color32::from_rgb(rgb[0], rgb[1], rgb[2]));
                changed = true;
            }
        }
    });
    changed
}

/// A one line text field of a fixed width; the text turns red with a `problem`.
pub fn text_cell(ui: &mut Ui, text: &mut String, width: f32, problem: bool) -> Response {
    let mut edit = egui::TextEdit::singleline(text).desired_width(width);
    if problem {
        edit = edit.text_color(Color32::from_rgb(0xd9, 0x4f, 0x4f));
    }
    ui.add(edit)
}

/// An integer with a slider and a number field. The field takes any `i32`; the slider only
/// covers `range` and clamps visually.
pub fn slider_int(ui: &mut Ui, value: &mut i32, range: std::ops::RangeInclusive<i32>, width: f32) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        let mut slider_value = (*value).clamp(*range.start(), *range.end());
        ui.spacing_mut().slider_width = width;
        if ui
            .add(egui::Slider::new(&mut slider_value, range.clone()).show_value(false))
            .changed()
        {
            *value = slider_value;
            changed = true;
        }
        changed |= ui.add(egui::DragValue::new(value).speed(0.2)).changed();
    });
    changed
}

/// A float with a slider and a number field (see [`slider_int`]).
pub fn slider_float(ui: &mut Ui, value: &mut f32, range: std::ops::RangeInclusive<f32>, width: f32) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        let mut slider_value = (*value).clamp(*range.start(), *range.end());
        ui.spacing_mut().slider_width = width;
        if ui
            .add(
                egui::Slider::new(&mut slider_value, range.clone())
                    .show_value(false)
                    .step_by(0.1),
            )
            .changed()
        {
            *value = (slider_value * 10.0).round() / 10.0;
            changed = true;
        }
        changed |= ui
            .add(egui::DragValue::new(value).speed(0.05).max_decimals(4))
            .changed();
    });
    changed
}

/// A text field bound to attribute `name` of `row`. The attribute is only created or changed
/// when the user types; an empty text removes an `optional` attribute. True when changed.
pub fn attr_cell(ui: &mut Ui, row: &mut Row, name: &str, width: f32, optional: bool, problem: bool) -> bool {
    let mut text = row.get_or(name, "").to_owned();
    if text_cell(ui, &mut text, width, problem).changed() {
        if text.is_empty() && optional {
            row.clear(name);
        } else {
            row.set(name, &text);
        }
        return true;
    }
    false
}

/// A switch bound to the boolean attribute `name` of `row` (`default` when it is missing).
pub fn flag_cell(ui: &mut Ui, row: &mut Row, name: &str, default: bool) -> bool {
    let mut on = row.flag(name, default);
    if switch(ui, &mut on).changed() {
        row.set_flag(name, on);
        return true;
    }
    false
}

/// A red "!" with the messages on hover, when `field` of `row` has any. Used in grids, where a
/// message line would break the table.
pub fn problem_mark(ui: &mut Ui, issues: &[Issue], row: Option<usize>, field: &str) {
    if let Some(message) = messages_for(issues, row, field) {
        let warning = issues
            .iter()
            .filter(|i| i.row == row && i.field == field)
            .all(|i| i.warning);
        let color = if warning {
            ui.visuals().warn_fg_color
        } else {
            ui.visuals().error_fg_color
        };
        ui.label(egui::RichText::new("!").strong().color(color))
            .on_hover_text(message);
    }
}

/// The list of problems on top of a form: one line each, `describe` names the row.
pub fn issue_list(ui: &mut Ui, issues: &[Issue], describe: &dyn Fn(usize) -> String) {
    for issue in issues {
        let color = if issue.warning {
            ui.visuals().warn_fg_color
        } else {
            ui.visuals().error_fg_color
        };
        let place = match issue.row {
            Some(row) => format!("{}: ", describe(row)),
            None => String::new(),
        };
        ui.colored_label(color, format!("{place}{}", issue.message));
    }
}

/// The red message(s) for one field of one row, if any.
pub fn show_problem(ui: &mut Ui, issues: &[Issue], row: Option<usize>, field: &str) {
    if let Some(message) = messages_for(issues, row, field) {
        let warning = issues
            .iter()
            .filter(|i| i.row == row && i.field == field)
            .all(|i| i.warning);
        let color = if warning {
            ui.visuals().warn_fg_color
        } else {
            ui.visuals().error_fg_color
        };
        ui.colored_label(color, message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_are_read_as_bbggrr() {
        // "ECF9A2": the default title colour, a pale cyan in the client.
        assert_eq!(l2_color("ECF9A2"), Some(Color32::from_rgb(0xA2, 0xF9, 0xEC)));
        assert_eq!(l2_color("FFFFFF"), Some(Color32::WHITE));
        assert_eq!(l2_color("0000C0"), Some(Color32::from_rgb(0xC0, 0, 0)));
        assert_eq!(l2_color("FF"), Some(Color32::from_rgb(0xFF, 0, 0)));
        assert_eq!(l2_color(""), None);
        assert_eq!(l2_color("12345G"), None);
        assert_eq!(l2_color("1234567"), None);
        assert_eq!(l2_hex(Color32::from_rgb(0xA2, 0xF9, 0xEC)), "ECF9A2");
    }
}
