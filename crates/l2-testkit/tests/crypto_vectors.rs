//! Shape checks for the committed `java-vectors/out/crypto_*.json` files (M1-JV-01).
//!
//! These tests only prove that each file parses into the expected structure and that every byte
//! field is lowercase hex. The crypto crates (M1-CRYPTO-*) check the actual values.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use l2_testkit::vectors::load;
use serde::Deserialize;
use serde_json::Value;

/// A hex string that must be lowercase and of even length.
fn assert_hex(s: &str) {
    assert!(
        s.len() % 2 == 0
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "not lowercase hex: {s:?}"
    );
}

#[derive(Deserialize)]
struct Header {
    source: String,
    generated_by: String,
}

fn check_header(name: &str) {
    let h: Header = load(name);
    assert_eq!(h.generated_by, "java-vectors", "{name}");
    assert!(!h.source.is_empty(), "{name}");
}

/// Every string value reachable under a key that holds bytes must be hex.
fn assert_hex_fields(value: &Value, hex_keys: &[&str]) {
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                if hex_keys.contains(&k.as_str()) {
                    if let Value::String(s) = v {
                        assert_hex(s);
                    } else {
                        assert!(v.is_null() || v.is_boolean() || v.is_number(), "{k}: {v}");
                    }
                }
                assert_hex_fields(v, hex_keys);
            }
        }
        Value::Array(items) => items.iter().for_each(|i| assert_hex_fields(i, hex_keys)),
        _ => {}
    }
}

#[derive(Deserialize)]
struct InOut {
    input: String,
    output: String,
}

#[derive(Deserialize)]
struct BlowfishCase {
    key: String,
    encrypt_block: Vec<InOut>,
    decrypt_block: Vec<InOut>,
}

#[derive(Deserialize)]
struct BlowfishFile {
    cases: Vec<BlowfishCase>,
}

#[test]
fn crypto_blowfish_shape() {
    check_header("crypto_blowfish");
    let f: BlowfishFile = load("crypto_blowfish");
    let key_lens: Vec<usize> = f.cases.iter().map(|c| c.key.len() / 2).collect();
    assert_eq!(key_lens, [16, 16, 22, 40]);
    for c in &f.cases {
        assert_hex(&c.key);
        assert!(!c.encrypt_block.is_empty() && !c.decrypt_block.is_empty());
        for b in c.encrypt_block.iter().chain(&c.decrypt_block) {
            assert_eq!(b.input.len(), 16);
            assert_eq!(b.output.len(), 16);
            assert_hex(&b.input);
            assert_hex(&b.output);
        }
    }
}

#[derive(Deserialize)]
struct XorCase {
    xor_key: i32,
    input: String,
    output: String,
}

#[derive(Deserialize)]
struct VerifyCase {
    data: String,
    #[allow(dead_code)]
    valid: bool,
}

#[derive(Deserialize)]
struct NewCryptBlowfish {
    key: String,
    crypt: Vec<InOut>,
    decrypt: Vec<InOut>,
}

#[derive(Deserialize)]
struct NewCryptFile {
    append_checksum: Vec<InOut>,
    verify_checksum: Vec<VerifyCase>,
    xor_pass: Vec<XorCase>,
    blowfish: Vec<NewCryptBlowfish>,
}

#[test]
fn crypto_newcrypt_shape() {
    check_header("crypto_newcrypt");
    let f: NewCryptFile = load("crypto_newcrypt");
    assert!(!f.append_checksum.is_empty());
    assert!(f.verify_checksum.iter().any(|c| c.valid));
    assert!(f.verify_checksum.iter().any(|c| !c.valid));
    assert!(!f.xor_pass.is_empty());
    assert!(!f.blowfish.is_empty());
    for c in &f.append_checksum {
        assert_eq!(c.input.len(), c.output.len());
        assert_hex(&c.input);
        assert_hex(&c.output);
    }
    for c in &f.verify_checksum {
        assert_hex(&c.data);
    }
    for c in &f.xor_pass {
        let _ = c.xor_key;
        assert_eq!(c.input.len(), c.output.len());
        assert_hex(&c.input);
        assert_hex(&c.output);
    }
    for c in &f.blowfish {
        assert_hex(&c.key);
        for b in c.crypt.iter().chain(&c.decrypt) {
            assert_eq!(b.input.len(), b.output.len());
            assert_eq!(b.input.len() % 16, 0);
            assert_hex(&b.input);
            assert_hex(&b.output);
        }
    }
}

#[derive(Deserialize)]
struct FirstPacket {
    payload: String,
    xor_key: i32,
    output: String,
}

#[derive(Deserialize)]
struct SessionPacket {
    payload: String,
    encrypted: String,
    decrypted: String,
    decrypt_valid: bool,
    corrupted: String,
    corrupted_valid: bool,
}

#[derive(Deserialize)]
struct LoginEncryptionFile {
    static_key: String,
    session_key: String,
    first_packet: Vec<FirstPacket>,
    session: Vec<SessionPacket>,
}

