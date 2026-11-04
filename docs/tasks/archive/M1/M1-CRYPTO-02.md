# M1-CRYPTO-02: RSA (no padding) and scrambled modulus

| Field | Value |
|---|---|
| Milestone | M1 |
| Model | sonnet |
| Review | full |
| Reference | no |
| Depends on | M1-CRYPTO-01 (crate layout) |
| Size | S |

## Goal
Raw RSA as L2 uses it: key generation (1024 for clients, 512 for GS links), no-padding decrypt/encrypt, the scrambled modulus for the `Init` packet, and Java `BigInteger.toByteArray` conversions.

## Java source (the spec)
Base: `C:\l2-rust\l2jmobius\java\org\l2jmobius\`
- `loginserver\controller\ScrambledKeyPair.java` (4 scramble steps; strips a leading 0x00 of a 129-byte modulus)
- `loginserver\controller\LoginController.java` (keygen 1024/F4, 10 pairs) and `loginserver\data\GameServerTable.java` `initRSAKeys` (512/F4)
- `loginserver\network\clientpackets\RequestAuthLogin.java` (`RSA/ECB/nopadding` decrypt of 128-byte blocks)
- `loginserver\network\gameserverpackets\receive\BlowFishKey.java` (decrypt, then strip leading zero bytes)
- `loginserver\network\gameserverpackets\send\InitLS.java` (sends `modulus.toByteArray()`: may have a leading 0x00)

## Reference implementation to copy
- `crates/l2-crypto/src/newcrypt.rs` and its vector test

## Files
Create:
- `crates/l2-crypto/src/rsa.rs`, `crates/l2-crypto/src/scramble.rs`
- `crates/l2-crypto/tests/vectors_rsa.rs`
Modify:
- `crates/l2-crypto/src/lib.rs` (add the `mod` lines and re-exports only)

## Requirements
1. `RsaKeyPair::generate(bits, rng)` with e = 65537. `public_modulus_be()`, `java_to_byte_array_modulus()` (Java `toByteArray` semantics: big-endian, minimal two's complement, so a leading 0x00 when the high bit is set).
2. `decrypt_raw(&self, block: &[u8]) -> Vec<u8>`: m = c^d mod n, output left-padded to the modulus length (Java `Cipher NoPadding` output). `encrypt_raw(pub_modulus, e, block)` for the proxy/fake-GS side.
3. `scramble_modulus(modulus_128: &[u8;128]) -> [u8;128]` and the inverse `unscramble_modulus` (needed by the proxy to read the real server's key; derive it by reversing the 4 steps, and prove it with a round-trip test).
4. Use the `rsa` crate's `hazmat`/raw operations or `num-bigint-dig` directly. No padding schemes anywhere.

## Out of scope
- Key pool management (the login server card does that).

## Tests
- All `crypto_scrambled_modulus.json` and `crypto_rsa_nopadding.json` cases pass.
- unscramble(scramble(m)) == m (property test).

## Done when
- [ ] vector tests pass
- [ ] committed as `M1-CRYPTO-02: <summary>`
