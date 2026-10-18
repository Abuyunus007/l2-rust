//! Java-vector tests for raw RSA and the scrambled modulus
//! (`crypto_rsa_nopadding.json`, `crypto_scrambled_modulus.json`), plus property tests.
//!
//! The Java keys in the vectors only list `n`, `e`, `d`, so those cases use the bare-number
//! functions; `RsaKeyPair` (which generates its own keys) is covered by seeded round trips.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use l2_crypto::rsa::{decrypt_raw_with, encrypt_raw, java_to_byte_array, strip_leading_zeros};
use l2_crypto::scramble::{scramble_modulus, unscramble_modulus};
use l2_crypto::{RsaError, RsaKeyPair};
use l2_testkit::{assert_bytes_eq, from_hex, vectors::load};
use proptest::prelude::*;
use rand_chacha::ChaCha12Rng;
use rand_chacha::rand_core::SeedableRng;
use serde::Deserialize;

// ---- crypto_rsa_nopadding.json ---------------------------------------------------------

#[derive(Deserialize)]
struct RsaFile {
    rsa_1024: RsaKey,
    rsa_512: RsaKey,
}

#[derive(Deserialize)]
struct RsaKey {
    n: String,
    e: String,
    d: String,
    cases: Vec<RsaCase>,
}

#[derive(Deserialize)]
struct RsaCase {
    plain: Option<String>,
    encrypted_with_public: String,
    decrypted_with_private: String,
}

fn check_key(key: &RsaKey, key_len: usize) {
    let n = from_hex(&key.n);
    let e = from_hex(&key.e);
    let d = from_hex(&key.d);
    assert_eq!(n.len(), key_len);
    assert!(!key.cases.is_empty());
    for case in &key.cases {
        let cipher = from_hex(&case.encrypted_with_public);
        let expected = from_hex(&case.decrypted_with_private);
        assert_eq!(expected.len(), key_len, "Java output is always key-sized");

        let decrypted = decrypt_raw_with(&n, &d, &cipher).unwrap();
        assert_bytes_eq(&decrypted, &expected);

        if let Some(plain) = &case.plain {
            let plain = from_hex(plain);
            assert_bytes_eq(&encrypt_raw(&n, &e, &plain).unwrap(), &cipher);
            // Java left-pads short plaintexts (the 22-byte Blowfish key case).
            assert_bytes_eq(strip_leading_zeros(&decrypted), strip_leading_zeros(&plain));
        }
    }
}

#[test]
fn rsa_1024_matches_java() {
    let file: RsaFile = load("crypto_rsa_nopadding");
    check_key(&file.rsa_1024, 128);
}

#[test]
fn rsa_512_matches_java() {
    let file: RsaFile = load("crypto_rsa_nopadding");
    check_key(&file.rsa_512, 64);
}

#[test]
fn java_modulus_with_sign_byte_is_accepted() {
    // InitLS sends `toByteArray()`; a proxy feeds that straight back in.
    let file: RsaFile = load("crypto_rsa_nopadding");
    let key = &file.rsa_1024;
    let n = from_hex(&key.n);
    let signed = java_to_byte_array(&n);
    assert_eq!(signed.len(), 129);
    let case = key.cases.iter().find(|c| c.plain.is_some()).unwrap();
    let plain = from_hex(case.plain.as_ref().unwrap());
    let cipher = encrypt_raw(&signed, &from_hex(&key.e), &plain).unwrap();
    assert_bytes_eq(&cipher, &from_hex(&case.encrypted_with_public));
}

// ---- crypto_scrambled_modulus.json -----------------------------------------------------

#[derive(Deserialize)]
struct ScrambleFile {
    cases: Vec<ScrambleCase>,
}

#[derive(Deserialize)]
struct ScrambleCase {
    what: String,
    modulus: String,
    to_byte_array_length: usize,
    scrambled: String,
}

fn to_128(bytes: &[u8]) -> [u8; 128] {
    bytes.try_into().unwrap()
}

#[test]
fn scrambled_modulus_matches_java() {
    let file: ScrambleFile = load("crypto_scrambled_modulus");
    assert!(file.cases.len() >= 6);
    for case in &file.cases {
        let modulus = to_128(&from_hex(&case.modulus));
        let expected = to_128(&from_hex(&case.scrambled));
        assert_eq!(
            java_to_byte_array(&modulus).len(),
            case.to_byte_array_length,
            "toByteArray length: {}",
            case.what
        );
        assert_bytes_eq(&scramble_modulus(&modulus), &expected);
        assert_bytes_eq(&unscramble_modulus(&expected), &modulus);
    }
}

// ---- toByteArray and helpers -----------------------------------------------------------

#[test]
fn java_to_byte_array_is_minimal_twos_complement() {
    assert_eq!(java_to_byte_array(&[]), vec![0]);
    assert_eq!(java_to_byte_array(&[0, 0]), vec![0]);
    assert_eq!(java_to_byte_array(&[0x7f]), vec![0x7f]);
    assert_eq!(java_to_byte_array(&[0x80]), vec![0x00, 0x80]);
    assert_eq!(java_to_byte_array(&[0x00, 0x00, 0x01, 0x02]), vec![0x01, 0x02]);
    assert_eq!(java_to_byte_array(&[0x00, 0xff, 0x01]), vec![0x00, 0xff, 0x01]);
}

#[test]
fn strip_leading_zeros_matches_extract_key_material() {
    assert_eq!(strip_leading_zeros(&[0, 0, 5, 0, 6]), &[5, 0, 6]);
    assert_eq!(strip_leading_zeros(&[1, 2]), &[1, 2]);
    assert!(strip_leading_zeros(&[0, 0, 0]).is_empty());
    assert!(strip_leading_zeros(&[]).is_empty());
}

