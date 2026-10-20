# M6-RUN-01: Runbook and milestone checks

| Field | Value |
|---|---|
| Milestone | M6 |
| Model | opus |
| Review | full |
| Reference | no (copy the merged M4/M5 code around it) |
| Depends on | E2E-01 |
| Size | S |

## Goal
`docs/runbooks/m6-social-test.md` with CP-M6a..d and the board's results.

## Java source (the spec)
- -

## Files
- `docs/runbooks/m6-social-test.md`
- `docs/tasks/BOARD.md`

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
- [ ] committed as `M6-RUN-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read CLAUDE.md (safety rules), `docs/design/m4-combat.md`, `docs/design/m5-skills.md`, `docs/design/m6-social.md` first.
- The orchestrator refines this card when its dependencies are merged (exact module names and APIs to call).
