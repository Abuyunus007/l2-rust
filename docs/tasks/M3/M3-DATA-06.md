# M3-DATA-06: Spawn data

| Field | Value |
|---|---|
| Milestone | M3 |
| Model | haiku |
| Review | spot |
| Reference | no (copy M3-DATA-05 / M2-DATA-02) |
| Depends on | none |
| Size | S |

## Goal
Loader for `data/spawns/**/*.xml`: spawn groups, territories (polygons with min/max z, banned territories), npc entries with count, respawn delay/random, chase range, fixed locations, spawn names and parameters.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\data\xml\SpawnData.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\data\SpawnTable.java`
- `C:\l2-rust\l2jmobius\dist\game\data\spawns\**`
- `C:\l2-rust\l2jmobius\dist\game\data\xsd\spawns.xsd`

## Reference implementation to copy
- crates/l2-data/src/initial_equipment.rs

## Files
- `crates/l2-data/src/spawn.rs`
- `crates/l2-data/src/lib.rs lines`
- `crates/l2-data/tests/spawns.rs`

## Requirements
1. Java's counts; territory point selection (random point in polygon with Java's algorithm and rejection rules) as a function that takes the world RNG, so the world can call it.
2. Spawns that Java only enables via events/quests/AI keep their flags; the world decides later.

## Out of scope
- Spawning into the world (M3-NPC-01).

## Tests (must exist and pass)
- Real data counts, polygon point tests with a fixed seed.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass
- [ ] committed as `M3-DATA-06: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read `docs/design/m3-world.md` (M3 rules) and `docs/design/world-loop.md` first. Then the CLAUDE.md safety rules (never touch a local database or real ports).
- The owner's real Java GS and DB run side by side with our servers (D-39): anything written to the DB must look exactly like Java writes it.
