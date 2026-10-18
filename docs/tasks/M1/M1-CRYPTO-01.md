# M1-CRYPTO-01: Blowfish-LE and NewCrypt

| Field | Value |
|---|---|
| Milestone | M1 |
| Model | sonnet |
| Review | full |
| Reference | yes (structure of `l2-crypto` and vector-test style) |
| Depends on | M1-WS-01, M1-JV-01 |
| Size | S |

## Goal
`l2-crypto` exists with L2's little-endian Blowfish and the `NewCrypt` helpers, byte-exact against Java.

## Java source (the spec)
Base: `C:\l2-rust\l2jmobius\java\org\l2jmobius\commons\crypt\`
- `BlowfishEngine.java`: note `bytesTo32bits`/`bits32ToBytes` are **little-endian** and the halves are swapped on output (`xr` first)
- `NewCrypt.java`: `verifyChecksum`, `appendChecksum`, `encXORPass`, `crypt`, `decrypt`; constructor from a `String` key uses UTF-8 bytes
- Background: `docs/analysis/02-network-crypto.md`

## Reference implementation to copy
none (this card creates it; follow `l2-util` from M1-WS-01 for layout)

## Files
Create:
- `crates/l2-crypto/{Cargo.toml,src/lib.rs,src/blowfish.rs,src/newcrypt.rs}`
- `crates/l2-crypto/tests/vectors_newcrypt.rs`

## Requirements
1. `L2Blowfish::new(key: &[u8]) -> Result<Self, CryptoError>` (accepts 4..=56 byte keys like Java; error otherwise). `encrypt_in_place(&mut [u8])`/`decrypt_in_place(&mut [u8])`: length must be a multiple of 8, otherwise an error, never a panic.
2. Use the `blowfish` crate's `BlowfishLE` (D-24). If it doesn't match the vectors exactly, implement the swap/endianness adapter around it. **Don't** copy the S-boxes by hand.
3. `checksum::append(buf)`, `checksum::verify(buf) -> bool` with Java's rules (size multiple of 4, > 4, the last word holds the XOR).
4. `xor_pass::encrypt(buf, key: i32)` = `encXORPass` with Java's wrapping `i32` arithmetic (`wrapping_add`).
5. All functions are pure, no allocation in the hot path.

## Out of scope
- RSA, login encryption, game XOR cipher (other cards).

## Tests
- Every case in `crypto_blowfish.json` and `crypto_newcrypt.json` passes (encrypt, decrypt, checksum valid and corrupted, xor pass).
- Property test: decrypt(encrypt(x)) == x for random keys/lengths.

## Done when
- [ ] all vector tests pass
- [ ] committed as `M1-CRYPTO-01: <summary>`
