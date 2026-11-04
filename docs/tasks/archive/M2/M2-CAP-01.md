# M2-CAP-01: Reference captures from the owner's Java GS

| Field | Value |
|---|---|
| Milestone | M2 |
| Model | opus (orchestrator) + owner |
| Review | full |
| Reference | yes (ground truth for GPROTO/ENTER) |
| Depends on | none |
| Size | S |

## Goal
Turn the owner's recorded Java sessions (`run/captures/`) into scrubbed fixtures for tests: character screen, create/delete, enter world, chat, logout, restart.

## Java source (the spec)
- none

## Reference implementation to copy
- crates/l2-testkit/src/capture_compare.rs, fixtures/captures/README.md

## Files
- `tools/scrub-capture (small Rust or Python tool: removes/replaces account names, character names, IPs, session keys in fields AND raw hex)`
- `fixtures/captures/java-gs-*.jsonl`
- `fixtures/captures/README.md`

## Requirements
1. The owner plays normally; the orchestrator picks sessions and asks the owner for specific actions when something is missing (e.g. create + delete a character).
2. Nothing personal in committed fixtures (public repo).

## Out of scope
- nothing beyond the files above

## Tests (must exist and pass)
- Scrubber unit tests; a test that every fixture parses.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass
- [ ] committed as `M2-CAP-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read `docs/design/world-loop.md` and the CLAUDE.md safety rules first (never touch a local database or real ports).
- The owner's real Java GS and DB run side by side with our servers (D-39): anything written to the DB must look exactly like Java writes it.
