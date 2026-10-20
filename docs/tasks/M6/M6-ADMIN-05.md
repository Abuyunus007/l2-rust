# M6-ADMIN-05: Admin commands for social and summons

| Field | Value |
|---|---|
| Milestone | M6 |
| Model | haiku |
| Review | spot |
| Reference | no (copy the merged M4/M5 code around it) |
| Depends on | CLAN-01, SUMMON-01 |
| Size | S |

## Goal
The admin handlers for parties, clans, karma/pk, summons and pets as Java registers them (`AdminPledge`, `AdminParty`?, `AdminPkKarma`-style, `AdminSummon`, `AdminPetName`...: grep the handler folder).

## Java source (the spec)
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\admincommandhandlers\*.java`

## Files
- `crates/l2-world/src/admin/*.rs`
- `tests`

## Requirements
1. Java behaviour, messages, packets and DB rows exactly (D-39: the Java GS shares the DB).
2. Single-owner world, ids not references, timers on the wheel, world RNG; no unwrap/expect on external data.
3. Resolve the `// M6:` markers in your area; list the ones you leave.

- Also (left over from M5-FU-01): the charskills.htm buttons `//get_skills`, `//reset_skills`, `//give_all_skills_fs`, `//setskill`, `//buff`, `//removereuse`, `//switch_gm_buffs` and the clan skill commands (`AdminSkill.java`); `//kill` on a non-GM player calls `stopAllEffects()` (the marker in `admin/combat.rs`).

## Out of scope
- Sieges/castles/fortresses/clan halls (M7), Olympiad/Seven Signs (M8), quests (later).

## Tests (must exist and pass)
- Each part with Java's numbers/messages/packets; DB tests skip without `L2_TEST_DATABASE_URL`.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`)
- [ ] committed as `M6-ADMIN-05: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read CLAUDE.md (safety rules), `docs/design/m4-combat.md`, `docs/design/m5-skills.md`, `docs/design/m6-social.md` first.
- The orchestrator refines this card when its dependencies are merged (exact module names and APIs to call).
