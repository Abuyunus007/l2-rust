# M5-RUN-01: Runbook and milestone checks

| Field | Value |
|---|---|
| Milestone | M5 |
| Model | opus |
| Review | full |
| Reference | no |
| Depends on | M5-E2E-01 |
| Size | S |

## Goal
`docs/runbooks/m5-skills-test.md` with CP-M5a..d steps and the board's milestone results.

## Java source (the spec)
- -

## Files
- `docs/runbooks/m5-skills-test.md`
- `docs/tasks/BOARD.md`

## Requirements
1. Steps the owner can follow without reading code.

## Out of scope
- Code.

## Tests (must exist and pass)
- -

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`)
- [ ] committed as `M5-RUN-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read CLAUDE.md (safety rules: no local DB, no real ports, never kill processes by name), `docs/design/m4-combat.md`, `docs/design/m5-skills.md` first.
- The orchestrator refines this card when its dependencies are merged (exact module names and APIs to call).
