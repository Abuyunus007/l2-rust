# M2-ADM-01: Game server admin status + supervisor entry

| Field | Value |
|---|---|
| Milestone | M2 |
| Model | haiku |
| Review | spot |
| Reference | no |
| Depends on | M2-GS-01 |
| Size | S |

## Goal
Admin protocol message for game server status (online players: name, account, level, class, ip, online since) served by the GS endpoint; supervisor example entry for the Rust GS.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\ui\*.java (what the Java GS UI shows)`

## Reference implementation to copy
- crates/login-server/src/admin.rs, crates/l2-admin-api/src/messages.rs

## Files
- `crates/l2-admin-api/src/messages.rs (GetGameStatus/GameStatus + event)`
- `crates/game-server/src/admin.rs`
- `crates/l2-supervisor/supervisor.example.ini`
- `docs/admin-api.md`

## Requirements
1. World answers through an AdminQuery command (no locks into the world).

## Out of scope
- nothing beyond the files above

## Tests (must exist and pass)
- Snapshot test; loopback test.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass
- [ ] committed as `M2-ADM-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read `docs/design/world-loop.md` and the CLAUDE.md safety rules first (never touch a local database or real ports).
- The owner's real Java GS and DB run side by side with our servers (D-39): anything written to the DB must look exactly like Java writes it.
