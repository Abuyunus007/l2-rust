//! The game-traffic stream cipher (`Encryption`) and the game session key generator
//! (`BlowFishKeygen`).
//!
//! Java: gameserver/network/Encryption.java, gameserver/network/BlowFishKeygen.java.
//!
//! The cipher is a running XOR over each packet, keyed by a 16-byte session key whose bytes 8..11
//! hold a little-endian rolling offset. After every packet the offset grows by the packet size, so
//! the same plaintext never gives the same ciphertext twice on one connection.

/// Session key length in bytes.
pub const KEY_LEN: usize = 16;
/// Start of the rolling offset (little-endian `u32`) inside the key.
const OFFSET_INDEX: usize = 8;
/// Fixed tail of every game session key (`BlowFishKeygen.KEY_TAIL_BYTES`).
const KEY_TAIL: [u8; 8] = [0xC8, 0x27, 0x93, 0x01, 0xA1, 0x6C, 0x31, 0x97];

/// Stateful game cipher with separate inbound (`decrypt`) and outbound (`encrypt`) key state.
#[derive(Clone, Debug)]
pub struct GameCipher {
    in_key: [u8; KEY_LEN],
    out_key: [u8; KEY_LEN],
    // One flag for both directions, as in Java's single `_isEnabled`.
    enabled: bool,
}

impl GameCipher {
    /// Starts both directions from the same session key.
    pub fn new(key: [u8; KEY_LEN]) -> Self {
        Self {
            in_key: key,
            out_key: key,
            enabled: false,
        }
    }

    /// Encrypts `buf` in place and advances the outbound offset by `buf.len()`.
    pub fn encrypt(&mut self, buf: &mut [u8]) {
        // NOTE(java-quirk): the first call only enables the cipher. That packet goes out in clear
        // and the key does not move. The flag is shared with `decrypt`, so enabling via encrypt
        // also enables decrypt.
        if !self.enabled {
            self.enabled = true;
            return;
        }
        if buf.is_empty() {
            return;
        }
        // Each byte is XORed with the key byte at (index mod 16) and with the previous
        // ciphertext byte. The index is the position inside this packet.
        let mut prev = 0u8;
        for (i, b) in buf.iter_mut().enumerate() {
            let cipher = *b ^ self.out_key[i % KEY_LEN] ^ prev;
            *b = cipher;
            prev = cipher;
        }
        advance_offset(&mut self.out_key, buf.len());
    }

    /// Decrypts `buf` in place and advances the inbound offset by `buf.len()`. Does nothing until
    /// the cipher has been enabled by an `encrypt` call.
    pub fn decrypt(&mut self, buf: &mut [u8]) {
        if !self.enabled || buf.is_empty() {
            return;
        }
        let mut last = 0u8;
        for (i, b) in buf.iter_mut().enumerate() {
            let enc = *b;
            *b = enc ^ self.in_key[i % KEY_LEN] ^ last;
            last = enc;
        }
        advance_offset(&mut self.in_key, buf.len());
    }
}

/// Adds the packet size to the little-endian offset at key bytes 8..11, wrapping like Java's
/// `int`.
fn advance_offset(key: &mut [u8; KEY_LEN], size: usize) {
    // Packets are far below 4 GiB, so the `usize` to `u32` cast cannot truncate in practice.
    let old = u32::from_le_bytes([
        key[OFFSET_INDEX],
        key[OFFSET_INDEX + 1],
        key[OFFSET_INDEX + 2],
        key[OFFSET_INDEX + 3],
    ]);
    let new = old.wrapping_add(size as u32).to_le_bytes();
    key[OFFSET_INDEX..OFFSET_INDEX + 4].copy_from_slice(&new);
}

/// Builds a game session key: 8 random bytes followed by the fixed tail (`getRandomKey`).
///
/// The caller supplies the random bytes, so the result is reproducible in tests.
pub fn game_key_from_random(random8: [u8; 8]) -> [u8; KEY_LEN] {
    let mut key = [0u8; KEY_LEN];
    key[..8].copy_from_slice(&random8);
    key[8..].copy_from_slice(&KEY_TAIL);
    key
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_is_random_prefix_then_fixed_tail() {
        let key = game_key_from_random([1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(&key[..8], &[1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(&key[8..], &KEY_TAIL);
    }

    #[test]
    fn first_encrypt_only_enables() {
        let mut cipher = GameCipher::new([0x11; KEY_LEN]);
        let mut buf = [1u8, 2, 3];
        cipher.encrypt(&mut buf);
        assert_eq!(buf, [1, 2, 3]);
        assert_eq!(cipher.out_key, [0x11; KEY_LEN]);
    }

    #[test]
    fn decrypt_is_a_no_op_until_enabled() {
        let mut cipher = GameCipher::new([0x22; KEY_LEN]);
        let mut buf = [9u8, 8, 7];
        cipher.decrypt(&mut buf);
        assert_eq!(buf, [9, 8, 7]);
        assert_eq!(cipher.in_key, [0x22; KEY_LEN]);
    }

    #[test]
    fn offset_wraps_like_java_int() {
        let mut key = [0u8; KEY_LEN];
        key[OFFSET_INDEX..OFFSET_INDEX + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        advance_offset(&mut key, 3);
        assert_eq!(key[OFFSET_INDEX..OFFSET_INDEX + 4], 2u32.to_le_bytes());
    }
}
