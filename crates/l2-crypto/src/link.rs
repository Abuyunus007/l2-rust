//! The login↔game server link cipher: seal and open for packets between the two servers.
//!
//! Java: loginserver/GameServerThread.java (`sendPacket`, the read loop in `run`,
//! `DEFAULT_BLOWFISH_KEY`) and gameserver/LoginServerThread.java (`sendPacket`, the 40-byte
//! session key). Both sides do the same byte work, so one type serves both.
//!
//! Sealed body (the bytes after the 2-byte length header, which framing adds):
//! payload, then zero bytes up to a multiple of 8 that leave room for the 4-byte checksum, then the
//! checksum over the body's last word, then Blowfish-ECB encryption. Java writes a zero int after
//! the payload, pads, then overwrites the tail with the checksum; that gives the same bytes.

use crate::blowfish::BLOCK_SIZE;
use crate::checksum::{self, CHECKSUM_SIZE};
use crate::{CryptoError, L2Blowfish};

/// The static key both servers use before the session key is exchanged.
///
/// Java: `"_;v.]05-31!|+-%xT!^[$\00"`. `\00` is a NUL byte, so the key is 22 bytes, not 21.
pub const DEFAULT_KEY: &[u8] = b"_;v.]05-31!|+-%xT!^[$\0";

/// Keyed link cipher (Java `NewCrypt` as used on the login↔game link).
#[derive(Clone, Debug)]
pub struct LinkCipher {
    cipher: L2Blowfish,
}

impl LinkCipher {
    /// The cipher with the static default key, used before the session key is exchanged.
    pub fn default_key() -> Result<Self, CryptoError> {
        Self::new(DEFAULT_KEY)
    }

    /// The cipher for a session key (the 40-byte key the login server sends). Keys of 4 to 56
    /// bytes are accepted, as for [`L2Blowfish`].
    pub fn new(key: &[u8]) -> Result<Self, CryptoError> {
        Ok(Self {
            cipher: L2Blowfish::new(key)?,
        })
    }

    /// Builds the encrypted body for `payload`, byte-identical to Java's `sendPacket` body.
    ///
    /// The body is `payload` plus zero padding so that `payload.len() + 4` is a multiple of 8.
    /// Its last 4 bytes hold the checksum. Java's `sendPacket` pads only when `(len + 4) % 8 != 0`,
    /// which is the same as padding to the next multiple of 8.
    pub fn seal(&self, payload: &[u8]) -> Vec<u8> {
        let padded_len = (payload.len() + CHECKSUM_SIZE).next_multiple_of(BLOCK_SIZE);
        let mut body = Vec::with_capacity(padded_len);
        body.extend_from_slice(payload);
        body.resize(padded_len, 0);
        // Cannot fail: the body is at least 8 bytes and a multiple of 8, so it is a valid
        // checksum target and a valid block buffer.
        let _ = checksum::append(&mut body);
        let _ = self.cipher.encrypt_in_place(&mut body);
        body
    }

    /// Decrypts `body` in place and verifies its checksum (Java `GameServerThread.run`).
    ///
    /// Returns the whole decrypted body, padding and checksum slot included, because Java hands the
    /// whole buffer to the packet readers. Errors on a non-block-aligned body and on a checksum
    /// mismatch; the caller closes the connection on either.
    pub fn open<'a>(&self, body: &'a mut [u8]) -> Result<&'a [u8], CryptoError> {
        self.cipher.decrypt_in_place(body)?;
        if !checksum::verify(body) {
            return Err(CryptoError::ChecksumMismatch);
        }
        Ok(body)
    }
}
