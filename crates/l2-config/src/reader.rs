//! `.ini` reader with the semantics of Java's `ConfigReader`.
// Java: org/l2jmobius/commons/util/ConfigReader.java (+ java.util.Properties, commons/time/TimeUtil.parseDuration)
//!
//! Despite the name, L2JMobius `.ini` files are plain `java.util.Properties` files, so this is
//! a port of `Properties.load(Reader)` plus the typed getters of `ConfigReader`.
//!
//! # File loading
//! * A missing file is **not** an error: one warning, then every getter returns its default
//!   (and warns again, "not found in file"). Same when the file cannot be read.
//! * The text is decoded as UTF-8 (`Charset.defaultCharset()` on Java 18+). Invalid bytes become
//!   U+FFFD, like Java's lenient decoder. A BOM is *not* stripped (Java does not either).
//! * The only hard error is a malformed `\uXXXX` escape (Java throws an uncaught
//!   `IllegalArgumentException`): [`ConfigError::MalformedUnicodeEscape`].
//!
//! # `Properties` syntax (all kept)
//! * Line ends: `\n`, `\r` or `\r\n`. Blank lines are ignored.
//! * A line whose first non-blank char is `#` or `!` is a comment. Comments only exist as whole
//!   lines: `Key = 5 # note` makes the value `5 # note`.
//! * Key/value separator: the first unescaped `=`, `:` or blank (space, tab, form feed). Blanks
//!   around the separator are skipped (`A=1`, `A = 1`, `A: 1` and `A 1` are the same).
//! * Leading blanks of the value are removed, **trailing blanks are kept**. `Port = 7777 ` has
//!   the value `7777 ` and then fails integer parsing (see below).
//! * A line ending in an odd number of backslashes continues on the next line (the next line's
//!   leading blanks are dropped; a `#` there is *not* a comment).
//! * Escapes in keys and values: `\t \r \n \f \uXXXX`; any other `\c` is just `c`.
//! * A repeated key: the last one wins.
//! * Keys are case-sensitive.
//!
//! # Typed getters
//! Every getter takes a default. A missing key logs a warning and returns the default; so does a
//! value that does not parse (the getters never fail). The warning texts mirror Java's.
//!
//! * No trimming for booleans, integers, enums and durations. `" 5"`, `"5 "` -> invalid.
//! * Integers: `Byte/Short/Integer/Long.parse*`: optional `+`/`-`, decimal digits, range checked.
//!   Non-ASCII Unicode digits (which Java accepts) are not supported.
//! * Floats: `Float/Double.parseDouble` semantics: surrounding whitespace is trimmed, `1.`, `.5`,
//!   `1e3`, `NaN`, `Infinity` and a trailing `f`/`F`/`d`/`D` are valid; `inf`/`nan` in other
//!   spellings and hexadecimal floats are not.
//! * Enums: exact, case-sensitive name match (the caller's `FromStr`).
//! * Durations: see [`parse_duration`].
//! * Int arrays: see [`IniFile::get_int_array`].

use std::collections::BTreeMap;
use std::fmt::{Debug, Display};
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use tracing::warn;

use crate::error::ConfigError;

/// One parsed `.ini` file. The port of `ConfigReader`.
#[derive(Debug, Clone, Default)]
pub struct IniFile {
    /// `File.getName()`: only used in log messages.
    name: String,
    values: BTreeMap<String, String>,
}

impl IniFile {
    /// Loads a file like `new ConfigReader(path)`.
    ///
    /// A missing or unreadable file gives an empty `IniFile` (and a warning), exactly like Java.
    /// Errors only for a malformed `\uXXXX` escape.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let path = path.as_ref();
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();

