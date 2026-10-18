# M2-WORLD-01: World loop skeleton

| Field | Value |
|---|---|
| Milestone | M2 |
| Model | opus (orchestrator) |
| Review | full |
| Reference | yes (every game-state card builds on it) |
| Depends on | none |
| Size | M |

## Goal
`l2-world` per `docs/design/world-loop.md`: World, WorldCommand, the 50 ms tick runner on its own thread, timer wheel, seedable RNG, the Outbox/PacketSink traits, a minimal Player, and a synchronous test harness.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\World.java (object registry only; regions are M3)`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\commons\threads\ThreadPool.java, C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\taskmanagers\* (what the timer wheel replaces)`

## Reference implementation to copy
- none (this card creates it)

## Files
- `crates/l2-world/{Cargo.toml,src/lib.rs,src/world.rs,src/command.rs,src/timer.rs,src/outbox.rs,src/player.rs,src/runner.rs}`
- `crates/l2-world/tests/world.rs`

## Requirements
1. Exactly the structure of docs/design/world-loop.md. No tokio/sqlx/socket dependency (MODULE_MAP rule).
2. `runner::spawn(world, rx) -> JoinHandle`: dedicated std thread, drains commands (bounded per tick), ticks every 50 ms, logs ticks > 50 ms.
3. TimerWheel: schedule/cancel by id, `ms_to_ticks` (ceil), fires in order; unit tested.
4. Player = CreatureCore + player fields needed by M2 (account, name, class, race, sex, appearance, level, exp, sp, position/heading, hp/mp/cp current+max, title, inventory/paperdoll item ids, skills, shortcuts). Fields documented with the Java field they mirror.

## Out of scope
- Regions/visibility (M3), stats/formulas (M4), packet encoding of specific packets (GPROTO cards).

## Tests (must exist and pass)
- Deterministic replay with a fixed seed; timers; command ordering; shutdown stores all players (via a recording Outbox).

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass
- [ ] committed as `M2-WORLD-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read `docs/design/world-loop.md` and the CLAUDE.md safety rules first (never touch a local database or real ports).
- The owner's real Java GS and DB run side by side with our servers (D-39): anything written to the DB must look exactly like Java writes it.
