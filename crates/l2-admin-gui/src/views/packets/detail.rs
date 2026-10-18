//! The detail pane: the decoded fields as a tree and the payload as a hex dump.
//!
//! The hex dump has the usual offset, hex and ASCII columns and a UTF-16 column: the payload is
//! read as little-endian `u16` pairs from the start of each 16-byte line, which is how L2
//! writes strings (names, chat). A string that starts at an odd offset shows up as noise in
//! that column; the ASCII column still shows its bytes.
//!
//! Clicking a field does not highlight its bytes: the decoded fields carry no byte offsets
//! yet (the capture format has none).

use egui::RichText;
use l2_proxy::Record;
use serde_json::Value;

use super::list::{channel_label, dir_arrow, dir_label, format_opcode, format_time, payload_len};

/// Bytes per hex dump line.
pub const BYTES_PER_LINE: usize = 16;
/// Array elements shown per array in the field tree (a long array would make the tree huge).
const MAX_ARRAY_ITEMS: usize = 500;

// ---------------------------------------------------------------------------------------------
// Hex dump
// ---------------------------------------------------------------------------------------------

/// One line of the hex dump, each column already formatted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HexLine {
    pub offset: usize,
    /// 16 byte slots of two characters, a gap after the eighth; short lines are padded with
    /// spaces so every line has the same width.
    pub hex: String,
    /// One character per byte: printable ASCII or `.`.
    pub ascii: String,
    /// One character per byte pair: the `u16` (little-endian) if it is a printable character,
    /// else `.`. A trailing odd byte has no character.
    pub utf16: String,
}

/// Splits `bytes` into dump lines.
pub fn hex_dump(bytes: &[u8]) -> Vec<HexLine> {
    bytes
        .chunks(BYTES_PER_LINE)
        .enumerate()
        .map(|(index, chunk)| HexLine {
            offset: index * BYTES_PER_LINE,
            hex: hex_column(chunk),
            ascii: chunk.iter().map(|b| ascii_char(*b)).collect(),
            utf16: chunk
                .chunks_exact(2)
                .map(|pair| utf16_char(u16::from_le_bytes([pair[0], pair[1]])))
                .collect(),
        })
        .collect()
}

fn hex_column(chunk: &[u8]) -> String {
    let mut out = String::with_capacity(BYTES_PER_LINE * 3 + 1);
    for slot in 0..BYTES_PER_LINE {
        if slot > 0 {
            out.push(' ');
            if slot == BYTES_PER_LINE / 2 {
                out.push(' ');
            }
        }
        match chunk.get(slot) {
            Some(byte) => out.push_str(&format!("{byte:02x}")),
            None => out.push_str("  "),
        }
    }
    out
}

fn ascii_char(byte: u8) -> char {
    if (0x20..0x7f).contains(&byte) {
        char::from(byte)
    } else {
        '.'
    }
}

fn utf16_char(unit: u16) -> char {
    match char::from_u32(u32::from(unit)) {
        // A lone surrogate is not a char; control characters and NUL show as a dot.
        Some(c) if !c.is_control() && (c.is_alphanumeric() || c.is_ascii_graphic() || c == ' ') => c,
        _ => '.',
    }
}

/// `0010  41 00 42 00 ...  |A.B.....|  AB..`
pub fn format_hex_line(line: &HexLine) -> String {
    format!(
        "{:04x}  {}  |{:<width$}|  {}",
        line.offset,
        line.hex,
        line.ascii,
        line.utf16,
        width = BYTES_PER_LINE
    )
}

/// The whole dump as text, one line per 16 bytes (what "Copy dump" puts on the clipboard).
pub fn hex_dump_text(bytes: &[u8]) -> String {
    hex_dump(bytes)
        .iter()
        .map(format_hex_line)
        .collect::<Vec<_>>()
        .join("\n")
}

/// The line above the columns of the dump.
pub fn hex_header() -> String {
    format!(
        "{:<4}  {:<hex_width$}  |{:<width$}|  utf-16",
        "off",
        "00 01 02 03 04 05 06 07  08 09 0a 0b 0c 0d 0e 0f",
        "ascii",
        hex_width = BYTES_PER_LINE * 3,
        width = BYTES_PER_LINE
    )
}

// ---------------------------------------------------------------------------------------------
// Field tree
// ---------------------------------------------------------------------------------------------