        if !path.exists() {
            let shown = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
            warn!("Configuration file not found: {}", shown.display());
            return Ok(Self::empty(name));
        }
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(e) => {
                warn!("Failed to load configurations from {name}: {e}");
                return Ok(Self::empty(name));
            }
        };
        Self::parse(name, &String::from_utf8_lossy(&bytes))
    }

    /// Parses `.ini` text. `name` is only used in log messages (Java: the file name).
    pub fn parse(name: impl Into<String>, text: &str) -> Result<Self, ConfigError> {
        let mut file = Self::empty(name.into());
        let chars: Vec<char> = text.chars().collect();
        let mut lines = LineReader {
            chars: &chars,
            pos: 0,
        };

        while let Some(line) = lines.read_line() {
            let (key, value) = split_key_value(&line);
            let malformed = || ConfigError::MalformedUnicodeEscape {
                file: file.name.clone(),
            };
            let key = load_convert(key).ok_or_else(malformed)?;
            let value = load_convert(value).ok_or_else(malformed)?;
            file.values.insert(key, value);
        }
        Ok(file)
    }

    /// An `IniFile` without any keys.
    pub fn empty(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            values: BTreeMap::new(),
        }
    }

    /// The file name used in log messages.
    pub fn file_name(&self) -> &str {
        &self.name
    }

    /// `containsKey`.
    pub fn contains_key(&self, key: &str) -> bool {
        self.values.contains_key(key)
    }

    /// `getValue`: the raw value, `None` if the key does not exist. Does not log.
    pub fn get_value(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    /// `getStringPropertyNames`: all keys, sorted (Java's order is unspecified).
    ///
    /// For pattern keys such as `BufferPool.*.Size`.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.values.keys().map(String::as_str)
    }

    /// Number of keys.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// True when the file has no keys (also the case for a missing file).
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    // ---- typed getters ----

    /// `getBoolean`.
    ///
    /// NOTE(java-quirk): `Boolean.parseBoolean` never fails. Only `true` (any case, no
    /// surrounding blanks) is true; `yes`, `1`, `True ` and typos are silently `false`, and the
    /// default is used for a *missing* key only.
    pub fn get_bool(&self, key: &str, default: bool) -> bool {
        self.get_with(key, "boolean", default, |v| Some(v.eq_ignore_ascii_case("true")))
    }

    /// `getByte`.
    pub fn get_byte(&self, key: &str, default: i8) -> i8 {
        self.get_with(key, "byte", default, |v| v.parse().ok())
    }

    /// `getShort`.
    pub fn get_short(&self, key: &str, default: i16) -> i16 {
        self.get_with(key, "short", default, |v| v.parse().ok())
    }

    /// `getInt`.
    pub fn get_int(&self, key: &str, default: i32) -> i32 {
        self.get_with(key, "int", default, |v| v.parse().ok())
    }

    /// `getLong`.
    pub fn get_long(&self, key: &str, default: i64) -> i64 {
        self.get_with(key, "long", default, |v| v.parse().ok())
    }

    /// `getFloat`.
    pub fn get_float(&self, key: &str, default: f32) -> f32 {
        self.get_with(key, "float", default, parse_java_float)
    }

    /// `getDouble`.
    pub fn get_double(&self, key: &str, default: f64) -> f64 {
        self.get_with(key, "double", default, parse_java_float)
    }

    /// `getString`: the raw value (trailing blanks included), or the default when missing.
    pub fn get_string(&self, key: &str, default: &str) -> String {
        match self.get_value(key) {
            Some(value) => value.to_owned(),
            None => {
                self.warn_missing(key, default);
                default.to_owned()
            }
        }
    }

    /// `getEnum`: `Enum.valueOf`, an exact case-sensitive match of the variant name. `T::from_str`
    /// must behave that way; the config generator (M2) emits such impls.
    pub fn get_enum<T: FromStr + Debug>(&self, key: &str, default: T) -> T {
        match self.get_value(key) {
            Some(value) => value.parse().unwrap_or_else(|_| {
                self.warn_invalid("enum", key, &format_args!("{default:?}"));
                default
            }),
            None => {
                self.warn_missing(key, format_args!("{default:?}"));
                default
            }
        }
    }

    /// `getDuration`: `5min`, `2hours`, ... (see [`parse_duration`]). The default is *text* too.
    ///
    /// Errors only if the default itself is not a valid duration (Java throws then as well).
    pub fn get_duration(&self, key: &str, default: &str) -> Result<Duration, ConfigError> {
        match self.get_value(key) {
            Some(value) => match parse_duration(value) {
                Ok(duration) => return Ok(duration),
                Err(_) => self.warn_invalid("duration", key, &default),
            },
            None => self.warn_missing(key, default),
        }
        parse_duration(default)
    }

    /// `getIntArray`: splits the value at `delimiter`, trims every piece, parses them as `i32`.
    ///
    /// NOTE(java-quirk): Java passes `delimiter` to `String.split`, i.e. it is a *regex*. Here it
    /// is a literal string (no ini file uses a regex delimiter). The rest is Java's `split`:
    /// trailing empty pieces are dropped (`"1,2,"` is `[1, 2]`, `","` is `[]`), an empty piece in
    /// the middle is a parse error. A bad value falls back to the default; a bad *default*
    /// gives an empty array.
    pub fn get_int_array(&self, key: &str, delimiter: &str, default: &str) -> Vec<i32> {
        match self.get_value(key) {
            Some(value) => match parse_int_array(value, delimiter) {
                Some(array) => return array,
                None => warn!(
                    "Invalid int array for config '{key}' in file '{}', using default values.",
                    self.name
                ),
            },
            None => warn!(
                "Config '{key}' not found in file '{}', using default values.",
                self.name
            ),
        }
        parse_int_array(default, delimiter).unwrap_or_else(|| {
            warn!(
                "Invalid default values for config '{key}' in file '{}', using empty array.",
                self.name
            );
            Vec::new()
        })
    }

    // ---- helpers ----

    /// Shared body of the scalar getters: missing -> warn + default, unparsable -> warn + default.
    fn get_with<T: Copy + Debug>(
        &self,
        key: &str,
        kind: &str,
        default: T,
        parse: impl FnOnce(&str) -> Option<T>,
    ) -> T {
        match self.get_value(key) {
            Some(value) => parse(value).unwrap_or_else(|| {
                self.warn_invalid(kind, key, &format_args!("{default:?}"));
                default
            }),
            None => {
                self.warn_missing(key, format_args!("{default:?}"));
                default
            }
        }
    }

    fn warn_missing(&self, key: &str, default: impl Display) {
        warn!(
            "Config '{key}' not found in file '{}', using default: {default}.",
            self.name
        );
    }

    fn warn_invalid(&self, kind: &str, key: &str, default: &dyn Display) {
        warn!(
            "Invalid {kind} for config '{key}' in file '{}', using default: {default}.",
            self.name
        );
    }
}

