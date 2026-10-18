//! L2's Blowfish: standard Blowfish with **little-endian** 32-bit words (ECB, no padding).
//!
//! Java: commons/crypt/BlowfishEngine.java (`bytesTo32bits`/`bits32ToBytes` are little-endian and
//! the output is written `xr` first, which is what the `blowfish` crate's `BlowfishLE` does,
//! D-24). The S-boxes and key schedule come from that crate; the Java vectors in
//! `crypto_blowfish.json` and `crypto_newcrypt.json` prove they match.

use blowfish::BlowfishLE;
use cipher::{Block, BlockCipherDecrypt, BlockCipherEncrypt, KeyInit};

use crate::CryptoError;

/// Blowfish block size in bytes.
pub const BLOCK_SIZE: usize = 8;

/// A keyed little-endian Blowfish cipher (Java `BlowfishEngine`, `NewCrypt.crypt`/`decrypt`).
#[derive(Clone, Debug)]
pub struct L2Blowfish {
    cipher: BlowfishLE,
}

impl L2Blowfish {
    /// Creates the cipher from a key of 4 to 56 bytes.
    ///
    /// For a text key (Java `NewCrypt(String)`) pass its UTF-8 bytes (`key.as_bytes()`).
    pub fn new(key: &[u8]) -> Result<Self, CryptoError> {
        let cipher =
            BlowfishLE::new_from_slice(key).map_err(|_| CryptoError::InvalidKeyLength { len: key.len() })?;
        Ok(Self { cipher })
    }

    /// Encrypts `buf` in place, block by block (ECB). Length must be a multiple of 8.
    pub fn encrypt_in_place(&self, buf: &mut [u8]) -> Result<(), CryptoError> {
        check_aligned(buf)?;
        for chunk in buf.chunks_exact_mut(BLOCK_SIZE) {
            if let Ok(block) = <&mut Block<BlowfishLE>>::try_from(chunk) {
                self.cipher.encrypt_block(block);
            }
        }
        Ok(())
    }

    /// Decrypts `buf` in place, block by block (ECB). Length must be a multiple of 8.
    pub fn decrypt_in_place(&self, buf: &mut [u8]) -> Result<(), CryptoError> {
        check_aligned(buf)?;
        for chunk in buf.chunks_exact_mut(BLOCK_SIZE) {
            if let Ok(block) = <&mut Block<BlowfishLE>>::try_from(chunk) {
                self.cipher.decrypt_block(block);
            }
        }
        Ok(())
    }
}

fn check_aligned(buf: &[u8]) -> Result<(), CryptoError> {
    if buf.len() % BLOCK_SIZE != 0 {
        return Err(CryptoError::NotBlockAligned { len: buf.len() });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_bad_key_lengths() {
        assert_eq!(
            L2Blowfish::new(&[1, 2, 3]).unwrap_err(),
            CryptoError::InvalidKeyLength { len: 3 }
        );
        assert_eq!(
            L2Blowfish::new(&[0; 57]).unwrap_err(),
            CryptoError::InvalidKeyLength { len: 57 }
        );
        assert!(L2Blowfish::new(&[0; 4]).is_ok());
        assert!(L2Blowfish::new(&[0; 56]).is_ok());
    }

    #[test]
    fn rejects_unaligned_buffers_without_touching_them() {
        let bf = L2Blowfish::new(b"some key").unwrap();
        let mut buf = [7u8; 12];
        assert_eq!(
            bf.encrypt_in_place(&mut buf),
            Err(CryptoError::NotBlockAligned { len: 12 })
        );
        assert_eq!(
            bf.decrypt_in_place(&mut buf),
            Err(CryptoError::NotBlockAligned { len: 12 })
        );
        assert_eq!(buf, [7u8; 12]);
    }

    #[test]
    fn empty_buffer_is_ok() {
        let bf = L2Blowfish::new(b"some key").unwrap();
        bf.encrypt_in_place(&mut []).unwrap();
        bf.decrypt_in_place(&mut []).unwrap();
    }
}
