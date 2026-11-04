# M4-DATA-11: Skill data (templates per level, effect templates, conditions)

| Field | Value |
|---|---|
| Milestone | M4 |
| Model | sonnet |
| Review | full |
| Reference | yes (uses the condition parser and `FuncTemplate` of M4-DATA-10) |
| Depends on | M4-DATA-10 |
| Size | L |

## Goal
`SkillData`: every skill of `data/stats/skills/*.xml` (+ custom when Java loads it) as one template
per id and level, including enchant levels, exactly as `DocumentSkill` builds them. Effects are
templates (name, scope, parameters, stat funcs, conditions); their handlers are ported later (M5),
but the stat funcs of passive skills are applied by M4-STAT-01.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\data\xml\SkillData.java` (load, `getSkillHashCode`, max levels, enchantable set, log lines)
- `...\util\DocumentSkill.java` (all of it: tables, `#` references, enchant routes `enchant1..8`, level-specific values, `makeSkills`)
- `...\util\DocumentBase.java` (`parseTemplate`, `attachFunc`, `attachEffect`, `parseBeanSet`, `parseElementValue`, `hasLevelSpecificValues`, `parseLevelSpecificValues`, `getValue`)
- `...\mechanics\skill\{Skill,EffectScope,SkillOperateType}.java`, `...\mechanics\skill\targets\*`
- `...\mechanics\effects\AbstractEffect.java` (what an effect template holds: name, attach cond, funcs, params, ticks)
- Data: `C:\l2-rust\l2jmobius\dist\game\data\stats\skills\*.xml`, `dist\game\data\xsd\skills.xsd`

## Files
- `crates/l2-data/src/skill/mod.rs`, `skill/document.rs`, `skill/template.rs`, `skill/effect.rs` (new)
- `crates/l2-data/src/lib.rs` (module lines, `Datapack` field, load call)
- `crates/l2-data/tests/skills.rs` (new)

## Requirements
1. Same skills, same levels, same values as Java: tables, `#` lookups per level and per enchant route, level-specific inline values, defaults. Same log lines (`SkillData: Loaded N skills.` etc.).
2. `SkillTemplate { id, level, name, operate_type, magic_level, ...typed fields M4 needs (is_magic, target_type, cast_range, effect_range, hit_time, cool_time, reuse_delay, mp_consume, mp_initial_consume, hp_consume, power, abnormal type/level/time, trait, element/attribute, next_action, ...) , set: StatSet-like map of everything else, effects: per EffectScope a list of EffectTemplate, conditions }`. Lookup `get(id, level)`, `max_level(id)`, `is_enchantable(id)`.
3. `EffectTemplate { name, scope, params (raw map), funcs: Vec<FuncTemplate>, attach_cond, apply_cond }`. Funcs inside effects use DATA-10's `FuncTemplate` and condition parser with the per-level table resolver.
4. Loading must be fast: parallel over files (scoped threads, like `crates/l2-data/src/html.rs`), same result regardless of thread count (merge in file order).
5. `operate_type.is_passive()` replaces the ad-hoc passive scan in `game-server/src/world_data.rs` **later** (STAT-01 swaps it); don't edit game-server.

## Out of scope
- Effect handlers, casting, buffs (M5). Skill trees are already loaded (`skill_tree.rs`).

## Tests (must exist and pass)
- Real datapack (skip if missing): total count, `max_level` for 10 skills, spot checks: 141 Weapon Mastery lv 1-3 (mul pAtk 1.085, add pAtk 2/3/4, magicLevel 5/10/15), 142 Armor Mastery lv 4 (`add rEvas 3` with `using kind=LIGHT`), 4408 lv 9 (`mul maxHp 0.5`), an enchanted skill (route 1 level 101), an active skill's mpConsume/castRange/hitTime.
- Inline XML tests for tables, enchant tables and level-specific values.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`)
- [ ] committed as `M4-DATA-11: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read `docs/design/m4-combat.md` first, then CLAUDE.md safety rules.
