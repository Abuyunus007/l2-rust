# M8-OLY-01: Olympiad

| Field | Value |
|---|---|
| Milestone | M8 |
| Model | opus |
| Review | full |
| Reference | no (copy the merged M0..M7 code around it) |
| Depends on | GPROTO-11, INST-01 |
| Size | L |

## Goal
`Olympiad` periods (competition, validation, weekly points, the period end), `olympiad_nobles` and `olympiad_nobles_eom`, registration (classed, non-classed, teams), `OlympiadGameManager`, `OlympiadGameTask` and the games in the stadiums (instances), the buffers and the observers/spectators (observer mode, the `FOLLOW-UP: observer mode` in `target.rs`), points, rewards and the manager NPC pages, `isInOlympiadMode` everywhere (the `// M8:` and `FOLLOW-UP(Olympiad)` markers), admin words. **Side by side (D-53):** the global rows of the Olympiad (the period/validation/weekly jobs, `olympiad_data`, the whole-table `olympiad_nobles` saves) are written only with `CycleDatabaseWrites = True` (`config/SideBySide.ini`, Rust-only, default False, loader in game-server); with it off Kain keeps them in memory, re-reads them at start and when they are shown (D-50), and writes per-player rows only where Java writes them per player.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\mechanics\olympiad\*.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\instance\OlympiadManager.java (grep)`

## Files
- `crates/l2-world/src/olympiad/*.rs` (new) + the markers
- crates/game-server, crates/l2-db
- tests

## Requirements
1. Java behaviour, messages, packets, HTML and DB rows exactly (D-39: the Java GS shares the DB; D-49..D-53 for global writes and fresh reads).
2. Single-owner world, ids not references, timers on the wheel, world RNG; no unwrap/expect on external data; no DB call or await in a tick (D-15: request/answer through the outbox).
3. Port the packets you need yourself in `crates/l2-protocol-game/src/{client,server}/<area>.rs` with round-trip tests, dispatch arms and proxy decoders (unless M8-GPROTO-11 already has them: check main first).
4. Small wire-ins in shared files are allowed: add your lines only. List every marker you resolve and every one you leave.

## Out of scope
- Quests, AI scripts, instance/event scripts, community board (the script milestone, D-05); customs, premium, item mall (M9).

## Tests (must exist and pass)
- A classed and a non-classed match from registration to points; a spectator; the period end with the switch on; the switch off case.

## Done when
- [ ] fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`, `-j 4`); `git merge main` before the final checks
- [ ] committed as `M8-OLY-01: <summary>`
- [ ] report delivered in the agent's report format (what you left and why, every guess)

## Notes from the orchestrator
- Read CLAUDE.md (safety rules), DECISIONS (D-15, D-39, D-49..D-53), the `docs/design/*.md` of the area you touch first.
- Up to seven workers run at the same time: in shared files (`lib.rs`, `world.rs`, `command.rs`, `outbox.rs`, `timer.rs`, `dispatch.rs`, `persist.rs`, `world_data.rs`) only add your lines; after `git merge main` watch for doubled `mod`/`use` lines.
- DB tests use `l2_db::GAME_SCHEMA`; add a schema copy to `crates/l2-db/sql/game/` for every new table.
