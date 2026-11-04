# M3-WREG-01: World regions, visibility, object tables

| Field | Value |
|---|---|
| Milestone | M3 |
| Model | opus (orchestrator) |
| Review | full |
| Reference | yes (every M3 world card builds on it) |
| Depends on | M2-ENTER-01, M3-GPROTO-05 |
| Size | L |

## Goal
Port `World`/`WorldRegion`: the 3D region grid, object tables (players, npcs, doors), add/remove/switch-region with Java's visibility packets, active regions, broadcast helpers; players entering the world are placed into regions.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\{World,WorldRegion,WorldObject}.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\Creature.java (setXYZ/region switch call sites)`

## Reference implementation to copy
- crates/l2-world (M2)

## Files
- `crates/l2-world/src/{region.rs,objects.rs,broadcast.rs}`
- `world.rs/lib.rs changes`
- `crates/l2-world/tests/regions.rs`

## Requirements
1. Exactly the packets and recipients of Java for add/remove/switch.
2. Broadcast helpers are the only cross-player send path.

## Out of scope
- Movement (MOVE-01), NPC spawning (NPC-01).

## Tests (must exist and pass)
- Region math vs Java constants, visibility with 2-3 fake players across region borders.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass
- [ ] committed as `M3-WREG-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read `docs/design/m3-world.md` (M3 rules) and `docs/design/world-loop.md` first. Then the CLAUDE.md safety rules (never touch a local database or real ports).
- The owner's real Java GS and DB run side by side with our servers (D-39): anything written to the DB must look exactly like Java writes it.