/// How a JSON scalar reads in the tree.
pub fn scalar_text(value: &Value) -> String {
    match value {
        Value::String(s) => format!("{s:?}"),
        other => other.to_string(),
    }
}

fn node(ui: &mut egui::Ui, label: &str, value: &Value, depth: usize) {
    match value {
        Value::Object(map) => {
            egui::CollapsingHeader::new(RichText::new(format!("{label} {{{}}}", map.len())).monospace())
                .id_salt(label)
                .default_open(depth < 2)
                .show(ui, |ui| {
                    for (key, child) in map {
                        node(ui, key, child, depth + 1);
                    }
                });
        }
        Value::Array(items) => {
            egui::CollapsingHeader::new(RichText::new(format!("{label} [{}]", items.len())).monospace())
                .id_salt(label)
                .default_open(depth < 2)
                .show(ui, |ui| {
                    for (index, child) in items.iter().take(MAX_ARRAY_ITEMS).enumerate() {
                        node(ui, &format!("[{index}]"), child, depth + 1);
                    }
                    if items.len() > MAX_ARRAY_ITEMS {
                        ui.weak(format!("... {} more", items.len() - MAX_ARRAY_ITEMS));
                    }
                });
        }
        scalar => {
            ui.label(RichText::new(format!("{label}: {}", scalar_text(scalar))).monospace());
        }
    }
}

fn fields_ui(ui: &mut egui::Ui, record: &Record) {
    match &record.fields {
        None | Some(Value::Null) => {
            ui.weak("No decoded fields: this packet is not decoded yet. The bytes are on the right.");
        }
        Some(Value::Object(map)) => {
            for (key, value) in map {
                node(ui, key, value, 0);
            }
        }
        Some(other) => node(ui, "value", other, 0),
    }
}

// ---------------------------------------------------------------------------------------------
// The pane
// ---------------------------------------------------------------------------------------------

/// One line describing the packet.
pub fn summary(record: &Record) -> String {
    let mut out = format!(
        "{}  {}  {} {}  {}  conn {}  {} bytes  {} UTC",
        record.name,
        format_opcode(record),
        dir_arrow(record.dir),
        dir_label(record.dir),
        channel_label(record.channel),
        record.conn,
        payload_len(record),
        format_time(record.ts_ms),
    );
    if let Some(state) = &record.state {
        out.push_str("  state ");
        out.push_str(state);
    }
    out
}

