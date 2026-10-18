//! Little-endian packet writer.
//!
//! Java: `commons/network/buffer/WriteBuffer.java` (game/login packets) and
//! `packet/SimpleWritablePacket.java` (LS↔GS link packets). Both write the same bytes.

/// Builds a packet payload (opcode included, no length header, no encryption).
#[derive(Debug, Default, Clone)]
pub struct PacketWriter {
    buf: Vec<u8>,
}

impl PacketWriter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            buf: Vec::with_capacity(capacity),
        }
    }

    pub fn len(&self) -> usize {
        self.buf.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.buf
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.buf
    }

    /// Java `writeByte(int)`: the low 8 bits.
    pub fn u8(&mut self, v: u8) -> &mut Self {
        self.buf.push(v);
        self
    }

    pub fn i8(&mut self, v: i8) -> &mut Self {
        self.buf.extend_from_slice(&v.to_le_bytes());
        self
    }

    /// Java `writeByte(boolean)`: 1 or 0.
    pub fn bool(&mut self, v: bool) -> &mut Self {
        self.u8(u8::from(v))
    }

    pub fn i16(&mut self, v: i16) -> &mut Self {
        self.buf.extend_from_slice(&v.to_le_bytes());
        self
    }

    pub fn u16(&mut self, v: u16) -> &mut Self {
        self.buf.extend_from_slice(&v.to_le_bytes());
        self
    }

    pub fn i32(&mut self, v: i32) -> &mut Self {
        self.buf.extend_from_slice(&v.to_le_bytes());
        self
    }

    pub fn u32(&mut self, v: u32) -> &mut Self {
        self.buf.extend_from_slice(&v.to_le_bytes());
        self
    }

    pub fn i64(&mut self, v: i64) -> &mut Self {
        self.buf.extend_from_slice(&v.to_le_bytes());
        self
    }

    pub fn f32(&mut self, v: f32) -> &mut Self {
        self.buf.extend_from_slice(&v.to_le_bytes());
        self
    }

    pub fn f64(&mut self, v: f64) -> &mut Self {
        self.buf.extend_from_slice(&v.to_le_bytes());
        self
    }

    pub fn bytes(&mut self, v: &[u8]) -> &mut Self {
        self.buf.extend_from_slice(v);
        self
    }

    /// Java `writeString(text)`: UTF-16LE code units + `0x0000` terminator.
    pub fn string(&mut self, text: &str) -> &mut Self {
        for unit in text.encode_utf16() {
            self.u16(unit);
        }
        self.u16(0)
    }

    /// Java `writeString(null)`: only the terminator.
    pub fn opt_string(&mut self, text: Option<&str>) -> &mut Self {
        self.string(text.unwrap_or(""))
    }

    /// Java `writeSizedString(text)`: `u16` length in code units, then the units, no terminator.
    /// Empty or null writes just a `0` length.
    ///
    /// NOTE(java-quirk): Java writes `(short) len`, so strings over 32767 units would wrap.
    /// Client strings never get close; we truncate the count the same way.
    pub fn sized_string(&mut self, text: &str) -> &mut Self {
        let units: Vec<u16> = text.encode_utf16().collect();
        self.u16(units.len() as u16);
        for unit in units {
            self.u16(unit);
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader::PacketReader;
    use proptest::prelude::*;

    #[test]
    fn writes_little_endian() {
        let mut w = PacketWriter::new();
        w.u8(0x0b).i32(-2).u16(0x1234);
        assert_eq!(w.as_bytes(), &[0x0b, 0xfe, 0xff, 0xff, 0xff, 0x34, 0x12]);
    }

    #[test]
    fn string_has_terminator() {
        let mut w = PacketWriter::new();
        w.string("Hi");
        assert_eq!(w.as_bytes(), &[b'H', 0, b'i', 0, 0, 0]);
    }

    #[test]
    fn null_and_empty_strings() {
        let mut w = PacketWriter::new();
        w.opt_string(None).string("").sized_string("");
        assert_eq!(w.as_bytes(), &[0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn sized_string_layout() {
        let mut w = PacketWriter::new();
        w.sized_string("ok");
        assert_eq!(w.as_bytes(), &[2, 0, b'o', 0, b'k', 0]);
    }

    #[test]
    fn surrogate_pairs_round_trip() {
        let text = "Grüße 💎 戦士";
        let mut w = PacketWriter::new();
        w.string(text).sized_string(text);
        let bytes = w.into_bytes();
        let mut r = PacketReader::new(&bytes);
        assert_eq!(r.string(), text);
        assert_eq!(r.sized_string(), text);
        assert_eq!(r.remaining(), 0);
    }

    proptest! {
        #[test]
        fn primitives_round_trip(a: u8, b: i8, c: i16, d: u16, e: i32, f: u32, g: i64, h: f64, flag: bool) {
            let mut w = PacketWriter::new();
            w.u8(a).i8(b).i16(c).u16(d).i32(e).u32(f).i64(g).f64(h).bool(flag);
            let bytes = w.into_bytes();
            let mut r = PacketReader::new(&bytes);
            prop_assert_eq!(r.u8().unwrap(), a);
            prop_assert_eq!(r.i8().unwrap(), b);
            prop_assert_eq!(r.i16().unwrap(), c);
            prop_assert_eq!(r.u16().unwrap(), d);
            prop_assert_eq!(r.i32().unwrap(), e);
            prop_assert_eq!(r.u32().unwrap(), f);
            prop_assert_eq!(r.i64().unwrap(), g);
            prop_assert_eq!(r.f64().unwrap().to_bits(), h.to_bits());
            prop_assert_eq!(r.bool().unwrap(), flag);
            prop_assert_eq!(r.remaining(), 0);
        }

        #[test]
        fn strings_round_trip(s in "[^\u{0}]{0,64}") {
            let mut w = PacketWriter::new();
            w.string(&s).sized_string(&s);
            let bytes = w.into_bytes();
            let mut r = PacketReader::new(&bytes);
            prop_assert_eq!(r.string(), s.clone());
            prop_assert_eq!(r.sized_string(), s);
        }
    }
}
