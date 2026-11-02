//! In-place editing of `.ini` files for the config editors (GUI cards G-02, G-05).
// Java: none (tooling). The syntax is `java.util.Properties`, see `reader.rs`.
//!
//! [`IniDoc`] keeps the file as its physical lines *with their line terminators*. Changing a value
//! rewrites only the part of one line after the `=` (or `:`), so comments, blank lines, order,
//! spacing around the separator and `\n` / `\r\n` endings stay as they were. Saving a document
//! without changes gives back the same bytes.
//!
//! [`IniDoc::entries`] lists the keys in file order with the **help text** (the comment lines
//! directly above the key, no blank line in between) and the **group** (the title of the last
//! `# ----` / `# Title` / `# ----` header block above it). A `Default: X` line in the help is
//! returned as `comment_default`, a fallback for keys the typed config structs do not know.

use std::path::Path;

/// Errors of [`IniDoc::set_value`] and [`save_changes`].
#[derive(Debug, thiserror::Error)]
pub enum IniEditError {
    #[error("key '{0}' not found")]
    KeyNotFound(String),
    #[error("key '{0}' cannot be edited in place (its key continues on the next line)")]
    NotEditable(String),
    #[error("value of '{0}' must be one line")]
    MultiLine(String),
    #[error("value of '{0}' must not start with a blank or end with a backslash")]
    BadEdge(String),
    #[error("file is not valid UTF-8, refusing to rewrite it")]
    NotUtf8,
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
}

/// One key of a file, as the editor shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IniEntry {
    pub key: String,
    /// The value as written in the file (escapes not resolved; continuation lines joined).
    pub value: String,
    /// Comment lines directly above the key, without the `#` and one following blank.
    pub help: Vec<String>,
    /// Title of the header block the key sits under (empty before the first header).
    pub group: String,
    /// The text after `Default:` in the help, if any.
    pub comment_default: Option<String>,
    /// 1-based line of the key.
    pub line: usize,
    /// False when the value cannot be rewritten in place.
    pub editable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Line {
    text: String,
    /// `"\n"`, `"\r\n"`, `"\r"` or `""` (last line without terminator).
    eol: String,
}

/// A parsed file that remembers its exact layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IniDoc {
    lines: Vec<Line>,
}

/// Where an entry sits in `lines`.
struct Slot {
    first: usize,
    last: usize,
    /// Byte index in `lines[first].text` where the value starts.
    value_start: usize,
    entry: IniEntry,
}

fn is_blank(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\x0c')
}

fn trailing_backslashes_odd(text: &str) -> bool {
    text.chars().rev().take_while(|&c| c == '\\').count() % 2 == 1
}

/// `# -----` style header line (also `# =====`).
fn is_dash_line(comment_body: &str) -> bool {
    let body = comment_body.trim();
    body.chars().count() >= 3
        && body
            .chars()
            .all(|c| matches!(c, '-' | '=' | '_' | '*' | '#' | '~'))
}

fn comment_body(trimmed: &str) -> &str {
    let body = &trimmed[1..];
    body.strip_prefix(' ').unwrap_or(body)
}

