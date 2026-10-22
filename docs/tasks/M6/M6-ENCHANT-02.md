# M6-ENCHANT-02: Skill enchanting, enchant options, //enchant

| Field | Value |
|---|---|
| Milestone | M6 |
| Model | sonnet |
| Review | full |
| Reference | no (copy the merged M4/M5 code around it) |
| Depends on | ENCHANT-01 |
| Size | M |

## Goal
Port the packets this card needs yourself in `crates/l2-protocol-game/src/{client,server}/<area>.rs` with round-trip tests, dispatch arms and proxy decoders. Skill enchanting (`EnchantSkillGroupsData`, `RequestExEnchantSkillInfo`, `RequestExEnchantSkillInfoDetail`, `RequestExEnchantSkill` (normal, safe, untrain, change route), `ExEnchantSkillList`, `ExEnchantSkillInfo`, `ExEnchantSkillInfoDetail`, `ExEnchantSkillResult`, the enchant skill book and sp/adena costs, the `character_skills` rows of enchanted levels as Java), enchant options (`EnchantItemOptionsData`, `applyEnchantStats` on equip, the stats note in `stats/skills.rs`), and the admin `//enchant` pages (`AdminEnchant`).

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\data\xml\{EnchantSkillGroupsData,EnchantItemOptionsData}.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\clientpackets\{RequestExEnchantSkill,RequestExEnchantSkillInfo,RequestExEnchantSkillInfoDetail,RequestExEnchantSkillSafe,RequestExEnchantSkillUntrain,RequestExEnchantSkillRouteChange}.java (grep)`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\admincommandhandlers\AdminEnchant.java`

## Files
- `crates/l2-data/src/enchant_skill.rs (new)`
- `crates/l2-world/src/enchant/skill.rs (new)`
- `crates/l2-world/src/admin/enchant.rs (new)`
- `crates/l2-protocol-game/src/{client,server}/enchant_skill.rs (new)`
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
- [ ] committed as `M6-ENCHANT-02: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read CLAUDE.md (safety rules), `docs/design/m4-combat.md`, `docs/design/m5-skills.md`, `docs/design/m6-social.md` first.
- Merged: `enchant/` (ENCHANT-01), `skills/learn.rs` (skill learning pattern, M5-LEARN-01), `admin/` (`admin/html.rs` pages). Other workers run at the same time: in shared files only add your lines. Use `-j 4` and CARGO_INCREMENTAL=0.
