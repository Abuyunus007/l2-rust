//! Raw RSA as L2 uses it: `RSA/ECB/NoPadding`, no padding scheme anywhere.
//!
//! Java: loginserver/controller/LoginController.java (1024-bit client keys),
//! loginserver/data/GameServerTable.java `initRSAKeys` (512-bit GS link keys),
//! loginserver/network/clientpackets/RequestAuthLogin.java,
//! loginserver/network/gameserverpackets/receive/BlowFishKey.java and
//! send/InitLS.java (`modulus.toByteArray()`).
//!
//! Java's `Cipher` without padding treats the input as a big-endian integer `c`, fails if
//! `c >= n` or the input is longer than the modulus, and returns `c^d mod n` left-padded with
//! zeros to the modulus length. The functions here do the same.

use rsa::BigUint;
use rsa::RsaPrivateKey;
use rsa::traits::{PrivateKeyParts, PublicKeyParts};
use thiserror::Error;

/// Errors from the raw RSA operations. Separate from [`CryptoError`](crate::CryptoError) so the
/// RSA card does not touch the shared enum.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum RsaError {
    /// Key generation failed (for example a bit size the `rsa` crate refuses).
    #[error("RSA key generation failed")]
    KeyGeneration,

    /// Modulus or exponent is zero or empty.
    #[error("invalid RSA key: modulus and exponent must be non-zero")]
    InvalidKey,

    /// Java throws when the block is longer than the modulus.
    #[error("RSA block of {len} bytes is longer than the {max}-byte modulus")]
    BlockTooLong { len: usize, max: usize },

    /// Java throws "Message is larger than modulus" when `c >= n`.
    #[error("RSA block value is not smaller than the modulus")]
    BlockOutOfRange,

    /// An empty block (Java behaviour not pinned down, so it is rejected).
    #[error("empty RSA block")]
    EmptyBlock,

    /// The scrambled modulus only exists for the 128-byte (1017..=1024 bit) client key.
    #[error("modulus has {len} bytes, the scrambled modulus needs exactly 128")]
    ModulusSize { len: usize },
}

/// Adapts a `rand` 0.10 RNG (the workspace RNG, D-13) to the `rand_core` 0.6 traits that `rsa`
/// 0.9 still uses for key generation.
struct RngCompat<'a, R: ?Sized>(&'a mut R);

impl<R: rand::Rng + ?Sized> rsa::rand_core::RngCore for RngCompat<'_, R> {
    fn next_u32(&mut self) -> u32 {
        self.0.next_u32()
    }

    fn next_u64(&mut self) -> u64 {
        self.0.next_u64()
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        self.0.fill_bytes(dest);
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rsa::rand_core::Error> {
        self.0.fill_bytes(dest);
        Ok(())
    }
}

impl<R: rand::CryptoRng + ?Sized> rsa::rand_core::CryptoRng for RngCompat<'_, R> {}

/// Java `BigInteger.toByteArray()` for a non-negative number given as big-endian bytes:
/// minimal two's complement, so a leading `0x00` when the top bit is set. Zero is `[0]`.
pub fn java_to_byte_array(value_be: &[u8]) -> Vec<u8> {
    let minimal = strip_leading_zeros(value_be);
    match minimal.first() {
        None => vec![0],
        Some(&b) if b & 0x80 != 0 => {
            let mut out = Vec::with_capacity(minimal.len() + 1);
            out.push(0);
            out.extend_from_slice(minimal);
            out
        }
        Some(_) => minimal.to_vec(),
    }
}

/// Drops leading zero bytes, like `BlowFishKey.extractKeyMaterial` after the RSA decrypt.
/// Returns an empty slice when everything is zero (Java then logs "key is empty").
pub fn strip_leading_zeros(bytes: &[u8]) -> &[u8] {
    let first = bytes.iter().position(|&b| b != 0).unwrap_or(bytes.len());
    &bytes[first..]
}

