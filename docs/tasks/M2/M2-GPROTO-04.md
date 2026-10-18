# M2-GPROTO-04: Chat, logout and restart packets

| Field | Value |
|---|---|
| Milestone | M2 |
| Model | haiku |
| Review | spot |
| Reference | no |
| Depends on | M2-GPROTO-01 |
| Size | S |

## Goal
Say2/CreatureSay, Logout/LeaveWorld, RequestRestart/RestartResponse, NetPing if the client sends it.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\clientpackets\{Say2,Logout,RequestRestart}.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\serverpackets\{CreatureSay,LeaveWorld,RestartResponse}.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\enums\ChatType.java`

## Reference implementation to copy
- crates/l2-protocol-game/src/server/charscreen.rs

## Files
- `crates/l2-protocol-game/src/client/ingame.rs`
- `crates/l2-protocol-game/src/server/chat.rs`
- `crates/l2-protocol-game/tests/chat.rs`

## Requirements
1. ChatType enum with all Java values.
2. Reference captures of the owner's Java GS sessions: `C:\l2-rust\run\captures\*.jsonl` (local only, never commit them raw; scrubbed copies go to `fixtures/captures/` via M2-CAP-01).

## Out of scope
- nothing beyond the files above

## Tests (must exist and pass)
- Capture comparison; round trips.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass
- [ ] committed as `M2-GPROTO-04: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read `docs/design/world-loop.md` and the CLAUDE.md safety rules first (never touch a local database or real ports).
- The owner's real Java GS and DB run side by side with our servers (D-39): anything written to the DB must look exactly like Java writes it.
