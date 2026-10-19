# M4-DATA-10: Stat enum, function templates, conditions, item stat data

| Field | Value |
|---|---|
| Milestone | M4 |
| Model | sonnet |
| Review | full |
| Reference | yes (first instance; DATA-11 copies the condition parser) |
| Depends on | M2-DATA-04 (item templates) |
| Size | L |

## Goal
The data model of Java's stat system, parse only (no evaluation): the `Stat` enum, `FuncTemplate`,
the `Condition` tree, and on item templates the `<stats>`, `<skills>` and `<conditions>` that
`crates/l2-data/src/item.rs` skips today.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\mechanics\stats\Stat.java` (names, `valueOfXml`, order)
- `...\mechanics\stats\functions\{FuncTemplate,StatFunction,AbstractFunction}.java` (kinds + default orders)
- `...\mechanics\conditions\*.java` (one Rust variant per class that the parser can create; keep the constructor arguments as typed fields)
- `...\util\DocumentBase.java`: `parseCondition` .. `parseGameCondition` (lines ~688-1604), `attachFunc`, `processAlternativeFuncFormat`, `getValue` (`#table` references), `joinAnd`
- `...\util\DocumentItem.java` (the `stats`, `skills`, `conditions` branches and the automatic `enchant` funcs)
- `...\entity\item\ItemTemplate.java` (`attach`, item skills `item_skill`, `_unequipSkill`, `_skillsOnMagic`/`_skillsOnCrit` holders, `checkCondition`)
- Data: `C:\l2-rust\l2jmobius\dist\game\data\stats\items\*.xml`, skills folder for the condition vocabulary.

## Files
- `crates/l2-data/src/stats/mod.rs`, `stats/stat.rs`, `stats/func.rs`, `stats/condition.rs` (new)
- `crates/l2-data/src/item.rs` and `crates/l2-data/src/item/*` (attach the parsed parts to `ItemTemplate`)
- `crates/l2-data/src/lib.rs` (module lines, re-exports)
- `crates/l2-data/tests/item_stats.rs` (new)

## Requirements
1. `Stat`: every Java constant in order, `xml_name()`, `from_xml(&str)` with Java's matching (`valueOfXml` is case-sensitive? check and copy). `NUM_STATS` equivalent.
2. `FuncKind` (`Set, Add, Sub, Mul, Div, Share, Enchant, EnchantHp`) with Java's default order per kind (`StatFunction`), `FuncTemplate { stat, kind, order, value, attach_cond, apply_cond }`.
3. `Condition` tree: `And/Or/Not` plus one variant per leaf the parser can produce (player/target/using/game), typed fields from the constructor arguments, plus the message parts (`msg`, `msgId`, `addName`). Port the parser branch for every attribute Java accepts, with its warnings for unknown ones (same text). It must take a value resolver so `#table` values work: `parse_condition(node, &dyn Fn(&str) -> Option<String>)` (DATA-11 passes the per-level table lookup).
4. Item templates get `funcs: Vec<FuncTemplate>` (exact mapping of the `stats` switch incl. `Unhandled stat type` warning), the automatic `enchant` funcs for armor/weapons, `conditions` (with message), item skills per `ItemSkillType` (ids + levels only, they are resolved later), keeping Java's order of attachment.
5. No evaluation code. Conditions that need world state are data only.

## Out of scope
- Evaluating conditions or funcs (M4-STAT-01). Skill XML (M4-DATA-11).

## Tests (must exist and pass)
- Unit tests for `Stat::from_xml` round trip over all variants, every `FuncKind` order, condition parsing of each leaf kind used in the datapack (`and/or/not`, `player races/level/hp/weight/invSize/...`, `using kind/slot`, `target ...`, `game ...`).
- Real datapack (skip if missing): counts of items with funcs/conditions/skills; spot checks: Short Sword (id 1) funcs = set pAtk 8, set mAtk 6, set critRate 8, set pAtkSpd 379, add randomDamage 10, set pAtkRange 40 + enchant pAtk/mAtk; a shield, an accessory, an armor piece with conditions.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`)
- [ ] committed as `M4-DATA-10: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read `docs/design/m4-combat.md` first, then CLAUDE.md safety rules.
- Names: keep Java's (`ConditionPlayerLevel` -> `Condition::PlayerLevel { level }` style), so reviewers can compare.