// ---- Properties.load port ----

/// Port of `Properties.LineReader.readLine`: yields one *logical* line (comments and blank lines
/// skipped, continuations joined, leading blanks removed), still escaped.
struct LineReader<'a> {
    chars: &'a [char],
    pos: usize,
}

impl LineReader<'_> {
    fn read_line(&mut self) -> Option<Vec<char>> {
        let mut buf: Vec<char> = Vec::new();
        let mut skip_white_space = true;
        let mut is_comment_line = false;
        let mut is_new_line = true;
        let mut appended_line_begin = false;
        let mut preceding_backslash = false;
        let mut skip_lf = false;

        loop {
            let Some(&c) = self.chars.get(self.pos) else {
                if buf.is_empty() || is_comment_line {
                    return None;
                }
                if preceding_backslash {
                    buf.pop();
                }
                return Some(buf);
            };
            self.pos += 1;

            if skip_lf {
                skip_lf = false;
                if c == '\n' {
                    continue;
                }
            }
            if skip_white_space {
                if c == ' ' || c == '\t' || c == '\x0c' {
                    continue;
                }
                if !appended_line_begin && (c == '\r' || c == '\n') {
                    continue;
                }
                skip_white_space = false;
                appended_line_begin = false;
            }
            if is_new_line {
                is_new_line = false;
                if c == '#' || c == '!' {
                    is_comment_line = true;
                    continue;
                }
            }

            if c != '\n' && c != '\r' {
                buf.push(c);
                // Flip the flag for every backslash: an even run is escaped backslashes.
                preceding_backslash = c == '\\' && !preceding_backslash;
            } else {
                if is_comment_line || buf.is_empty() {
                    is_comment_line = false;
                    is_new_line = true;
                    skip_white_space = true;
                    buf.clear();
                    continue;
                }
                if self.pos >= self.chars.len() {
                    // NOTE(java-quirk): a file ending in "\\\n" yields an empty key with an
                    // empty value, because the backslash is dropped after the empty-line check.
                    if preceding_backslash {
                        buf.pop();
                    }
                    return Some(buf);
                }
                if preceding_backslash {
                    // The backslash at the end of the line is not part of the value.
                    buf.pop();
                    skip_white_space = true;
                    appended_line_begin = true;
                    preceding_backslash = false;
                    if c == '\r' {
                        skip_lf = true;
                    }
                } else {
                    return Some(buf);
                }
            }
        }
    }
}

