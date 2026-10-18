//! The login server's client packet cipher: static-key first packet, then the session Blowfish.
//!
//! Java: loginserver/network/LoginEncryption.java (plus `NewCrypt`, see [`crate::checksum`] and
//! [`crate::xor_pass`]).
//!
//! Both types here work on the packet **body**, i.e. without the 2-byte length header; framing
//! is someone else's job.
//!
//! - [`LoginCipher`] is the server side (Java `LoginEncryption`): encrypts server packets,
//!   decrypts client packets.
//! - [`LoginClientCipher`] is the mirror image for the proxy and fake clients in tests: decrypts
//!   server packets (including the static first packet) and encrypts client packets.
//!
//! Wire format:
//!
//! - **First server packet (`Init`)**: `payload | zero padding | xor key (4) | zero (4)`, XOR
//!   pass over the words from offset 4, then Blowfish with the static key. No checksum.
//! - **Later packets (both directions)**: `payload | zero padding | checksum (4)`, then Blowfish
//!   with the session key. The checksum is the XOR of all earlier 32-bit words
//!   ([`crate::checksum`]).

use crate::blowfish::BLOCK_SIZE;
use crate::{CryptoError, L2Blowfish, checksum};

/// The static Blowfish key of the first packet (Java `STATIC_BLOWFISH_KEY`).
pub const STATIC_BLOWFISH_KEY: [u8; 16] = [
    0x6b, 0x60, 0xcb, 0x5b, 0x82, 0xce, 0x90, 0xb1, 0xcc, 0x2b, 0x6c, 0x55, 0x6c, 0x6c, 0x6c, 0x6c,
];

/// Java `STATIC_HEADER_SIZE`: room for the XOR pass (4 bytes key + 4 bytes untouched).
const STATIC_HEADER_SIZE: usize = 8;
/// Java `DYNAMIC_HEADER_SIZE`: the checksum slot.
const DYNAMIC_HEADER_SIZE: usize = 4;
/// Java `CHECKSUM_SIZE`: extra bytes appended after the padding (despite the name).
const TRAILER_SIZE: usize = 8;

/// Size of an encrypted server packet body for a payload of `payload_len` bytes
/// (Java `LoginEncryption.encryptedSize`). `first` selects the static first packet.
///
/// NOTE(java-quirk): the padding is `BLOCK - remainder`, not `remainder == 0 ? 0 : ...`, so an
/// already aligned size still gets a full extra block of 8 bytes (for example a 4-byte session
/// payload: 4 + 4 = 8, +8 padding, +8 trailer = 24, not 16). The client does not care, but the
/// sizes must match Java for byte-exact replays.
pub fn encrypted_size(payload_len: usize, first: bool) -> usize {
    let header = if first {
        STATIC_HEADER_SIZE
    } else {
        DYNAMIC_HEADER_SIZE
    };
    let with_header = payload_len + header;
    let padded = with_header + (BLOCK_SIZE - with_header % BLOCK_SIZE);
    padded + TRAILER_SIZE
}

/// Body size of a client packet in [`LoginClientCipher::encrypt`]: payload + checksum slot,
/// rounded up to a whole block (no extra block when already aligned).
///
/// The Java server accepts any block-aligned body with a valid checksum, so this is a free
/// choice for a test client (the real client's exact padding was not captured).
fn client_encrypted_size(payload_len: usize) -> usize {
    (payload_len + DYNAMIC_HEADER_SIZE).next_multiple_of(BLOCK_SIZE)
}

/// Server side of the client cipher (Java `LoginEncryption`).
///
/// The first [`encrypt`](Self::encrypt) call produces the `Init` packet with the static key; all
/// later calls use the session key. Decryption always uses the session key.
#[derive(Clone, Debug)]
pub struct LoginCipher {
    static_bf: L2Blowfish,
    session_bf: L2Blowfish,
    /// Java `_usingStaticKey`: true until the first packet was encrypted.
    using_static_key: bool,
}

impl LoginCipher {
    /// Creates the cipher for a client whose Blowfish session key is `session_key` (the 16 bytes
    /// sent to the client in `Init`).
    pub fn new(session_key: &[u8; 16]) -> Self {
        Self {
            static_bf: blowfish_16(&STATIC_BLOWFISH_KEY),
            session_bf: blowfish_16(session_key),
            using_static_key: true,
        }
    }

    /// Size of the next packet [`encrypt`](Self::encrypt) will produce for `payload_len` bytes.
    pub fn next_encrypted_size(&self, payload_len: usize) -> usize {
        encrypted_size(payload_len, self.using_static_key)
    }

