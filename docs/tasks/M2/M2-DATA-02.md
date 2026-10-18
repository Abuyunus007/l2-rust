# M2-DATA-02: Experience table, initial equipment, initial shortcuts

| Field | Value |
|---|---|
| Milestone | M2 |
| Model | haiku |
| Review | spot |
| Reference | no |
| Depends on | M2-DATA-01 |
| Size | S |

## Goal
Loaders for experience.xml, initialEquipment.xml and initialShortcuts.xml.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\data\xml\ExperienceData.java, InitialEquipmentData.java, InitialShortcutData.java`
- `C:\l2-rust\l2jmobius\dist\game\data\stats\players\{experience,initialEquipment,initialShortcuts}.xml`

## Reference implementation to copy
- crates/l2-data/src/player_template.rs

## Files
- `crates/l2-data/src/{experience.rs,initial_equipment.rs,initial_shortcuts.rs}`
- `crates/l2-data/tests/players_misc.rs`

## Requirements
1. Same lookups as Java: exp for level, max level, percent helpers; equipment by class id (with the Java fallback rules); shortcuts per class + the general ones; macro shortcuts if present.

## Out of scope
- nothing beyond the files above

## Tests (must exist and pass)
- Real files load with Java counts; lookups for a few classes/levels.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass
- [ ] committed as `M2-DATA-02: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read `docs/design/world-loop.md` and the CLAUDE.md safety rules first (never touch a local database or real ports).
- The owner's real Java GS and DB run side by side with our servers (D-39): anything written to the DB must look exactly like Java writes it.
