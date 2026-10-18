# M2-DATA-03: Skill trees (ids and levels only)

| Field | Value |
|---|---|
| Milestone | M2 |
| Model | haiku |
| Review | spot |
| Reference | no |
| Depends on | M2-DATA-01 |
| Size | S |

## Goal
Load the class skill trees so character creation can give the auto-learned level-1 skills (skill id + level only, no skill logic).

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\data\xml\SkillTreeData.java (classSkillTree, parent class chain, autoGet)`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\clientpackets\CharacterCreate.java (which skills it gives)`
- `C:\l2-rust\l2jmobius\dist\game\data\stats\players\skillTrees\**\*.xml`

## Reference implementation to copy
- crates/l2-data/src/player_template.rs

## Files
- `crates/l2-data/src/skill_tree.rs`
- `crates/l2-data/tests/skill_trees.rs`

## Requirements
1. Class skill tree with parent inheritance like Java; `auto_get_skills(class, level)` exactly as CharacterCreate/rewardSkills uses it.

## Out of scope
- Skill definitions/effects (M5).

## Tests (must exist and pass)
- A fighter and a mystic class get the same initial skills as Java (compare with a Java-created character in the owner's DB: character_skills, read by the orchestrator, not by you).

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass
- [ ] committed as `M2-DATA-03: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read `docs/design/world-loop.md` and the CLAUDE.md safety rules first (never touch a local database or real ports).
- The owner's real Java GS and DB run side by side with our servers (D-39): anything written to the DB must look exactly like Java writes it.
