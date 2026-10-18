# M1-PROXY-02: Game traffic pass-through with opcode decoding

| Field | Value |
|---|---|
| Milestone | M1 |
| Model | sonnet |
| Review | full |
| Reference | no |
| Depends on | M1-PROXY-01, M1-GEN-01, M1-CRYPTO-04 |
| Size | S |

## Goal
The proxy also relays the game connection (client ↔ Java or Rust GS), splits frames, decrypts them if the game cipher is on, and records every packet with its opcode name.

## Java source (the spec)
Base: `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\`
- `GamePacketHandler.java` (0xD0 ex packets with u16 sub-opcode; 0xD0 0x51 has an i32 sub-id)
- `serverpackets\KeyPacket.java` (the key's 8 random bytes + `PacketEncryption` flag; the first server packet is plaintext), `GameClient.java` (`enableCrypt`), `Encryption.java`
- Opcode names: `crates/l2-protocol-game/src/generated/opcodes.rs` (from M1-GEN-01)

## Reference implementation to copy
- `crates/l2-proxy/src/login.rs`

## Files
Create:
- `crates/l2-proxy/src/game.rs`, `crates/l2-proxy/tests/game_passthrough.rs`
Modify:
- `crates/l2-proxy/src/lib.rs`, `crates/l2-proxy/Cargo.toml`, `crates/l2-proxy/src/bin/l2-proxy.rs`

## Requirements
1. Forward bytes unchanged in both directions (the proxy never re-encrypts game traffic). Decode a copy for the capture only.
2. Detect `KeyPacket` (server opcode `VERSION_CHECK`), read the key + encryption flag, and if encryption is on, run two `GameCipher` instances to decrypt each direction for the capture.
3. Record: opcode, ex opcode, name from the generated table (`"unknown"` otherwise), connection state guess, raw hex. `fields: null` for now: field decoding arrives with the M2 packet structs.
4. Handle the client connecting to the per-server port from PROXY-01's rewritten ServerList.

## Out of scope
- Field-level decoding of game packets (M2+), modifying traffic.

## Tests
- Fake GS sends KeyPacket (encryption on and off) + some frames, a fake client replies: the capture shows correct names, and the forwarded bytes are identical.

## Done when
- [ ] tests pass
- [ ] committed as `M1-PROXY-02: <summary>`
