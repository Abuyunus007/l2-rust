# M2-GUI-01: Online players view

| Field | Value |
|---|---|
| Milestone | M2 |
| Model | haiku |
| Review | spot |
| Reference | no |
| Depends on | M2-ADM-01 |
| Size | S |

## Goal
GUI view listing online players of the Rust GS (live).

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\ui\*.java`

## Reference implementation to copy
- crates/l2-admin-gui/src/views/servers.rs

## Files
- `crates/l2-admin-gui/src/views/players.rs`
- `crates/l2-admin-gui/src/views/mod.rs (register)`

## Requirements
1. Columns: name, account, level, class, ip, online since; sortable; filter.

## Out of scope
- Admin actions (kick etc.) — later.

## Tests (must exist and pass)
- View-model unit tests.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass
- [ ] committed as `M2-GUI-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read `docs/design/world-loop.md` and the CLAUDE.md safety rules first (never touch a local database or real ports).
- The owner's real Java GS and DB run side by side with our servers (D-39): anything written to the DB must look exactly like Java writes it.
