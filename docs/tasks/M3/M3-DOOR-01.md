# M3-DOOR-01: Doors and fences in the world

| Field | Value |
|---|---|
| Milestone | M3 |
| Model | sonnet |
| Review | full |
| Reference | no |
| Depends on | M3-WREG-01, M3-DATA-09, M3-GEO-01, M3-GPROTO-06 |
| Size | M |

## Goal
Doors/fences/static objects as world objects: spawned from data, visible (DoorInfo/DoorStatusUpdate/StaticObjectInfo), open/close state owned by the world, the geoengine's `DynamicObstacles` implemented over it (and the snapshot for path workers).

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\instance\Door.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\data\xml\DoorData.java (spawn part)`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\geoengine\GeoEngine.java (door/fence checks)`

## Reference implementation to copy
- crates/l2-world (M3-WREG-01), crates/l2-geo DynamicObstacles

## Files
- `crates/l2-world/src/door.rs`
- `world.rs changes`
- `crates/l2-world/tests/doors.rs`

## Requirements
1. Closed doors block movement and line of sight exactly like Java.
2. Default open/close states as Java at startup.

## Out of scope
- Door AI/timers, castle doors in sieges.

## Tests (must exist and pass)
- Blocked/unblocked moves through a door, snapshot for workers.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass
- [ ] committed as `M3-DOOR-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read `docs/design/m3-world.md` (M3 rules) and `docs/design/world-loop.md` first. Then the CLAUDE.md safety rules (never touch a local database or real ports).
- The owner's real Java GS and DB run side by side with our servers (D-39): anything written to the DB must look exactly like Java writes it.
