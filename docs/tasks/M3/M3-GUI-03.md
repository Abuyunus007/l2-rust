# M3-GUI-03: GUI geodata viewer

| Field | Value |
|---|---|
| Milestone | M3 |
| Model | haiku |
| Review | spot |
| Reference | no |
| Depends on | M3-GEO-01 |
| Size | S |

## Goal
An admin GUI view that renders a geodata region (heightmap + NSWE, like l2-geotool's PNG render) for both sets side by side, with the cursor showing world coords and height.

## Java source (the spec)
- `none (tooling)`

## Reference implementation to copy
- crates/l2-geotool/src/render.rs, crates/l2-admin-gui/src/views/

## Files
- `crates/l2-admin-gui/src/views/geodata.rs`
- `views/mod.rs + settings.rs lines`

## Requirements
1. Region picker (x_y), set A/B/override, zoom; rendering off the UI thread.
2. Uses the l2-geo engine's coordinate conversion (no copy of the math).

## Out of scope
- nothing beyond the files above

## Tests (must exist and pass)
- View-model tests (coords <-> pixels).

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass
- [ ] committed as `M3-GUI-03: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read `docs/design/m3-world.md` (M3 rules) and `docs/design/world-loop.md` first. Then the CLAUDE.md safety rules (never touch a local database or real ports).
- The owner's real Java GS and DB run side by side with our servers (D-39): anything written to the DB must look exactly like Java writes it.
