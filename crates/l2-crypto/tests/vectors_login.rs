//! Java-vector tests for the login client cipher (`crypto_login_encryption.json`), plus
//! server<->client round trips.
//!
//! The first-packet vectors are a replay: Java draws the XOR key from `Rnd`, so the generator
//! recovered it from the output and the vector lists it as `xor_key`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use l2_crypto::login::{decrypt_init, encrypted_size};
use l2_crypto::{CryptoError, LoginCipher, LoginClientCipher};
use l2_testkit::{assert_bytes_eq, from_hex, vectors::load};
use serde::Deserialize;

#[derive(Deserialize)]
struct LoginFile {
    session_key: String,
    first_packet: Vec<FirstCase>,
    session: Vec<SessionCase>,
}

#[derive(Deserialize)]
struct FirstCase {
    payload: String,
    xor_key: i32,
    output: String,
}

#[derive(Deserialize)]
struct SessionCase {
    payload: String,
    encrypted: String,
    decrypted: String,
    decrypt_valid: bool,
    corrupted: String,
    corrupted_valid: bool,
}

fn key16(hex: &str) -> [u8; 16] {
    from_hex(hex).try_into().unwrap()
}

#[test]
fn covers_payload_sizes_1_to_40() {
    let file: LoginFile = load("crypto_login_encryption");
    let sizes: Vec<usize> = file
        .first_packet
        .iter()
        .map(|c| from_hex(&c.payload).len())
        .collect();
    assert_eq!(sizes, (1..=40).collect::<Vec<_>>());
    assert_eq!(file.session.len(), 40);
}

#[test]
fn first_packet_matches_java() {
    let file: LoginFile = load("crypto_login_encryption");
    for c in &file.first_packet {
        let payload = from_hex(&c.payload);
        let expected = from_hex(&c.output);
        // A fresh cipher per case: the first call is the static one.
        let mut cipher = LoginCipher::new(&key16(&file.session_key));
        assert_eq!(cipher.next_encrypted_size(payload.len()), expected.len());
        let out = cipher.encrypt(&payload, c.xor_key);
        assert_eq!(out.len(), encrypted_size(payload.len(), true));
        assert_bytes_eq(&out, &expected);
    }
}

#[test]
fn session_packets_match_java() {
    let file: LoginFile = load("crypto_login_encryption");
    for c in &file.session {
        let payload = from_hex(&c.payload);
        let expected = from_hex(&c.encrypted);
        let mut cipher = LoginCipher::new(&key16(&file.session_key));
        // Spend the first (static) call, like the generator.
        cipher.encrypt(&[0], 0);
        assert_eq!(cipher.next_encrypted_size(payload.len()), expected.len());
        // The xor_key is ignored from the second call on.
        let out = cipher.encrypt(&payload, 0x1234_5678);
        assert_eq!(out.len(), encrypted_size(payload.len(), false));
        assert_bytes_eq(&out, &expected);
    }
}

#[test]
fn server_decrypt_matches_java() {
    let file: LoginFile = load("crypto_login_encryption");
    let cipher = LoginCipher::new(&key16(&file.session_key));
    for c in &file.session {
        assert!(c.decrypt_valid && !c.corrupted_valid);

        let mut body = from_hex(&c.encrypted);
        cipher.decrypt(&mut body).unwrap();
        assert_bytes_eq(&body, &from_hex(&c.decrypted));

        // Corrupted: rejected, and the buffer is left untouched like Java.
        let mut body = from_hex(&c.corrupted);
        let before = body.clone();
        assert_eq!(cipher.decrypt(&mut body), Err(CryptoError::ChecksumMismatch));
        assert_eq!(body, before);
    }
}

#[test]
fn client_decrypts_java_first_packet() {
    let file: LoginFile = load("crypto_login_encryption");
    for c in &file.first_packet {
        let payload = from_hex(&c.payload);
        let mut body = from_hex(&c.output);

        let mut client = LoginClientCipher::new(&key16(&file.session_key));
        client.decrypt(&mut body).unwrap();

        let len = body.len();
        assert_bytes_eq(&body[..payload.len()], &payload);
        assert!(body[payload.len()..len - 8].iter().all(|b| *b == 0));
        // The key slot holds the final rolling key, the last 4 bytes are zero.
        assert_eq!(body[len - 4..], [0, 0, 0, 0]);

        // The start key of the pass is recoverable and equals the replayed Rnd value.
        let mut again = from_hex(&c.output);
        assert_eq!(decrypt_init(&mut again).unwrap(), c.xor_key);
        assert_eq!(again, body);
    }
}

#[test]
fn client_decrypts_java_session_packets() {
    let file: LoginFile = load("crypto_login_encryption");
    let mut client = LoginClientCipher::after_init(&key16(&file.session_key));
    for c in &file.session {
        let mut body = from_hex(&c.encrypted);
        client.decrypt(&mut body).unwrap();
        assert_bytes_eq(&body, &from_hex(&c.decrypted));
        let mut bad = from_hex(&c.corrupted);
        assert_eq!(client.decrypt(&mut bad), Err(CryptoError::ChecksumMismatch));
    }
}

#[test]
fn full_conversation_round_trip() {
    let file: LoginFile = load("crypto_login_encryption");
    let key = key16(&file.session_key);
    for len in 1..=40usize {
        let payload: Vec<u8> = (0..len)
            .map(|i| (i as u8).wrapping_mul(31).wrapping_add(3))
            .collect();
        let mut server = LoginCipher::new(&key);
        let mut client = LoginClientCipher::new(&key);

        // Init (static, XOR pass), then a normal server packet, then a client packet.
        let mut init = server.encrypt(&payload, -77_777 * len as i32);
        client.decrypt(&mut init).unwrap();
        assert_bytes_eq(&init[..len], &payload);

        let mut s2c = server.encrypt(&payload, 0);
        client.decrypt(&mut s2c).unwrap();
        assert_bytes_eq(&s2c[..len], &payload);

        let mut c2s = client.encrypt(&payload);
        server.decrypt(&mut c2s).unwrap();
        assert_bytes_eq(&c2s[..len], &payload);
    }
}
