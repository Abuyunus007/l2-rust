# M3-MOVE-01: Movement with geodata and pathfinding

| Field | Value |
|---|---|
| Milestone | M3 |
| Model | sonnet |
| Review | full |
| Reference | no |
| Depends on | M3-WREG-01, M3-GEO-02, M3-GPROTO-05 |
| Size | L |

## Goal
Player movement as Java: client MoveToLocation -> checks (geo canMoveToTarget/getValidLocation, pathfinding via the worker pool when Java would) -> per-tick updatePosition -> MoveToLocation broadcast, StopMove, CannotMoveAnymore, ValidatePosition corrections, region switches while moving.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\Creature.java (moveToLocation, updatePosition, stopMove, isOnGeodataPath...)`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\taskmanagers\MovementTaskManager.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\clientpackets\{MoveToLocation,ValidatePosition,CannotMoveAnymore}.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\ai\CreatureAI.java + PlayerAI.java (only the move intention path)`

## Reference implementation to copy
- crates/l2-world (M3-WREG-01)

## Files
- `crates/l2-world/src/movement.rs`
- `world.rs/command.rs changes (PathFound command)`
- `crates/game-server (path pool wiring)`
- `crates/l2-world/tests/movement.rs`

## Requirements
1. Java's formulas and update interval; same float/int behaviour.
2. Pathfinding requests never block the tick (D-14); stale results are dropped.
3. Speeds from the player's stats as far as M3 models them (base run/walk speed; buffs are M5).

## Out of scope
- NPC movement/AI (M4).

## Tests (must exist and pass)
- Step-by-step movement vs Java numbers, geo-blocked moves, path following, validate-position corrections.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass
- [ ] committed as `M3-MOVE-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read `docs/design/m3-world.md` (M3 rules) and `docs/design/world-loop.md` first. Then the CLAUDE.md safety rules (never touch a local database or real ports).
- The owner's real Java GS and DB run side by side with our servers (D-39): anything written to the DB must look exactly like Java writes it.
