# M1-CRYPTO-03: Login client encryption

| Field | Value |
|---|---|
| Milestone | M1 |
| Model | sonnet |
| Review | full |
| Reference | no |
| Depends on | M1-CRYPTO-01 |
| Size | S |

## Goal
The client↔LS packet cipher: static-key first packet with XOR pass, then session Blowfish with checksum, including the exact padding sizes.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\loginserver\network\LoginEncryption.java`: `encryptedSize`, `encrypt`, `decrypt`, the static key bytes
- `docs/analysis/01-login-server.md` (Encryption section, **[verify]** notes)

## Reference implementation to copy
- `crates/l2-crypto/src/newcrypt.rs`

## Files
Create:
- `crates/l2-crypto/src/login.rs`, `crates/l2-crypto/tests/vectors_login.rs`
Modify:
- `crates/l2-crypto/src/lib.rs` (`mod` + re-export only)

## Requirements
1. `LoginCipher::new(session_key: &[u8;16])`. `encrypt(&mut self, payload: &[u8], xor_key: i32) -> Vec<u8>` returns the encrypted body (without the length header). The **first** call uses the static key + XOR pass (`xor_key`), and later calls use checksum + session key (`xor_key` ignored). The XOR key is passed in, not drawn internally, so callers use `WorldRng`/OS rng and tests are deterministic.
2. `decrypt(&self, body: &mut [u8]) -> Result<(), CryptoError>`: session-key decrypt + checksum verify. An invalid checksum is an error (the caller closes the connection).
3. `encrypted_size(payload_len, first)` reproduces Java exactly, including the "always add padding" quirk. Mark it `// NOTE(java-quirk):`.
4. A **client-side** counterpart `LoginClientCipher` (decrypts server packets incl. the static first packet: decrypt with the static key, then undo the XOR pass; encrypts client packets) for the proxy and fake-client tests.

## Out of scope
- RSA, framing.

## Tests
- All `crypto_login_encryption.json` cases (sizes 1..40, first and later packets).
- Round trip server-encrypt → client-decrypt and back, including the first packet.

## Done when
- [ ] vector tests pass
- [ ] committed as `M1-CRYPTO-03: <summary>`
