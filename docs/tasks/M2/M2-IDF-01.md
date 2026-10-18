# M2-IDF-01: IdFactory (object ids) with a separate Rust range

| Field | Value |
|---|---|
| Milestone | M2 |
| Model | sonnet |
| Review | full |
| Reference | no |
| Depends on | M2-DB-01 |
| Size | S |

## Goal
Port Java's IdManager (startup scan of used ids, free-id allocation) with D-39's separate high range so Java and Rust can run on the same DB at the same time.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\managers\IdManager.java (or data/IdManager — find it: FIRST_OID/LAST_OID, the tables it scans, cleanup queries it runs at startup)`

## Reference implementation to copy
- crates/l2-db/src/game/characters.rs

## Files
- `crates/l2-db/src/game/ids.rs (scan queries)`
- `crates/l2-world or game-server: crates/game-server/src/ids.rs (allocator)`
- `tests (unit + DB-gated)`

## Requirements
1. Scan the exact tables Java scans. Allocate only from the Rust range (proposal: 0x60000000..=0x7FFFFFFF; Java starts at 0x10000000 and grows upward). Document the range in DECISIONS (D-39 note) and refuse to start if the range is exhausted.
2. Do NOT port Java's startup cleanup DELETE queries (they would delete rows while the Java GS runs) — list them in the report; the orchestrator decides.
3. Thread-safe allocate/release (used by the client tasks for character creation and by the world).

## Out of scope
- nothing beyond the files above

## Tests (must exist and pass)
- Allocator unit tests; DB-gated scan in CI.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass
- [ ] committed as `M2-IDF-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read `docs/design/world-loop.md` and the CLAUDE.md safety rules first (never touch a local database or real ports).
- The owner's real Java GS and DB run side by side with our servers (D-39): anything written to the DB must look exactly like Java writes it.
