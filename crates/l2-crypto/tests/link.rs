//! Tests for the login↔game link cipher (`LinkCipher`).
//!
//! Java: loginserver/GameServerThread.java (`sendPacket`, `run`, `DEFAULT_BLOWFISH_KEY`) and
//! gameserver/LoginServerThread.java (`sendPacket`). Three kinds of check:
//! - `java_send_packet` is a literal port of the Java byte work, including the
//!   `appendChecksum(data, 2, size)` bounds quirk. `seal` must match it for every size.
//! - The checksum step is checked against Java's `appendChecksum` vectors in
//!   `crypto_newcrypt.json`. Seal of such a payload, decrypted, must give Java's checksummed block.
//! - Round trips and tamper checks for the default key and a 40-byte session key.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use l2_crypto::link::DEFAULT_KEY;
use l2_crypto::{CryptoError, L2Blowfish, LinkCipher};
use l2_testkit::{assert_bytes_eq, from_hex, vectors::load};
use serde::Deserialize;

/// `LoginServerThread.BLOWFISH_KEY_SIZE`: the session key the login server sends is 40 bytes.
const SESSION_KEY_LEN: usize = 40;

#[derive(Deserialize)]
struct NewCryptFile {
    append_checksum: Vec<Pair>,
}

#[derive(Deserialize)]
struct Pair {
    input: String,
    output: String,
}

fn session_key() -> Vec<u8> {
    (0..SESSION_KEY_LEN)
        .map(|i| (i as u8).wrapping_mul(37).wrapping_add(11))
        .collect()
}

/// Both keys the link uses: the static default key and a 40-byte session key.
fn keys() -> Vec<Vec<u8>> {
    vec![DEFAULT_KEY.to_vec(), session_key()]
}

fn payload_of(n: usize) -> Vec<u8> {
    (0..n).map(|i| (i * 7 + 3) as u8).collect()
}

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

/// Literal port of the byte work in `GameServerThread.sendPacket` (same as
/// `LoginServerThread.sendPacket`): a 2-byte header, the payload, `writeInt(0)`, padding to 8,
/// `appendChecksum(data, 2, size)`, then `crypt(data, 2, size)`. Returns the bytes after the header.
fn java_send_packet(key: &[u8], payload: &[u8]) -> Vec<u8> {
    const HEADER: usize = 2;
    let mut data = vec![0u8; HEADER];
    data.extend_from_slice(payload);
    data.extend_from_slice(&[0; 4]); // writeInt(0): reserved checksum slot
    let padding = (data.len() - HEADER) % 8;
    if padding != 0 {
        data.resize(data.len() + 8 - padding, 0);
    }
    let size = data.len() - HEADER;
    java_append_checksum(&mut data, HEADER, size);
    let bf = L2Blowfish::new(key).unwrap();
    for block in data[HEADER..].chunks_exact_mut(8) {
        bf.encrypt_in_place(block).unwrap();
    }
    data[HEADER..].to_vec()
}

#[test]
fn default_key_is_the_java_static_key_with_nul() {
    // Java: "_;v.]05-31!|+-%xT!^[$\00". The \00 escape is one NUL byte, so 21 text bytes + NUL.
    let mut expected = b"_;v.]05-31!|+-%xT!^[$".to_vec();
    expected.push(0);
    assert_eq!(DEFAULT_KEY, expected.as_slice());
    assert_eq!(DEFAULT_KEY.len(), 22);
    assert!(LinkCipher::default_key().is_ok());
}

#[test]
fn seal_matches_java_send_packet_for_all_sizes() {
    for key in keys() {
        let cipher = LinkCipher::new(&key).unwrap();
        for n in 0..=64 {
            let payload = payload_of(n);
            assert_bytes_eq(&cipher.seal(&payload), &java_send_packet(&key, &payload));
        }
    }
}

#[test]
fn seal_open_round_trip_for_sizes_1_to_64() {
    for key in keys() {
        let cipher = LinkCipher::new(&key).unwrap();
        for n in 1..=64 {
            let payload = payload_of(n);
            let sealed = cipher.seal(&payload);
            let padded_len = (n + 4).next_multiple_of(8);
            assert_eq!(sealed.len(), padded_len, "payload {n}");

            let mut wire = sealed.clone();
            let body = cipher.open(&mut wire).unwrap();
            assert_eq!(body.len(), padded_len, "payload {n}");
            assert_eq!(&body[..n], payload.as_slice(), "payload {n}");
            // Padding and the checksum slot are zero, except the final 4 bytes (the checksum).
            assert!(body[n..body.len() - 4].iter().all(|&b| b == 0), "payload {n}");
        }
    }
}

#[test]
fn open_rejects_any_single_byte_tamper() {
    for key in keys() {
        let cipher = LinkCipher::new(&key).unwrap();
        let sealed = cipher.seal(b"tamper me, please");
        for i in 0..sealed.len() {
            let mut wire = sealed.clone();
            wire[i] ^= 0x01;
            assert_eq!(
                cipher.open(&mut wire),
                Err(CryptoError::ChecksumMismatch),
                "flipped byte {i}"
            );
        }
    }
}

#[test]
fn open_rejects_bad_lengths_and_wrong_key() {
    let cipher = LinkCipher::new(&session_key()).unwrap();
    assert_eq!(
        cipher.open(&mut [0u8; 12]),
        Err(CryptoError::NotBlockAligned { len: 12 })
    );
    // Empty body: no checksum slot, so Java's verifyChecksum returns false.
    assert_eq!(cipher.open(&mut []), Err(CryptoError::ChecksumMismatch));

    let sealed = cipher.seal(b"session payload");
    let other = LinkCipher::default_key().unwrap();
    let mut wire = sealed;
    assert_eq!(other.open(&mut wire), Err(CryptoError::ChecksumMismatch));
}

#[test]
fn new_rejects_bad_key_lengths() {
    assert_eq!(
        LinkCipher::new(&[1, 2, 3]).unwrap_err(),
        CryptoError::InvalidKeyLength { len: 3 }
    );
    assert_eq!(
        LinkCipher::new(&[0; 57]).unwrap_err(),
        CryptoError::InvalidKeyLength { len: 57 }
    );
}

/// The checksum step against Java: `crypto_newcrypt.json` `append_checksum` pairs give the
/// checksummed plaintext Java produces. Seal of the same payload, decrypted, must give that block,
/// and the sealed bytes must equal Blowfish of it. Only pairs whose length is a multiple of 8 map
/// to a seal with no extra padding; the empty-payload pair is skipped.
#[test]
fn seal_checksum_step_matches_java_append_checksum_vectors() {
    let file: NewCryptFile = load("crypto_newcrypt");
    let mut tested = 0;
    for pair in &file.append_checksum {
        let input = from_hex(&pair.input);
        if input.len() < 8 || input.len() % 8 != 0 {
            continue;
        }
        let java_block = from_hex(&pair.output);
        let payload = &input[..input.len() - 4];
        for key in keys() {
            let cipher = LinkCipher::new(&key).unwrap();
            let sealed = cipher.seal(payload);

            let bf = L2Blowfish::new(&key).unwrap();
            let mut expected = java_block.clone();
            bf.encrypt_in_place(&mut expected).unwrap();
            assert_bytes_eq(&sealed, &expected);

            let mut plain = sealed.clone();
            bf.decrypt_in_place(&mut plain).unwrap();
            assert_bytes_eq(&plain, &java_block);
        }
        tested += 1;
    }
    assert!(
        tested >= 3,
        "expected several usable append_checksum pairs, got {tested}"
    );
}
