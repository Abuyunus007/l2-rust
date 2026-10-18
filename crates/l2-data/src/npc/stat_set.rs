//! A small `StatSet`: the typed key/value bag Java's `NpcData` fills and `NpcTemplate` reads.
//!
//! Java: `gameserver/util/StatSet.java` and the `parseInteger`/`parseDouble`/`parseBoolean` helpers
//! of `commons/util/IXmlReader.java`.
//!
//! Porting the loader through the same bag (instead of straight into struct fields) keeps Java's
//! quirks without special cases: a `Double` read with `getInt` is truncated, a value that was never
//! set gets the *reader's* default, and a text value is parsed (and can fail) only when it is read.

use std::collections::HashMap;

use crate::loader::DataError;

/// What Java's `StatSet` stores for one key (`Object`).
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Value {
    Int(i32),
    Double(f64),
    Bool(bool),
    Text(String),
}

#[derive(Debug, Default)]
pub(super) struct StatSet(HashMap<&'static str, Value>);

fn invalid(what: &str, key: &str, found: impl std::fmt::Display) -> DataError {
    DataError::Invalid(format!("{what} value required for {key}, found: {found}"))
}

impl StatSet {
    /// `set(String, Integer)`: Java's `Object` overload, a `null` (absent attribute) is not stored.
    pub(super) fn set_int(&mut self, key: &'static str, value: Option<i32>) {
        if let Some(value) = value {
            self.0.insert(key, Value::Int(value));
        }
    }

    /// `set(String, Double)`.
    pub(super) fn set_double(&mut self, key: &'static str, value: Option<f64>) {
        if let Some(value) = value {
            self.0.insert(key, Value::Double(value));
        }
    }

    /// `set(String, Boolean)`.
    pub(super) fn set_bool(&mut self, key: &'static str, value: Option<bool>) {
        if let Some(value) = value {
            self.0.insert(key, Value::Bool(value));
        }
    }

    /// `set(String, String)`.
    pub(super) fn set_text(&mut self, key: &'static str, value: Option<&str>) {
        if let Some(value) = value {
            self.0.insert(key, Value::Text(value.to_owned()));
        }
    }

    /// `getBoolean(key, default)`: only a stored `Boolean` or a `String` counts, anything else is the default.
    pub(super) fn get_bool(&self, key: &str, default: bool) -> bool {
        match self.0.get(key) {
            Some(Value::Bool(value)) => *value,
            Some(Value::Text(text)) => text.eq_ignore_ascii_case("true"),
            _ => default,
        }
    }

    /// `getInt(key, default)`: numbers convert like `Number.intValue()`, text like `Integer.parseInt`.
    pub(super) fn get_int(&self, key: &str, default: i32) -> Result<i32, DataError> {
        match self.0.get(key) {
            None => Ok(default),
            Some(Value::Int(value)) => Ok(*value),
            // Rust's float to int cast saturates and maps NaN to 0, like Java's `(int)`.
            Some(Value::Double(value)) => Ok(*value as i32),
            Some(Value::Text(text)) => text.parse().map_err(|_| invalid("Integer", key, text)),
            Some(Value::Bool(value)) => Err(invalid("Integer", key, value)),
        }
    }

    /// `getByte(key, default)`: `Number.byteValue()` wraps (a level of 200 becomes -56).
    pub(super) fn get_byte(&self, key: &str, default: i8) -> Result<i8, DataError> {
        match self.0.get(key) {
            None => Ok(default),
            Some(Value::Int(value)) => Ok(*value as i8),
            Some(Value::Double(value)) => Ok(*value as i32 as i8),
            Some(Value::Text(text)) => text.parse().map_err(|_| invalid("Byte", key, text)),
            Some(Value::Bool(value)) => Err(invalid("Byte", key, value)),
        }
    }

    /// `getFloat(key, default)`.
    pub(super) fn get_float(&self, key: &str, default: f32) -> Result<f32, DataError> {
        match self.0.get(key) {
            None => Ok(default),
            Some(Value::Int(value)) => Ok(*value as f32),
            Some(Value::Double(value)) => Ok(*value as f32),
            Some(Value::Text(text)) => parse_java_double(text)
                .map(|value| value as f32)
                .ok_or_else(|| invalid("Float", key, text)),
            Some(Value::Bool(value)) => Err(invalid("Float", key, value)),
        }
    }

    /// `getDouble(key, default)`.
    pub(super) fn get_double(&self, key: &str, default: f64) -> Result<f64, DataError> {
        match self.0.get(key) {
            None => Ok(default),
            Some(Value::Int(value)) => Ok(f64::from(*value)),
            Some(Value::Double(value)) => Ok(*value),
            Some(Value::Text(text)) => parse_java_double(text).ok_or_else(|| invalid("Double", key, text)),
            Some(Value::Bool(value)) => Err(invalid("Double", key, value)),
        }
    }

    /// `getString(key, default)`: `String.valueOf` of whatever is stored.
    pub(super) fn get_string(&self, key: &str, default: &str) -> String {
        match self.0.get(key) {
            None => default.to_owned(),
            Some(Value::Text(text)) => text.clone(),
            Some(Value::Int(value)) => value.to_string(),
            Some(Value::Double(value)) => value.to_string(),
            Some(Value::Bool(value)) => value.to_string(),
        }
    }

    /// `getEnum(key, class, default)`: `Enum.valueOf` of the stored text, exact and case sensitive.
    pub(super) fn get_enum<T>(
        &self,
        key: &str,
        default: T,
        from_name: fn(&str) -> Option<T>,
    ) -> Result<T, DataError> {
        match self.0.get(key) {
            None => Ok(default),
            Some(value) => {
                let name = match value {
                    Value::Text(text) => text.clone(),
                    Value::Int(value) => value.to_string(),
                    Value::Double(value) => value.to_string(),
                    Value::Bool(value) => value.to_string(),
                };
                from_name(&name).ok_or_else(|| invalid("Enum", key, name))
            }
        }
    }
}

/// Java `Integer.decode`: optional sign, then `0x`/`0X`/`#` hex, a leading `0` octal, else decimal.
/// Used by `IXmlReader.parseInteger`, so XML like `id="0x10"` is legal.
pub(super) fn decode_i32(text: &str) -> Option<i32> {
    i32::try_from(decode_i64(text)?).ok()
}

/// Java `Long.decode`.
pub(super) fn decode_i64(text: &str) -> Option<i64> {
    let (negative, rest) = match text.as_bytes().first()? {
        b'-' => (true, &text[1..]),
        b'+' => (false, &text[1..]),
        _ => (false, text),
    };
    let (radix, digits) = if let Some(hex) = rest.strip_prefix("0x").or_else(|| rest.strip_prefix("0X")) {
        (16, hex)
    } else if let Some(hex) = rest.strip_prefix('#') {
        (16, hex)
    } else if rest.len() > 1 && rest.starts_with('0') {
        (8, &rest[1..])
    } else {
        (10, rest)
    };
    // `Integer.valueOf(digits, radix)` accepts no second sign.
    if digits.is_empty() || !digits.chars().all(|c| c.is_digit(radix)) {
        return None;
    }
    let magnitude = i128::from_str_radix(digits, radix).ok()?;
    i64::try_from(if negative { -magnitude } else { magnitude }).ok()
}

/// Java `Double.valueOf(String)` for the decimal forms the datapack can contain: surrounding
/// whitespace is trimmed, a trailing `d`/`f` type suffix is allowed, `NaN` and `Infinity` are
/// accepted. (Hex floats are not supported.)
pub(super) fn parse_java_double(text: &str) -> Option<f64> {
    let trimmed = text.trim_matches(|c: char| c <= ' ');
    if trimmed.is_empty() {
        return None;
    }
    match trimmed {
        "NaN" | "+NaN" | "-NaN" => return Some(f64::NAN),
        "Infinity" | "+Infinity" => return Some(f64::INFINITY),
        "-Infinity" => return Some(f64::NEG_INFINITY),
        _ => {}
    }
    let number = trimmed.strip_suffix(['d', 'D', 'f', 'F']).unwrap_or(trimmed);
    // Rust also accepts "inf", "nan" and "infinity"; Java does not.
    if !number
        .chars()
        .all(|c| c.is_ascii_digit() || matches!(c, '.' | 'e' | 'E' | '+' | '-'))
    {
        return None;
    }
    number.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_matches_integer_decode() {
        assert_eq!(decode_i32("12"), Some(12));
        assert_eq!(decode_i32("-12"), Some(-12));
        assert_eq!(decode_i32("+12"), Some(12));
        assert_eq!(decode_i32("0x1F"), Some(31));
        assert_eq!(decode_i32("-0x10"), Some(-16));
        assert_eq!(decode_i32("#ff"), Some(255));
        assert_eq!(decode_i32("010"), Some(8));
        assert_eq!(decode_i32("0"), Some(0));
        assert_eq!(decode_i32("-2147483648"), Some(i32::MIN));
        assert_eq!(decode_i32("2147483648"), None);
        assert_eq!(decode_i32(""), None);
        assert_eq!(decode_i32("-"), None);
        assert_eq!(decode_i32("1.5"), None);
        assert_eq!(decode_i32(" 5"), None);
        assert_eq!(decode_i32("--5"), None);
        assert_eq!(decode_i32("09"), None);
        assert_eq!(decode_i64("9223372036854775807"), Some(i64::MAX));
        assert_eq!(decode_i64("-9223372036854775808"), Some(i64::MIN));
        assert_eq!(decode_i64("9223372036854775808"), None);
    }

    #[test]
    fn double_parsing_matches_double_value_of() {
        assert_eq!(parse_java_double("1.5"), Some(1.5));
        assert_eq!(parse_java_double(" 2 "), Some(2.0));
        assert_eq!(parse_java_double("1.5f"), Some(1.5));
        assert_eq!(parse_java_double(".5"), Some(0.5));
        assert_eq!(parse_java_double("1e3"), Some(1000.0));
        assert_eq!(parse_java_double("-0"), Some(-0.0));
        assert_eq!(parse_java_double("Infinity"), Some(f64::INFINITY));
        assert!(parse_java_double("NaN").is_some_and(f64::is_nan));
        assert_eq!(parse_java_double("inf"), None);
        assert_eq!(parse_java_double("nan"), None);
        assert_eq!(parse_java_double(""), None);
        assert_eq!(parse_java_double("abc"), None);
        assert_eq!(parse_java_double("1,5"), None);
    }

    #[test]
    fn readers_convert_like_java() {
        let mut set = StatSet::default();
        set.set_double("d", Some(8.9));
        set.set_int("i", Some(200));
        set.set_text("t", Some("7"));
        set.set_text("bad", Some("x"));
        set.set_bool("b", Some(true));
        set.set_int("absent", None);
        assert_eq!(set.get_int("d", 0).unwrap(), 8);
        assert_eq!(set.get_byte("i", 0).unwrap(), -56);
        assert_eq!(set.get_int("t", 0).unwrap(), 7);
        assert!(set.get_int("bad", 0).is_err());
        assert!(set.get_int("b", 0).is_err());
        assert_eq!(set.get_int("absent", 5).unwrap(), 5);
        assert_eq!(set.get_double("i", 0.0).unwrap(), 200.0);
        assert!(set.get_bool("b", false));
        assert!(!set.get_bool("d", false));
        assert!(set.get_bool("d", true));
        assert_eq!(set.get_string("t", "").as_str(), "7");
    }
}
