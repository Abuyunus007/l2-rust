# M7-NPC-01: Castle NPCs

| Field | Value |
|---|---|
| Milestone | M7 |
| Model | sonnet |
| Review | full |
| Reference | no (copy the merged M6 code around it) |
| Depends on | RES-01, SIEGE-01 |
| Size | L |

## Goal
Port the packets this card needs yourself in `crates/l2-protocol-game/src/{client,server}/<area>.rs` with round-trip tests, dispatch arms and proxy decoders if GPROTO-10 did not. The castle residence scripts built in like CLAN-02's `ClanMaster` (bypass handler + the scripts' html pages): `CastleChamberlain` (tax rate, doors and traps upgrades, functions, treasury, manor link, siege time), `CastleSiegeManager`, `CastleMercenaryManager`, `CastleWarehouse` (the castle vault), `CastleTeleporter`, `CastleBlacksmith`, `CastleCourtMagician` (squad skills: CLAN-03's `show_sub_unit_skill_list`), `CastleAmbassador`, `MercenaryCaptain`, the `CastleDoorman`, `SiegeGuards` AI.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\ai\others\{CastleChamberlain,CastleSiegeManager,CastleMercenaryManager,CastleWarehouse,CastleTeleporter,CastleBlacksmith,CastleCourtMagician,CastleAmbassador,MercenaryCaptain,SiegeGuards.java}`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\instance\{CastleDoorman,Doorman}.java`

## Files
- `crates/l2-world/src/residence/npc/*.rs (new)`
- `crates/l2-world/src/bypass.rs (registrations)`
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
- [ ] committed as `M7-NPC-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Also: the `CastleAmbassador` dialog (FORT-01 left `fort_castle_id_by_ambassador` and `fort_contracted_castle` for it) and the guard AIs: `SiegeGuards` for castles and Java's `FortSiegeGuardAI` (FORT-01 spawns the fort guards but has no AI for them). The manor link of the chamberlain is a page link only (MANOR-01 wired `on_npc_manor_bypass`).
- Read CLAUDE.md (safety rules), `docs/DECISIONS.md` (D-39, D-49, D-50, D-51), `docs/design/m6-social.md` and `docs/design/m7-residences.md` first.
- Up to seven workers run at the same time. In shared files (`world.rs`, `command.rs`, `timer.rs`, `outbox.rs`, `player.rs`, `pvp.rs`, `zones.rs`, `dispatch.rs`, `client/enter.rs`, `persist.rs`) only add your lines: no reformatting or moving code.
- Use `-j 4` and CARGO_INCREMENTAL=0. Before your final checks run `git merge main` and resolve conflicts keeping both sides; make sure no test crate fails to compile.
- DB tests use `l2_db::GAME_SCHEMA` (every `crates/l2-db/sql/game/*.sql`); add a schema copy there for every new table.
