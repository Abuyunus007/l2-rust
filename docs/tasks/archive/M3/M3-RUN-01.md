# M3-RUN-01: M3 runbook and geodata decision

| Field | Value |
|---|---|
| Milestone | M3 |
| Model | opus (orchestrator) |
| Review | full |
| Reference | no |
| Depends on | M3-E2E-01 |
| Size | S |

## Goal
Runbook for the M3 checkpoints, the geodata default-set decision (D-26) from the owner's in-game check, config notes (PathFinding, override list), supervisor/GUI updates.

## Java source (the spec)
- `none`

## Reference implementation to copy
- docs/runbooks/m1-login-test.md

## Files
- `docs/runbooks/m3-world-test.md`
- `docs/DECISIONS.md (D-26 outcome)`

## Requirements
1. Owner decision recorded with the evidence (regions compared).

## Out of scope
- nothing beyond the files above

## Tests (must exist and pass)
- none (docs)

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass
- [ ] committed as `M3-RUN-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read `docs/design/m3-world.md` (M3 rules) and `docs/design/world-loop.md` first. Then the CLAUDE.md safety rules (never touch a local database or real ports).
- The owner's real Java GS and DB run side by side with our servers (D-39): anything written to the DB must look exactly like Java writes it.
