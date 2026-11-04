# M2-PERSIST-01: Persistence task and autosave

| Field | Value |
|---|---|
| Milestone | M2 |
| Model | sonnet |
| Review | full |
| Reference | no |
| Depends on | M2-WORLD-01, M2-DB-02 |
| Size | S |

## Goal
The async writer for player snapshots (ordered per character), autosave like Java's PlayerAutoSaveTaskManager, and the shutdown drain (D-15).

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\taskmanagers\PlayerAutoSaveTaskManager.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\Player.java (store(), storeCharBase, items via ItemContainer)`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\Shutdown.java (save order)`

## Reference implementation to copy
- docs/design/world-loop.md (Persistence section)

## Files
- `crates/game-server/src/persist.rs`
- `crates/l2-world/src/snapshot.rs`

## Requirements
1. Logout: store completes before PlayerLogout goes to the LS.
2. Never writes columns/tables M2 doesn't model.

## Out of scope
- nothing beyond the files above

## Tests (must exist and pass)
- Ordering tests with a recording store; DB-gated store/restore round trip in CI.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass
- [ ] committed as `M2-PERSIST-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read `docs/design/world-loop.md` and the CLAUDE.md safety rules first (never touch a local database or real ports).
- The owner's real Java GS and DB run side by side with our servers (D-39): anything written to the DB must look exactly like Java writes it.
