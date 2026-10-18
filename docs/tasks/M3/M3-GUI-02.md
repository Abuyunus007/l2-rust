# M3-GUI-02: GUI data browser (NPCs, items)

| Field | Value |
|---|---|
| Milestone | M3 |
| Model | haiku |
| Review | spot |
| Reference | no |
| Depends on | M3-DATA-05 |
| Size | S |

## Goal
An admin GUI view that loads the datapack (path setting) with `l2-data` and lets you search NPC templates and items by id/name with a detail panel.

## Java source (the spec)
- `none (tooling)`

## Reference implementation to copy
- crates/l2-admin-gui/src/views/players.rs (view-model + view split)

## Files
- `crates/l2-admin-gui/src/views/data_browser.rs`
- `views/mod.rs + settings.rs lines`

## Requirements
1. Loading runs on a background thread, the UI stays responsive.
2. Sortable/filterable tables; view-model logic unit-tested without egui.

## Out of scope
- Editing data.

## Tests (must exist and pass)
- View-model tests (filter, sort, detail).

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass
- [ ] committed as `M3-GUI-02: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read `docs/design/m3-world.md` (M3 rules) and `docs/design/world-loop.md` first. Then the CLAUDE.md safety rules (never touch a local database or real ports).
- The owner's real Java GS and DB run side by side with our servers (D-39): anything written to the DB must look exactly like Java writes it.
