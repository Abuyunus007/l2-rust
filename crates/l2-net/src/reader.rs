//! Bounds-checked little-endian packet reader.
//!
//! Java: `commons/network/packet/ReadablePacket.java` (game/login client packets) and
//! `SimpleReadablePacket.java` (LS↔GS link packets).

use crate::error::PacketError;

/// Reads packet fields from a byte slice. Every read checks bounds and returns an error instead
/// of panicking, except the two string reads, which copy Java's lenient behaviour.
#[derive(Debug, Clone)]
pub struct PacketReader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> PacketReader<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    /// Bytes left to read (Java `remaining()`).
    pub fn remaining(&self) -> usize {
        self.buf.len() - self.pos
    }

    /// Current offset from the start of the buffer.
    pub fn position(&self) -> usize {
        self.pos
    }

    /// The unread rest of the buffer, without consuming it.
    pub fn rest(&self) -> &'a [u8] {
        &self.buf[self.pos..]
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], PacketError> {
        if self.remaining() < n {
            return Err(PacketError::Truncated {
                offset: self.pos,
                needed: n,
                remaining: self.remaining(),
            });
        }
        let slice = &self.buf[self.pos..self.pos + n];
        self.pos += n;
        Ok(slice)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], PacketError> {
        let mut out = [0u8; N];
        out.copy_from_slice(self.take(N)?);
        Ok(out)
    }

    /// Java `readByte()` (signed).
    pub fn i8(&mut self) -> Result<i8, PacketError> {
        Ok(i8::from_le_bytes(self.array()?))
    }

    /// Java `readUnsignedByte()`.
    pub fn u8(&mut self) -> Result<u8, PacketError> {
        Ok(self.array::<1>()?[0])
    }

    /// Java `readBoolean()`: any non-zero byte is `true`.
    pub fn bool(&mut self) -> Result<bool, PacketError> {
        Ok(self.u8()? != 0)
    }

    /// Java `readShort()`.
    pub fn i16(&mut self) -> Result<i16, PacketError> {
        Ok(i16::from_le_bytes(self.array()?))
    }

    /// Java `readShort() & 0xFFFF` / `readChar()`.
    pub fn u16(&mut self) -> Result<u16, PacketError> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    /// Java `readInt()`.
    pub fn i32(&mut self) -> Result<i32, PacketError> {
        Ok(i32::from_le_bytes(self.array()?))
    }

    /// Java `readInt()` reinterpreted as unsigned (for flags and ids the client treats as u32).
    pub fn u32(&mut self) -> Result<u32, PacketError> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    /// Java `readLong()`.
    pub fn i64(&mut self) -> Result<i64, PacketError> {
        Ok(i64::from_le_bytes(self.array()?))
    }

    /// Java `readFloat()`.
    pub fn f32(&mut self) -> Result<f32, PacketError> {
        Ok(f32::from_le_bytes(self.array()?))
    }

    /// Java `readDouble()`.
    pub fn f64(&mut self) -> Result<f64, PacketError> {
        Ok(f64::from_le_bytes(self.array()?))
    }

    /// Java `readBytes(length)`.
    pub fn bytes(&mut self, len: usize) -> Result<&'a [u8], PacketError> {
        self.take(len)
    }

    /// Fixed-size byte array (e.g. a 128-byte RSA block).
    pub fn byte_array<const N: usize>(&mut self) -> Result<[u8; N], PacketError> {
        self.array()
    }

    /// Java `readString()`: UTF-16LE code units up to a `0x0000` terminator.
    ///
    /// NOTE(java-quirk): Java catches the exception when the data ends before the terminator and
    /// returns what it read so far, so a missing terminator is **not** an error here either.
    /// The reader is then at the end of the buffer (a dangling odd byte stays unread).
    /// Unpaired surrogates (legal in Java strings) become U+FFFD.
    pub fn string(&mut self) -> String {
        let mut units = Vec::new();
        while let Ok(unit) = self.u16() {
            if unit == 0 {
                break;
            }
            units.push(unit);
        }
        String::from_utf16_lossy(&units)
    }

    /// Java `readSizedString()`: `u16` length in code units, then that many UTF-16LE units.
    ///
    /// NOTE(java-quirk): a length `<= 0` (read as signed `short`) or truncated data gives `""`,
    /// as Java swallows the exception. On truncation nothing after the length is consumed.
    pub fn sized_string(&mut self) -> String {
        let Ok(count) = self.i16() else {
            return String::new();
        };
        if count <= 0 {
            return String::new();
        }
        let count = count as usize; // positive i16, fits
        match self.take(count * 2) {
            Ok(raw) => {
                let units: Vec<u16> = raw
                    .chunks_exact(2)
                    .map(|c| u16::from_le_bytes([c[0], c[1]]))
                    .collect();
                String::from_utf16_lossy(&units)
            }
            Err(_) => String::new(),
        }
    }

    /// Skips `n` bytes (fields the server ignores).
    pub fn skip(&mut self, n: usize) -> Result<(), PacketError> {
        self.take(n).map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_little_endian() {
        let data = [0x01, 0xff, 0xfe, 0x78, 0x56, 0x34, 0x12];
        let mut r = PacketReader::new(&data);
        assert_eq!(r.u8().unwrap(), 1);
        assert_eq!(r.i16().unwrap(), -257); // bytes ff fe = 0xfeff
        assert_eq!(r.u32().unwrap(), 0x1234_5678);
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn every_read_reports_truncation() {
        let empty: [u8; 0] = [];
        assert!(PacketReader::new(&empty).u8().is_err());
        assert!(PacketReader::new(&[1]).i16().is_err());
        assert!(PacketReader::new(&[1, 2, 3]).i32().is_err());
        assert!(PacketReader::new(&[1, 2, 3]).u32().is_err());
        assert!(PacketReader::new(&[0; 7]).i64().is_err());
        assert!(PacketReader::new(&[0; 3]).f32().is_err());
        assert!(PacketReader::new(&[0; 7]).f64().is_err());
        assert!(PacketReader::new(&[0; 3]).bytes(4).is_err());
        assert!(PacketReader::new(&[0; 3]).byte_array::<4>().is_err());
        assert!(PacketReader::new(&[0; 3]).skip(4).is_err());
    }

    #[test]
    fn truncation_error_has_position() {
        let mut r = PacketReader::new(&[1, 2, 3]);
        r.u8().unwrap();
        assert_eq!(
            r.i32(),
            Err(PacketError::Truncated {
                offset: 1,
                needed: 4,
                remaining: 2
            })
        );
        // A failed read consumes nothing.
        assert_eq!(r.remaining(), 2);
    }

    #[test]
    fn string_stops_at_terminator() {
        // "Hi" + terminator + trailing byte
        let mut r = PacketReader::new(&[b'H', 0, b'i', 0, 0, 0, 0x7f]);
        assert_eq!(r.string(), "Hi");
        assert_eq!(r.u8().unwrap(), 0x7f);
    }

    #[test]
    fn string_without_terminator_is_lenient_like_java() {
        let mut r = PacketReader::new(&[b'a', 0, b'b', 0, b'c']);
        assert_eq!(r.string(), "ab");
        assert_eq!(r.remaining(), 1);
    }

    #[test]
    fn empty_string() {
        let mut r = PacketReader::new(&[0, 0]);
        assert_eq!(r.string(), "");
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn sized_string_cases() {
        assert_eq!(PacketReader::new(&[2, 0, b'o', 0, b'k', 0]).sized_string(), "ok");
        assert_eq!(PacketReader::new(&[0, 0]).sized_string(), "");
        // Negative length (0xFFFF = -1) → "".
        assert_eq!(PacketReader::new(&[0xff, 0xff, b'x', 0]).sized_string(), "");
        // Truncated → "".
        assert_eq!(PacketReader::new(&[3, 0, b'x', 0]).sized_string(), "");
    }
}
