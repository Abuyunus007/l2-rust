# M4-COMBAT-01: Auto-attack (players and NPCs)

| Field | Value |
|---|---|
| Milestone | M4 |
| Model | opus |
| Review | full |
| Reference | yes (first instance) |
| Depends on | M4-STAT-01, M4-GPROTO-07 |
| Size | XL |

## Goal
Melee and bow auto-attack like Java: `AttackRequest`/`Action` on an attackable target, `CreatureAI` attack intention and move-to-pawn chase, `Creature.doAttack` (checks, timing from attack speed, `Attack` packet with hits), `HitTask`/`onHitTimer` (damage via `Formulas`), `AttackStanceTaskManager` (AutoAttackStart/Stop), `reduceCurrentHp` and HP status broadcast, and the hook that DEATH-01 fills (`doDie`).

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\Creature.java (`doAttack`, `doAttackHitSimple`, `doAttackHitByBow`, `onHitTimer`, `onAttackFinish`, `reduceCurrentHp`, `isAttackingDisabled`)`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\tasks\creature\HitTask.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\ai\{AbstractAI,CreatureAI,PlayableAI,PlayerAI,Intention,NextAction}.java (attack intention parts)`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\taskmanagers\AttackStanceTaskManager.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\mechanics\stats\Formulas.java (hit, crit, shield, physical damage, attack speed, attack break)`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\clientpackets\{AttackRequest,Action}.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\Attackable.java (`reduceCurrentHp`, `addDamage`: the aggro list for AI-01)`

## Files
- `crates/l2-world/src/combat/*.rs (new)`
- `crates/l2-world/src/formulas.rs (new)`
- `crates/l2-world/src/ai/*.rs (new: the intention machine; AI-01 adds the NPC side)`
- `crates/l2-world/src/{world.rs,interact.rs,movement.rs,lib.rs,command.rs} (wire-in)`
- `crates/game-server/src/client/* (dispatch)`
- `crates/l2-world/tests/combat.rs (new)`

## Requirements
1. All timing through the world's timer wheel, all randomness through the world RNG; formula code takes an RNG trait so tests replay M4-JV-01's scripted draws.
2. Formulas match the M4-JV-01 vectors exactly.
3. Packets and their order/recipients exactly as Java (Attack, AutoAttackStart/Stop, StatusUpdate, system messages for damage/miss/crit).
4. Arrows/soulshots: consumption hooks only (soulshots M5); bows need arrows equipped like Java.

## Out of scope
- Skills/magic (M5), PvP flagging/karma beyond leaving hooks, summons.

## Tests (must exist and pass)
- Vector tests; scenario tests: player kills a weak monster in N hits with a scripted RNG; attack stance start/stop; out-of-range chase.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`)
- [ ] committed as `M4-COMBAT-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read `docs/design/m4-combat.md`, `docs/design/world-loop.md`, `docs/design/m3-world.md` first, then CLAUDE.md safety rules.
- The orchestrator refines this card when its dependencies are merged (exact module names and APIs to call).
