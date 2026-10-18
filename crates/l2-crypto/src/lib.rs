//! L2 crypto primitives, byte-exact against the Java server (checked with `java-vectors/out/`).
//!
//! - [`blowfish`]: Blowfish with L2's little-endian word order (`L2Blowfish`).
//! - [`checksum`]: the XOR checksum `NewCrypt.appendChecksum` / `verifyChecksum`.
//! - [`login`]: the login server's client packet cipher (`LoginEncryption`), server and client side.
//! - [`xor_pass`]: the `NewCrypt.encXORPass` pass used in the login `Init` packet.
//! - [`rsa`]: raw (no padding) RSA, key generation and Java `toByteArray` conversions.
//! - [`scramble`]: the scrambled RSA modulus of the login `Init` packet (and its inverse).
//!
//! Everything here is pure and allocation-free. Malformed input (bad key or buffer length)
//! returns [`CryptoError`], never a panic: the buffers come from the network.
//!
//! Layout for later crypto cards (RSA, login encryption, game cipher): one module per primitive,
//! one shared [`CryptoError`], vector tests in `tests/vectors_*.rs`.

pub mod blowfish;
pub mod checksum;
pub mod game;
pub mod login;
pub mod rsa;
pub mod scramble;
pub mod link;
pub mod xor_pass;

mod error;

pub use self::rsa::{RsaError, RsaKeyPair};
pub use blowfish::L2Blowfish;
pub use error::CryptoError;
pub use game::{GameCipher, game_key_from_random};
pub use login::{LoginCipher, LoginClientCipher};
pub use link::LinkCipher;
