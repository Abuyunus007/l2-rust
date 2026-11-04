# M7-SIEGE-02: Door and wall damage, siege guard AIs

| Field | Value |
|---|---|
| Milestone | M7 |
| Model | opus |
| Review | full |
| Reference | no (copy the merged M4 combat and M7 siege code around it) |
| Depends on | SIEGE-01, FORT-01 |
| Size | L |

## Goal
Doors and walls as attackable creatures of the stat engine (Java `Door` extends `Creature`): door stats and HP from the door data, `Door.reduceCurrentHp` with the wall rule (walls only take siege-weapon/skill damage), AoE and skills on doors, `isAutoAttackable` for doors (castle, fortress, clan hall), door death ("The castle gate has been destroyed." etc.), the door status packets (`DoorStatusUpdate`, `StaticObject` HP), repair at siege start/end (SIEGE-01's `castle_spawn_door`, FORT-01's `fort_reset_doors`), door upgrades applied to HP (SIEGE-01's `castle_set_door_upgrade`). Siege guard AIs: Java `SiegeGuardAI` (castle) and `FortSiegeGuardAI` (fortress) on the M4 intention machine, including their target choice (attackers only, never defenders/clan), return home and the archer/mage behaviour.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\instance\Door.java`, `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\status\DoorStatus.java`, `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\stat\DoorStat.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\ai\{{SiegeGuardAI,FortSiegeGuardAI,DoorAI}}.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\serverpackets\{{DoorStatusUpdate,StaticObjectInfo}}.java` (grep)

## Files
- `crates/l2-world/src/door.rs` + `crates/l2-world/src/siege/doors.rs` + stat/combat wire-ins
- `crates/l2-world/src/ai/{{siege_guard,fort_siege_guard}}.rs` (new)
- `tests`

## Requirements
1. Java behaviour, messages, packets and DB rows exactly (D-39); global writes only with `SiegeDatabaseWrites` (D-51).
2. Single-owner world, ids not references, timers on the wheel, world RNG; no unwrap/expect on external data.
3. Resolve the `// M7-SIEGE-02` markers; list the ones you leave.

## Out of scope
- Olympiad, Seven Signs (M8); scripts beyond the guards (later).

## Tests (must exist and pass)
- A gate broken by attackers during a GM test siege (messages, packets, the door state), a wall that only siege damage hurts, a door repaired at siege end, guards attacking attackers and ignoring defenders.

## Done when
- [ ] fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`, `-j 4`); `git merge main` before the final checks
- [ ] committed as `M7-SIEGE-02: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read CLAUDE.md, DECISIONS (D-15, D-39, D-49..51), `docs/design/m7-residences.md` (the residence, fortress and siege APIs) first.
- Other M7 workers run at the same time (castle NPCs, conquerable halls, admin): in shared files only add your lines.