/// Port of the key/value split in `Properties.load0`. Returns the still-escaped key and value.
fn split_key_value(line: &[char]) -> (&[char], &[char]) {
    let limit = line.len();
    let mut key_len = 0;
    let mut value_start = limit;
    let mut has_separator = false;
    let mut preceding_backslash = false;

    while key_len < limit {
        let c = line[key_len];
        if (c == '=' || c == ':') && !preceding_backslash {
            value_start = key_len + 1;
            has_separator = true;
            break;
        } else if (c == ' ' || c == '\t' || c == '\x0c') && !preceding_backslash {
            value_start = key_len + 1;
            break;
        }
        preceding_backslash = c == '\\' && !preceding_backslash;
        key_len += 1;
    }
    while value_start < limit {
        let c = line[value_start];
        if c != ' ' && c != '\t' && c != '\x0c' {
            if !has_separator && (c == '=' || c == ':') {
                has_separator = true;
            } else {
                break;
            }
        }
        value_start += 1;
    }
    (&line[..key_len], &line[value_start..])
}

/// Port of `Properties.loadConvert`: resolves escapes. `None` for a malformed `\uXXXX`.
fn load_convert(input: &[char]) -> Option<String> {
    // UTF-16 so that `😀` becomes one character, as in Java.
    let mut out: Vec<u16> = Vec::with_capacity(input.len());
    let mut i = 0;
    while i < input.len() {
        let c = input[i];
        i += 1;
        if c != '\\' {
            push_char(&mut out, c);
            continue;
        }
        // A lone trailing backslash cannot happen (the line reader removes it).
        let Some(&escaped) = input.get(i) else { break };
        i += 1;
        if escaped == 'u' {
            let digits = input.get(i..i + 4)?;
            let mut value: u16 = 0;
            for d in digits {
                value = (value << 4) | u16::try_from(d.to_digit(16)?).ok()?;
            }
            i += 4;
            out.push(value);
        } else {
            push_char(
                &mut out,
                match escaped {
                    't' => '\t',
                    'r' => '\r',
                    'n' => '\n',
                    'f' => '\x0c',
                    other => other,
                },
            );
        }
    }
    Some(String::from_utf16_lossy(&out))
}

fn push_char(out: &mut Vec<u16>, c: char) {
    let mut unit = [0u16; 2];
    out.extend_from_slice(c.encode_utf16(&mut unit));
}

// ---- number parsing with Java rules ----

/// Java's `String.trim()`: removes every char <= U+0020 from both ends.
fn java_trim(s: &str) -> &str {
    s.trim_matches(|c: char| c <= ' ')
}

/// `Float.parseFloat` / `Double.parseDouble` for decimal input.
///
/// Rust's parser differs from Java's in what it accepts, so the text is checked against Java's
/// grammar first: trim, optional sign, `NaN`/`Infinity` (exact case), or
/// `digits[.digits][e[sign]digits]` with at least one mantissa digit, then an optional `f/F/d/D`.
/// NOTE(java-quirk): hexadecimal floats (`0x1p3`) are valid in Java; not supported here.
pub(crate) fn parse_java_float<T: FromStr>(s: &str) -> Option<T> {
    let trimmed = java_trim(s);
    let unsigned = trimmed.strip_prefix(['+', '-']).unwrap_or(trimmed);
    if unsigned == "NaN" || unsigned == "Infinity" {
        return trimmed.parse().ok();
    }

    let bytes = unsigned.as_bytes();
    let digits_at = |mut i: usize| {
        let start = i;
        while bytes.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        (i, i - start)
    };

    let (mut i, int_digits) = digits_at(0);
    let mut mantissa_digits = int_digits;
    if bytes.get(i) == Some(&b'.') {
        let (end, frac_digits) = digits_at(i + 1);
        i = end;
        mantissa_digits += frac_digits;
    }
    if mantissa_digits == 0 {
        return None;
    }
    if matches!(bytes.get(i), Some(b'e' | b'E')) {
        i += 1;
        if matches!(bytes.get(i), Some(b'+' | b'-')) {
            i += 1;
        }
        let (end, exp_digits) = digits_at(i);
        if exp_digits == 0 {
            return None;
        }
        i = end;
    }
    let number_end = i;
    if matches!(bytes.get(i), Some(b'f' | b'F' | b'd' | b'D')) {
        i += 1;
    }
    if i != bytes.len() {
        return None;
    }

    let sign_len = trimmed.len() - unsigned.len();
    trimmed[..sign_len + number_end].parse().ok()
}

