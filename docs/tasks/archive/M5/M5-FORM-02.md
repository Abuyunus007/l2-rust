# M5-FORM-02: Skill formulas as pure functions

| Field | Value |
|---|---|
| Milestone | M5 |
| Model | sonnet |
| Review | full |
| Reference | no (extend M4-FORM-01's crates/l2-world/src/formulas/) |
| Depends on | M5-JV-03 |
| Size | M |

## Goal
The skill parts of `Formulas` as pure functions over plain inputs plus the `Rnd` trait, checked against M5-JV-03's `skill_formulas_*.json`: magic damage (sps/bss, m.crit), skill physical damage, blow/backstab damage and success, effect/magic success and resists, m.crit, skill mastery, abnormal time, cast/reuse time parts, mana damage, resurrect restore, trait and attribute bonuses, the Heal effect number.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\mechanics\stats\Formulas.java`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\skill\effects\Heal.java`
- java-vectors/out/skill_formulas_*.json + README

## Files
- `crates/l2-world/src/formulas/{magic.rs,skill.rs,...} (new)`
- `crates/l2-world/src/formulas/mod.rs (module lines)`
- `crates/l2-world/tests/skill_formula_vectors.rs (new)`

## Requirements
1. Inputs named after the Java getters; exact Java arithmetic (int/float/double casts).
2. Every vector case passes, incl. the recorded draw sequence (method, bound).

## Out of scope
- Reading creatures (CAST-01 / EFX-01 fill the inputs).

## Tests (must exist and pass)
- Vector replay per file; unit tests per function.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`)
- [ ] committed as `M5-FORM-02: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read CLAUDE.md (safety rules: no local DB, no real ports, never kill processes by name), `docs/design/m4-combat.md`, `docs/design/m5-skills.md` first.
- `calc_effect_success` and the mastery/resist parts of `calc_effect_abnormal_time` in `crates/l2-world/src/effects/apply.rs` are stubs marked `// M5-FORM-02:`: wire your functions into them (inputs from the stat engine), keep the rest of that file as it is. JV-03 note: `calcMagicAffected` uses `nextGaussian`; replay the recorded `returned` value (the harness Rnd is not a real Gaussian).
- The orchestrator refines this card when its dependencies are merged (exact module names and APIs to call).
