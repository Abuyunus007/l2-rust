//! Hex helpers and byte-slice assertions.

use std::fmt::Write as _;

/// Decodes a hex string (case-insensitive, whitespace ignored). Panics on invalid input.
pub fn from_hex(s: &str) -> Vec<u8> {
    let compact: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    hex::decode(&compact).unwrap_or_else(|e| panic!("invalid hex {s:?}: {e}"))
}

/// Lowercase hex without separators (the format the vector files use).
pub fn to_hex(bytes: &[u8]) -> String {
    hex::encode(bytes)
}

/// Asserts two byte slices are equal. On failure, shows the length difference, the first
/// differing offset and both slices around it as a hex dump.
#[track_caller]
pub fn assert_bytes_eq(actual: &[u8], expected: &[u8]) {
    if actual == expected {
        return;
    }
    let first_diff = actual
        .iter()
        .zip(expected)
        .position(|(a, e)| a != e)
        .unwrap_or(actual.len().min(expected.len()));
    let start = first_diff.saturating_sub(16) & !15;
    let end = (first_diff + 32).min(actual.len().max(expected.len()));
    let mut msg = String::new();
    let _ = writeln!(
        msg,
        "byte slices differ: actual {} bytes, expected {} bytes, first difference at offset {first_diff} (0x{first_diff:x})",
        actual.len(),
        expected.len()
    );
    let _ = writeln!(msg, "actual:   {}", dump(actual, start, end, first_diff));
    let _ = writeln!(msg, "expected: {}", dump(expected, start, end, first_diff));
    panic!("{msg}");
}

fn dump(bytes: &[u8], start: usize, end: usize, mark: usize) -> String {
    let mut out = format!("[{start:04x}] ");
    for i in start..end {
        match bytes.get(i) {
            Some(b) if i == mark => {
                let _ = write!(out, ">{b:02x}<");
            }
            Some(b) => {
                let _ = write!(out, "{b:02x} ");
            }
            None => out.push_str("-- "),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trip() {
        let bytes = vec![0x00, 0x7f, 0x80, 0xff];
        assert_eq!(to_hex(&bytes), "007f80ff");
        assert_eq!(from_hex("00 7F 80 ff"), bytes);
    }

    #[test]
    fn equal_slices_pass() {
        assert_bytes_eq(&[1, 2, 3], &[1, 2, 3]);
    }

    #[test]
    #[should_panic(expected = "first difference at offset 2")]
    fn reports_first_difference() {
        assert_bytes_eq(&[1, 2, 3, 4], &[1, 2, 9, 4]);
    }

    #[test]
    #[should_panic(expected = "actual 2 bytes, expected 3 bytes")]
    fn reports_length_difference() {
        assert_bytes_eq(&[1, 2], &[1, 2, 3]);
    }
}
