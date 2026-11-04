# M5-EFX-01: Effect handlers: damage and healing

| Field | Value |
|---|---|
| Milestone | M5 |
| Model | sonnet |
| Review | full |
| Reference | no (copy M5-EFFECT-01's handlers/buff.rs) |
| Depends on | M5-EFFECT-01, M5-FORM-02 |
| Size | L |

## Goal
The damage and healing handlers: PhysicalDamage, PhysicalDamageHpLink, PhysicalSoulDamage, MagicalDamage, MagicalDamageByAbnormal, MagicalDamageMp, MagicalSoulDamage, EnergyDamage, StaticDamage, Lethal, FatalBlow, Backstab, SoulBlow, DeathLink, HpDrain, DamOverTime, DamOverTimePercent, ManaDamOverTime, CpDamPercent, Heal, HealOverTime, HealPercent, HpByLevel, ManaHeal, ManaHealByLevel, ManaHealOverTime, ManaHealPercent, CpHeal, CpHealOverTime, CpHealPercent, MaxHp, MaxCp, RebalanceHP, TransferDamage (as far as no summon is involved).

## Java source (the spec)
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\skill\effects\<each name>.java`

## Files
- `crates/l2-world/src/effects/handlers/*.rs (one file per group)`
- `crates/l2-world/src/effects/registry.rs (registrations)`
- `crates/l2-world/tests/effects_damage_heal.rs (new)`

## Requirements
1. Numbers through the FORM-02 functions; messages and packets exactly as Java.
2. Each handler names its Java class; quirks kept.

## Out of scope
- Summon/party parts (M6).

## Tests (must exist and pass)
- Per handler with a real skill of the datapack.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`)
- [ ] committed as `M5-EFX-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read CLAUDE.md (safety rules: no local DB, no real ports, never kill processes by name), `docs/design/m4-combat.md`, `docs/design/m5-skills.md` first.
- Effect engine API (M5-EFFECT-01, merged): add a handler as `crates/l2-world/src/effects/handlers/<name>.rs` (struct + `fn create(&EffectTemplate) -> Result<Box<dyn EffectHandler>, String>` parsing the params, only the hooks Java overrides: `effect_type`, `effect_flags`, `is_instant`, `calc_success`, `can_start`, `on_start`, `on_action_time`, `on_exit`, `check_condition`), plus `mod x;` and one `registry.register("JavaName", X::create)` line in `effects/handlers/mod.rs`. `handlers/buff.rs` is the reference. All 165 names exist as `Unported` stand-ins; yours replace them. World API: `apply_effects*`, `activate_skill`, `stop_*`, `is_affected(flag)`, `is_affected_by_abnormal_type`, `effect_list(id)`, `abnormal visual effects` (see `effects/mod.rs`).
- Damage/heal numbers: M5-FORM-02's functions (`crates/l2-world/src/formulas/`) when merged; FORM-02 runs right now, so start with the heal/over-time handlers and merge main before the damage ones.
- The orchestrator refines this card when its dependencies are merged (exact module names and APIs to call).
