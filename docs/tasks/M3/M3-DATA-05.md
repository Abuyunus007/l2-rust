# M3-DATA-05: NPC templates

| Field | Value |
|---|---|
| Milestone | M3 |
| Model | sonnet |
| Review | full |
| Reference | no (copy M2-DATA-04) |
| Depends on | none |
| Size | M |

## Goal
Loader for `data/stats/npcs/*.xml` (+ custom when enabled): everything NpcInfo, spawning and dialogs need now (id, display id, name/title + server-side flags, type/instance class, level, race, sex, collision, speeds, base stats, weapons in hand, aggro/clan flags kept raw, `canBeAttacked`/`targetable`/`talkable`/`showName` style flags, teleport/html related parameters).

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\data\xml\NpcData.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\templates\{NpcTemplate,CreatureTemplate}.java`
- `C:\l2-rust\l2jmobius\dist\game\data\stats\npcs\*.xml`
- `C:\l2-rust\l2jmobius\dist\game\data\xsd\npcs.xsd`

## Reference implementation to copy
- crates/l2-data/src/item.rs (DocumentItem-style loader with Java parsing rules)

## Files
- `crates/l2-data/src/npc.rs (+ submodules)`
- `crates/l2-data/src/lib.rs lines`
- `crates/l2-data/tests/npcs.rs`

## Requirements
1. Java's counts and parsing rules (StatSet getters, booleans, enums exact-case); skills/drops/AI parameters are parsed tolerantly and kept raw or skipped (M4/M5) — say which.
2. Custom NPCs (CustomNpcData / custom folder) behind the same config flag as Java.

## Out of scope
- Drops, skills, AI (M4/M5).

## Tests (must exist and pass)
- Real datapack counts (skip if missing), spot checks of 10 NPCs incl. a gatekeeper, a merchant, a monster, a raid boss.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass
- [ ] committed as `M3-DATA-05: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read `docs/design/m3-world.md` (M3 rules) and `docs/design/world-loop.md` first. Then the CLAUDE.md safety rules (never touch a local database or real ports).
- The owner's real Java GS and DB run side by side with our servers (D-39): anything written to the DB must look exactly like Java writes it.
