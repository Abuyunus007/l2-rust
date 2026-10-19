# M5-SAVE-01: Buffs and reuse times across relog

| Field | Value |
|---|---|
| Milestone | M5 |
| Model | haiku |
| Review | spot |
| Reference | no |
| Depends on | M5-EFFECT-01, M5-CAST-01 |
| Size | S |

## Goal
Java `Player.storeEffect`/`restoreEffects` (`character_skills_save`: buffs with remaining time, reuse timestamps, `restore_type`, `buff_index`), store on logout/autosave, restore at enter world, so a character moving between Kain and Sieghardt keeps them (D-39).

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\Player.java (storeEffect, restoreEffects, STORE_SKILL_SAVE, RESTORE_SKILL_SAVE, DELETE_SKILL_SAVE)`

## Files
- `crates/l2-db/src/game/skills_save.rs (new)`
- `crates/game-server/src/{persist.rs,client/enter.rs}`
- `crates/l2-world/src/{effects/*,skills/reuse.rs,enter.rs} (snapshot/restore)`
- `tests next to them`

## Requirements
1. Exactly Java's SQL and values.

## Out of scope
- Pets/summons (M6).

## Tests (must exist and pass)
- Store/restore round trip (DB test skips locally), remaining times, reuse.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`)
- [ ] committed as `M5-SAVE-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read CLAUDE.md (safety rules: no local DB, no real ports, never kill processes by name), `docs/design/m4-combat.md`, `docs/design/m5-skills.md` first.
- The orchestrator refines this card when its dependencies are merged (exact module names and APIs to call).
