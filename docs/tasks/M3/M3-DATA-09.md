# M3-DATA-09: Doors and fences data

| Field | Value |
|---|---|
| Milestone | M3 |
| Model | haiku |
| Review | spot |
| Reference | no |
| Depends on | none |
| Size | S |

## Goal
Loaders for `data/Doors.xml` and `data/FenceData.xml` (+ StaticObjects.xml): door templates (id, position, nodes/polygon, open/close defaults, hp, targetable, open method, group), fences, static objects.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\data\xml\{DoorData,FenceData,StaticObjectData}.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\templates\DoorTemplate.java`
- `C:\l2-rust\l2jmobius\dist\game\data\{Doors,FenceData,StaticObjects}.xml`

## Reference implementation to copy
- crates/l2-data/src/initial_equipment.rs

## Files
- `crates/l2-data/src/{door.rs,fence.rs,static_object.rs}`
- `lib.rs lines`
- `crates/l2-data/tests/doors.rs`

## Requirements
1. Java counts and defaults.
2. The geometry fields the geoengine's door checks need (see M3-GEO-01 `DynamicObstacles`).

## Out of scope
- Doors in the world (M3-DOOR-01).

## Tests (must exist and pass)
- Counts, spot checks.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass
- [ ] committed as `M3-DATA-09: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read `docs/design/m3-world.md` (M3 rules) and `docs/design/world-loop.md` first. Then the CLAUDE.md safety rules (never touch a local database or real ports).
- The owner's real Java GS and DB run side by side with our servers (D-39): anything written to the DB must look exactly like Java writes it.
