# M1-LPROTO-02: Login↔game server link packets

| Field | Value |
|---|---|
| Milestone | M1 |
| Model | haiku |
| Review | spot |
| Reference | no (copy M1-LPROTO-01) |
| Depends on | M1-LPROTO-01 |
| Size | M |

## Goal
All 18 LS↔GS link packets as structs in `l2-protocol-login`, for both directions (the Rust LS and the fake GS in tests use them, and so will the Rust GS in M2).

## Java source (the spec)
Base: `C:\l2-rust\l2jmobius\java\org\l2jmobius\`
- LS sends: `loginserver\network\gameserverpackets\send\{InitLS,LoginServerFail,AuthResponse,PlayerAuthResponse,KickPlayer,RequestCharacters,ChangePasswordResponse}.java`
- LS receives: `loginserver\network\gameserverpackets\receive\{BlowFishKey,GameServerAuth,PlayerInGame,PlayerLogout,ChangeAccessLevel,PlayerAuthRequest,ServerStatus,PlayerTracert,ReplyCharacters,RequestTempBan,ChangePassword}.java`
- GS-side writers of the same packets (to confirm layouts): `gameserver\network\loginserverpackets\send\*.java`, readers: `gameserver\network\loginserverpackets\receive\*.java`
- Dispatch by state: `loginserver\network\GameServerPacketHandler.java` (CONNECTED / BF_CONNECTED / AUTHED)
- Spec tables: `docs/analysis/01-login-server.md` (LS ↔ GS section)

## Reference implementation to copy
- `crates/l2-protocol-login/src/server.rs` and `tests/vectors.rs`

## Files
Create:
- `crates/l2-protocol-login/src/link.rs` (all link packets + `LinkState` + `decode_from_gs(state, ...)` / `decode_from_ls(...)`)
- `crates/l2-protocol-login/tests/link_vectors.rs`
Modify:
- `crates/l2-protocol-login/src/lib.rs` (`mod` + re-export only)

## Requirements
1. Layouts exactly as in Java, including: `GameServerAuth` limits (hexId ≤ 64, host pairs ≤ 32 → decode error), `ServerStatus` as a list of `(type, value)` pairs with named type/value consts, `ReplyCharacters` with `i64` delete times, `RequestTempBan` with an optional reason, `PlayerInGame` with a `u16` count + strings.
2. `ChangePassword` (0x0B) is the only GS→LS packet whose Java handler reads its own opcode. Keep the same layout.
3. Unknown opcode in a state → a typed error that tells the caller to `forceClose(NOT_AUTHED)` (Java behaviour), not `Ok(None)`.

## Out of scope
- Link crypto (M1-CRYPTO-05), the handling logic (M1-LS-04/05).

## Tests
- Every link packet in `login_packets.json`: encode/decode byte-exact.
- Round trip for all 18 packets; dispatch-by-state tests.

## Done when
- [ ] tests pass
- [ ] committed as `M1-LPROTO-02: <summary>`