impl IniDoc {
    /// Splits `text` into lines (`\n`, `\r\n` or `\r`, like `Properties.load`).
    pub fn parse(text: &str) -> Self {
        let mut lines = Vec::new();
        let mut start = 0;
        let bytes = text.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            match bytes[i] {
                b'\n' => {
                    lines.push(Line {
                        text: text[start..i].to_owned(),
                        eol: "\n".to_owned(),
                    });
                    i += 1;
                    start = i;
                }
                b'\r' => {
                    let crlf = bytes.get(i + 1) == Some(&b'\n');
                    lines.push(Line {
                        text: text[start..i].to_owned(),
                        eol: if crlf { "\r\n" } else { "\r" }.to_owned(),
                    });
                    i += if crlf { 2 } else { 1 };
                    start = i;
                }
                _ => i += 1,
            }
        }
        if start < text.len() {
            lines.push(Line {
                text: text[start..].to_owned(),
                eol: String::new(),
            });
        }
        Self { lines }
    }

    /// The whole file again; identical to the parsed text until [`Self::set_value`] changes a line.
    pub fn render(&self) -> String {
        let mut out = String::new();
        for line in &self.lines {
            out.push_str(&line.text);
            out.push_str(&line.eol);
        }
        out
    }

    /// All keys in file order (a repeated key appears once per occurrence; the last one wins in
    /// the server, and is the one [`Self::set_value`] changes).
    pub fn entries(&self) -> Vec<IniEntry> {
        self.scan().into_iter().map(|s| s.entry).collect()
    }

    fn scan(&self) -> Vec<Slot> {
        let mut slots = Vec::new();
        let mut group = String::new();
        // Indexes of the comment lines directly above the current line.
        let mut run: Vec<usize> = Vec::new();
        let mut i = 0;
        while i < self.lines.len() {
            let text = self.lines[i].text.as_str();
            let trimmed = text.trim_start_matches(is_blank);
            if trimmed.is_empty() {
                run.clear();
                i += 1;
                continue;
            }
            if trimmed.starts_with('#') || trimmed.starts_with('!') {
                if is_dash_line(comment_body(trimmed)) {
                    // A title block is the text between the previous dash line and this one.
                    if let Some(prev) = run.iter().rposition(|&r| self.is_dash(r)) {
                        let title: Vec<&str> = run[prev + 1..]
                            .iter()
                            .map(|&r| comment_body(self.lines[r].text.trim_start_matches(is_blank)).trim())
                            .filter(|t| !t.is_empty())
                            .collect();
                        if !title.is_empty() {
                            group = title.join(" ");
                        }
                    }
                }
                run.push(i);
                i += 1;
                continue;
            }

            // A key line: find the logical extent (continuation lines).
            let mut last = i;
            while trailing_backslashes_odd(&self.lines[last].text) && last + 1 < self.lines.len() {
                last += 1;
            }
            let (key, value_start, key_complete) = split_first_line(text);
            let mut value = text[value_start..].to_owned();
            for n in i + 1..=last {
                if trailing_backslashes_odd(&value) {
                    value.pop();
                }
                value.push_str(self.lines[n].text.trim_start_matches(is_blank));
            }
            let from = run.iter().rposition(|&r| self.is_dash(r)).map_or(0, |p| p + 1);
            let help: Vec<String> = run[from..]
                .iter()
                .map(|&r| comment_body(self.lines[r].text.trim_start_matches(is_blank)).to_owned())
                .collect();
            let comment_default = help
                .iter()
                .find_map(|h| h.trim().strip_prefix("Default:").map(|d| d.trim().to_owned()));
            let editable = key_complete || last == i;
            slots.push(Slot {
                first: i,
                last,
                value_start,
                entry: IniEntry {
                    key: key.to_owned(),
                    value,
                    help,
                    group: group.clone(),
                    comment_default,
                    line: i + 1,
                    editable,
                },
            });
            run.clear();
            i = last + 1;
        }
        slots
    }

    fn is_dash(&self, line: usize) -> bool {
        is_dash_line(comment_body(self.lines[line].text.trim_start_matches(is_blank)))
    }

    /// Replaces the value of `key` (the last occurrence) with `raw`, written exactly as it should
    /// stand in the file. Returns `Ok(false)` and changes nothing when the value is the same.
    ///
    /// A key whose value is empty (`Token =`) gets the new text right after the separator.
    pub fn set_value(&mut self, key: &str, raw: &str) -> Result<bool, IniEditError> {
        if raw.contains(['\n', '\r']) {
            return Err(IniEditError::MultiLine(key.to_owned()));
        }
        if raw.starts_with(is_blank) || trailing_backslashes_odd(raw) {
            return Err(IniEditError::BadEdge(key.to_owned()));
        }
        let slots = self.scan();
        let slot = slots
            .iter()
            .rev()
            .find(|s| s.entry.key == key)
            .ok_or_else(|| IniEditError::KeyNotFound(key.to_owned()))?;
        if !slot.entry.editable {
            return Err(IniEditError::NotEditable(key.to_owned()));
        }
        if slot.entry.value == raw {
            return Ok(false);
        }
        let eol = self.lines[slot.last].eol.clone();
        let mut text = self.lines[slot.first].text[..slot.value_start].to_owned();
        // A key without any separator (`Token`): add one so the value is not part of the key.
        if !text.contains(['=', ':']) && !text.ends_with(is_blank) {
            text.push_str(" = ");
        }
        if slot.entry.value.is_empty() && text.ends_with(['=', ':']) {
            text.push(' ');
        }
        text.push_str(raw);
        self.lines.splice(slot.first..=slot.last, [Line { text, eol }]);
        Ok(true)
    }
}

