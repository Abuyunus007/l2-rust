# M4-DATA-12: Armor sets, enchant HP bonus, hit condition bonus, hennas

| Field | Value |
|---|---|
| Milestone | M4 |
| Model | haiku |
| Review | spot |
| Reference | no (copy `crates/l2-data/src/experience.rs` / `player_template.rs` loaders) |
| Depends on | none |
| Size | S |

## Goal
Four small loaders the stat engine reads.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\data\xml\ArmorSetData.java` + `entity\item\holders\ArmorSet.java` (or wherever `ArmorSet` lives: grep)
- `...\data\xml\EnchantItemHPBonusData.java`
- `...\data\xml\HitConditionBonusData.java`
- `...\data\xml\HennaData.java` + `entity\item\Henna.java`
- Data: `C:\l2-rust\l2jmobius\dist\game\data\stats\{armorsets\*.xml,enchantHPBonus.xml,hitConditionBonus.xml,hennaList.xml}` and the xsd files in `dist\game\data\xsd`

## Files
- `crates/l2-data/src/armor_set.rs`, `enchant_hp_bonus.rs`, `hit_condition_bonus.rs`, `henna.rs` (new)
- `crates/l2-data/src/lib.rs` (module lines, fields on `Datapack`, load calls)
- `crates/l2-data/tests/small_stats.rs` (new)

## Requirements
1. Same parsing rules, defaults and log lines (`Loaded N armor sets.` etc.) as Java.
2. Armor sets: every piece list (chest, legs, head, gloves, feet, shield), skills, shield skills, enchant6 skills, the six stat bonuses; Java's lookup by chest id.
3. Enchant HP bonus: per crystal grade list; Java's `getHPBonus(item)` logic as a function taking (grade, enchant level, is full-body armor) with Java's bounds handling.
4. Hit condition bonus: the seven values, Java defaults when missing.
5. Hennas: dye id, item id, level, the six stat values, wear/cancel count+fee, allowed classes.

## Out of scope
- Applying any of it (M4-STAT-01).

## Tests (must exist and pass)
- Real datapack counts + spot checks (skip if missing); small inline XML tests for defaults and bounds.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`)
- [ ] committed as `M4-DATA-12: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read `docs/design/m4-combat.md` first, then CLAUDE.md safety rules.
- Another worker edits `crates/l2-data/src/item.rs` and adds `stats/`; don't touch those. Keep your `lib.rs` changes to added lines.
