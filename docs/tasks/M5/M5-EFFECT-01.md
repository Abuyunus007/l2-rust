# M5-EFFECT-01: Effect engine (effect list, buff info, stacking, durations, ticks)

| Field | Value |
|---|---|
| Milestone | M5 |
| Model | opus |
| Review | full |
| Reference | yes (first instance; every effect-handler card copies it) |
| Depends on | M5-GPROTO-08 (only for `AbnormalStatusUpdate`/`ShortBuffStatusUpdate`; start without it and merge main when it lands) |
| Size | XL |

## Goal
Java's effect system without the individual handlers: a creature's `EffectList` (buffs, debuffs,
dances/songs, toggles, triggered, passives), `BuffInfo` (one applied skill: its effects, time,
ticks, stat funcs), abnormal type/level stacking and the replace rules, the buff/debuff/dance/
trigger slot limits, durations and periodic ticks on the timer wheel, stat funcs through the stat
engine, icons to the client, and an effect-handler registry that the handler cards (EFX-01..03)
fill. Also `Skill.applyEffects`/`applyEffectScope`/`getTargetList`-independent parts, so casting
(M5-CAST-01) and items (M5-ITEM-01) call one entry point.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\holders\creature\EffectList.java` (all of it)
- `...\mechanics\skill\{BuffInfo,BuffFinishTask,Skill (applyEffects, applyEffectScope, the abnormal fields, isDebuff/isDance/isToggle/isTriggered…),EffectScope,AbnormalType,AbnormalVisualEffect}.java`
- `...\mechanics\effects\{AbstractEffect,EffectFlag,EffectType,EffectTickTask?}.java` (grep the package)
- `...\entity\actor\Creature.java` (`getEffectList`, `stopAllEffects*`, `updateEffectIcons`, `isAffected(flag)`), `Player.java` (`updateEffectIcons`, `sendSkillList` interplay)
- `...\network\serverpackets\{AbnormalStatusUpdate,ShortBuffStatusUpdate}.java`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\EffectMasterHandler.java` (the registry Java builds)
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\skill\effects\Buff.java` (the simplest handler: port it here as the reference handler)

## Files
- `crates/l2-world/src/effects/{mod.rs,list.rs,buff_info.rs,registry.rs,handlers/mod.rs,handlers/buff.rs}` (new)
- `crates/l2-world/src/{world.rs,timer.rs,lib.rs,leave.rs,stats/*}` (wire-in: owner `FuncOwner::Effect`, passive skills move onto the effect list if Java keeps them there)
- `crates/l2-world/tests/effects.rs` (new)

## Requirements
1. Port per `docs/design/m5-skills.md` ("Rules"): side table per creature, timers on the wheel, ids not references, no locks.
2. Stacking and limits exactly as Java (`MaxBuffAmount`, `MaxDanceAmount`, `MaxTriggeredBuffAmount`, abnormal type/level, `stackOrder`), the same icon order and remaining times in `AbnormalStatusUpdate`.
3. A handler trait (or enum) with Java's hooks (`onStart`, `onActionTime`/tick, `onExit`, `canStart`, `isInstant`, effect flags/type), and the registry with Java's names; names without a handler yet do nothing and log once.
4. The existing passive-skill funcs (M4-STAT-01) keep their numbers (the stat vectors must still pass).

## Out of scope
- The other ~164 handlers (EFX-01..03), casting (CAST-01), saving buffs (SAVE-01).

## Tests (must exist and pass)
- Stacking/replace cases, limits, durations and ticks, stat funcs on/off, icon packets, the `Buff` handler end to end with a real skill template.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`)
- [ ] committed as `M5-EFFECT-01: <summary>`
- [ ] report delivered

## Notes from the orchestrator
- Read CLAUDE.md, `docs/design/world-loop.md`, `docs/design/m4-combat.md`, `docs/design/m5-skills.md` first.