/// Key, byte index of the value start, and whether the key was fully on this line.
fn split_first_line(text: &str) -> (&str, usize, bool) {
    let start = text.len() - text.trim_start_matches(is_blank).len();
    let mut chars = text[start..].char_indices();
    let mut key_end = text.len();
    let mut complete = true;
    while let Some((off, c)) = chars.next() {
        if c == '\\' {
            if chars.next().is_none() {
                complete = false; // the key itself continues on the next line
            }
            continue;
        }
        if c == '=' || c == ':' || is_blank(c) {
            key_end = start + off;
            break;
        }
    }
    let key = &text[start..key_end];
    let rest = &text[key_end..];
    let after_blanks = rest.trim_start_matches(is_blank);
    let mut value_start = key_end + (rest.len() - after_blanks.len());
    if after_blanks.starts_with(['=', ':']) {
        value_start += 1;
        let tail = &text[value_start..];
        value_start += tail.len() - tail.trim_start_matches(is_blank).len();
    }
    (key, value_start, complete)
}

/// Applies `changes` (key, raw value) to the file at `path` and writes it back. Nothing is
/// written when no value really changes. Returns the number of keys changed.
///
/// The write goes to a temporary file next to the original, then replaces it, so a crash cannot
/// leave half a config.
pub fn save_changes(path: &Path, changes: &[(String, String)]) -> Result<usize, IniEditError> {
    let bytes = std::fs::read(path)?;
    let text = String::from_utf8(bytes).map_err(|_| IniEditError::NotUtf8)?;
    let (rendered, count) = apply_changes(&text, changes)?;
    if count > 0 {
        write_atomic(path, &rendered)?;
    }
    Ok(count)
}

/// [`save_changes`] without the file: the new text and the number of keys changed.
pub fn apply_changes(text: &str, changes: &[(String, String)]) -> Result<(String, usize), IniEditError> {
    let mut doc = IniDoc::parse(text);
    let mut count = 0;
    for (key, value) in changes {
        if doc.set_value(key, value)? {
            count += 1;
        }
    }
    Ok((doc.render(), count))
}

/// Writes `text` to `path` through a temporary file in the same folder.
pub fn write_atomic(path: &Path, text: &str) -> Result<(), IniEditError> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = std::path::PathBuf::from(tmp);
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Text of a new `GeoOverride.ini` (Rust-only, D-26 / D-36): the file is not part of `dist/`
/// because the game server runs without it. The editor offers it as a template, with the keys
/// the game server reads (`game-server/src/geo.rs`, `l2-geo/src/engine.rs`).
pub const GEO_OVERRIDE_TEMPLATE: &str = "\
# Rust-only geodata settings (D-26, D-36). Same syntax as the Java .ini files.
# A missing file or an empty path means: no override.

# Folder with a second geodata set (X_Y.l2j files). A relative path is relative to DatapackRoot.
# Default:
GeoDataOverridePath =

# Regions to read from the override folder instead of GeoDataPath, as X_Y separated by commas.
# Default:
GeoDataOverrideRegions =

# Number of worker threads of the path finding. 0 = half of the processors (at least 1).
# Default: 0
PathWorkerThreads = 0
";

/// Keys whose value must be masked in a UI and only written when changed: any key containing
/// `Password` or `Token`, and `Login` of `Database.ini` (the database user).
pub fn is_secret(file: &str, key: &str) -> bool {
    key.contains("Password") || key.contains("Token") || (file == "Database.ini" && key == "Login")
}

