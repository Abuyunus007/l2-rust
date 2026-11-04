# M6-AUGMENT-01: Augmentation

| Field | Value |
|---|---|
| Milestone | M6 |
| Model | sonnet |
| Review | full |
| Reference | no (copy the merged M4/M5 code around it) |
| Depends on | none |
| Size | M |

## Goal
Port the packets this card needs yourself in `crates/l2-protocol-game/src/{client,server}/<area>.rs` with round-trip tests, dispatch arms and proxy decoders (layouts from the Java packet classes; `C:\l2-rust\PACKET_DOCS\` is the index). `AugmentationData`/`OptionData`, life stones and gemstones (`RequestConfirmTargetItem`, `RequestConfirmRefinerItem`, `RequestConfirmGemStone`, `RequestRefine`, `RequestConfirmCancelItem`, `RequestRefineCancel`), the `item_attributes` rows, augment options as stats and skills in the stat engine, augments in the item packets.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\data\xml\{AugmentationData,OptionData}.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\ (grep Augmentation)`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\clientpackets\{RequestRefine,RequestConfirmTargetItem,RequestConfirmRefinerItem,RequestConfirmGemStone,RequestRefineCancel}.java`

## Files
- `crates/l2-data/src/augment.rs (new)`
- `crates/l2-world/src/augment.rs (new)`
- `crates/l2-protocol-game/src/{client,server}/refine.rs (new)`
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
- [ ] committed as `M6-AUGMENT-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read CLAUDE.md (safety rules), `docs/design/m4-combat.md`, `docs/design/m5-skills.md`, `docs/design/m6-social.md` first.
- Up to seven M6 workers run at the same time. In shared files (`skills/targets.rs`, `enter.rs`, `player_view.rs`, `leave.rs`, `death.rs`, `world.rs`, `dispatch.rs`, `client/enter.rs`) only add your lines: no reformatting or moving code.
- Use `-j 4` for cargo clippy/test (parallel linking crashed under load) and CARGO_INCREMENTAL=0.
- Merged APIs to reuse: `trade/` (`TradeList`, `World::transfer_item` in `inventory/transfer.rs`, `check_item_manipulation`, the request state in `trade/request.rs`), `party/` (`World::party_of`), `clan/` (`ClanTable`, privileges), `pvp.rs`, `transform/`. Item rows go through the persistence outbox like TRADE-01 does.
