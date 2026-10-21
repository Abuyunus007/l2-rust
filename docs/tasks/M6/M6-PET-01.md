# M6-PET-01: Pets

| Field | Value |
|---|---|
| Milestone | M6 |
| Model | sonnet |
| Review | full |
| Reference | no (copy the merged M4/M5 code around it) |
| Depends on | SUMMON-01 |
| Size | L |

## Goal
Pets on top of the servitor core: pet data XML (`PetDataTable`), pet collars (`SummonItems`), pet inventory and items (`RequestPetGetItem`, `RequestGiveItemToPet`/`GetItemFromPet`, `PetItemList`), feeding and hunger, pet levels/exp, the `pets` table rows (D-39), beast shots (`BeastSoulShot`/`BeastSpiritShot`).

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\data\xml\PetDataTable.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\instance\Pet.java`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\items\{SummonItems,PetFood,BeastSoulShot,BeastSpiritShot}.java`

## Files
- `crates/l2-data/src/pet.rs (new)`
- `crates/l2-world/src/summon/pet.rs`
- `crates/l2-db (pets repository)`
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
- [ ] committed as `M6-PET-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read CLAUDE.md (safety rules), `docs/design/m4-combat.md`, `docs/design/m5-skills.md`, `docs/design/m6-social.md` first.
- SUMMON-01 is merged: `crates/l2-world/src/summon/` (`SummonWorld`, `SummonKind { Servitor, Pet }`, `ai.rs`, `actions.rs`, `info.rs`, `spawn.rs`, `shots.rs` with the beast shots), summon creatures in `World::npcs` with `NpcClass::Summon`. Pet item/name client packets currently go to the not-ported path: route them to your code.
- Markers for you: PcBody dead-pet target (`skills/targets.rs`), `effects/revive.rs` (pet resurrection), pet inventory and food. `character_summons`/`pets` rows exactly as Java (D-39).
- Item moves (`RequestGiveItemToPet`/`GetItemFromPet`) use `World::transfer_item` (`inventory/transfer.rs`).
- Up to seven M6 workers run at the same time. In shared files (`skills/targets.rs`, `enter.rs`, `player_view.rs`, `leave.rs`, `death.rs`, `world.rs`, `dispatch.rs`, `client/enter.rs`) only add your lines: no reformatting or moving code.
- Use `-j 4` for cargo clippy/test (parallel linking crashed under load) and CARGO_INCREMENTAL=0.