#[test]
fn crypto_login_encryption_shape() {
    check_header("crypto_login_encryption");
    let f: LoginEncryptionFile = load("crypto_login_encryption");
    assert_eq!(f.static_key.len(), 32);
    assert_eq!(f.session_key.len(), 32);
    assert_eq!(f.first_packet.len(), 40);
    assert_eq!(f.session.len(), 40);
    for (i, p) in f.first_packet.iter().enumerate() {
        let _ = p.xor_key;
        assert_eq!(p.payload.len() / 2, i + 1);
        assert_eq!(p.output.len() % 16, 0);
        assert_hex(&p.output);
    }
    for (i, p) in f.session.iter().enumerate() {
        assert_eq!(p.payload.len() / 2, i + 1);
        assert_eq!(p.encrypted.len(), p.decrypted.len());
        assert_eq!(p.encrypted.len(), p.corrupted.len());
        assert!(p.decrypt_valid && !p.corrupted_valid);
        assert_hex(&p.encrypted);
        assert_hex(&p.decrypted);
        assert_hex(&p.corrupted);
    }
}

#[derive(Deserialize)]
struct ScrambleCase {
    modulus: String,
    to_byte_array_length: usize,
    scrambled: String,
}

#[derive(Deserialize)]
struct ScrambleFile {
    cases: Vec<ScrambleCase>,
}

#[test]
fn crypto_scrambled_modulus_shape() {
    check_header("crypto_scrambled_modulus");
    let f: ScrambleFile = load("crypto_scrambled_modulus");
    assert!(f.cases.iter().any(|c| c.to_byte_array_length == 129));
    assert!(f.cases.iter().any(|c| c.to_byte_array_length == 128));
    for c in &f.cases {
        assert_hex(&c.modulus);
        assert_eq!(c.scrambled.len(), 256, "scrambled modulus is 128 bytes");
        assert_hex(&c.scrambled);
    }
}

#[derive(Deserialize)]
struct RsaCase {
    plain: Option<String>,
    encrypted_with_public: String,
    decrypted_with_private: String,
}

#[derive(Deserialize)]
struct RsaKey {
    n: String,
    e: String,
    d: String,
    cases: Vec<RsaCase>,
}

#[derive(Deserialize)]
struct RsaFile {
    rsa_1024: RsaKey,
    rsa_512: RsaKey,
}

#[test]
fn crypto_rsa_nopadding_shape() {
    check_header("crypto_rsa_nopadding");
    let f: RsaFile = load("crypto_rsa_nopadding");
    for (key, size) in [(&f.rsa_1024, 128), (&f.rsa_512, 64)] {
        assert_eq!(key.n.len(), size * 2);
        assert_eq!(key.e, "010001");
        assert_hex(&key.n);
        assert_hex(&key.d);
        assert!(!key.cases.is_empty());
        for c in &key.cases {
            if let Some(p) = &c.plain {
                assert_hex(p);
            }
            assert_eq!(c.encrypted_with_public.len(), size * 2);
            assert_eq!(c.decrypted_with_private.len(), size * 2);
            assert_hex(&c.encrypted_with_public);
            assert_hex(&c.decrypted_with_private);
        }
    }
}

#[derive(Deserialize)]
struct GameCall {
    enabled_before: bool,
    input: String,
    output: String,
    key_after: String,
}

#[derive(Deserialize)]
struct GameSequence {
    key: String,
    encrypt_calls: Vec<GameCall>,
    decrypt_calls: Vec<GameCall>,
}

#[derive(Deserialize)]
struct Keygen {
    length: usize,
    fixed_tail: String,
}

#[derive(Deserialize)]
struct GameXorFile {
    sequences: Vec<GameSequence>,
    blowfish_keygen: Keygen,
}

#[test]
fn crypto_game_xor_shape() {
    check_header("crypto_game_xor");
    let f: GameXorFile = load("crypto_game_xor");
    assert!(f.sequences.len() >= 2);
    for s in &f.sequences {
        assert_eq!(s.key.len(), 32);
        // 1 enabling call + at least 5 packets; 1 not-yet-enabled decrypt + the same packets.
        assert!(s.encrypt_calls.len() >= 6);
        assert_eq!(s.decrypt_calls.len(), s.encrypt_calls.len());
        assert!(!s.encrypt_calls[0].enabled_before);
        assert!(!s.decrypt_calls[0].enabled_before);
        for c in s.encrypt_calls.iter().chain(&s.decrypt_calls) {
            assert_eq!(c.input.len(), c.output.len());
            assert_eq!(c.key_after.len(), 32);
            assert_hex(&c.input);
            assert_hex(&c.output);
            assert_hex(&c.key_after);
        }
        assert!(s.encrypt_calls[1..].iter().all(|c| c.enabled_before));
    }
    assert_eq!(f.blowfish_keygen.length, 16);
    assert_eq!(f.blowfish_keygen.fixed_tail, "c8279301a16c3197");
}

#[test]
fn all_byte_fields_are_lowercase_hex() {
    let hex_keys = [
        "key",
        "input",
        "output",
        "data",
        "payload",
        "static_key",
        "session_key",
        "encrypted",
        "decrypted",
        "corrupted",
        "modulus",
        "scrambled",
        "n",
        "e",
        "d",
        "plain",
        "encrypted_with_public",
        "decrypted_with_private",
        "key_after",
        "fixed_tail",
    ];
    for name in [
        "crypto_blowfish",
        "crypto_newcrypt",
        "crypto_login_encryption",
        "crypto_scrambled_modulus",
        "crypto_rsa_nopadding",
        "crypto_game_xor",
    ] {
        let v: Value = load(name);
        assert_hex_fields(&v, &hex_keys);
    }
}
