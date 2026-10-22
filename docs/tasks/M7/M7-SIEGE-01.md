# M7-SIEGE-01: Castle sieges

| Field | Value |
|---|---|
| Milestone | M7 |
| Model | opus |
| Review | full |
| Reference | no (copy the merged M6 code around it) |
| Depends on | RES-01, GPROTO-10 |
| Size | XL |

## Goal
Port the packets this card needs yourself in `crates/l2-protocol-game/src/{client,server}/<area>.rs` with round-trip tests, dispatch arms and proxy decoders if GPROTO-10 did not. `SiegeManager`/`Siege`/`SiegeClan`: the schedule (`SiegeSchedule.xml`, `Siege.ini`), registration windows and the attacker/defender lists (`siege_clans` rows), start/end, siege zones and the siege relation bits in `pvp.rs` (the `// M7:` markers), `SiegeFlag` headquarters and respawn at flag/castle (`SiegeTeleportWhoType`), siege guards (`SiegeGuardManager`, `castle_siege_guards`), mercenary tickets (`MercTicketManager`, incl. the pet pickup marker), attackable and repairable doors and walls (`castle_doorupgrade`), the artifact and engraving, the owner change and its broadcasts, siege kill counts, the GM `//siege` start used by D-51 test sieges. All global writes behind `SiegeDatabaseWrites` (D-51).

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\managers\{SiegeManager,SiegeGuardManager,MercTicketManager}.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\mechanics\siege\{Siege,SiegeClan,SiegeClanType,SiegeTeleportWhoType,SiegeScheduleDate,Siegable}.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\instance\SiegeFlag.java`
- `C:\l2-rust\l2jmobius\dist\game\config\Siege.ini, SiegeSchedule.xml`

## Files
- `crates/l2-world/src/siege/*.rs (new) + wire-ins (pvp.rs, death.rs, respawn.rs, door.rs, summon/ pickup)`
- `crates/l2-db (siege repositories)`
- `crates/game-server/src/{persist.rs,world_data.rs}`
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
- [ ] committed as `M7-SIEGE-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read CLAUDE.md (safety rules), `docs/DECISIONS.md` (D-39, D-49, D-50, D-51), `docs/design/m6-social.md` and `docs/design/m7-residences.md` first.
- Up to seven workers run at the same time. In shared files (`world.rs`, `command.rs`, `timer.rs`, `outbox.rs`, `player.rs`, `pvp.rs`, `zones.rs`, `dispatch.rs`, `client/enter.rs`, `persist.rs`) only add your lines: no reformatting or moving code.
- Use `-j 4` and CARGO_INCREMENTAL=0. Before your final checks run `git merge main` and resolve conflicts keeping both sides; make sure no test crate fails to compile.
- DB tests use `l2_db::GAME_SCHEMA` (every `crates/l2-db/sql/game/*.sql`); add a schema copy there for every new table.
