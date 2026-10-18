use thiserror::Error;

/// Errors from the crypto primitives. Raised for bad key or buffer sizes instead of panicking.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CryptoError {
    /// Blowfish keys must be 4 to 56 bytes.
    #[error("invalid blowfish key length {len}: must be 4..=56 bytes")]
    InvalidKeyLength { len: usize },

    /// Blowfish works on whole 8-byte blocks.
    #[error("buffer length {len} is not a multiple of the 8-byte blowfish block size")]
    NotBlockAligned { len: usize },

    /// The buffer is too short for the operation (for example no room for the 4-byte checksum).
    #[error("buffer too short: {len} bytes, need at least {min}")]
    TooShort { len: usize, min: usize },

    /// The checksum covers whole 32-bit words.
    #[error("buffer length {len} is not a multiple of 4")]
    NotWordAligned { len: usize },
}
