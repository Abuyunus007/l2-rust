# M6-GPROTO-09: Party, PvP, trade, clan and summon packets

| Field | Value |
|---|---|
| Milestone | M6 |
| Model | haiku |
| Review | spot |
| Reference | no (copy the merged M4/M5 code around it) |
| Depends on | none |
| Size | L |

## Goal
All packets M6 needs (find them by name in the Java packet folders and in `C:\l2-rust\PACKET_DOCS\`): party (`RequestJoinParty`, `RequestAnswerJoinParty`, `RequestWithDrawalParty`, `RequestOustPartyMember`, `RequestChangePartyLeader`, `AskJoinParty`, `JoinParty`, `PartySmallWindowAll/Add/Delete/DeleteAll/Update`, `PartyMemberPosition`, `ExPartyPetWindow*`, `PartySpelled` exists), trade (`TradeRequest`, `AnswerTradeRequest`, `AddTradeItem`, `TradeDone`, `TradeStart`, `TradeOwnAdd`, `TradeOtherAdd`, `TradeOtherDone`, `TradeDone` server, `SendTradeRequest`), private stores (`SetPrivateStoreList*`, `PrivateStoreList*`, `PrivateStoreMsg*`, `RequestPrivateStore*`), pledge/clan/ally (`RequestJoinPledge`, `AskJoinPledge`, `RequestPledge*`, `PledgeShowMemberList*`, `PledgeInfo`, `PledgeStatusChanged`, `PledgeCrest`, `RequestSetPledgeCrest`, `AllianceInfo`, `RequestJoinAlly`, ...), summon/pet (`PetInfo`, `PetStatusShow`, `PetStatusUpdate`, `PetDelete`, `PetItemList`, `RequestPetGetItem`, `RequestActionUse` summon actions, ...). Byte layouts exactly as Java, round trips, fixture checks where recorded, dispatch + proxy decoders.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\clientpackets\*`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\serverpackets\*`

## Files
- `crates/l2-protocol-game/src/{client,server}/{party,trade,store,pledge,summon}.rs (new)`
- `dispatch.rs, mod.rs`
- `crates/l2-proxy/src/game_fields.rs`
- `crates/l2-protocol-game/tests/m6_packets.rs (new)`

## Requirements
1. Java behaviour, messages, packets and DB rows exactly (D-39: the Java GS shares the DB).
2. Single-owner world, ids not references, timers on the wheel, world RNG; no unwrap/expect on external data.
3. Resolve the `// M6:` markers in your area; list the ones you leave.

## Out of scope
- Sieges/castles/fortresses/clan halls (M7), Olympiad/Seven Signs (M8), quests (later).

## Tests (must exist and pass)
- Each part with Java's numbers/messages/packets; DB tests skip without `L2_TEST_DATABASE_URL`.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`)
- [ ] committed as `M6-GPROTO-09: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read CLAUDE.md (safety rules), `docs/design/m4-combat.md`, `docs/design/m5-skills.md`, `docs/design/m6-social.md` first.
- The orchestrator refines this card when its dependencies are merged (exact module names and APIs to call).
