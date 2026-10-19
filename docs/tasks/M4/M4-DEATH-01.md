# M4-DEATH-01: Death, respawn, corpses

| Field | Value |
|---|---|
| Milestone | M4 |
| Model | sonnet |
| Review | full |
| Reference | no (uses COMBAT-01's hooks) |
| Depends on | M4-COMBAT-01 |
| Size | L |

## Goal
`doDie` for players and NPCs: `Die` packet with restart-point flags, stop attack/move/regen, corpse decay (`DecayTaskManager`), NPC respawn (`Spawn.decreaseCount`, respawn delay + random, `RespawnTaskManager`), player `RequestRestartPoint` (to village via map regions, fixed, ...), `Revive`, death penalty hook (exp loss is EXP-01).

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\{Creature,Npc,Attackable,Player}.java (`doDie`, `onDecay`, `doRevive`)`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\Spawn.java (`decreaseCount`, `respawnNpc`, `doSpawn`)`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\taskmanagers\{DecayTaskManager,RespawnTaskManager}.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\clientpackets\RequestRestartPoint.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\serverpackets\{Die,Revive}.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\data\xml\MapRegionData.java (`getTeleToLocation`; already ported in M3)`

## Files
- `crates/l2-world/src/death.rs (new)`
- `crates/l2-world/src/{npc.rs,teleport.rs,world.rs,lib.rs,command.rs} (wire-in)`
- `crates/game-server/src/client/* (dispatch)`
- `crates/l2-world/tests/death.rs (new)`

## Requirements
1. Respawn timing exactly as Java (`respawnDelay`, `respawnRandom`), world RNG.
2. Packets/recipients as Java; a dead player cannot act (Java's checks).

## Out of scope
- Resurrection skills (M5), siege/fort/clan hall restart points beyond what map regions give.

## Tests (must exist and pass)
- NPC dies, decays, respawns at its spawn; player dies, restarts to village; packet checks.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`)
- [ ] committed as `M4-DEATH-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read `docs/design/m4-combat.md`, `docs/design/world-loop.md`, `docs/design/m3-world.md` first, then CLAUDE.md safety rules.
- The orchestrator refines this card when its dependencies are merged (exact module names and APIs to call).