/// Shared core: `block^exp mod n` with Java's no-padding input checks and a left-padded output.
fn raw_modpow(modulus_be: &[u8], exp_be: &[u8], block: &[u8]) -> Result<Vec<u8>, RsaError> {
    let n = BigUint::from_bytes_be(modulus_be);
    let exp = BigUint::from_bytes_be(exp_be);
    if n.bits() == 0 || exp.bits() == 0 {
        return Err(RsaError::InvalidKey);
    }
    if block.is_empty() {
        return Err(RsaError::EmptyBlock);
    }
    let k = n.bits().div_ceil(8);
    if block.len() > k {
        return Err(RsaError::BlockTooLong {
            len: block.len(),
            max: k,
        });
    }
    let c = BigUint::from_bytes_be(block);
    if c >= n {
        return Err(RsaError::BlockOutOfRange);
    }
    // The result is < n, so it fits in `k` bytes. `to_bytes_be` of zero is `[0]`.
    let out = c.modpow(&exp, &n).to_bytes_be();
    let mut padded = vec![0u8; k];
    let start = k.saturating_sub(out.len());
    let skip = out.len().saturating_sub(k);
    padded[start..].copy_from_slice(&out[skip..]);
    Ok(padded)
}

/// Raw public-key operation `m^e mod n` (Java `Cipher.ENCRYPT_MODE`, no padding), for the
/// proxy and fake-GS side. `modulus_be` and `exponent_be` are big-endian; a leading `0x00`
/// sign byte (from `toByteArray`) is fine.
pub fn encrypt_raw(modulus_be: &[u8], exponent_be: &[u8], block: &[u8]) -> Result<Vec<u8>, RsaError> {
    raw_modpow(modulus_be, exponent_be, block)
}

/// Raw private-key operation `c^d mod n` from bare numbers (Java `Cipher.DECRYPT_MODE`).
/// [`RsaKeyPair::decrypt_raw`] is the normal entry point; this form exists for keys that only
/// have `n` and `d`, such as the Java test vectors.
pub fn decrypt_raw_with(
    modulus_be: &[u8],
    private_exponent_be: &[u8],
    block: &[u8],
) -> Result<Vec<u8>, RsaError> {
    raw_modpow(modulus_be, private_exponent_be, block)
}

/// An RSA key pair with e = 65537, used without any padding.
pub struct RsaKeyPair {
    key: RsaPrivateKey,
}

impl RsaKeyPair {
    /// Generates a key (`RSAKeyGenParameterSpec(bits, F4)`): 1024 bits for client keys, 512 for
    /// GS links. `rng` is the workspace `rand` 0.10 RNG; seed it for reproducible keys.
    pub fn generate<R: rand::CryptoRng + ?Sized>(bits: usize, rng: &mut R) -> Result<Self, RsaError> {
        let key = RsaPrivateKey::new(&mut RngCompat(rng), bits).map_err(|_| RsaError::KeyGeneration)?;
        Ok(Self { key })
    }

    /// The modulus as minimal big-endian bytes (128 bytes for a 1024-bit key, no sign byte).
    pub fn public_modulus_be(&self) -> Vec<u8> {
        self.key.n().to_bytes_be()
    }

    /// The public exponent as minimal big-endian bytes (`01 00 01`).
    pub fn public_exponent_be(&self) -> Vec<u8> {
        self.key.e().to_bytes_be()
    }

    /// `modulus.toByteArray()` as sent by `InitLS`: 129 bytes with a leading `0x00` for a
    /// 1024-bit key, 65 bytes for a 512-bit key whose top bit is set.
    pub fn java_to_byte_array_modulus(&self) -> Vec<u8> {
        java_to_byte_array(&self.public_modulus_be())
    }

    /// The scrambled modulus for the client `Init` packet. Only defined for the 128-byte key
    /// (Java would throw for a shorter one).
    pub fn scrambled_modulus(&self) -> Result<[u8; 128], RsaError> {
        let n = self.public_modulus_be();
        let bytes: [u8; 128] = n
            .as_slice()
            .try_into()
            .map_err(|_| RsaError::ModulusSize { len: n.len() })?;
        Ok(crate::scramble::scramble_modulus(&bytes))
    }

    /// Raw `c^d mod n` of one block, left-padded to the modulus length (Java no-padding
    /// `Cipher`). For `RequestAuthLogin` pass each 128-byte block; for `BlowFishKey` follow up
    /// with [`strip_leading_zeros`].
    pub fn decrypt_raw(&self, block: &[u8]) -> Result<Vec<u8>, RsaError> {
        raw_modpow(&self.key.n().to_bytes_be(), &self.key.d().to_bytes_be(), block)
    }
}

impl std::fmt::Debug for RsaKeyPair {
    // Never print the private exponent.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RsaKeyPair")
            .field("bits", &self.key.n().bits())
            .finish_non_exhaustive()
    }
}
