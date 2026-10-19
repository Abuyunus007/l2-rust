# M4-FORM-01: Combat and regen formulas as pure functions

| Field | Value |
|---|---|
| Milestone | M4 |
| Model | sonnet |
| Review | full |
| Reference | yes (first instance) |
| Depends on | M4-JV-01 |
| Size | M |

## Goal
The number parts of `Formulas` as pure Rust functions over plain inputs plus an RNG trait, checked
against M4-JV-01's vectors. M4-COMBAT-01 and M4-REGEN-01 feed them from the stat engine
(M4-STAT-01); this card does not touch the world or the stat engine.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\mechanics\stats\Formulas.java`: `calcHitMiss`, `calcCrit` (no skill), `calcShldUse`, `calcPhysDam` (no skill), `calcPAtkSpd`, `calcAtkSpd`, `calcAtkBreak`, `calcHpRegen`, `calcMpRegen`, `calcCpRegen`, `calcLvlBonusMod`, `getRegeneratePeriod`, and every helper they call (level-diff penalties, `HitConditionBonusData.getConditionBonus`, position checks' results, trait bonuses as plain numbers)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\commons\util\Rnd.java` (how each draw maps a uniform value to `get(bound)`, `get(min,max)`, `nextDouble`)
- Vectors: `C:\l2-rust\java-vectors\out\formulas_*.json` (+ README: `script` = scripted uniform values, `rnd` = every draw with method, bound, returned), `formulas_creatures.json` (creature numbers + config values)

## Files
- `crates/l2-world/src/formulas/{mod.rs,hit.rs,damage.rs,speed.rs,regen.rs,rng.rs}` (new)
- `crates/l2-world/src/lib.rs` (module line only)
- `crates/l2-world/tests/formula_vectors.rs` (new)

## Requirements
1. Inputs are plain structs of numbers/flags (accuracy, evasion, levels, position relation, night, z diff, shield rate, p.atk, p.def, crit damage stats, soulshot, weapon type, config values ...) that the caller fills from the stat engine. Name the fields after the Java getters they replace.
2. An RNG trait mirroring Java `Rnd`'s methods; the world's seedable RNG implements it (`WorldRng`), and tests implement it with the vector's scripted draws. Each test also checks that the sequence of calls (method, bound) equals the recorded `rnd` list.
3. Exact Java arithmetic: int/long/float/double types, casts and rounding as written (`(int)`, `Math.round`, float vs double).
4. Every case in the `formulas_*.json` files passes (skip with a message when the files are missing).

## Out of scope
- Reading stats from creatures, the world, packets (COMBAT-01 / REGEN-01). Skill/magic formulas (M5).

## Tests (must exist and pass)
- Vector tests for every formulas file; small unit tests per function.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`)
- [ ] committed as `M4-FORM-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read `docs/design/m4-combat.md` and `java-vectors/README.md` first, then CLAUDE.md safety rules.
- Another worker (M4-STAT-01) works in `crates/l2-world/src/stats/` and the wiring files at the same time: don't touch them; your `lib.rs` change is one added line.
