# M5-EFX-02: Effect handlers: buffs, debuffs, control

| Field | Value |
|---|---|
| Milestone | M5 |
| Model | sonnet |
| Review | full |
| Reference | no (copy M5-EFFECT-01's handlers/buff.rs) |
| Depends on | M5-EFFECT-01 |
| Size | L |

## Goal
The status handlers: Debuff, ImmobileBuff, Stun, Sleep, Root, Paralyze, Petrification, Fear, Confuse, Betray, Mute, PhysicalMute, PhysicalDamageMute, Disarm, Invincible, Hide, SilentMove, FakeDeath, Relax, ChameleonRest, BlockAction, BlockChat, BlockResurrection, BlockAbnormalSlot, EnlargeAbnormalSlot, ResistSkill, AttackTrait, DefenceTrait, Lucky, NoblesseBless, ProtectionBlessing, CharmOfLuck-style flags, Passive, Flag, TargetCancel, Bluff, Distrust, RunAway, SkillTurning, EnemyCharge, FocusEnergy/FocusMaxEnergy (charges), FocusSouls/SoulEating (souls).

## Java source (the spec)
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\skill\effects\<each name>.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\mechanics\effects\EffectFlag.java`

## Files
- `crates/l2-world/src/effects/handlers/*.rs`
- `crates/l2-world/src/effects/registry.rs`
- `crates/l2-world/src/{ai/*,combat/*,movement.rs,status.rs} (the checks Java makes on these flags: isStunned, isRooted, isAffected(flag) ...)`
- `crates/l2-world/tests/effects_control.rs (new)`

## Requirements
1. Every `isAffected(flag)`/`isStunned`/... check Java makes in movement, attack, cast and AI is wired to the effect flags (replacing the M4 `// M5:` markers).
2. Packets (`AbnormalVisualEffect` updates, StopMove on root, ...) as Java.

## Out of scope
- Transformation, summons.

## Tests (must exist and pass)
- Per handler: applied, effect visible, blocks what Java blocks, ends cleanly.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`)
- [ ] committed as `M5-EFX-02: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read CLAUDE.md (safety rules: no local DB, no real ports, never kill processes by name), `docs/design/m4-combat.md`, `docs/design/m5-skills.md` first.
- The orchestrator refines this card when its dependencies are merged (exact module names and APIs to call).
