# M4-DROP-01: Drops, items on the ground, pickup

| Field | Value |
|---|---|
| Milestone | M4 |
| Model | sonnet |
| Review | full |
| Reference | no |
| Depends on | M4-DEATH-01, M4-INV-01 |
| Size | L |

## Goal
`Attackable.doItemDrop` with `Rates.ini` (drop chance/amount, adena, raid/grand boss rates, item multipliers), grouped drop lists, `Item.dropMe`/`SpawnItem`/`DropItem`, items on the ground as world objects (visibility, regions), pickup (`Action` on an item, `doPickupItem`, owner protection), `AutoDestroyDroppedItemAfter`, and `SaveDroppedItem` & co. of `General.ini` exactly as Java (default off: memory only).

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\Attackable.java (`doItemDrop`, `dropItem`)`
- (drop holders: grep `class DropHolder`, `DropGroupHolder`)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\item\instance\Item.java (`dropMe`, `pickupMe`)`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\Player.java (`doPickupItem`, `addItem`)`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\taskmanagers\ItemsAutoDestroyTaskManager.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\managers\ItemsOnGroundManager.java (only what `SaveDroppedItem` needs)`

## Files
- `crates/l2-world/src/ground_item.rs, rewards/drop.rs (new)`
- `crates/l2-world/src/{visibility.rs,objects.rs,region.rs,interact.rs,world.rs,lib.rs} (wire-in)`
- `crates/game-server/src/persist.rs (item rows; `itemsonground` only when enabled)`
- `crates/l2-world/tests/drops.rs (new)`

## Requirements
1. Seeded RNG; chances computed like Java (incl. integer/double quirks).
2. Ground items use ids from the Rust range (D-39).

## Out of scope
- Spoil/sweep (M5: needs the skill), pet pickup.

## Tests (must exist and pass)
- Seeded drop rolls per list type; pickup into inventory; auto-destroy timer; visibility to a second player.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`)
- [ ] committed as `M4-DROP-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read `docs/design/m4-combat.md`, `docs/design/world-loop.md`, `docs/design/m3-world.md` first, then CLAUDE.md safety rules.
- The orchestrator refines this card when its dependencies are merged (exact module names and APIs to call).
