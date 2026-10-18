# M1-LPROTO-01: Client↔login server packets

| Field | Value |
|---|---|
| Milestone | M1 |
| Model | sonnet |
| Review | full |
| Reference | yes (THE packet-struct pattern: ~700 game packets will copy it) |
| Depends on | M1-NET-01, M1-JV-02 |
| Size | M |

## Goal
`l2-protocol-login` with every client↔LS packet as a plain struct with `Encode`/`Decode`, the reason enums, and a dispatcher that maps opcodes + connection state to client packets.

## Java source (the spec)
Base: `C:\l2-rust\l2jmobius\java\org\l2jmobius\loginserver\`
- `network\LoginClientPackets.java` (opcodes + allowed states), `network\LoginServerPackets.java`, `network\ConnectionState.java`
- `network\clientpackets\*.java`: AuthGameGuard, RequestAuthLogin (128 vs 256 byte variants), RequestServerList, RequestServerLogin
- `network\serverpackets\*.java`: Init, LoginFail, AccountKicked, LoginOk, ServerList, PlayFail, PlayOk, GGAuth
- `enums\LoginFailReason.java`, `PlayFailReason.java`, `AccountKickedReason.java`
- Spec tables: `docs/analysis/01-login-server.md`

## Reference implementation to copy
- `crates/l2-net/src/packet.rs` (doc example)

## Files
Create:
- `crates/l2-protocol-login/{Cargo.toml,src/lib.rs,src/client.rs,src/server.rs,src/enums.rs,src/dispatch.rs}`
- `crates/l2-protocol-login/tests/vectors.rs`

## Requirements
1. Server packets: `Init { session_id, protocol: u32, scrambled_modulus: [u8;128], gg: [u32;4], blowfish_key: [u8;16] }` etc. Constants from Java (0x0000C621, the GG values, 0x3EA in LoginOk) are named consts with a comment. `ServerList` has a `servers: Vec<ServerEntry>` + `char_info: Option<Vec<ServerCharInfo>>` (None writes `u8 0`, as Java does when `_charsOnServers == null`).
2. Client packets: `RequestAuthLogin` keeps the raw RSA blocks (`Rsa128` or `Rsa256` variant). Decoding the credentials happens in the login server, not here. Provide the pure helper `credentials_from_decrypted(&[u8]) -> (String, String)` with the exact offsets and trimming from Java.
3. Decoders tolerate trailing bytes (the client pads packets). Enforce Java's minimum lengths (`remaining() >= 20` etc.) and return an error otherwise.
4. `dispatch::decode_client(state, opcode, reader) -> Result<Option<ClientPacket>, _>`: unknown opcode or wrong state → `Ok(None)` (Java ignores them). 0x0E/0x0F are recognized and ignored.
5. Enums: `#[repr(u8)]`/`#[repr(u32)]` with every Java value and name.

## Out of scope
- Encryption, framing, any login logic. LS↔GS packets (M1-LPROTO-02).

## Tests
- Every packet in `login_packets.json` (client↔LS part): encode == Java bytes, decode(Java bytes) == struct.
- Round trip for every packet; state-dispatch tests for every opcode × state.

## Done when
- [ ] vector tests pass
- [ ] committed as `M1-LPROTO-01: <summary>`

## Notes from the orchestrator
Keep each packet as small and boring as possible: one struct, `impl Encode`, `impl Decode`, a `// Java:` line. Haiku agents will clone this file's shape hundreds of times.
