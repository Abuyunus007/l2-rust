# M5-EFX-04: Effect handlers: damage, Heal, hate chances (rest of EFX-01/EFX-03)

| Field | Value |
|---|---|
| Milestone | M5 |
| Model | sonnet |
| Review | full |
| Reference | no (copy the merged handlers in `crates/l2-world/src/effects/handlers/`) |
| Depends on | M5-FORM-02, M5-EFX-01 (part 1), M5-EFX-03 (part 1) |
| Size | L |

## Goal
The handlers that waited for the skill formulas (M5-FORM-02, merged):
- From EFX-01: `Heal` (incl. the skill-4051 special case), PhysicalDamage, PhysicalDamageHpLink, PhysicalSoulDamage, MagicalDamage, MagicalDamageByAbnormal, MagicalDamageMp, MagicalSoulDamage, EnergyDamage, Lethal, FatalBlow, Backstab, SoulBlow, DeathLink, HpDrain.
- From EFX-03: DeleteHate, DeleteHateOfMe, RandomizeHate, TransferHate, TargetMeProbability (their `calcSuccess` = `Formulas.calcProbability`), and Resurrection's non-player branch (`calculateSkillResurrectRestorePercent`).
- Fix found by EFX-03: `effects/apply.rs` `is_creature` must include doors (Java `Door extends Creature`), so OpenDoor / DetectHiddenObjects run on door targets.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\skill\effects\<each name>.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\mechanics\stats\Formulas.java` (`calcDamageReflected`, `calcPhysicalSkillEvasion`, `calcBlowSuccess`, `calcMagicAffected`, already ported in `formulas/` except where noted)

## Files
- `crates/l2-world/src/effects/handlers/{physical_damage,magical_damage,blow,heal,...}.rs`, `handlers/mod.rs` (register lines)
- `crates/l2-world/src/effects/apply.rs` (the door fix only)
- `crates/l2-world/tests/effects_damage_heal.rs`, `effects_misc.rs` (extend)

## Requirements
1. Numbers through the `formulas/` functions (vector-checked); fill their inputs from the stat engine like `effects/apply.rs::calc_effect_success` does. Shots: pass the charge state as `false` with a `// M5-SHOT-01:` marker (soulshots card).
2. Messages/packets exactly as Java (`MagicDamResult`/`ManaDamResult` say which: resisted, half damage, drain; crit messages; `C1HasDoneS3PointsOfDamageToC2`), damage through `reduce_current_hp_by_skill` (EFX-01's `damage_support.rs`).
3. If `calcDamageReflected` is not in `formulas/`, port it there with a unit test (no vector exists).

## Out of scope
- Shots (SHOT-01), casting (CAST-01), parties/summons (M6).

## Tests (must exist and pass)
- Per handler with a real datapack skill; seeded RNG; resist/half/crit paths.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`)
- [ ] committed as `M5-EFX-04: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read CLAUDE.md, `docs/design/m5-skills.md` first. M5-CAST-01 and M5-EFX-02 run at the same time: keep `handlers/mod.rs` changes to added lines.