// ---- generated keys --------------------------------------------------------------------

#[test]
fn generated_1024_key_round_trips_and_scrambles() {
    let mut rng = ChaCha12Rng::seed_from_u64(0x4c32);
    let pair = RsaKeyPair::generate(1024, &mut rng).unwrap();
    let n = pair.public_modulus_be();
    assert_eq!(n.len(), 128);
    assert_eq!(pair.public_exponent_be(), vec![0x01, 0x00, 0x01]);

    // The rsa crate always sets the top bit, so toByteArray has the sign byte.
    let signed = pair.java_to_byte_array_modulus();
    assert_eq!(signed.len(), 129);
    assert_eq!(signed[0], 0);
    assert_bytes_eq(&signed[1..], &n);

    let scrambled = pair.scrambled_modulus().unwrap();
    assert_bytes_eq(&scrambled, &scramble_modulus(&to_128(&n)));
    assert_bytes_eq(&unscramble_modulus(&scrambled), &n);

    // RequestAuthLogin style: a 128-byte block, first byte below the modulus' first byte.
    let mut plain = [0u8; 128];
    for (i, b) in plain.iter_mut().enumerate() {
        *b = (i as u8).wrapping_mul(7).wrapping_add(3);
    }
    plain[0] = 0;
    let cipher = encrypt_raw(&n, &pair.public_exponent_be(), &plain).unwrap();
    assert_eq!(cipher.len(), 128);
    assert_bytes_eq(&pair.decrypt_raw(&cipher).unwrap(), &plain);
}

#[test]
fn generated_512_key_decrypts_short_blowfish_key() {
    let mut rng = ChaCha12Rng::seed_from_u64(7);
    let pair = RsaKeyPair::generate(512, &mut rng).unwrap();
    assert_eq!(pair.public_modulus_be().len(), 64);
    assert_eq!(pair.java_to_byte_array_modulus().len(), 65);
    assert_eq!(pair.scrambled_modulus(), Err(RsaError::ModulusSize { len: 64 }));

    // GS -> LS: a 22-byte Blowfish key (not starting with 0) is sent as one RSA block.
    let key: Vec<u8> = (1..=22).collect();
    let cipher = encrypt_raw(&pair.public_modulus_be(), &pair.public_exponent_be(), &key).unwrap();
    let decrypted = pair.decrypt_raw(&cipher).unwrap();
    assert_eq!(decrypted.len(), 64);
    assert_bytes_eq(strip_leading_zeros(&decrypted), &key);
}

#[test]
fn same_seed_gives_same_key() {
    let a = RsaKeyPair::generate(512, &mut ChaCha12Rng::seed_from_u64(99)).unwrap();
    let b = RsaKeyPair::generate(512, &mut ChaCha12Rng::seed_from_u64(99)).unwrap();
    assert_eq!(a.public_modulus_be(), b.public_modulus_be());
    assert!(format!("{a:?}").contains("bits"), "Debug shows only the size");
}

// ---- errors ----------------------------------------------------------------------------

#[test]
fn invalid_inputs_return_errors() {
    let file: RsaFile = load("crypto_rsa_nopadding");
    let n = from_hex(&file.rsa_512.n);
    let d = from_hex(&file.rsa_512.d);

    assert_eq!(decrypt_raw_with(&n, &d, &[]), Err(RsaError::EmptyBlock));
    assert_eq!(
        decrypt_raw_with(&n, &d, &[1u8; 65]),
        Err(RsaError::BlockTooLong { len: 65, max: 64 })
    );
    // c >= n
    assert_eq!(decrypt_raw_with(&n, &d, &n), Err(RsaError::BlockOutOfRange));
    assert_eq!(
        decrypt_raw_with(&n, &d, &[0xff; 64]),
        Err(RsaError::BlockOutOfRange)
    );
    assert_eq!(decrypt_raw_with(&[], &d, &[1]), Err(RsaError::InvalidKey));
    assert_eq!(decrypt_raw_with(&n, &[0], &[1]), Err(RsaError::InvalidKey));
    assert_eq!(
        RsaKeyPair::generate(8, &mut ChaCha12Rng::seed_from_u64(1)).err(),
        Some(RsaError::KeyGeneration)
    );
}

// ---- properties ------------------------------------------------------------------------

proptest! {
    #[test]
    fn unscramble_inverts_scramble(m in any::<[u8; 128]>()) {
        prop_assert_eq!(unscramble_modulus(&scramble_modulus(&m)), m);
        prop_assert_eq!(scramble_modulus(&unscramble_modulus(&m)), m);
    }

    #[test]
    fn java_to_byte_array_round_trips_value(bytes in proptest::collection::vec(any::<u8>(), 0..40)) {
        let out = java_to_byte_array(&bytes);
        let minimal = strip_leading_zeros(&bytes);
        // The value is unchanged, never negative (top bit clear after the optional sign byte).
        prop_assert_eq!(strip_leading_zeros(&out), minimal);
        prop_assert_eq!(out[0] & 0x80, 0);
        let needs_sign = minimal.first().is_some_and(|b| b & 0x80 != 0);
        prop_assert_eq!(out.len(), if minimal.is_empty() { 1 } else { minimal.len() + usize::from(needs_sign) });
    }

    #[test]
    fn decrypt_never_panics_and_is_key_sized(block in proptest::collection::vec(any::<u8>(), 0..80)) {
        let file: RsaFile = load("crypto_rsa_nopadding");
        let n = from_hex(&file.rsa_512.n);
        let d = from_hex(&file.rsa_512.d);
        if let Ok(out) = decrypt_raw_with(&n, &d, &block) {
            prop_assert_eq!(out.len(), 64);
        }
    }
}
