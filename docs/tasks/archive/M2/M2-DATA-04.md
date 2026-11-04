# M2-DATA-04: Item templates (basic fields)

| Field | Value |
|---|---|
| Milestone | M2 |
| Model | sonnet |
| Review | full |
| Reference | no |
| Depends on | M2-DATA-01 |
| Size | M |

## Goal
Load all item templates with the fields needed for inventories, the character screen, UserInfo and ItemList (no stats/skills/conditions yet).

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\data\xml\ItemData.java, C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\util\DocumentItem.java, DocumentBase.java (how items are parsed)`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\item\ItemTemplate.java, Weapon.java, Armor.java, EtcItem.java`
- `C:\l2-rust\l2jmobius\dist\game\data\stats\items\*.xml (197 files)`

## Reference implementation to copy
- crates/l2-data/src/player_template.rs

## Files
- `crates/l2-data/src/item.rs (+ submodules if needed)`
- `crates/l2-data/tests/items.rs`

## Requirements
1. Per item: id, name, type (weapon/armor/etc + subtypes), body part (slot bitmask like Java), weight, price, stackable, crystal type/count, default action, is_questitem/tradeable/dropable/sellable flags, enchantable, durability (mana), plus anything ItemList/UserInfo/CharSelectionInfo write.
2. `<set>` handling and `<table>` references exactly like DocumentItem for these fields; stats/conditions/skills are skipped but parsed without errors.

## Out of scope
- Item stats, skills, conditions (M4/M5).

## Tests (must exist and pass)
- Java's item count; spot checks (Adena 57, a weapon, an armor, a stackable etc item).

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass
- [ ] committed as `M2-DATA-04: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read `docs/design/world-loop.md` and the CLAUDE.md safety rules first (never touch a local database or real ports).
- The owner's real Java GS and DB run side by side with our servers (D-39): anything written to the DB must look exactly like Java writes it.
