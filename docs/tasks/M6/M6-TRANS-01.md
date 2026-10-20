# M6-TRANS-01: Transformations

| Field | Value |
|---|---|
| Milestone | M6 |
| Model | sonnet |
| Review | full |
| Reference | no (copy the merged M4/M5 code around it) |
| Depends on | none (M5) |
| Size | L |

## Goal
Transformation data, the `Transformation` effect and `TransformDispel`, transformed stats (the stat engine's `transform` hooks left `None` in M4-STAT-01), transformed skills and action list, shortcut/equipment rules while transformed, `UserInfo`/`CharInfo` transform id, untransform on death/relog.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\data\xml\TransformData.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\transform\*.java`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\skill\effects\Transformation.java`

## Files
- `crates/l2-data/src/transform.rs (new)`
- `crates/l2-world/src/transform/*.rs (new) + stats hooks`
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
- [ ] committed as `M6-TRANS-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read CLAUDE.md (safety rules), `docs/design/m4-combat.md`, `docs/design/m5-skills.md`, `docs/design/m6-social.md` first.
- The orchestrator refines this card when its dependencies are merged (exact module names and APIs to call).
