# M2-RUN-01: Side-by-side run setup, runbook, cross-compat check

| Field | Value |
|---|---|
| Milestone | M2 |
| Model | opus (orchestrator) + owner |
| Review | full |
| Reference | no |
| Depends on | M2-E2E-01, M2-ADM-01 |
| Size | S |

## Goal
Rust GS runs next to the Java GS (D-39): own server id and port 7778, supervisor entry, runbook; verify that characters move between both servers.

## Java source (the spec)
- none

## Reference implementation to copy
- docs/runbooks/m1-login-test.md

## Files
- `docs/runbooks/m2-game-test.md`
- `run/ (local only: game folder, hexid for the Rust GS, supervisor entry)`

## Requirements
1. Cross-compat: a character created on the Rust GS loads and plays on the Java GS, and a Java character enters the world on the Rust GS with its items/skills.

## Out of scope
- nothing beyond the files above

## Tests (must exist and pass)
- Owner checkpoint CP-M2c.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass
- [ ] committed as `M2-RUN-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read `docs/design/world-loop.md` and the CLAUDE.md safety rules first (never touch a local database or real ports).
- The owner's real Java GS and DB run side by side with our servers (D-39): anything written to the DB must look exactly like Java writes it.
