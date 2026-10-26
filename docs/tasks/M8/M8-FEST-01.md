# M8-FEST-01: Festival of Darkness and Dimensional Rift

| Field | Value |
|---|---|
| Milestone | M8 |
| Model | sonnet |
| Review | full |
| Reference | no (copy the merged M0..M7 code around it) |
| Depends on | SS-01, INST-01 |
| Size | M |

## Goal
`SevenSignsFestival` (the festival rounds, scores, `seven_signs_festival` rows) and `DimensionalRift`/`DimensionalRiftManager` (rooms, jumps, the boss room). **Side by side (D-53):** the global rows of the festival results are written only with `CycleDatabaseWrites = True` (`config/SideBySide.ini`, Rust-only, default False, loader in game-server); with it off Kain keeps them in memory, re-reads them at start and when they are shown (D-50), and writes per-player rows only where Java writes them per player.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\mechanics\sevensigns\SevenSignsFestival.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\mechanics\DimensionalRift*.java (grep)`

## Files
- `crates/l2-world/src/sevensigns/{festival,rift}.rs` (new)
- tests

## Requirements
1. Java behaviour, messages, packets, HTML and DB rows exactly (D-39: the Java GS shares the DB; D-49..D-53 for global writes and fresh reads).
2. Single-owner world, ids not references, timers on the wheel, world RNG; no unwrap/expect on external data; no DB call or await in a tick (D-15: request/answer through the outbox).
3. Port the packets you need yourself in `crates/l2-protocol-game/src/{client,server}/<area>.rs` with round-trip tests, dispatch arms and proxy decoders (unless M8-GPROTO-11 already has them: check main first).
4. Small wire-ins in shared files are allowed: add your lines only. List every marker you resolve and every one you leave.

## Out of scope
- Quests, AI scripts, instance/event scripts, community board (the script milestone, D-05); customs, premium, item mall (M9).

## Tests (must exist and pass)
- A festival round, a rift run.

## Done when
- [ ] fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`, `-j 4`); `git merge main` before the final checks
- [ ] committed as `M8-FEST-01: <summary>`
- [ ] report delivered in the agent's report format (what you left and why, every guess)

## Notes from the orchestrator
- Read CLAUDE.md (safety rules), DECISIONS (D-15, D-39, D-49..D-53), the `docs/design/*.md` of the area you touch first.
- Up to seven workers run at the same time: in shared files (`lib.rs`, `world.rs`, `command.rs`, `outbox.rs`, `timer.rs`, `dispatch.rs`, `persist.rs`, `world_data.rs`) only add your lines; after `git merge main` watch for doubled `mod`/`use` lines.
- DB tests use `l2_db::GAME_SCHEMA`; add a schema copy to `crates/l2-db/sql/game/` for every new table.
