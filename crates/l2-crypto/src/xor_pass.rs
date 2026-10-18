//! `NewCrypt.encXORPass`: the rolling XOR the login server applies to the `Init` packet.
//!
//! Java: commons/crypt/NewCrypt.java. Words are little-endian `i32`; the key is added with Java's
//! wrapping `int` arithmetic.

use crate::CryptoError;

/// Offset of the first encrypted word (the first 4 bytes stay clear).
const START: usize = 4;
/// The last 8 bytes are not encrypted words: 4 bytes of padding and the 4-byte XOR key slot.
const TAIL: usize = 8;

/// Encrypts `buf` in place and writes the final rolling key right after the last encrypted word
/// (`encXORPass(buf, 0, buf.len(), key)`).
///
/// `buf` needs room for the key, so it must be at least 8 bytes. Bytes `0..4` are left alone.
pub fn encrypt(buf: &mut [u8], key: i32) -> Result<(), CryptoError> {
    if buf.len() < TAIL {
        return Err(CryptoError::TooShort {
            len: buf.len(),
            min: TAIL,
        });
    }
    let end = buf.len() - TAIL;
    let mut pos = START;
    let mut rolling = key;
    while pos < end {
        // pos < end = len - 8, so pos + 4 is inside the buffer.
        let Some(word) = buf.get_mut(pos..pos + 4) else {
            break;
        };
        let mut data = i32::from_le_bytes([word[0], word[1], word[2], word[3]]);
        rolling = rolling.wrapping_add(data);
        data ^= rolling;
        word.copy_from_slice(&data.to_le_bytes());
        pos += 4;
    }
    // The loop stops at pos < end + 4 = len - 4, so the 4-byte key slot fits.
    if let Some(slot) = buf.get_mut(pos..pos + 4) {
        slot.copy_from_slice(&rolling.to_le_bytes());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_buffers_without_room_for_the_key() {
        assert_eq!(
            encrypt(&mut [0u8; 7], 1),
            Err(CryptoError::TooShort { len: 7, min: 8 })
        );
    }

    #[test]
    fn eight_bytes_only_stores_the_key() {
        let mut buf = [9u8, 9, 9, 9, 0, 0, 0, 0];
        encrypt(&mut buf, 0x0102_0304).unwrap();
        assert_eq!(buf, [9, 9, 9, 9, 4, 3, 2, 1]);
    }

    #[test]
    fn wraps_like_java_int() {
        // key + word overflows i32; Java wraps silently.
        let mut buf = [0u8; 16];
        buf[4..8].copy_from_slice(&i32::MAX.to_le_bytes());
        encrypt(&mut buf, i32::MAX).unwrap();
        // rolling = MAX + MAX = -2; data = MAX ^ -2 = 0x8000_0001.
        assert_eq!(buf[4..8], 0x8000_0001u32.to_le_bytes());
        assert_eq!(buf[8..12], (-2i32).to_le_bytes());
    }
}
