//! Java-vector tests for `L2Blowfish`, `checksum` and `xor_pass`
//! (`crypto_blowfish.json`, `crypto_newcrypt.json`), plus property tests.
//!
//! Pattern for crypto vector tests: one `#[derive(Deserialize)]` struct per JSON shape, one test
//! per vector group, `assert_bytes_eq` for byte results.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use l2_crypto::{CryptoError, L2Blowfish, checksum, xor_pass};
use l2_testkit::{assert_bytes_eq, from_hex, vectors::load};
use proptest::prelude::*;
use serde::Deserialize;

#[derive(Deserialize)]
struct Pair {
    input: String,
    output: String,
}

// ---- crypto_blowfish.json --------------------------------------------------------------

#[derive(Deserialize)]
struct BlowfishFile {
    cases: Vec<BlowfishCase>,
}

#[derive(Deserialize)]
struct BlowfishCase {
    key: String,
    encrypt_block: Vec<Pair>,
    decrypt_block: Vec<Pair>,
}

#[test]
fn blowfish_blocks_match_java() {
    let file: BlowfishFile = load("crypto_blowfish");
    assert!(!file.cases.is_empty());
    for case in &file.cases {
        let bf = L2Blowfish::new(&from_hex(&case.key)).unwrap();
        assert!(!case.encrypt_block.is_empty() && !case.decrypt_block.is_empty());
        for p in &case.encrypt_block {
            let mut buf = from_hex(&p.input);
            bf.encrypt_in_place(&mut buf).unwrap();
            assert_bytes_eq(&buf, &from_hex(&p.output));
        }
        for p in &case.decrypt_block {
            let mut buf = from_hex(&p.input);
            bf.decrypt_in_place(&mut buf).unwrap();
            assert_bytes_eq(&buf, &from_hex(&p.output));
        }
    }
}

// ---- crypto_newcrypt.json --------------------------------------------------------------

#[derive(Deserialize)]
struct NewCryptFile {
    append_checksum: Vec<Pair>,
    verify_checksum: Vec<VerifyCase>,
    xor_pass: Vec<XorCase>,
    blowfish: Vec<NewCryptBlowfish>,
}

#[derive(Deserialize)]
struct VerifyCase {
    data: String,
    valid: bool,
}

#[derive(Deserialize)]
struct XorCase {
    xor_key: i32,
    input: String,
    output: String,
}

#[derive(Deserialize)]
struct NewCryptBlowfish {
    key: String,
    crypt: Vec<Pair>,
    decrypt: Vec<Pair>,
}

#[test]
fn append_checksum_matches_java() {
    let file: NewCryptFile = load("crypto_newcrypt");
    assert!(!file.append_checksum.is_empty());
    for p in &file.append_checksum {
        let mut buf = from_hex(&p.input);
        checksum::append(&mut buf).unwrap();
        assert_bytes_eq(&buf, &from_hex(&p.output));
        assert!(buf.len() == 4 || checksum::verify(&buf));
    }
}

#[test]
fn verify_checksum_matches_java() {
    let file: NewCryptFile = load("crypto_newcrypt");
    assert!(!file.verify_checksum.is_empty());
    for c in &file.verify_checksum {
        assert_eq!(checksum::verify(&from_hex(&c.data)), c.valid, "data {}", c.data);
    }
}

#[test]
fn xor_pass_matches_java() {
    let file: NewCryptFile = load("crypto_newcrypt");
    assert!(!file.xor_pass.is_empty());
    for c in &file.xor_pass {
        let mut buf = from_hex(&c.input);
        xor_pass::encrypt(&mut buf, c.xor_key).unwrap();
        assert_bytes_eq(&buf, &from_hex(&c.output));
    }
}

#[test]
fn newcrypt_blowfish_matches_java() {
    let file: NewCryptFile = load("crypto_newcrypt");
    assert!(!file.blowfish.is_empty());
    for case in &file.blowfish {
        let bf = L2Blowfish::new(&from_hex(&case.key)).unwrap();
        assert!(!case.crypt.is_empty() && !case.decrypt.is_empty());
        for p in &case.crypt {
            let mut buf = from_hex(&p.input);
            bf.encrypt_in_place(&mut buf).unwrap();
            assert_bytes_eq(&buf, &from_hex(&p.output));
        }
        for p in &case.decrypt {
            let mut buf = from_hex(&p.input);
            bf.decrypt_in_place(&mut buf).unwrap();
            assert_bytes_eq(&buf, &from_hex(&p.output));
        }
    }
}

// ---- Java's (buf, offset, size) checksum call ------------------------------------------