/// `TimeUtil.parseDuration`: `<digits><unit>`, e.g. `5min`, `2hours`, `1week`.
///
/// Units (case-insensitive, no blanks): `sec(s)`, `min(s)`, `hour(s)`, `day(s)`, `week(s)` (7 days),
/// `month(s)` (30 days), `year(s)` (365 days). The number is a non-negative `i32`.
/// NOTE(java-quirk): Java's `Character.isDigit` also accepts non-ASCII digits; here ASCII only.
pub fn parse_duration(text: &str) -> Result<Duration, ConfigError> {
    let split = text.bytes().take_while(u8::is_ascii_digit).count();
    if split == 0 || split == text.len() {
        return Err(ConfigError::InvalidDuration(text.to_owned()));
    }
    let value: i32 = text[..split]
        .parse()
        .map_err(|_| ConfigError::InvalidDuration(text.to_owned()))?;
    // Non-negative by construction (digits only).
    let value = u64::from(value.unsigned_abs());
    let unit = text[split..].to_lowercase();

    const MINUTE: u64 = 60;
    const HOUR: u64 = 60 * MINUTE;
    const DAY: u64 = 24 * HOUR;
    let seconds = match unit.as_str() {
        "sec" | "secs" => value,
        "min" | "mins" => value * MINUTE,
        "hour" | "hours" => value * HOUR,
        "day" | "days" => value * DAY,
        "week" | "weeks" => value * 7 * DAY,
        "month" | "months" => value * 30 * DAY,
        "year" | "years" => value * 365 * DAY,
        _ => return Err(ConfigError::UnknownTimeUnit(unit)),
    };
    Ok(Duration::from_secs(seconds))
}

/// The parsing half of `getIntArray`; `None` where Java throws `NumberFormatException`.
fn parse_int_array(text: &str, delimiter: &str) -> Option<Vec<i32>> {
    java_split(text, delimiter)
        .into_iter()
        .map(|piece| java_trim(piece).parse().ok())
        .collect()
}

