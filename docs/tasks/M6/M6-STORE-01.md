# M6-STORE-01: Private stores (sell, buy, package sell)

| Field | Value |
|---|---|
| Milestone | M6 |
| Model | sonnet |
| Review | full |
| Reference | no (copy the merged M4/M5 code around it) |
| Depends on | TRADE-01 |
| Size | M |

## Goal
Sit-down private stores: sell/buy/package-sell lists, store messages, the buyer side (`RequestPrivateStoreBuy`/`Sell`), price and adena checks, `PrivateStoreMsg*` broadcast, store mode in CharInfo, items/adena moved like Java. Offline trade only if trivial (config `OfflineTradeEnable`), else a marker.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\clientpackets\{SetPrivateStoreList*,RequestPrivateStore*}.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\Player.java (private store parts)`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\TradeList.java`

## Files
- `crates/l2-world/src/trade/store.rs (new)`
- `crates/l2-world/src/player_view.rs (store mode)`
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
- [ ] committed as `M6-STORE-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read CLAUDE.md (safety rules), `docs/design/m4-combat.md`, `docs/design/m5-skills.md`, `docs/design/m6-social.md` first.
- The orchestrator refines this card when its dependencies are merged (exact module names and APIs to call).
