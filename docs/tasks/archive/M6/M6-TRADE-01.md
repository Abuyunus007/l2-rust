# M6-TRADE-01: Private trade

| Field | Value |
|---|---|
| Milestone | M6 |
| Model | sonnet |
| Review | full |
| Reference | no (copy the merged M4/M5 code around it) |
| Depends on | GPROTO-09 |
| Size | M |

## Goal
Player-to-player trade: request/answer, both trade lists, lock/confirm, the item transfer (stack merge, new object ids for split stacks from the Rust range, D-39), weight/slot checks, cancel on move/death/logout, messages exactly as Java, rows written like Java.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\TradeList.java (or grep TradeList)`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\clientpackets\{TradeRequest,AnswerTradeRequest,AddTradeItem,TradeDone}.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\Player.java (trade parts)`

## Files
- `crates/l2-world/src/trade/*.rs (new)`
- `crates/l2-world/src/inventory/* (transfer helper)`
- `crates/game-server/src/client/enter.rs`
- `tests`

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
- [ ] committed as `M6-TRADE-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read CLAUDE.md (safety rules), `docs/design/m4-combat.md`, `docs/design/m5-skills.md`, `docs/design/m6-social.md` first.
- Packets are merged (GPROTO-09): client `TradeRequest`, `AnswerTradeRequest`, `AddTradeItem`, `TradeDone` (`l2_protocol_game::client::trade`); server `SendTradeRequest`, `TradeStart`, `TradeOwnAdd`, `TradeOtherAdd`, `TradeUpdate`, `TradeOtherDone`, `TradeDone` (`server::trade`). Dispatch arms exist; route them in `enter.rs`.
- Write the transfer helper so STORE-01 (private stores, next card) can reuse it: `TradeList` with `TradeItem`s, `transfer_items(from, to, list)` doing Java's `TradeList.validate`/`doExchange` checks.
- Split stacks take new object ids from the Rust id range (D-39); item rows written like Java's `Item.updateDatabase` (owner_id/count/loc changes) through the existing persistence queue.
- Cancel on move/teleport/death/logout/attack exactly where Java calls `cancelActiveTrade`.
- PVP-01 and PARTY-01 run at the same time; touch `leave.rs`/`death.rs` only to add the cancel call.