    /// Encrypts one server packet and returns the body (without the length header).
    ///
    /// The first call uses the static key and the XOR pass with `xor_key` (Java draws
    /// `Rnd.nextInt()` here; the caller passes it in so tests are deterministic). Later calls use
    /// checksum + session key and ignore `xor_key`.
    pub fn encrypt(&mut self, payload: &[u8], xor_key: i32) -> Vec<u8> {
        let size = encrypted_size(payload.len(), self.using_static_key);
        let mut raw = padded_buffer(payload, size);
        // `size` is a multiple of 8 and at least 16, so the primitives below cannot fail.
        if self.using_static_key {
            let xor = crate::xor_pass::encrypt(&mut raw, xor_key);
            let bf = self.static_bf.encrypt_in_place(&mut raw);
            debug_assert!(xor.is_ok() && bf.is_ok());
            self.using_static_key = false;
        } else {
            let sum = checksum::append(&mut raw);
            let bf = self.session_bf.encrypt_in_place(&mut raw);
            debug_assert!(sum.is_ok() && bf.is_ok());
        }
        raw
    }

    /// Decrypts a client packet body in place with the session key and verifies its checksum.
    ///
    /// On an invalid checksum returns [`CryptoError::ChecksumMismatch`] and leaves `body` untouched
    /// (like Java); the caller closes the connection.
    pub fn decrypt(&self, body: &mut [u8]) -> Result<(), CryptoError> {
        session_decrypt(&self.session_bf, body)
    }
}

/// Client side of the login cipher, for the proxy and fake clients in tests.
///
/// Not in Java. It reverses [`LoginCipher`]: the first [`decrypt`](Self::decrypt) call expects the
/// static `Init` packet (static Blowfish, then the inverse of the XOR pass, no checksum); later
/// calls use the session key and verify the checksum.
#[derive(Clone, Debug)]
pub struct LoginClientCipher {
    session_bf: L2Blowfish,
    /// True while the next server packet is still the static `Init` packet.
    expecting_init: bool,
}

impl LoginClientCipher {
    /// Creates a cipher that still expects the static `Init` packet. `session_key` is the key the
    /// `Init` packet carries; a test client that knows it in advance passes it here.
    pub fn new(session_key: &[u8; 16]) -> Self {
        Self {
            session_bf: blowfish_16(session_key),
            expecting_init: true,
        }
    }

    /// Creates a cipher for a client that already decrypted `Init` itself with [`decrypt_init`]
    /// and read the session key from it (what a proxy does).
    pub fn after_init(session_key: &[u8; 16]) -> Self {
        Self {
            session_bf: blowfish_16(session_key),
            expecting_init: false,
        }
    }

    /// Decrypts a server packet body in place.
    ///
    /// The first call is the `Init` packet: static key, then the XOR pass is undone, so `body`
    /// holds the payload followed by zero padding, the XOR key and 4 zero bytes. Later calls use
    /// the session key, verify the checksum and leave the payload, padding and checksum slot.
    /// On error `body` is left untouched and the state does not advance.
    pub fn decrypt(&mut self, body: &mut [u8]) -> Result<(), CryptoError> {
        if self.expecting_init {
            decrypt_init(body)?;
            self.expecting_init = false;
            Ok(())
        } else {
            session_decrypt(&self.session_bf, body)
        }
    }

    /// Encrypts one client packet with the session key and returns the body (without the length
    /// header): payload, zero padding, checksum.
    pub fn encrypt(&self, payload: &[u8]) -> Vec<u8> {
        let mut raw = padded_buffer(payload, client_encrypted_size(payload.len()));
        // The size is a multiple of 8 and at least 8, so the primitives cannot fail.
        let sum = checksum::append(&mut raw);
        let bf = self.session_bf.encrypt_in_place(&mut raw);
        debug_assert!(sum.is_ok() && bf.is_ok());
        raw
    }
}

/// Decrypts the static first packet (`Init`) in place: static-key Blowfish, then
/// [`dec_xor_pass`]. Returns the XOR key the server started the pass with.
///
/// A client needs this before it can know the session key, hence the free function.
pub fn decrypt_init(body: &mut [u8]) -> Result<i32, CryptoError> {
    let static_bf = blowfish_16(&STATIC_BLOWFISH_KEY);
    static_bf.decrypt_in_place(body)?;
    dec_xor_pass(body)
}

