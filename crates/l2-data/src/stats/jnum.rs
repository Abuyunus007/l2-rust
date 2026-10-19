//! Java's number and string splitting rules, as far as `DocumentBase` uses them.
//!
//! The NPC loader has private copies of `decode`/`parseDouble` (`npc/stat_set.rs`); these are the
//! same rules, shared here by the condition and function parsers (and by the skill loader).

/// Java `Integer.decode`: optional sign, then `0x`/`0X`/`#` hex, a leading `0` octal, else decimal.
pub fn decode_i32(text: &str) -> Option<i32> {
    i32::try_from(decode_i64(text)?).ok()
}

/// Java `Long.decode`.
pub fn decode_i64(text: &str) -> Option<i64> {
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

/// Java `Integer.parseInt(String)`: optional sign and decimal digits, no whitespace.
pub fn parse_int(text: &str) -> Option<i32> {
    text.parse().ok()
}

/// Java `Double.parseDouble` for the decimal forms the datapack can contain: surrounding
/// whitespace is trimmed, a trailing `d`/`f` type suffix is allowed, `NaN` and `Infinity` are
/// accepted. (Hex floats are not supported.)
pub fn parse_double(text: &str) -> Option<f64> {
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

/// Java `Boolean.parseBoolean`: only `true` (any case) is true.
pub fn parse_boolean(text: &str) -> bool {
    text.eq_ignore_ascii_case("true")
}

/// Java `String.split(regex)` for a plain one-character delimiter: no match returns the input,
/// trailing empty strings are dropped (`"".split(",")` is `[""]`, `",".split(",")` is `[]`).
pub fn java_split(text: &str, delimiter: char) -> Vec<&str> {
    if !text.contains(delimiter) {
        return vec![text];
    }
    let mut parts: Vec<&str> = text.split(delimiter).collect();
    while parts.last() == Some(&"") {
        parts.pop();
    }
    parts
}

/// Java `new StringTokenizer(text, delimiter)`: the non-empty pieces between delimiters.
pub fn tokens(text: &str, delimiter: char) -> Vec<&str> {
    text.split(delimiter).filter(|piece| !piece.is_empty()).collect()
}

/// The `NumberFormatException` message Java prints for a bad number.
pub fn number_format_message(text: &str) -> String {
    if text.is_empty() {
        "empty String".to_owned()
    } else {
        format!("For input string: \"{text}\"")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_follows_java() {
        assert_eq!(decode_i32("40"), Some(40));
        assert_eq!(decode_i32("-7"), Some(-7));
        assert_eq!(decode_i32("+7"), Some(7));
        assert_eq!(decode_i32("0x10"), Some(16));
        assert_eq!(decode_i32("#ff"), Some(255));
        assert_eq!(decode_i32("010"), Some(8));
        assert_eq!(decode_i32("0"), Some(0));
        assert_eq!(decode_i32(""), None);
        assert_eq!(decode_i32("1 "), None);
        assert_eq!(decode_i32("--1"), None);
        assert_eq!(decode_i32("2147483648"), None);
        assert_eq!(decode_i32("-2147483648"), Some(i32::MIN));
        assert_eq!(decode_i32("09"), None);
    }

    #[test]
    fn parse_int_is_strict_decimal() {
        assert_eq!(parse_int("12"), Some(12));
        assert_eq!(parse_int("-12"), Some(-12));
        assert_eq!(parse_int("0x10"), None);
        assert_eq!(parse_int(" 1"), None);
        assert_eq!(parse_int(""), None);
    }

    #[test]
    fn parse_double_follows_java() {
        assert_eq!(parse_double(" 8 "), Some(8.0));
        assert_eq!(parse_double("0.5"), Some(0.5));
        assert_eq!(parse_double("-.5"), Some(-0.5));
        assert_eq!(parse_double("1e3"), Some(1000.0));
        assert_eq!(parse_double("1.5f"), Some(1.5));
        assert_eq!(parse_double("2D"), Some(2.0));
        assert_eq!(parse_double(""), None);
        assert_eq!(parse_double("abc"), None);
        assert_eq!(parse_double("inf"), None);
        assert_eq!(parse_double("#t"), None);
        assert!(parse_double("NaN").is_some_and(f64::is_nan));
        assert_eq!(parse_double("-Infinity"), Some(f64::NEG_INFINITY));
    }

    #[test]
    fn split_and_tokens() {
        assert_eq!(java_split("", ','), vec![""]);
        assert_eq!(java_split("a,b,,", ','), vec!["a", "b"]);
        assert_eq!(java_split(",", ','), Vec::<&str>::new());
        assert_eq!(java_split("a,,b", ','), vec!["a", "", "b"]);
        assert_eq!(tokens("a,,b,", ','), vec!["a", "b"]);
        assert_eq!(tokens("", ','), Vec::<&str>::new());
        assert_eq!(tokens(" ,a", ','), vec![" ", "a"]);
    }

    #[test]
    fn booleans() {
        assert!(parse_boolean("true"));
        assert!(parse_boolean("TRUE"));
        assert!(!parse_boolean("1"));
        assert!(!parse_boolean(""));
    }

    #[test]
    fn number_format_messages() {
        assert_eq!(number_format_message(""), "empty String");
        assert_eq!(number_format_message("x"), "For input string: \"x\"");
    }
}
