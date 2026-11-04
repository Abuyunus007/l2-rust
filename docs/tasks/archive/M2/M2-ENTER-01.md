# M2-ENTER-01: Enter world, chat, logout, restart

| Field | Value |
|---|---|
| Milestone | M2 |
| Model | sonnet |
| Review | full |
| Reference | no |
| Depends on | M2-CHAR-01, M2-GPROTO-02, M2-GPROTO-03, M2-GPROTO-04, M2-WORLD-01 |
| Size | M |

## Goal
CharacterSelect loads the full player (DB) → WorldCommand::EnterWorld → the world sends the EnterWorld packet set; PlayerInGame to the LS; chat (in M2: to self and to everyone in the world, since there are no regions yet); Logout and RequestRestart store the player and leave the world.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\clientpackets\{EnterWorld,Say2,Logout,RequestRestart}.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\Player.java (load/restore, store, onPlayerEnter parts relevant to a plain character)`

## Reference implementation to copy
- crates/l2-world (WORLD-01) and crates/game-server/src/client/

## Files
- `crates/l2-world/src/{enter.rs,chat.rs,leave.rs}`
- `crates/game-server/src/client/enter.rs`
- `tests`

## Requirements
1. Reference captures of the owner's Java GS sessions: `C:\l2-rust\run\captures\*.jsonl` (local only, never commit them raw; scrubbed copies go to `fixtures/captures/` via M2-CAP-01). The packet *sequence* after EnterWorld must match a captured Java session for a plain character (order and types; values where deterministic).
2. Unknown in-game packets are ignored (logged at debug) instead of disconnecting.

## Out of scope
- Movement and visibility (M3).

## Tests (must exist and pass)
- World unit tests with a recording Outbox; DB-gated e2e in M2-E2E-01.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass
- [ ] committed as `M2-ENTER-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read `docs/design/world-loop.md` and the CLAUDE.md safety rules first (never touch a local database or real ports).
- The owner's real Java GS and DB run side by side with our servers (D-39): anything written to the DB must look exactly like Java writes it.
