# M5-EFX-03: Effect handlers: dispel, hate, movement, misc

| Field | Value |
|---|---|
| Milestone | M5 |
| Model | haiku |
| Review | spot |
| Reference | no (copy M5-EFFECT-01's handlers/buff.rs) |
| Depends on | M5-EFFECT-01 |
| Size | M |

## Goal
DispelAll, DispelByCategory, DispelBySlot, DispelBySlotProbability, StealAbnormal, AddHate, DeleteHate, DeleteHateOfMe, RandomizeHate, TransferHate, GetAgro, TargetMe, TargetMeProbability, Teleport, TeleportToTarget, Escape, Blink, ThrowUp, Recovery, Restoration, RestorationRandom, Resurrection, ResurrectionSpecial, GiveSp, VitalityPointUp, CallSkill, ApplySkillEffects, SetSkill, TriggerSkillBy* (4), Detection, DetectHiddenObjects, OpenDoor, OpenChest, ConsumeBody, ConvertItem, ChangeFace/HairColor/HairStyle, MpConsumePerLevel, Grow, NevitsHourglass.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\skill\effects\<each name>.java`

## Files
- `crates/l2-world/src/effects/handlers/*.rs`
- `crates/l2-world/src/effects/registry.rs`
- `crates/l2-world/tests/effects_misc.rs (new)`

## Requirements
1. Same messages, packets, DB effects as Java; use existing world functions (teleport, inventory add, revive, aggro list).

## Out of scope
- Summons, clan/siege (HeadquarterCreate, TakeCastle, ...: M7+), fishing, crafting books.

## Tests (must exist and pass)
- Per handler.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`)
- [ ] committed as `M5-EFX-03: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read CLAUDE.md (safety rules: no local DB, no real ports, never kill processes by name), `docs/design/m4-combat.md`, `docs/design/m5-skills.md` first.
- Effect engine API (M5-EFFECT-01, merged): add a handler as `crates/l2-world/src/effects/handlers/<name>.rs` (struct + `fn create(&EffectTemplate) -> Result<Box<dyn EffectHandler>, String>` parsing the params, only the hooks Java overrides: `effect_type`, `effect_flags`, `is_instant`, `calc_success`, `can_start`, `on_start`, `on_action_time`, `on_exit`, `check_condition`), plus `mod x;` and one `registry.register("JavaName", X::create)` line in `effects/handlers/mod.rs`. `handlers/buff.rs` is the reference. All 165 names exist as `Unported` stand-ins; yours replace them. World API: `apply_effects*`, `activate_skill`, `stop_*`, `is_affected(flag)`, `is_affected_by_abnormal_type`, `effect_list(id)`, `abnormal visual effects` (see `effects/mod.rs`).
- The orchestrator refines this card when its dependencies are merged (exact module names and APIs to call).
