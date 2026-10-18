//! Java-vector tests for the game cipher and key generator (`crypto_game_xor.json`).
//!
//! Each sequence runs its packets in order on one instance, so the key evolves exactly as in Java.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use l2_crypto::{GameCipher, game_key_from_random};
use l2_testkit::{assert_bytes_eq, from_hex, vectors::load};
use serde::Deserialize;

#[derive(Deserialize)]
struct GameXorFile {
    sequences: Vec<Sequence>,
    blowfish_keygen: KeygenVector,
}

#[derive(Deserialize)]
struct Sequence {
    what: String,
    key: String,
    encrypt_calls: Vec<Call>,
    decrypt_calls: Vec<Call>,
}

#[derive(Deserialize)]
struct Call {
    enabled_before: bool,
    input: String,
    output: String,
    // Present in the file for key-state checks; the public API does not expose the key, so the
    // output bytes (which depend on the offset) are what this test compares.
    #[allow(dead_code)]
    key_after: String,
}

#[derive(Deserialize)]
struct KeygenVector {
    length: usize,
    fixed_tail: String,
}

fn key16(hex: &str) -> [u8; 16] {
    from_hex(hex).try_into().expect("vector key is 16 bytes")
}

#[test]
fn encrypt_matches_java() {
    let file: GameXorFile = load("crypto_game_xor");
    assert!(!file.sequences.is_empty());
    for seq in &file.sequences {
        let mut cipher = GameCipher::new(key16(&seq.key));
        assert!(!seq.encrypt_calls.is_empty(), "{}", seq.what);
        for call in &seq.encrypt_calls {
            let mut buf = from_hex(&call.input);
            cipher.encrypt(&mut buf);
            assert_bytes_eq(&buf, &from_hex(&call.output));
        }
    }
}

#[test]
fn decrypt_matches_java() {
    let file: GameXorFile = load("crypto_game_xor");
    assert!(!file.sequences.is_empty());
    for seq in &file.sequences {
        let mut cipher = GameCipher::new(key16(&seq.key));
        assert!(!seq.decrypt_calls.is_empty(), "{}", seq.what);
        for (i, call) in seq.decrypt_calls.iter().enumerate() {
            if i == 1 {
                // The Java generator enabled the decrypt instance with a 1-byte encrypt call
                // before the second decrypt call. Enabling is shared, so this enables decrypt.
                cipher.encrypt(&mut [0u8]);
            }
            assert_eq!(call.enabled_before, i > 0, "{}: decrypt call {i}", seq.what);
            let mut buf = from_hex(&call.input);
            cipher.decrypt(&mut buf);
            assert_bytes_eq(&buf, &from_hex(&call.output));
        }
    }
}

#[test]
fn blowfish_keygen_matches_java() {
    let file: GameXorFile = load("crypto_game_xor");
    let random = [1u8, 2, 3, 4, 5, 6, 7, 8];
    let key = game_key_from_random(random);
    assert_eq!(key.len(), file.blowfish_keygen.length);
    assert_bytes_eq(&key[..8], &random);
    assert_bytes_eq(&key[8..], &from_hex(&file.blowfish_keygen.fixed_tail));
}

#[test]
fn client_decrypts_what_server_encrypts() {
    let key = game_key_from_random([0x5a, 0x13, 0xc7, 0x80, 0x2e, 0xf1, 0x09, 0x6b]);
    let mut server = GameCipher::new(key);
    let mut client = GameCipher::new(key);

    // The first server packet only enables the cipher and goes out in clear. The client side
    // is enabled the same way, as the Java generator does.
    let mut hello = [0x0eu8, 0x00, 0x21, 0x01];
    server.encrypt(&mut hello);
    assert_eq!(hello, [0x0e, 0x00, 0x21, 0x01]);
    client.encrypt(&mut [0u8]);

    let sizes = [1usize, 5, 8, 16, 17, 33, 0, 64, 9, 100];
    for (n, size) in sizes.into_iter().enumerate() {
        let plain: Vec<u8> = (0..size).map(|i| (i * 31 + n * 7 + 3) as u8).collect();
        let mut wire = plain.clone();
        server.encrypt(&mut wire);
        if size > 0 {
            assert_ne!(wire, plain, "packet {n} was not encrypted");
        }
        client.decrypt(&mut wire);
        assert_bytes_eq(&wire, &plain);
    }
}
