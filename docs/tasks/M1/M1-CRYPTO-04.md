# M1-CRYPTO-04: Game XOR cipher and Blowfish key generator

| Field | Value |
|---|---|
| Milestone | M1 |
| Model | haiku |
| Review | spot |
| Reference | no |
| Depends on | M1-CRYPTO-01, M1-JV-01 |
| Size | S |

## Goal
The game-traffic stream cipher and key generator (used by the proxy now and by the game server in M2).

## Java source (the spec)
Base: `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\`
- `Encryption.java`: the in/out key states, `encrypt` (the first call only sets `_isEnabled`), `decrypt`, `advanceOffset` (bytes 8..11 as a u32 LE += size)
- `BlowFishKeygen.java`: 8 random bytes + the fixed tail `C8 27 93 01 A1 6C 31 97`

## Reference implementation to copy
- `crates/l2-crypto/src/newcrypt.rs` and `tests/vectors_newcrypt.rs`

## Files
Create:
- `crates/l2-crypto/src/game.rs`, `crates/l2-crypto/tests/vectors_game.rs`
Modify:
- `crates/l2-crypto/src/lib.rs` (`mod` + re-export only)

## Requirements
1. `GameCipher::new(key: [u8;16])` with separate in/out state. `encrypt(&mut self, buf: &mut [u8])`, `decrypt(&mut self, buf: &mut [u8])`. Keep Java's "first encrypt only enables" behaviour exactly and comment it with `// NOTE(java-quirk):`.
2. `game_key_from_random(random8: [u8;8]) -> [u8;16]` (the caller supplies the random bytes).
3. Wrapping u32/i32 arithmetic, as in Java.

## Out of scope
- Anything outside these two functions.

## Tests
- All `crypto_game_xor.json` cases, in sequence (the key evolves across packets).
- Round trip: a client-side instance decrypts what a server-side instance encrypts, over 10 packets.

## Done when
- [ ] vector tests pass
- [ ] committed as `M1-CRYPTO-04: <summary>`
