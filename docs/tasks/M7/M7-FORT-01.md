# M7-FORT-01: Fortresses and fortress sieges

| Field | Value |
|---|---|
| Milestone | M7 |
| Model | sonnet |
| Review | full |
| Reference | no (copy the merged M6 code around it) |
| Depends on | RES-01, GPROTO-10 |
| Size | L |

## Goal
Port the packets this card needs yourself in `crates/l2-protocol-game/src/{client,server}/<area>.rs` with round-trip tests, dispatch arms and proxy decoders if GPROTO-10 did not. `FortManager`/`Fort`/`FortSiege`/`FortSiegeManager`/`FortSiegeGuardManager`: fortress ownership, fort functions, `FortUpdater` (the periodic fort tasks), the fortress siege (registration, commanders, the `CombatFlag`, the fort spawns, the owner change, castle contracts and envoys), the `FortManager`/`FortDoorman`/`FortLogistics`/`FortCommander` instances and the `FortressSiegeManager`/`FortressArcherCaptain` scripts, `//fortsiege`. Global writes behind `SiegeDatabaseWrites` (D-51).

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\managers\{FortManager,FortSiegeManager,FortSiegeGuardManager}.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\mechanics\siege\{Fort,FortSiege,CombatFlag,FortSpawnHolder,FortTeleportWhoType,FortUpdater,FortUpdaterType}.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\instance\{FortCommander,FortDoorman,FortLogistics,FortManager}.java`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\ai\others\{FortressSiegeManager,FortressArcherCaptain}`

## Files
- `crates/l2-world/src/fort/*.rs (new) + wire-ins`
- `crates/l2-db (fort repositories)`
- `tests`

## Requirements
1. Java behaviour, messages, packets and DB rows exactly (D-39: the Java GS shares the DB).
2. Single-owner world, ids not references, timers on the wheel, world RNG; no unwrap/expect on external data; no DB waits inside a tick (D-15).
3. Global siege-like writes only with `SiegeDatabaseWrites` (D-51); shared rows re-read before acting on them (D-50).
4. Resolve the `// M7:` markers in your area; list the ones you leave.

## Out of scope
- Olympiad, Seven Signs, festivals, instances (M8); quests and scripts beyond the residence NPCs named here (later, D-05).

## Tests (must exist and pass)
- Each part with Java's numbers/messages/packets; DB tests skip without `L2_TEST_DATABASE_URL`.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`, `-j 4`)
- [ ] committed as `M7-FORT-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- RES-01 is merged: its API is listed in `docs/design/m7-residences.md` ("The residence API"). Build on it; route every global write through `siege_write` / `write_residence` (D-51) and every shared read through `request_residence_reread` (D-50).
- Read CLAUDE.md (safety rules), `docs/DECISIONS.md` (D-39, D-49, D-50, D-51), `docs/design/m6-social.md` and `docs/design/m7-residences.md` first.
- Up to seven workers run at the same time. In shared files (`world.rs`, `command.rs`, `timer.rs`, `outbox.rs`, `player.rs`, `pvp.rs`, `zones.rs`, `dispatch.rs`, `client/enter.rs`, `persist.rs`) only add your lines: no reformatting or moving code.
- Use `-j 4` and CARGO_INCREMENTAL=0. Before your final checks run `git merge main` and resolve conflicts keeping both sides; make sure no test crate fails to compile.
- DB tests use `l2_db::GAME_SCHEMA` (every `crates/l2-db/sql/game/*.sql`); add a schema copy there for every new table.