/// Draws the pane for `record` (`None` = nothing selected).
pub fn detail_ui(ui: &mut egui::Ui, record: Option<&Record>) {
    let Some(record) = record else {
        ui.weak("Select a packet to see its fields and bytes.");
        return;
    };
    let bytes = record.raw_bytes();

    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(summary(record)).strong());
        if ui.button("Copy hex").clicked() {
            ui.ctx().copy_text(record.raw.clone());
        }
        if ui
            .add_enabled(bytes.is_ok(), egui::Button::new("Copy dump"))
            .clicked()
            && let Ok(bytes) = &bytes
        {
            ui.ctx().copy_text(hex_dump_text(bytes));
        }
        if ui
            .add_enabled(record.fields.is_some(), egui::Button::new("Copy fields"))
            .clicked()
            && let Some(fields) = &record.fields
        {
            ui.ctx()
                .copy_text(serde_json::to_string_pretty(fields).unwrap_or_default());
        }
    });
    ui.separator();

    egui::Panel::left("packet_detail_fields")
        .resizable(true)
        .default_size(320.0)
        .show(ui, |ui| {
            ui.label(RichText::new("Fields").weak());
            egui::ScrollArea::vertical()
                .id_salt("packet_fields_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| fields_ui(ui, record));
        });
    egui::CentralPanel::default().show(ui, |ui| {
        ui.label(RichText::new(hex_header()).monospace().weak());
        match &bytes {
            Ok(bytes) => {
                let lines = hex_dump(bytes);
                let row_height = ui.text_style_height(&egui::TextStyle::Monospace);
                egui::ScrollArea::both()
                    .id_salt("packet_hex_scroll")
                    .auto_shrink([false, false])
                    .show_rows(ui, row_height, lines.len(), |ui, range| {
                        for line in &lines[range] {
                            ui.label(RichText::new(format_hex_line(line)).monospace());
                        }
                    });
            }
            Err(e) => {
                ui.colored_label(
                    ui.visuals().error_fg_color,
                    format!("The raw bytes are not valid hex: {e}"),
                );
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use l2_proxy::{Channel, Dir};
    use serde_json::json;

    use super::*;

    #[test]
    fn a_full_line_has_offset_hex_ascii_and_utf16() {
        // "Hi there" as UTF-16 LE: 16 bytes, every other one NUL.
        let mut bytes = Vec::new();
        for unit in "Hi there".encode_utf16() {
            bytes.extend(unit.to_le_bytes());
        }
        assert_eq!(bytes.len(), 16);
        let lines = hex_dump(&bytes);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].hex, "48 00 69 00 20 00 74 00  68 00 65 00 72 00 65 00");
        assert_eq!(lines[0].ascii, "H.i. .t.h.e.r.e.");
        assert_eq!(lines[0].utf16, "Hi there");
        assert_eq!(
            format_hex_line(&lines[0]),
            "0000  48 00 69 00 20 00 74 00  68 00 65 00 72 00 65 00  |H.i. .t.h.e.r.e.|  Hi there"
        );
    }

    #[test]
    fn utf16_column_reads_little_endian_pairs() {
        let mut bytes = Vec::new();
        for unit in "Ab Z".encode_utf16() {
            bytes.extend(unit.to_le_bytes());
        }
        bytes.extend([0x00, 0x00]); // NUL
        bytes.extend([0x01, 0x00]); // control
        bytes.extend([0x3d, 0x04]); // U+043D, a Cyrillic letter
        bytes.extend([0x00, 0xd8]); // a lone surrogate
        assert_eq!(bytes.len(), 16);
        let line = &hex_dump(&bytes)[0];
        assert_eq!(line.utf16, "Ab Z..н.");
    }

    #[test]
    fn short_and_multiple_lines() {
        let bytes: Vec<u8> = (0u8..20).collect();
        let lines = hex_dump(&bytes);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[1].offset, 16);
        assert_eq!(lines[1].hex, format!("10 11 12 13{}", " ".repeat(48 - 11)));
        assert_eq!(lines[1].ascii, "....");
        assert_eq!(lines[1].utf16.chars().count(), 2, "four bytes: two pairs");
        // The same width for every line, so the columns line up.
        assert_eq!(lines[0].hex.len(), lines[1].hex.len());
        assert_eq!(
            format_hex_line(&lines[1]).find('|'),
            format_hex_line(&lines[0]).find('|')
        );
        assert!(format_hex_line(&lines[1]).starts_with("0010  10 11 12 13 "));
    }

    #[test]
    fn an_odd_byte_count_has_no_character_for_the_last_byte() {
        let line = &hex_dump(&[0x41, 0x00, 0x42])[0];
        assert_eq!(line.utf16, "A");
        assert_eq!(line.ascii, "A.B");
    }

    #[test]
    fn empty_payload_has_no_lines() {
        assert!(hex_dump(&[]).is_empty());
        assert_eq!(hex_dump_text(&[]), "");
    }

    #[test]
    fn dump_text_joins_lines() {
        let text = hex_dump_text(&[0u8; 17]);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with("0000  00 00"));
        assert!(lines[1].starts_with("0010  00 "));
    }

    #[test]
    fn header_columns_line_up_with_the_lines() {
        let line = format_hex_line(&hex_dump(&[0u8; 16])[0]);
        let header = hex_header();
        assert_eq!(header.find('|'), line.find('|'));
        assert_eq!(header.find("00 01"), line.find("00 00"));
        assert_eq!(header.find("utf-16"), line.rfind('|').map(|i| i + 3));
    }

    #[test]
    fn scalars_read_naturally() {
        assert_eq!(scalar_text(&json!("abc")), "\"abc\"");
        assert_eq!(scalar_text(&json!(12)), "12");
        assert_eq!(scalar_text(&json!(true)), "true");
        assert_eq!(scalar_text(&json!(null)), "null");
    }

    #[test]
    fn summary_names_the_packet() {
        let mut record = Record::new(
            3,
            Dir::C2s,
            Channel::Game,
            0xD0,
            "REQUEST_GOTO_LOBBY",
            None,
            &[0xD0, 0x36, 0],
        );
        record.ex_opcode = Some(0x36);
        record.ts_ms = 1_700_000_000_123;
        record.state = Some("IN_GAME".into());
        assert_eq!(
            summary(&record),
            "REQUEST_GOTO_LOBBY  0xd0:0x0036  → client to server  game  conn 3  3 bytes  \
             22:13:20.123 UTC  state IN_GAME"
        );
    }
}