/// Inverse of [`crate::xor_pass::encrypt`] (Java has no `decXORPass`; only the client does this).
///
/// The forward pass is `rolling += word; word ^= rolling` over the words in `4..len-8`, and then
/// stores the final `rolling` right behind them (at `len-8`). Backwards: read that final value,
/// and for each word from the last to the first: `word ^= rolling` gives the plain word, and
/// `rolling -= word` gives the previous rolling value. What is left after the first word is the
/// start key, which is returned. The key slot itself is left in place.
fn dec_xor_pass(buf: &mut [u8]) -> Result<i32, CryptoError> {
    if buf.len() < 8 {
        return Err(CryptoError::TooShort {
            len: buf.len(),
            min: 8,
        });
    }
    if buf.len() % 4 != 0 {
        return Err(CryptoError::NotWordAligned { len: buf.len() });
    }
    // The pass stores the key at `len - 8`, or at 4 for an 8-byte buffer (it never touches 0..4).
    let (head, tail) = buf.split_at_mut((buf.len() - 8).max(4));
    let mut rolling = match tail.first_chunk::<4>() {
        Some(slot) => i32::from_le_bytes(*slot),
        None => {
            return Err(CryptoError::TooShort {
                len: tail.len(),
                min: 4,
            });
        }
    };
    // head.len() = len - 8 >= 0; the first 4 bytes are never touched by the pass.
    if let Some(words) = head.get_mut(4..) {
        for word in words.chunks_exact_mut(4).rev() {
            let cipher = i32::from_le_bytes([word[0], word[1], word[2], word[3]]);
            let plain = cipher ^ rolling;
            rolling = rolling.wrapping_sub(plain);
            word.copy_from_slice(&plain.to_le_bytes());
        }
    }
    Ok(rolling)
}

/// Session-key decrypt + checksum check shared by both ciphers. `body` is only modified when the
/// checksum is valid (Java decrypts a copy and writes it back on success).
fn session_decrypt(bf: &L2Blowfish, body: &mut [u8]) -> Result<(), CryptoError> {
    let mut raw = body.to_vec();
    bf.decrypt_in_place(&mut raw)?;
    if !checksum::verify(&raw) {
        return Err(CryptoError::ChecksumMismatch);
    }
    body.copy_from_slice(&raw);
    Ok(())
}

/// A zeroed buffer of `size` bytes starting with `payload` (`size` > `payload.len()`).
fn padded_buffer(payload: &[u8], size: usize) -> Vec<u8> {
    let mut raw = vec![0u8; size];
    if let Some(dst) = raw.get_mut(..payload.len()) {
        dst.copy_from_slice(payload);
    }
    raw
}

