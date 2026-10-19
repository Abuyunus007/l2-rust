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
- Effect engine API (M5-EFFECT-01, merged): add a handler as `crates/l2-world/src/effects/handlers/<name>.rs` (struct + `fn create(&EffectTemplate) -> Result<Box<dyn EffectHandler>, String>` parsing the params, only the hooks Java overrides: `effect_type`, `effect_flags`, `is_instant`, `calc_success`, `can_start`, `on_start`, `on_action_time`, `on_exit`, `check_condition`), plus `mod x;` and one `registry.register("JavaName", X::create)` line in `effects/handlers/mod.rs`. `handlers/buff.rs` is the reference. All 165 names exist as `Unported` stand-ins; yours replace them. World API: `apply_effects*`, `activate_skill`, `stop_*`, `is_affected(flag)`, `is_affected_by_abnormal_type`, `effect_list(id)`, `abnormal visual effects` (see `effects/mod.rs`).
- Also: replace the M4 `// M5:` markers that check stun/root/... in `movement.rs`, `combat/`, `ai/`, `status.rs` with `is_affected(flag)`; port the onStart/onExit of TalismanSlot, EnableCloak, CrystalGradeModify (they are scanned from passives in `stats/skills.rs` today) and the stat conditions that read effects (`ActiveEffect`, `CheckAbnormal`, `TargetActiveEffect`, `TargetAbnormal` in `stats/conditions.rs`). Hook the abnormal visual effect masks into `UserInfo`/`CharInfo`/`NpcInfo` (`World::abnormal_visual_effects(id)`; `enter.rs`, `npc.rs`, `player_view.rs`).
- The orchestrator refines this card when its dependencies are merged (exact module names and APIs to call).
