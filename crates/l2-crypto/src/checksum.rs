//! The packet checksum of `NewCrypt.appendChecksum` / `verifyChecksum`.
//!
//! Java: commons/crypt/NewCrypt.java. The buffer is read as little-endian 32-bit words; the last
//! word is the checksum slot and holds the XOR of all words before it.
//!
//! NOTE(java-quirk): Java's `appendChecksum/verifyChecksum(buf, offset, size)` compute the end of
//! the data as `size - 4` **without adding `offset`**, so they are only right when called as
//! `(data, 2, len - 2)` on `[2-byte length header][body]`, which is exactly what
//! `GameServerThread.sendPacket` and `LoginServerThread.sendPacket` do. That works out to the
//! checksum over the body alone. So these functions take the *body slice* (offset-0 semantics):
//! pass `&mut data[2..]`. The test `matches_java_offset_call_on_length_prefixed_packet` in
//! `tests/vectors_newcrypt.rs` pins this against a literal port of the Java loop.

use crate::CryptoError;

/// Size of the checksum slot at the end of the buffer.
pub const CHECKSUM_SIZE: usize = 4;

/// Stores the XOR of all words except the last in the last 4 bytes (`appendChecksum`).
///
/// `buf.len()` must be a multiple of 4 and at least 4; a 4-byte buffer gets checksum 0 like Java.
pub fn append(buf: &mut [u8]) -> Result<(), CryptoError> {
    check_len(buf.len())?;
    let (data, slot) = buf.split_at_mut(buf.len() - CHECKSUM_SIZE);
    slot.copy_from_slice(&xor_words(data).to_le_bytes());
    Ok(())
}

/// Checks the trailing checksum (`verifyChecksum`). Like Java, returns `false` unless the length
/// is a multiple of 4 and greater than 4 (a checksum slot alone proves nothing).
pub fn verify(buf: &[u8]) -> bool {
    if buf.len() % 4 != 0 || buf.len() <= CHECKSUM_SIZE {
        return false;
    }
    let (data, slot) = buf.split_at(buf.len() - CHECKSUM_SIZE);
    let stored = u32::from_le_bytes([slot[0], slot[1], slot[2], slot[3]]);
    stored == xor_words(data)
}

fn check_len(len: usize) -> Result<(), CryptoError> {
    if len < CHECKSUM_SIZE {
        return Err(CryptoError::TooShort {
            len,
            min: CHECKSUM_SIZE,
        });
    }
    if len % 4 != 0 {
        return Err(CryptoError::NotWordAligned { len });
    }
    Ok(())
}

/// XOR of the little-endian words of `data` (a trailing partial word is ignored; callers pass
/// multiples of 4).
fn xor_words(data: &[u8]) -> u32 {
    data.chunks_exact(4)
        .fold(0, |acc, w| acc ^ u32::from_le_bytes([w[0], w[1], w[2], w[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_rejects_bad_lengths() {
        assert_eq!(
            append(&mut [0u8; 3]),
            Err(CryptoError::TooShort { len: 3, min: 4 })
        );
        assert_eq!(append(&mut [0u8; 6]), Err(CryptoError::NotWordAligned { len: 6 }));
        assert_eq!(append(&mut []), Err(CryptoError::TooShort { len: 0, min: 4 }));
    }

    #[test]
    fn append_then_verify() {
        let mut buf = [1u8, 2, 3, 4, 5, 6, 7, 8, 0, 0, 0, 0];
        append(&mut buf).unwrap();
        assert!(verify(&buf));
        buf[0] ^= 1;
        assert!(!verify(&buf));
    }

    #[test]
    fn verify_needs_more_than_the_slot() {
        assert!(!verify(&[]));
        assert!(!verify(&[0; 4]));
        assert!(!verify(&[0; 6]));
    }
}