/// Literal port of `NewCrypt.appendChecksum(byte[], int offset, int size)`, quirk included:
/// `dataLength = size - 4` without adding `offset`.
fn java_append_checksum(raw: &mut [u8], offset: usize, size: usize) {
    let mut calculated: u32 = 0;
    let data_length = size - 4;
    let mut position = offset;
    while position < data_length {
        calculated ^= u32::from_le_bytes(raw[position..position + 4].try_into().unwrap());
        position += 4;
    }
    raw[position..position + 4].copy_from_slice(&calculated.to_le_bytes());
}

/// Literal port of `NewCrypt.verifyChecksum(byte[], int offset, int size)`.
fn java_verify_checksum(raw: &[u8], offset: usize, size: usize) -> bool {
    if (size & 3) != 0 || size <= 4 {
        return false;
    }
    let mut calculated: u32 = 0;
    let data_length = size - 4;
    let mut position = offset;
    while position < data_length {
        calculated ^= u32::from_le_bytes(raw[position..position + 4].try_into().unwrap());
        position += 4;
    }
    u32::from_le_bytes(raw[position..position + 4].try_into().unwrap()) == calculated
}

#[test]
fn matches_java_offset_call_on_length_prefixed_packet() {
    // `[2-byte header][body]` as in GameServerThread.sendPacket: Java calls
    // appendChecksum(data, 2, len - 2); the body is a multiple of 8 bytes incl. the 4-byte slot.
    for body_len in [8usize, 16, 24, 40, 64] {
        let mut data = vec![0u8; 2 + body_len];
        data[0] = (data.len() & 0xff) as u8;
        data[1] = (data.len() >> 8) as u8;
        for (i, b) in data[2..body_len - 2].iter_mut().enumerate() {
            *b = (i as u8).wrapping_mul(37).wrapping_add(11);
        }

        let mut java = data.clone();
        let size = java.len() - 2;
        java_append_checksum(&mut java, 2, size);

        let mut rust = data.clone();
        checksum::append(&mut rust[2..]).unwrap();

        assert_bytes_eq(&rust, &java);
        assert!(java_verify_checksum(&java, 2, java.len() - 2));
        assert!(checksum::verify(&rust[2..]));
    }
}

// ---- errors and properties -------------------------------------------------------------

#[test]
fn invalid_inputs_return_errors() {
    assert!(L2Blowfish::new(&[1, 2, 3]).is_err());
    let bf = L2Blowfish::new(b"abcd").unwrap();
    assert_eq!(
        bf.encrypt_in_place(&mut [0u8; 9]),
        Err(CryptoError::NotBlockAligned { len: 9 })
    );
    assert!(checksum::append(&mut [0u8; 5]).is_err());
    assert!(xor_pass::encrypt(&mut [0u8; 4], 0).is_err());
}

proptest! {
    #[test]
    fn blowfish_round_trip(
        key in proptest::collection::vec(any::<u8>(), 4..=56),
        blocks in proptest::collection::vec(any::<[u8; 8]>(), 0..16),
    ) {
        let plain: Vec<u8> = blocks.concat();
        let bf = L2Blowfish::new(&key).unwrap();
        let mut buf = plain.clone();
        bf.encrypt_in_place(&mut buf).unwrap();
        if !plain.is_empty() {
            prop_assert_ne!(&buf, &plain);
        }
        bf.decrypt_in_place(&mut buf).unwrap();
        prop_assert_eq!(buf, plain);
    }

    #[test]
    fn blowfish_never_panics_on_any_length(
        key in proptest::collection::vec(any::<u8>(), 0..64),
        mut buf in proptest::collection::vec(any::<u8>(), 0..100),
    ) {
        if let Ok(bf) = L2Blowfish::new(&key) {
            let aligned = buf.len() % 8 == 0;
            prop_assert_eq!(bf.encrypt_in_place(&mut buf).is_ok(), aligned);
            prop_assert_eq!(bf.decrypt_in_place(&mut buf).is_ok(), aligned);
        }
    }

    #[test]
    fn checksum_append_then_verify(mut buf in proptest::collection::vec(any::<u8>(), 4..64)) {
        buf.truncate(buf.len() / 4 * 4); // multiple of 4, at least 4
        checksum::append(&mut buf).unwrap();
        prop_assert_eq!(checksum::verify(&buf), buf.len() > 4);
        if buf.len() > 4 {
            buf[0] ^= 0x01;
            prop_assert!(!checksum::verify(&buf));
        }
    }

    #[test]
    fn xor_pass_keeps_head_and_never_panics(
        mut buf in proptest::collection::vec(any::<u8>(), 0..64),
        key in any::<i32>(),
    ) {
        let head: Vec<u8> = buf.iter().take(4).copied().collect();
        let len = buf.len();
        let result = xor_pass::encrypt(&mut buf, key);
        prop_assert_eq!(result.is_ok(), len >= 8);
        prop_assert_eq!(&buf[..head.len()], &head[..]);
    }
}
