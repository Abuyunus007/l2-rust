//! The per-frame encryption hook.
//!
//! Java: `Client.encrypt/decrypt` overrides (LoginClient, GameClient) and the link threads.
//! The concrete ciphers live in `l2-crypto`; their users wrap them in this trait, so `l2-net`
//! doesn't depend on `l2-crypto`.

use thiserror::Error;

/// Decryption failed (bad checksum, wrong length, ...). The connection is closed.
#[derive(Debug, Error, PartialEq, Eq)]
#[error("frame cipher: {0}")]
pub struct CipherError(pub String);

/// Encrypts outgoing bodies and decrypts incoming ones, one frame at a time.
///
/// A connection owns exactly one cipher. Protocol code may replace it between frames
/// (see [`crate::Control::set_cipher`]), e.g. when the LS↔GS link switches keys.
pub trait FrameCipher: Send {
    /// Turns a plaintext payload into the body that goes on the wire (may grow it: padding,
    /// checksums).
    fn encrypt(&mut self, payload: Vec<u8>) -> Vec<u8>;

    /// Decrypts a received body in place.
    fn decrypt(&mut self, body: &mut Vec<u8>) -> Result<(), CipherError>;
}

/// No encryption (game traffic with `PacketEncryption = False`, tests).
#[derive(Debug, Default, Clone, Copy)]
pub struct NoCipher;

impl FrameCipher for NoCipher {
    fn encrypt(&mut self, payload: Vec<u8>) -> Vec<u8> {
        payload
    }

    fn decrypt(&mut self, _body: &mut Vec<u8>) -> Result<(), CipherError> {
        Ok(())
    }
}
