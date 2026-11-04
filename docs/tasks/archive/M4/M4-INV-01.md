# M4-INV-01: Inventory and equipping

| Field | Value |
|---|---|
| Milestone | M4 |
| Model | sonnet |
| Review | full |
| Reference | no (first instance; follow M4-STAT-01's hooks) |
| Depends on | M4-STAT-01, M4-GPROTO-07 |
| Size | L |

## Goal
Equip, unequip, use, drop to the ground (hand-off to DROP-01), destroy items like Java: paperdoll rules, two-handed/shield/bow-arrow rules, armor sets and item skills, weight and slot limits, `InventoryUpdate`/`ItemList`/`UserInfo` updates, and the DB rows (`items.loc`, `loc_data`, `count`) written through the persistence outbox exactly as Java writes them.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\clientpackets\{UseItem,RequestUnEquipItem,RequestDestroyItem,RequestItemList,RequestDropItem}.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\item\container\{Inventory,PlayerInventory,ItemContainer}.java (paperdoll, listeners: StatsListener, ArmorSetListener, ItemSkillsListener, BowCrossRodListener)`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\item\instance\Item.java (`updateDatabase`, `setItemLocation`)`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\Player.java (`useEquippableItem`, `refreshOverloaded`, `refreshExpertisePenalty`, `getMaxLoad`)`
- dist: game\data\scripts\handlers\itemhandlers\* (only to know which items are not equippable; item handlers themselves are M5)

## Files
- `crates/l2-world/src/inventory/*.rs (new; may absorb `item_ops.rs`)`
- `crates/l2-world/src/{player.rs,world.rs,lib.rs,command.rs} (wire-in)`
- `crates/game-server/src/client/* (dispatch of the new client packets)`
- `crates/game-server/src/persist.rs (item row writes)`
- `crates/l2-world/tests/inventory.rs (new)`

## Requirements
1. Every equip/unequip path of `Inventory.setPaperdollItem` incl. the listeners' stat/skill effects (calls STAT-01's `on_equip`/`on_unequip`).
2. Java's checks and system messages (`SystemMessageId` names as in Java), expertise and weight penalties (`refreshExpertisePenalty`, `refreshOverloaded`).
3. `InventoryUpdate` / `UserInfo` / `CharInfo` broadcast exactly as Java (who gets what, in which order).
4. DB: same row changes as Java (D-39); the Java GS must see the same paperdoll after a switch of servers.

## Out of scope
- Item handlers (potions, scrolls, soulshots: M5), enchanting, augments, pets.

## Tests (must exist and pass)
- Equip/unequip of weapon, two-hander vs shield, full armor vs legs, jewels, armor set bonus on/off, weight limit; packet order checks; DB rows (DB test skips without `L2_TEST_DATABASE_URL`).

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`)
- [ ] committed as `M4-INV-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read `docs/design/m4-combat.md`, `docs/design/world-loop.md`, `docs/design/m3-world.md` first, then CLAUDE.md safety rules.
- The stat engine is merged (M4-STAT-01, `crates/l2-world/src/stats/`): read stats through `world.stats(id)` (a read-only `StatView`: `p_atk(target)`, `p_def(target)`, `accuracy()`, `evasion_rate(target)`, `critical_hit(..)`, `p_atk_spd()`, `max_hp()`, ... named after the Java getters; grep the module). Equipment changes call `stats::on_equip` / `stats::on_unequip`; skills go through `stats::skills::add_skill` / `remove_skill`; `refresh_overloaded` and `refresh_expertise_penalty` exist; `refresh_max_vitals` keeps `core.max_*` in sync.
- The orchestrator refines this card when its dependencies are merged (exact module names and APIs to call).
