# M4-RUN-01: Runbook and milestone checks

| Field | Value |
|---|---|
| Milestone | M4 |
| Model | opus |
| Review | full |
| Reference | no |
| Depends on | M4-E2E-01 |
| Size | S |

## Goal
`docs/runbooks/m4-combat-test.md` with CP-M4a..d steps, side-by-side checks against Sieghardt, and the board's milestone results.

## Java source (the spec)
- -

## Files
- `docs/runbooks/m4-combat-test.md`
- `docs/tasks/BOARD.md`

## Requirements
1. Steps the owner can follow without reading code.

## Out of scope
- Code.

## Tests (must exist and pass)
- -

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`)
- [ ] committed as `M4-RUN-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read `docs/design/m4-combat.md`, `docs/design/world-loop.md`, `docs/design/m3-world.md` first, then CLAUDE.md safety rules.
- The orchestrator refines this card when its dependencies are merged (exact module names and APIs to call).
