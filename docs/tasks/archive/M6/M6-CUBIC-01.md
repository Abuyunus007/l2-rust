# M6-CUBIC-01: Cubics

| Field | Value |
|---|---|
| Milestone | M6 |
| Model | sonnet |
| Review | full |
| Reference | no (copy the merged M4/M5 code around it) |
| Depends on | SUMMON-01 |
| Size | M |

## Goal
Cubic data, the `SummonCubic` effect, cubic actions (attack/heal/debuff skills on their timers), the cubic branch of `calcMagicDam` and `calcCubicSkillSuccess`, cubics in UserInfo/CharInfo, `CubicMastery`.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\holders\ (cubics: grep Cubic)`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\skill\effects\{SummonCubic,CubicMastery}.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\mechanics\stats\Formulas.java (cubic parts)`

## Files
- `crates/l2-data (cubic data)`
- `crates/l2-world/src/summon/cubic.rs`
- `tests`

## Requirements
1. Java behaviour, messages, packets and DB rows exactly (D-39: the Java GS shares the DB).
2. Single-owner world, ids not references, timers on the wheel, world RNG; no unwrap/expect on external data.
3. Resolve the `// M6:` markers in your area; list the ones you leave.

## Out of scope
- Sieges/castles/fortresses/clan halls (M7), Olympiad/Seven Signs (M8), quests (later).

## Tests (must exist and pass)
- Each part with Java's numbers/messages/packets; DB tests skip without `L2_TEST_DATABASE_URL`.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`)
- [ ] committed as `M6-CUBIC-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read CLAUDE.md (safety rules), `docs/design/m4-combat.md`, `docs/design/m5-skills.md`, `docs/design/m6-social.md` first.
- SUMMON-01 is merged (`crates/l2-world/src/summon/`). The `SummonCubic` effect is the unported example in `effects/registry.rs`; DUEL-01 left `World::duel_on_buff` for the cubic code to call (Java's cubic debuff/heal path).
- Cubic ids and timers on the wheel; cubics in UserInfo/CharInfo (`enter.rs`/`player_view.rs`).
- Up to seven M6 workers run at the same time. In shared files (`skills/targets.rs`, `enter.rs`, `player_view.rs`, `leave.rs`, `death.rs`, `world.rs`, `dispatch.rs`, `client/enter.rs`) only add your lines: no reformatting or moving code.
- Use `-j 4` for cargo clippy/test (parallel linking crashed under load) and CARGO_INCREMENTAL=0.
