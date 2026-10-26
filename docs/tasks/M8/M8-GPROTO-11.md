# M8-GPROTO-11: Olympiad, hero, Seven Signs, instance, vehicle, mini game packets

| Field | Value |
|---|---|
| Milestone | M8 |
| Model | haiku |
| Review | spot |
| Reference | no (copy the merged M0..M7 code around it) |
| Depends on | none |
| Size | L |

## Goal
All client and server packets M8 needs, with round-trip tests, dispatch arms and proxy decoders: `ExOlympiad*`, `ExReceiveOlympiad`, `ExHeroList`, `RequestOlympiadMatchList`, `RequestExOlympiadMatchListRefresh`, `RequestOlympiadObserverEnd`, `ObserverReturn`, `RequestWriteHeroWords`, `SSQInfo`, `SSQStatus`, `RequestSSQStatus`, `ExShowSevenSignsMapInfo`, `ExSendUIEvent`, `VehicleInfo`, `VehicleDeparture`, `VehicleStarted`, `VehicleCheckLocation`, `MoveToLocationInVehicle`, `StopMoveInVehicle`, `GetOnVehicle`, `GetOffVehicle`, `RequestGetOnVehicle`, `RequestGetOffVehicle`, `RequestMoveToLocationInVehicle`, `CannotMoveAnymoreInVehicle`, `ExAirShip*`, `ExGetOnAirShip`, `MoveToLocationAirShip`, `MoveToLocationInAirShip`, `ExCubeGame*`, `RequestExCubeGameChangeTeam`, `RequestExCubeGameReadyAnswer`, `ExCursedWeaponList`, `ExCursedWeaponLocation`, `RequestCursedWeaponList`, `RequestCursedWeaponLocation`, `RequestGetBossRecord`, `ExGetBossRecord`, `RequestPVPMatchRecord`, `ExPVPMatchRecord`, `ExPVPMatchUserDie`, `RequestSeedPhase`, `ExShowSeedMapInfo`, instance packets (`ExShowScreenMessage` exists?), `ExUC*`/Underground Coliseum packets, `ExKrateiCube*` if any, `EndScenePlayer`, `ExStartScenePlayer`.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\clientpackets\ (the names above)`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\serverpackets\ (the names above)`

## Files
- `crates/l2-protocol-game/src/{client,server}/{olympiad,ssq,vehicle,minigame,boss}.rs` (new) + mod/dispatch
- `crates/l2-proxy/src/game_fields.rs`
- tests

## Requirements
1. Java behaviour, messages, packets, HTML and DB rows exactly (D-39: the Java GS shares the DB; D-49..D-53 for global writes and fresh reads).
2. Single-owner world, ids not references, timers on the wheel, world RNG; no unwrap/expect on external data; no DB call or await in a tick (D-15: request/answer through the outbox).
3. Port the packets you need yourself in `crates/l2-protocol-game/src/{client,server}/<area>.rs` with round-trip tests, dispatch arms and proxy decoders (unless M8-GPROTO-11 already has them: check main first).
4. Small wire-ins in shared files are allowed: add your lines only. List every marker you resolve and every one you leave.

## Out of scope
- Quests, AI scripts, instance/event scripts, community board (the script milestone, D-05); customs, premium, item mall (M9).

## Tests (must exist and pass)
- Round trip and a Java-layout byte test per packet.

## Done when
- [ ] fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`, `-j 4`); `git merge main` before the final checks
- [ ] committed as `M8-GPROTO-11: <summary>`
- [ ] report delivered in the agent's report format (what you left and why, every guess)

## Notes from the orchestrator
- `game_server::cycle_database_writes(config_dir)` reads the D-53 switch (already on main); pass it into your settings, do not add a second loader.
- Read CLAUDE.md (safety rules), DECISIONS (D-15, D-39, D-49..D-53), the `docs/design/*.md` of the area you touch first.
- Up to seven workers run at the same time: in shared files (`lib.rs`, `world.rs`, `command.rs`, `outbox.rs`, `timer.rs`, `dispatch.rs`, `persist.rs`, `world_data.rs`) only add your lines; after `git merge main` watch for doubled `mod`/`use` lines.
- DB tests use `l2_db::GAME_SCHEMA`; add a schema copy to `crates/l2-db/sql/game/` for every new table.