/// Builds a cipher from a fixed 16-byte key.
#[allow(clippy::expect_used)]
fn blowfish_16(key: &[u8; 16]) -> L2Blowfish {
    // Blowfish accepts 4..=56 byte keys, so a 16-byte key can never be rejected.
    L2Blowfish::new(key).expect("16-byte blowfish key is always valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: [u8; 16] = [
        0x1c, 0x4f, 0x9a, 0xe0, 0xf7, 0xc1, 0x4b, 0x2a, 0x9f, 0x68, 0x77, 0x25, 0x33, 0x02, 0xfe, 0x93,
    ];

    #[test]
    fn sizes_match_java_formula() {
        // first: ((n + 8) rounded up, at least +1) + 8
        assert_eq!(encrypted_size(0, true), 24);
        assert_eq!(encrypted_size(1, true), 24);
        assert_eq!(encrypted_size(7, true), 24);
        assert_eq!(encrypted_size(8, true), 32); // aligned: full extra block (java-quirk)
        // later: ((n + 4) rounded up, at least +1) + 8
        assert_eq!(encrypted_size(1, false), 16);
        assert_eq!(encrypted_size(3, false), 16);
        assert_eq!(encrypted_size(4, false), 24); // aligned: full extra block (java-quirk)
        assert_eq!(encrypted_size(11, false), 24);
        assert_eq!(encrypted_size(12, false), 32);
    }

    #[test]
    fn dec_xor_pass_rejects_bad_lengths() {
        assert_eq!(
            dec_xor_pass(&mut [0u8; 7]),
            Err(CryptoError::TooShort { len: 7, min: 8 })
        );
        assert_eq!(
            dec_xor_pass(&mut [0u8; 10]),
            Err(CryptoError::NotWordAligned { len: 10 })
        );
    }

    #[test]
    fn dec_xor_pass_inverts_enc_xor_pass() {
        for len in [8usize, 16, 24, 32, 64] {
            for key in [0, 1, -1, i32::MAX, i32::MIN, 0x1234_5678] {
                let mut plain: Vec<u8> = (0..len)
                    .map(|i| (i as u8).wrapping_mul(53).wrapping_add(7))
                    .collect();
                // The last 8 bytes are the key slot area; clear them like a real packet.
                plain[len - 8..].fill(0);
                let mut buf = plain.clone();
                crate::xor_pass::encrypt(&mut buf, key).unwrap();
                assert_eq!(dec_xor_pass(&mut buf).unwrap(), key, "len {len} key {key}");
                // Plain data is back; the key slot still holds the final rolling key.
                assert_eq!(buf[..len - 8], plain[..len - 8]);
                if len > 8 {
                    assert_eq!(buf[len - 4..], plain[len - 4..]);
                }
            }
        }
    }

    #[test]
    fn first_packet_round_trip() {
        let mut server = LoginCipher::new(&KEY);
        let mut client = LoginClientCipher::new(&KEY);
        let payload = [0x00, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let mut body = server.encrypt(&payload, 0x0bad_cafe);
        assert_eq!(body.len(), encrypted_size(payload.len(), true));
        client.decrypt(&mut body).unwrap();
        assert_eq!(&body[..payload.len()], &payload);
        assert!(body[payload.len()..body.len() - 8].iter().all(|b| *b == 0));
    }

    #[test]
    fn decrypt_init_returns_the_xor_key() {
        let mut server = LoginCipher::new(&KEY);
        let mut body = server.encrypt(&[9, 8, 7], -1_234_567);
        assert_eq!(decrypt_init(&mut body).unwrap(), -1_234_567);
    }

    #[test]
    fn session_round_trip_both_directions() {
        let mut server = LoginCipher::new(&KEY);
        let mut client = LoginClientCipher::new(&KEY);
        let mut init = server.encrypt(&[0u8; 20], 5);
        client.decrypt(&mut init).unwrap();

        for len in 0..40usize {
            let payload: Vec<u8> = (0..len).map(|i| (i as u8) ^ 0x5a).collect();

            // server -> client
            let mut body = server.encrypt(&payload, 0);
            assert_eq!(body.len(), encrypted_size(len, false));
            client.decrypt(&mut body).unwrap();
            assert_eq!(&body[..len], &payload[..]);

            // client -> server
            let mut body = client.encrypt(&payload);
            assert_eq!(body.len() % 8, 0);
            server.decrypt(&mut body).unwrap();
            assert_eq!(&body[..len], &payload[..]);
        }
    }

    #[test]
    fn corrupted_packet_is_rejected_and_left_untouched() {
        let mut server = LoginCipher::new(&KEY);
        let client = LoginClientCipher::new(&KEY);
        let mut body = client.encrypt(&[1, 2, 3, 4, 5]);
        let last = body.len() - 1;
        body[last] ^= 1;
        let corrupted = body.clone();
        assert_eq!(server.decrypt(&mut body), Err(CryptoError::ChecksumMismatch));
        assert_eq!(body, corrupted);
        // The failed decrypt did not consume the static phase of the encrypt side.
        assert_eq!(server.encrypt(&[1], 0).len(), 24);
    }

    #[test]
    fn unaligned_or_empty_bodies_are_errors() {
        let server = LoginCipher::new(&KEY);
        assert_eq!(
            server.decrypt(&mut [0u8; 12]),
            Err(CryptoError::NotBlockAligned { len: 12 })
        );
        // An empty body has no checksum slot.
        assert_eq!(server.decrypt(&mut []), Err(CryptoError::ChecksumMismatch));
        let mut client = LoginClientCipher::new(&KEY);
        assert!(client.decrypt(&mut [0u8; 5]).is_err());
        assert!(client.decrypt(&mut []).is_err());
    }

    #[test]
    fn failed_init_does_not_advance_the_client() {
        let mut client = LoginClientCipher::new(&KEY);
        assert!(client.decrypt(&mut [0u8; 4]).is_err());
        let mut server = LoginCipher::new(&KEY);
        let mut body = server.encrypt(&[1, 2], 3);
        client.decrypt(&mut body).unwrap();
    }

    #[test]
    fn after_init_skips_the_static_phase() {
        let mut server = LoginCipher::new(&KEY);
        let mut init = server.encrypt(&[1, 2, 3], 7);
        decrypt_init(&mut init).unwrap();
        let mut client = LoginClientCipher::after_init(&KEY);
        let mut body = server.encrypt(&[4, 5, 6], 0);
        client.decrypt(&mut body).unwrap();
        assert_eq!(&body[..3], &[4, 5, 6]);
    }
}
