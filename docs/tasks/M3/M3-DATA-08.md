# M3-DATA-08: Html cache and teleporter lists

| Field | Value |
|---|---|
| Milestone | M3 |
| Model | haiku |
| Review | spot |
| Reference | no |
| Depends on | none |
| Size | S |

## Goal
`HtmCache` port (load `data/html/**`, Java's file filters and size checks, lookup by path) and the teleporter data loader (`data/teleporters/**`, Java `TeleporterData`).

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\cache\HtmCache.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\data\xml\TeleporterData.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\instance\Teleporter.java (what it needs from the data)`
- `C:\l2-rust\l2jmobius\dist\game\data\html`
- `C:\l2-rust\l2jmobius\dist\game\data\teleporters`

## Reference implementation to copy
- crates/l2-data/src/experience.rs

## Files
- `crates/l2-data/src/{html.rs,teleporter.rs}`
- `lib.rs lines`
- `crates/l2-data/tests/html_teleporters.rs`

## Requirements
1. Always preload (see docs/design/m3-world.md); log the LazyCache note once.
2. Same path normalisation as Java (case, separators); the html text is kept exactly (encoding as Java reads it).
3. Teleport lists with costs/items/conditions as data.

## Out of scope
- Showing html / bypass handling (M3-TALK-01).

## Tests (must exist and pass)
- Counts, a few files by path, teleport lists of 2 gatekeepers.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass
- [ ] committed as `M3-DATA-08: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read `docs/design/m3-world.md` (M3 rules) and `docs/design/world-loop.md` first. Then the CLAUDE.md safety rules (never touch a local database or real ports).
- The owner's real Java GS and DB run side by side with our servers (D-39): anything written to the DB must look exactly like Java writes it.
