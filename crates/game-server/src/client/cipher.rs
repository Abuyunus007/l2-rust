//! The game traffic cipher as an `l2-net` frame cipher.
// Java: org/l2jmobius/gameserver/network/GameClient.java (`encrypt`, `decrypt`, `enableCrypt`)
// Java: org/l2jmobius/gameserver/network/Encryption.java (the cipher itself, in l2-crypto)

use l2_crypto::GameCipher;
use l2_net::{CipherError, FrameCipher};

/// `GameCipher` for one connection. It is installed when `KeyPacket` is sent: the cipher's first
/// `encrypt` only switches it on, so `KeyPacket` itself still goes out in clear and every packet
/// after it (both directions) is encrypted, exactly like Java's `Encryption._isEnabled`.
pub struct GameFrameCipher {
    inner: GameCipher,
}

impl GameFrameCipher {
    pub fn new(key: [u8; l2_crypto::game::KEY_LEN]) -> Self {
        Self {
            inner: GameCipher::new(key),
        }
    }
}

impl FrameCipher for GameFrameCipher {
    fn encrypt(&mut self, mut payload: Vec<u8>) -> Vec<u8> {
        self.inner.encrypt(&mut payload);
        payload
    }

    fn decrypt(&mut self, body: &mut Vec<u8>) -> Result<(), CipherError> {
        self.inner.decrypt(body);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_packet_goes_out_in_clear_then_traffic_is_encrypted() {
        let key = l2_crypto::game_key_from_random([1, 2, 3, 4, 5, 6, 7, 8]);
        let mut server = GameFrameCipher::new(key);
        // Before the first encrypt nothing is decrypted (ProtocolVersion arrives in clear).
        let mut early = vec![9, 9, 9];
        server.decrypt(&mut early).unwrap();
        assert_eq!(early, [9, 9, 9]);
        // KeyPacket: in clear.
        assert_eq!(server.encrypt(vec![0x2E, 1, 2]), [0x2E, 1, 2]);
        // From here on both directions are encrypted with the same key as the client's.
        let mut client = GameCipher::new(key);
        client.encrypt(&mut []); // the client's copy is switched on by the key packet
        let sent = server.encrypt(vec![0x09, 0x10, 0x11]);
        assert_ne!(sent, [0x09, 0x10, 0x11]);
        let mut received = sent;
        client.decrypt(&mut received);
        assert_eq!(received, [0x09, 0x10, 0x11]);
        let mut from_client = vec![0x2B, 0x00, 0x01];
        client.encrypt(&mut from_client);
        server.decrypt(&mut from_client).unwrap();
        assert_eq!(from_client, [0x2B, 0x00, 0x01]);
    }
}
