# M7-GPROTO-10: Siege, castle, fortress, manor, territory packets

| Field | Value |
|---|---|
| Milestone | M7 |
| Model | haiku |
| Review | spot |
| Reference | no (copy the merged M6 code around it) |
| Depends on | none |
| Size | L |

## Goal
All packets M7 needs (find them by name in the Java packet folders and in `C:\l2-rust\PACKET_DOCS\`): sieges (`SiegeInfo`, `SiegeAttackerList`, `SiegeDefenderList`, `RequestSiegeInfo`, `RequestSiegeAttackerList`, `RequestSiegeDefenderList`, `RequestJoinSiege`, `RequestConfirmSiegeWaitingList`, `ExShowCastleInfo`), fortresses (`ExShowFortressInfo`, `ExShowFortressSiegeInfo`, `ExShowFortressMapInfo`, `RequestAllFortressInfo`, `RequestFortressSiegeInfo`, `RequestFortressMapInfo`), manor (`ExShowManorDefaultInfo`, `ExShowSeedInfo`, `ExShowCropInfo`, `ExShowSeedSetting`, `ExShowCropSetting`, `ExShowProcureCropDetail`, `ExShowSellCropList`, `RequestManorList`, `RequestProcureCropList`, `RequestSetSeed`, `RequestSetCrop`, `RequestBuySeed`, `RequestProcureCrop`), territory war (`ExDominionWarStart`, `ExDominionWarEnd`, `ExShowDominionRegistry`, `RequestJoinDominionWar`, `RequestDominionInfo`, `ExReplyDominionInfo`, `ExDominionChannelSet`), clan hall auctions as Java has them. Byte layouts exactly as Java, round trips, dispatch arms and proxy decoders.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\clientpackets\*`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\serverpackets\*`

## Files
- `crates/l2-protocol-game/src/{client,server}/{siege,fort,manor,territory}.rs (new)`
- `dispatch.rs, mod.rs`
- `crates/l2-proxy/src/game_fields.rs`
- `crates/l2-protocol-game/tests/m7_packets.rs (new)`

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
- [ ] committed as `M7-GPROTO-10: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read CLAUDE.md (safety rules), `docs/DECISIONS.md` (D-39, D-49, D-50, D-51), `docs/design/m6-social.md` and `docs/design/m7-residences.md` first.
- Up to seven workers run at the same time. In shared files (`world.rs`, `command.rs`, `timer.rs`, `outbox.rs`, `player.rs`, `pvp.rs`, `zones.rs`, `dispatch.rs`, `client/enter.rs`, `persist.rs`) only add your lines: no reformatting or moving code.
- Use `-j 4` and CARGO_INCREMENTAL=0. Before your final checks run `git merge main` and resolve conflicts keeping both sides; make sure no test crate fails to compile.
- DB tests use `l2_db::GAME_SCHEMA` (every `crates/l2-db/sql/game/*.sql`); add a schema copy there for every new table.
