# M1-CRYPTO-05: LS↔GS link cipher

| Field | Value |
|---|---|
| Milestone | M1 |
| Model | haiku |
| Review | spot |
| Reference | no |
| Depends on | M1-CRYPTO-01 |
| Size | S |

## Goal
Seal/open helpers for packets on the login↔game server link (zero padding to 8, checksum, Blowfish-LE), with the default key and key replacement.

## Java source (the spec)
Base: `C:\l2-rust\l2jmobius\java\org\l2jmobius\`
- `loginserver\GameServerThread.java`: `sendPacket` (writes an `int 0` for the checksum, pads to 8, appendChecksum, crypt) and `run` (decrypt + verifyChecksum), `DEFAULT_BLOWFISH_KEY`
- `gameserver\LoginServerThread.java`: the GS side of the same (sendPacket / read loop, the 40-byte key)

## Reference implementation to copy
- `crates/l2-crypto/src/login.rs`

## Files
Create:
- `crates/l2-crypto/src/link.rs`, `crates/l2-crypto/tests/link.rs`
Modify:
- `crates/l2-crypto/src/lib.rs` (`mod` + re-export only)

## Requirements
1. `LinkCipher::default_key()` uses the 22 bytes `"_;v.]05-31!|+-%xT!^[$"` + NUL (check the exact byte count in the Java source: `"\00"` is an octal escape for NUL). `LinkCipher::new(key: &[u8])` for the 40-byte session key.
2. `seal(&self, payload: &[u8]) -> Vec<u8>`: payload + 4 zero bytes, zero padding to a multiple of 8, append checksum, encrypt. This must be byte-identical to Java's `sendPacket` body.
3. `open(&self, body: &mut [u8]) -> Result<&[u8], CryptoError>`: decrypt, verify the checksum, return the payload (with the padding still attached, as Java hands the whole buffer to the packet readers).

## Out of scope
- Framing, RSA.

## Tests
- Seal/open round trip for sizes 1..64 with both keys.
- Seal output matches a vector built from the Java `NewCrypt` calls (add one hand-computed case from `crypto_newcrypt.json` vectors if no direct vector exists; document it).

## Done when
- [ ] tests pass
- [ ] committed as `M1-CRYPTO-05: <summary>`

## Notes from the orchestrator
From the JV-01 report: `NewCrypt.verifyChecksum/appendChecksum(buf, offset, size)` compute `dataLength = size - 4` **without** adding `offset`, so they are only correct as "operate on the slice starting at offset". `GameServerThread.sendPacket` calls `appendChecksum(data, 2, len - 2)`, which works out to the checksum over the body. Implement checksum functions on a body slice (offset 0 semantics) and add a test showing that equals Java's `(buf, 2, len-2)` call.
