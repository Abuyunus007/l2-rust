//! The key pools: RSA pairs for clients (1024 bit) and game server links (512 bit), and the
//! Blowfish keys handed to clients.
// Java: org/l2jmobius/loginserver/controller/LoginController.java (constructor, `generateBlowFishKeys`)
// Java: org/l2jmobius/loginserver/controller/ScrambledKeyPair.java
// Java: org/l2jmobius/loginserver/data/GameServerTable.java (`initRSAKeys`)
//
// All pools are created once at startup and only read afterwards, so they need no locking.
// Generating an RSA key takes long, so [`generate_client_keys`] and friends run on a blocking
// thread and keep the runtime responsive.

use l2_crypto::{RsaError, RsaKeyPair};
use rand::{CryptoRng, Rng};
use tracing::info;

/// Number of RSA-1024 pairs for clients (`new ScrambledKeyPair[10]`).
pub const CLIENT_KEY_PAIRS: usize = 10;
/// RSA modulus size for clients.
pub const CLIENT_RSA_BITS: usize = 1024;
/// Number of Blowfish keys (`BLOWFISH_KEYS`).
pub const BLOWFISH_KEYS: usize = 20;
/// Length of one client Blowfish key.
pub const BLOWFISH_KEY_LEN: usize = 16;
/// Number of RSA-512 pairs for game server links (`KEYS_SIZE`).
pub const GAME_SERVER_KEY_PAIRS: usize = 10;
/// RSA modulus size for game server links.
pub const GAME_SERVER_RSA_BITS: usize = 512;

#[derive(Debug, thiserror::Error)]
pub enum KeyError {
    #[error("RSA key generation failed: {0}")]
    Rsa(#[from] RsaError),
    #[error("key generation thread failed: {0}")]
    Thread(String),
}

/// A client RSA pair with its scrambled modulus, precomputed because the `Init` packet of every
/// connection carries it.
pub struct ScrambledKeyPair {
    pub key_pair: RsaKeyPair,
    pub scrambled_modulus: [u8; 128],
}

impl ScrambledKeyPair {
    /// A fresh RSA-1024 pair.
    pub fn generate<R: CryptoRng + ?Sized>(rng: &mut R) -> Result<Self, KeyError> {
        let key_pair = RsaKeyPair::generate(CLIENT_RSA_BITS, rng)?;
        let scrambled_modulus = key_pair.scrambled_modulus()?;
        Ok(Self {
            key_pair,
            scrambled_modulus,
        })
    }
}

/// Everything a client connection draws from: RSA pairs and Blowfish keys.
pub struct ClientKeys {
    pub pairs: Vec<ScrambledKeyPair>,
    pub blowfish: Vec<[u8; BLOWFISH_KEY_LEN]>,
}

/// RSA-512 pairs for the game server links.
pub struct GameServerKeys {
    pub pairs: Vec<RsaKeyPair>,
}

impl ClientKeys {
    /// `pairs` RSA-1024 pairs and `blowfish` random 16-byte keys.
    pub fn generate<R: CryptoRng + Rng + ?Sized>(
        rng: &mut R,
        pairs: usize,
        blowfish: usize,
    ) -> Result<Self, KeyError> {
        let pairs = (0..pairs)
            .map(|_| ScrambledKeyPair::generate(rng))
            .collect::<Result<Vec<_>, _>>()?;
        let blowfish = (0..blowfish)
            .map(|_| {
                let mut key = [0u8; BLOWFISH_KEY_LEN];
                rng.fill_bytes(&mut key);
                key
            })
            .collect();
        Ok(Self { pairs, blowfish })
    }
}

impl GameServerKeys {
    /// `count` RSA-512 pairs.
    pub fn generate<R: CryptoRng + ?Sized>(rng: &mut R, count: usize) -> Result<Self, KeyError> {
        let pairs = (0..count)
            .map(|_| RsaKeyPair::generate(GAME_SERVER_RSA_BITS, rng).map_err(KeyError::from))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { pairs })
    }
}

/// Generates the client pools on a blocking thread and logs like `LoginController`'s constructor.
pub async fn generate_client_keys() -> Result<ClientKeys, KeyError> {
    info!("Loading LoginController...");
    let keys = tokio::task::spawn_blocking(|| {
        ClientKeys::generate(&mut rand::rng(), CLIENT_KEY_PAIRS, BLOWFISH_KEYS)
    })
    .await
    .map_err(|error| KeyError::Thread(error.to_string()))??;
    info!("Cached {} KeyPairs for RSA communication.", keys.pairs.len());
    info!("Stored {} keys for Blowfish communication.", keys.blowfish.len());
    Ok(keys)
}

/// Generates the game server link pairs on a blocking thread.
pub async fn generate_game_server_keys() -> Result<GameServerKeys, KeyError> {
    let keys =
        tokio::task::spawn_blocking(|| GameServerKeys::generate(&mut rand::rng(), GAME_SERVER_KEY_PAIRS))
            .await
            .map_err(|error| KeyError::Thread(error.to_string()))??;
    info!(
        "Cached {} RSA keys for Game Server communication.",
        keys.pairs.len()
    );
    Ok(keys)
}

#[cfg(test)]
mod tests {
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    use super::*;

    #[test]
    fn client_pool_has_scrambled_moduli_that_match_the_keys() {
        let mut rng = ChaCha8Rng::seed_from_u64(1);
        let keys = ClientKeys::generate(&mut rng, 2, 3).unwrap();
        assert_eq!(keys.pairs.len(), 2);
        assert_eq!(keys.blowfish.len(), 3);
        for pair in &keys.pairs {
            assert_eq!(pair.key_pair.public_modulus_be().len(), 128);
            assert_eq!(pair.key_pair.public_exponent_be(), [1, 0, 1]);
            assert_eq!(pair.scrambled_modulus, pair.key_pair.scrambled_modulus().unwrap());
            assert_ne!(
                pair.scrambled_modulus.as_slice(),
                pair.key_pair.public_modulus_be()
            );
        }
        assert_ne!(
            keys.pairs[0].key_pair.public_modulus_be(),
            keys.pairs[1].key_pair.public_modulus_be()
        );
        assert_ne!(keys.blowfish[0], keys.blowfish[1]);
    }

    #[test]
    fn game_server_pairs_are_512_bit() {
        let mut rng = ChaCha8Rng::seed_from_u64(2);
        let keys = GameServerKeys::generate(&mut rng, 2).unwrap();
        assert_eq!(keys.pairs.len(), 2);
        for pair in &keys.pairs {
            assert_eq!(pair.public_modulus_be().len(), 64);
        }
    }

    #[test]
    fn same_seed_gives_the_same_keys() {
        let a = ClientKeys::generate(&mut ChaCha8Rng::seed_from_u64(9), 1, 2).unwrap();
        let b = ClientKeys::generate(&mut ChaCha8Rng::seed_from_u64(9), 1, 2).unwrap();
        assert_eq!(a.blowfish, b.blowfish);
        assert_eq!(a.pairs[0].scrambled_modulus, b.pairs[0].scrambled_modulus);
    }

    #[tokio::test]
    async fn pool_sizes_match_java() {
        let client = generate_client_keys().await.unwrap();
        assert_eq!(client.pairs.len(), 10);
        assert_eq!(client.blowfish.len(), 20);
        let gs = generate_game_server_keys().await.unwrap();
        assert_eq!(gs.pairs.len(), 10);
    }
}
