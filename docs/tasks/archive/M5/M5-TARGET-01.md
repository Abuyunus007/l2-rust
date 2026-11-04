# M5-TARGET-01: The 34 skill target handlers

| Field | Value |
|---|---|
| Milestone | M5 |
| Model | sonnet |
| Review | full |
| Reference | no (first instance of a handler table; follow the bypass/admin registries' style) |
| Depends on | none (casting, M5-CAST-01, will call it) |
| Size | M |

## Goal
Java's target handlers (`ITargetTypeHandler.getTargetList(skill, activeChar, onlyFirst, target)`)
as one world function `World::skill_target_list(skill, caster, only_first, target) -> Vec<ObjectId>`
(or a `TargetResult` that also carries Java's "fail with this system message" cases), dispatching on
the skill template's `target_type` to one function per Java handler. Plus the `AffectScope` /
`AffectObject` helpers they use and `Skill.getTargetList` / `getFirstOfTargetList` /
`checkForAreaOffensiveSkills` as Java has them.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\skill\targets\*.java` (all 34)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\mechanics\skill\{Skill (getTargetList, getFirstOfTargetList, checkForAreaOffensiveSkills, addSummon, addCharacter, ...),targets\{TargetType,AffectScope,AffectObject}}.java`
- `...\handler\ITargetTypeHandler.java`, `...\handler\TargetHandler.java`

## Files
- `crates/l2-world/src/skills/{mod.rs,targets.rs}` (new; `skills/` is the casting card's module too: create it with only these files)
- `crates/l2-world/src/lib.rs` (module line)
- `crates/l2-world/tests/skill_targets.rs` (new)

## Requirements
1. Every handler ported; ones that need things M5 does not have (summons, pets, clans, command channels, sieges, flag poles) follow Java's code path for "none of those exist": party = only the caster (no party yet), clan = only the caster (no clan yet), summon targets find nothing. Mark each with `// M6:`.
2. Exact Java checks: range from the skill's `affectRange`/`castRange`, `isInsideRadius` 2D/3D as Java uses them, line of sight via geodata where Java checks it, PvP/peace-zone checks as far as zones exist (M3 zones), dead/alive filters, `onlyFirst`.
3. Messages Java sends on failure (`TARGET_IS_INCORRECT`, ...) are returned, not sent: casting decides.
4. Uses the skill template from `l2_data::skill` (M4-DATA-11), creature positions and states from the world (read-only).

## Out of scope
- Casting, effects.

## Tests (must exist and pass)
- Each handler with a few creatures around (in range/out of range/dead/behind/in front/corpses), `onlyFirst`, failure messages.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`)
- [ ] committed as `M5-TARGET-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read CLAUDE.md, `docs/design/m5-skills.md` first. The effect engine (M5-EFFECT-01) and packets run at the same time; you touch only `skills/`.