/// `String.split(delimiter)` for a literal delimiter: no match gives the whole text, otherwise
/// trailing empty pieces are removed (a leading empty piece stays).
fn java_split<'a>(text: &'a str, delimiter: &str) -> Vec<&'a str> {
    if delimiter.is_empty() {
        // The empty regex matches between every char.
        if text.is_empty() {
            return vec![text];
        }
        return text
            .char_indices()
            .map(|(i, c)| &text[i..i + c.len_utf8()])
            .collect();
    }
    let mut pieces: Vec<&str> = text.split(delimiter).collect();
    if pieces.len() > 1 {
        while pieces.last() == Some(&"") {
            pieces.pop();
        }
    }
    pieces
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ini(text: &str) -> IniFile {
        IniFile::parse("test.ini", text).unwrap()
    }

    #[test]
    fn float_grammar_matches_java() {
        assert_eq!(parse_java_float::<f64>("1."), Some(1.0));
        assert_eq!(parse_java_float::<f64>(".5"), Some(0.5));
        assert_eq!(parse_java_float::<f64>("1.e2"), Some(100.0));
        assert_eq!(parse_java_float::<f64>("  -2.5\t"), Some(-2.5));
        assert_eq!(parse_java_float::<f64>("1.5f"), Some(1.5));
        assert_eq!(parse_java_float::<f64>("+3D"), Some(3.0));
        assert_eq!(parse_java_float::<f32>("1e3"), Some(1000.0));
        assert_eq!(parse_java_float::<f32>("1E-2"), Some(0.01));
        assert_eq!(parse_java_float::<f64>("1e999"), Some(f64::INFINITY));
        assert_eq!(parse_java_float::<f64>("Infinity"), Some(f64::INFINITY));
        assert_eq!(parse_java_float::<f64>("-Infinity"), Some(f64::NEG_INFINITY));
        assert!(parse_java_float::<f64>("NaN").is_some_and(f64::is_nan));
        for bad in [
            "", ".", "e5", "1e", "1e+", "inf", "infinity", "nan", "NAN", "0x1p3", "1,5", "1f2", "--1", "1 2",
            "f",
        ] {
            assert_eq!(parse_java_float::<f64>(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn duration_units() {
        let secs = |s: &str| parse_duration(s).unwrap().as_secs();
        assert_eq!(secs("5sec"), 5);
        assert_eq!(secs("5secs"), 5);
        assert_eq!(secs("2min"), 120);
        assert_eq!(secs("2MINS"), 120);
        assert_eq!(secs("1hour"), 3600);
        assert_eq!(secs("3Hours"), 10_800);
        assert_eq!(secs("1day"), 86_400);
        assert_eq!(secs("2days"), 172_800);
        assert_eq!(secs("1week"), 7 * 86_400);
        assert_eq!(secs("2months"), 60 * 86_400);
        assert_eq!(secs("1year"), 365 * 86_400);
        assert_eq!(secs("2147483647years"), 2_147_483_647 * 365 * 86_400);
        assert_eq!(secs("007sec"), 7);
    }

    #[test]
    fn duration_errors() {
        for bad in [
            "",
            "5",
            "min",
            "-5min",
            "5 min",
            "5min ",
            "99999999999min",
            "+5min",
        ] {
            assert!(
                matches!(
                    parse_duration(bad),
                    Err(ConfigError::InvalidDuration(_) | ConfigError::UnknownTimeUnit(_))
                ),
                "{bad:?}"
            );
        }
        assert_eq!(
            parse_duration("5fortnights"),
            Err(ConfigError::UnknownTimeUnit("fortnights".into()))
        );
        assert_eq!(parse_duration("5"), Err(ConfigError::InvalidDuration("5".into())));
    }

    #[test]
    fn split_matches_java() {
        assert_eq!(java_split("1,2,3", ","), ["1", "2", "3"]);
        assert_eq!(java_split("1,2,,", ","), ["1", "2"]);
        assert_eq!(java_split(",1", ","), ["", "1"]);
        assert_eq!(java_split(",", ","), Vec::<&str>::new());
        assert_eq!(java_split("", ","), [""]);
        assert_eq!(java_split("1,,2", ","), ["1", "", "2"]);
        assert_eq!(java_split("ab", ""), ["a", "b"]);
    }

    #[test]
    fn properties_basics() {
        let f = ini("# comment\n! also comment\n\nA=1\nB = 2\nC: 3\nD 4\n  E   =   5  \nF=\nG\n");
        assert_eq!(f.get_value("A"), Some("1"));
        assert_eq!(f.get_value("B"), Some("2"));
        assert_eq!(f.get_value("C"), Some("3"));
        assert_eq!(f.get_value("D"), Some("4"));
        // trailing blanks stay
        assert_eq!(f.get_value("E"), Some("5  "));
        assert_eq!(f.get_value("F"), Some(""));
        assert_eq!(f.get_value("G"), Some(""));
        assert_eq!(f.len(), 7);
        assert!(!f.contains_key("# comment"));
    }

    #[test]
    fn properties_line_ends_and_duplicates() {
        let f = ini("A=1\r\nB=2\rC=3\n\r\nA=4");
        assert_eq!(f.get_value("A"), Some("4"));
        assert_eq!(f.get_value("B"), Some("2"));
        assert_eq!(f.get_value("C"), Some("3"));
    }

    #[test]
    fn properties_no_inline_comments_and_extra_separators() {
        let f = ini("A = 5 # note\nB = x=y:z\nC == 1\nD =: 2\nE:=1\n");
        assert_eq!(f.get_value("A"), Some("5 # note"));
        assert_eq!(f.get_value("B"), Some("x=y:z"));
        assert_eq!(f.get_value("C"), Some("= 1"));
        assert_eq!(f.get_value("D"), Some(": 2"));
        assert_eq!(f.get_value("E"), Some("=1"));
    }

    #[test]
    fn properties_continuation() {
        let f = ini("A = one \\\n    two\\\r\n  three\nB = x\\\\\nC = 1\\\n# not a comment\nD=1\\\n\nE=2");
        assert_eq!(f.get_value("A"), Some("one twothree"));
        // even number of backslashes: no continuation
        assert_eq!(f.get_value("B"), Some("x\\"));
        assert_eq!(f.get_value("C"), Some("1# not a comment"));
        // an empty line ends the continuation
        assert_eq!(f.get_value("D"), Some("1"));
        assert_eq!(f.get_value("E"), Some("2"));
    }

    #[test]
    fn properties_escapes() {
        let f = ini("A\\ B = c\\td\\ne\\\\f\\#g\nU = \\u0041\\u00e9\\uD83D\\uDE00\nK\\=1 = v\nZ = \\q");
        assert_eq!(f.get_value("A B"), Some("c\td\ne\\f#g"));
        assert_eq!(f.get_value("U"), Some("A\u{e9}\u{1F600}"));
        assert_eq!(f.get_value("K=1"), Some("v"));
        assert_eq!(f.get_value("Z"), Some("q"));
    }

    #[test]
    fn properties_malformed_unicode_is_an_error() {
        for text in ["A=\\u12", "A=\\u12G4", "A=\\u"] {
            assert_eq!(
                IniFile::parse("x.ini", text).unwrap_err(),
                ConfigError::MalformedUnicodeEscape { file: "x.ini".into() },
                "{text:?}"
            );
        }
    }

    #[test]
    fn properties_file_ending_in_backslash() {
        // Java: lone trailing backslash is dropped
        assert_eq!(ini("A=b\\").get_value("A"), Some("b"));
        // NOTE(java-quirk): "\\\n" at EOF is an entry with empty key and value
        assert_eq!(ini("A=1\n\\\n").get_value(""), Some(""));
    }

    #[test]
    fn comment_at_eof_and_bom() {
        assert!(ini("# only a comment").is_empty());
        let f = ini("\u{feff}A=1");
        assert_eq!(f.get_value("A"), None);
        assert_eq!(f.get_value("\u{feff}A"), Some("1"));
    }

    #[test]
    fn getters_valid_values() {
        let f = ini(
            "B=True\nBY=-128\nSH=32767\nI=+42\nL=9223372036854775807\nF=1.\nD= .5 \nS=hello \nDU=3days\nIA=1, 2 ,3,\n",
        );
        assert!(f.get_bool("B", false));
        assert_eq!(f.get_byte("BY", 0), -128);
        assert_eq!(f.get_short("SH", 0), 32767);
        assert_eq!(f.get_int("I", 0), 42);
        assert_eq!(f.get_long("L", 0), i64::MAX);
        assert_eq!(f.get_float("F", 9.0), 1.0);
        assert_eq!(f.get_double("D", 9.0), 0.5);
        assert_eq!(f.get_string("S", "x"), "hello ");
        assert_eq!(
            f.get_duration("DU", "1sec").unwrap(),
            Duration::from_secs(3 * 86_400)
        );
        assert_eq!(f.get_int_array("IA", ",", "9"), [1, 2, 3]);
    }

    #[test]
    fn getters_missing_use_default() {
        let f = IniFile::empty("none.ini");
        assert!(f.get_bool("B", true));
        assert_eq!(f.get_byte("X", 7), 7);
        assert_eq!(f.get_short("X", 7), 7);
        assert_eq!(f.get_int("X", 7), 7);
        assert_eq!(f.get_long("X", 7), 7);
        assert_eq!(f.get_float("X", 7.5), 7.5);
        assert_eq!(f.get_double("X", 7.5), 7.5);
        assert_eq!(f.get_string("X", "d"), "d");
        assert_eq!(f.get_enum("X", Color::Green), Color::Green);
        assert_eq!(f.get_duration("X", "2min").unwrap(), Duration::from_secs(120));
        assert_eq!(f.get_int_array("X", ";", "1;2"), [1, 2]);
    }

    #[test]
    fn getters_invalid_use_default() {
        let f = ini("A=abc\nB=128\nC=-129\nD=32768\nE=2147483648\nG=1.5\nH= 5\nJ=5 \nK=1_000\nL=\n");
        assert_eq!(f.get_byte("A", 1), 1);
        assert_eq!(f.get_byte("B", 1), 1);
        assert_eq!(f.get_byte("C", 1), 1);
        assert_eq!(f.get_short("D", 1), 1);
        assert_eq!(f.get_int("E", 1), 1);
        assert_eq!(f.get_int("G", 1), 1);
        assert_eq!(f.get_long("A", 1), 1);
        // leading blanks of the value are stripped by Properties, trailing ones are not
        assert_eq!(f.get_int("H", 1), 5);
        assert_eq!(f.get_int("J", 1), 1);
        assert_eq!(f.get_int("K", 1), 1);
        assert_eq!(f.get_int("L", 1), 1);
        assert_eq!(f.get_float("A", 2.0), 2.0);
        assert_eq!(f.get_double("L", 2.0), 2.0);
    }

    #[test]
    fn bool_quirk_never_falls_back() {
        let f = ini("A=yes\nB=1\nC=TRUE\nD=true \nE=false\nF=\n");
        assert!(!f.get_bool("A", true));
        assert!(!f.get_bool("B", true));
        assert!(f.get_bool("C", false));
        assert!(!f.get_bool("D", true));
        assert!(!f.get_bool("E", true));
        assert!(!f.get_bool("F", true));
    }

    #[derive(Debug, PartialEq, Clone, Copy)]
    enum Color {
        Red,
        Green,
    }

    impl FromStr for Color {
        type Err = ();
        fn from_str(s: &str) -> Result<Self, ()> {
            match s {
                "Red" => Ok(Self::Red),
                "Green" => Ok(Self::Green),
                _ => Err(()),
            }
        }
    }

    #[test]
    fn enum_getter() {
        let f = ini("A=Red\nB=red\nC=Red \nD=Blue\n");
        assert_eq!(f.get_enum("A", Color::Green), Color::Red);
        // case-sensitive and not trimmed, like Enum.valueOf
        assert_eq!(f.get_enum("B", Color::Green), Color::Green);
        assert_eq!(f.get_enum("C", Color::Green), Color::Green);
        assert_eq!(f.get_enum("D", Color::Green), Color::Green);
    }

    #[test]
    fn duration_getter_fallbacks() {
        let f = ini("A=5 min\nB=5\n");
        assert_eq!(f.get_duration("A", "1hour").unwrap(), Duration::from_secs(3600));
        assert_eq!(f.get_duration("B", "1hour").unwrap(), Duration::from_secs(3600));
        assert_eq!(
            f.get_duration("A", "garbage"),
            Err(ConfigError::InvalidDuration("garbage".into()))
        );
    }

    #[test]
    fn int_array_getter() {
        let f = ini("A=1;2;3\nB=1;x\nC=;\nD=1;;2\nE=\nF=1 2  3\nG=1,2\n");
        assert_eq!(f.get_int_array("A", ";", "0"), [1, 2, 3]);
        assert_eq!(f.get_int_array("B", ";", "7;8"), [7, 8]);
        // NOTE(java-quirk): only separators gives an empty array, not the default
        assert_eq!(f.get_int_array("C", ";", "7"), Vec::<i32>::new());
        assert_eq!(f.get_int_array("D", ";", "7"), [7]);
        // "" splits to [""], parseInt("") fails
        assert_eq!(f.get_int_array("E", ";", "7"), [7]);
        assert_eq!(f.get_int_array("F", " ", "7"), [7]); // double blank -> empty piece
        // wrong delimiter: whole text is one piece
        assert_eq!(f.get_int_array("G", ";", "7"), [7]);
        // bad default -> empty array
        assert_eq!(f.get_int_array("B", ";", "x"), Vec::<i32>::new());
        assert_eq!(f.get_int_array("none", ";", ""), Vec::<i32>::new());
    }

    #[test]
    fn keys_lists_all() {
        let f = ini("BufferPool.A.Size=1\nBufferPool.B.Size=2\nOther=3\n");
        let pool: Vec<&str> = f.keys().filter(|k| k.starts_with("BufferPool.")).collect();
        assert_eq!(pool, ["BufferPool.A.Size", "BufferPool.B.Size"]);
    }
}
