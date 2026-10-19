# M5-ITEM-01: Item handlers: potions, herbs, scrolls, elixirs, extractables

| Field | Value |
|---|---|
| Milestone | M5 |
| Model | sonnet |
| Review | full |
| Reference | no (first item handler table; follow the effect registry) |
| Depends on | M5-CAST-01, M5-EFX-01 |
| Size | L |

## Goal
`UseItem` for non-equippable items through an item-handler registry: ItemSkills/ItemSkillsTemplate (potions, scrolls, herbs: herbs now apply their skill instead of vanishing — replace M4-DROP-01's herb skip), Elixir, ExtractableItems (parse `<capsuled_items>`, which M4-DATA-10 left unparsed), shared reuse groups (`ExUseSharedGroupItem`), item skill reuse, Book/Maps/Calculator only if trivial.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\items\{ItemSkills,ItemSkillsTemplate,Elixir,ExtractableItems,Book,Maps,Calculator}.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\clientpackets\UseItem.java (handler branch)`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\item\EtcItem.java / ItemTemplate (handler names, capsuled items)`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\util\DocumentItem.java (capsuled_items)`

## Files
- `crates/l2-world/src/inventory/handlers/*.rs (new)`
- `crates/l2-world/src/inventory/use_item.rs, ground_item/pickup.rs (herbs)`
- `crates/l2-data/src/item.rs + item/* (capsuled items)`
- `crates/l2-world/tests/item_handlers.rs (new)`

## Requirements
1. Consumption, reuse, messages and packets as Java.
2. Herbs on pickup cast their skill like Java.

## Out of scope
- Soulshots (SHOT-01), pet items, summon items (M6), enchant scrolls (later).

## Tests (must exist and pass)
- HP potion heals over time, herb buff on pickup, scroll of escape, an extractable box.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`)
- [ ] committed as `M5-ITEM-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read CLAUDE.md (safety rules: no local DB, no real ports, never kill processes by name), `docs/design/m4-combat.md`, `docs/design/m5-skills.md` first.
- The orchestrator refines this card when its dependencies are merged (exact module names and APIs to call).