/// What to show instead of a secret value: nothing for an empty value, else dots.
pub fn mask_value(value: &str) -> String {
    if value.is_empty() {
        String::new()
    } else {
        "\u{2022}".repeat(8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "# ---------\n# Rates\n# ---------\n\n# XP\n# Default: 1\nRateXp = 1\nRateSp=2\r\n\n# A\n\n# B\nOther : x\nEmpty =\nTrailing = a b \nLast = 9";

    #[test]
    fn roundtrip_keeps_every_byte() {
        for text in [
            SAMPLE,
            "",
            "\n",
            "A=1\r\nB=2\r\n",
            "A=1\rB=2\r",
            "a\\\n  b\nc=1\n",
        ] {
            assert_eq!(IniDoc::parse(text).render(), text);
        }
    }

    #[test]
    fn lists_entries_with_help_and_group() {
        let doc = IniDoc::parse(SAMPLE);
        let e = doc.entries();
        let keys: Vec<_> = e.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys, ["RateXp", "RateSp", "Other", "Empty", "Trailing", "Last"]);
        assert_eq!(e[0].help, ["XP", "Default: 1"]);
        assert_eq!(e[0].comment_default.as_deref(), Some("1"));
        assert_eq!(e[0].group, "Rates");
        assert_eq!(e[0].line, 7);
        assert_eq!(e[1].value, "2");
        // The comment "A" is separated by a blank line, so it is not help of "Other".
        assert_eq!(e[2].help, ["B"]);
        assert_eq!(e[2].value, "x");
        assert_eq!(e[3].value, "");
        assert_eq!(e[4].value, "a b ");
        assert_eq!(e[5].value, "9");
    }

    #[test]
    fn one_change_touches_one_line() {
        let mut doc = IniDoc::parse(SAMPLE);
        assert!(doc.set_value("RateSp", "5").unwrap());
        assert_eq!(doc.render(), SAMPLE.replace("RateSp=2", "RateSp=5"));
        assert!(doc.set_value("Empty", "v").unwrap());
        assert!(doc.render().contains("\nEmpty = v\n"));
        assert!(doc.set_value("Last", "10").unwrap());
        assert!(doc.render().ends_with("Last = 10"));
        assert!(!doc.set_value("Last", "10").unwrap());
    }

    #[test]
    fn crlf_is_kept() {
        let mut doc = IniDoc::parse("A = 1\r\nB = 2\r\n");
        doc.set_value("A", "7").unwrap();
        assert_eq!(doc.render(), "A = 7\r\nB = 2\r\n");
    }

    #[test]
    fn continuation_value_is_replaced_whole() {
        let mut doc = IniDoc::parse("A = 1,\\\n    2\nB = 3\n");
        assert_eq!(doc.entries()[0].value, "1,2");
        doc.set_value("A", "9").unwrap();
        assert_eq!(doc.render(), "A = 9\nB = 3\n");
    }

    #[test]
    fn last_duplicate_wins() {
        let mut doc = IniDoc::parse("A = 1\nA = 2\n");
        doc.set_value("A", "3").unwrap();
        assert_eq!(doc.render(), "A = 1\nA = 3\n");
    }

    #[test]
    fn rejects_bad_values_and_unknown_keys() {
        let mut doc = IniDoc::parse("A = 1\n");
        assert!(matches!(
            doc.set_value("A", "x\ny"),
            Err(IniEditError::MultiLine(_))
        ));
        assert!(matches!(doc.set_value("A", " x"), Err(IniEditError::BadEdge(_))));
        assert!(matches!(doc.set_value("A", "x\\"), Err(IniEditError::BadEdge(_))));
        assert!(matches!(
            doc.set_value("Z", "x"),
            Err(IniEditError::KeyNotFound(_))
        ));
        assert_eq!(doc.render(), "A = 1\n");
    }

    #[test]
    fn apply_changes_counts_real_changes() {
        let (text, n) = apply_changes(
            "A=1\nB=2\n",
            &[("A".into(), "1".into()), ("B".into(), "3".into())],
        )
        .unwrap();
        assert_eq!((text.as_str(), n), ("A=1\nB=3\n", 1));
    }

    #[test]
    fn save_changes_writes_only_on_change() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("X.ini");
        std::fs::write(&path, "A=1\r\n# c\r\nB=2").unwrap();
        assert_eq!(save_changes(&path, &[("A".into(), "1".into())]).unwrap(), 0);
        assert_eq!(save_changes(&path, &[("B".into(), "5".into())]).unwrap(), 1);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "A=1\r\n# c\r\nB=5");
        assert!(!dir.path().join("X.ini.tmp").exists());
    }

    #[test]
    fn geo_override_template_lists_the_keys_the_server_reads() {
        let e = IniDoc::parse(GEO_OVERRIDE_TEMPLATE).entries();
        let keys: Vec<_> = e.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(
            keys,
            [
                "GeoDataOverridePath",
                "GeoDataOverrideRegions",
                "PathWorkerThreads"
            ]
        );
        assert!(
            e.iter()
                .all(|e| !e.help.is_empty() && e.comment_default.is_some())
        );
    }

    #[test]
    fn secrets_are_masked() {
        assert!(is_secret("Database.ini", "Password"));
        assert!(is_secret("Admin.ini", "Token"));
        assert!(is_secret("Database.ini", "Login"));
        assert!(!is_secret("Server.ini", "Login"));
        assert!(!is_secret("Database.ini", "URL"));
        assert_eq!(mask_value(""), "");
        assert!(!mask_value("hunter2").contains("hunter2"));
        assert_eq!(mask_value("a").chars().count(), 8);
    }
}
