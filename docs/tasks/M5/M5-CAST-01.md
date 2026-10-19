# M5-CAST-01: Casting

| Field | Value |
|---|---|
| Milestone | M5 |
| Model | opus |
| Review | full |
| Reference | yes (first instance) |
| Depends on | M5-EFFECT-01, M5-FORM-02, M5-TARGET-01, M5-GPROTO-08 |
| Size | XL |

## Goal
`RequestMagicSkillUse` → `Player.useMagic` → `Creature.doCast`: every use check (MP/HP/item consume, reuse, conditions, cast range, line of sight, target handler result, dead/disabled/casting already), the cast phases on the timer wheel (`MagicUseTask`: launched, hit, finalizer, hit time/cool time from FORM-02), `MagicSkillUse`/`MagicSkillLaunched`/`MagicSkillCanceled`, interruption (`breakCast`, attack break from M4-FORM-01), reuse and `SkillCoolTime`, the CAST intention in the AI (move into range, `notifyActionReadyToAct`), `Skill.applyEffects` into the effect engine, toggles, skill shortcuts.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\Creature.java (doCast, beginCast, onMagicLaunchedTimer, onMagicHitTimer, onMagicFinalizer, breakCast, abortCast, isCastingNow, getSkillReuse*)`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\Player.java (useMagic, checkUseMagicConditions, reuse timestamps)`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\tasks\creature\{MagicUseTask,QueuedMagicUseTask}.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\ai\{CreatureAI,PlayerAI,AbstractAI}.java (cast intention)`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\clientpackets\RequestMagicSkillUse.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\mechanics\skill\Skill.java (checkCondition, useSkill, applyEffects, activateSkill)`

## Files
- `crates/l2-world/src/skills/{cast.rs,checks.rs,reuse.rs} (new; `skills/targets.rs` exists from TARGET-01)`
- `crates/l2-world/src/{ai/*,combat/*,world.rs,timer.rs,lib.rs,command.rs} (wire-in)`
- `crates/game-server/src/client/* (dispatch)`
- `crates/l2-world/tests/cast.rs (new)`

## Requirements
1. Timers on the wheel, world RNG, packets/recipients/order exactly as Java.
2. One entry point that items (ITEM-01) and NPC AI (AI-03) also use.

## Out of scope
- The individual effect handlers (EFX-01..03), item handlers (ITEM-01), shots (SHOT-01).

## Tests (must exist and pass)
- Cast a nuke/heal/buff with the reference Buff handler; reuse; interruption; out of range move; MP/item consume; failures with Java's messages.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`)
- [ ] committed as `M5-CAST-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read CLAUDE.md (safety rules: no local DB, no real ports, never kill processes by name), `docs/design/m4-combat.md`, `docs/design/m5-skills.md` first.
- The orchestrator refines this card when its dependencies are merged (exact module names and APIs to call).
