# M5-ADMIN-04: Admin commands for skills

| Field | Value |
|---|---|
| Milestone | M5 |
| Model | haiku |
| Review | spot |
| Reference | no (copy M4-ADMIN-02's admin modules) |
| Depends on | M5-CAST-01 |
| Size | S |

## Goal
AdminSkill and AdminBuffs commands as Java registers them (`//give_skill`, `//remove_skill`, `//skill_list`, `//give_all_skills`, `//remove_all_skills`, `//add_clan_skill` only if trivial, `//getbuffs`, `//stopbuff`, `//stopallbuffs`, `//areacancel`), and AdminCreateItem's leftovers if ITEM-01 makes them possible.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\admincommandhandlers\{AdminSkill,AdminBuffs}.java`

## Files
- `crates/l2-world/src/admin/skills.rs (new)`
- `crates/l2-world/tests/admin_skills.rs (new)`

## Requirements
1. Names, access checks, messages as Java.

## Out of scope
- Clan skills (M6).

## Tests (must exist and pass)
- Each command.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`)
- [ ] committed as `M5-ADMIN-04: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read CLAUDE.md (safety rules: no local DB, no real ports, never kill processes by name), `docs/design/m4-combat.md`, `docs/design/m5-skills.md` first.
- Casting is merged (M5-CAST-01, `crates/l2-world/src/skills/cast.rs`): players and items cast through `World::use_magic(player, SkillKey, force_use, dont_move)`; potions/herbs through `do_simultaneous_cast`; NPCs through `set_npc_target(npc, Some(t))` + `do_cast(npc, key)`; triggered skills `do_cast_at`; reuse in `skills/reuse.rs`; `// M5-SHOT-01:` markers (spiritshot 0.6x cast time, `rechargeShots` at the hit and finalizer, the shot flags in `effects/apply.rs` and EFX handlers) are the shots card's to fill.
- The orchestrator refines this card when its dependencies are merged (exact module names and APIs to call).
