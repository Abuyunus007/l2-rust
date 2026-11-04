# M3-NPC-01: NPC spawns in the world (no AI)

| Field | Value |
|---|---|
| Milestone | M3 |
| Model | sonnet |
| Review | full |
| Reference | no |
| Depends on | M3-WREG-01, M3-DATA-05, M3-DATA-06, M3-GPROTO-05 |
| Size | M |

## Goal
Spawn every NPC Java spawns at startup (SpawnData groups, territories, counts, heading, geo Z via the engine), into regions with NpcInfo to nearby players; despawn/delete; the server log line counts like Java.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\data\xml\SpawnData.java, C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\data\SpawnTable.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\Spawn.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\Npc.java (onSpawn, info packets)`

## Reference implementation to copy
- crates/l2-world (M3-WREG-01)

## Files
- `crates/l2-world/src/npc.rs`
- `world.rs changes`
- `crates/game-server startup wiring`
- `crates/l2-world/tests/npcs.rs`

## Requirements
1. Same NPCs at the same places as Java (deterministic parts); random territory points via the world RNG.
2. Startup time and memory for all spawns reported.

## Out of scope
- AI, respawn after death, attack (M4).

## Tests (must exist and pass)
- Spawn counts per group vs Java's log, NpcInfo on entering a region.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass
- [ ] committed as `M3-NPC-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read `docs/design/m3-world.md` (M3 rules) and `docs/design/world-loop.md` first. Then the CLAUDE.md safety rules (never touch a local database or real ports).
- The owner's real Java GS and DB run side by side with our servers (D-39): anything written to the DB must look exactly like Java writes it.
