# M3-CFG-02: Datapack options from the game config

| Field | Value |
|---|---|
| Milestone | M3 |
| Model | haiku |
| Review | spot |
| Reference | no |
| Depends on | M3-DATA-05, M3-DATA-06, M3-DATA-08 |
| Size | S |

## Goal
`Datapack::load` takes a `DatapackOptions` built from the game config, so the Rust GS loads exactly what the owner's Java GS loads (same shared DB, D-39): `CustomItemsLoad`, `CustomNpcData` (General.ini), `EnableFakePlayers` (Custom/FakePlayers.ini), NPC.ini `MaxAggroRange`/`DefaultCorpseTime`, General.ini `HtmCache`/`LazyCache`/`CheckHtmlEncoding`, spawn options. Today every loader gets Java's code defaults (shipped General.ini says `CustomItemsLoad = True`, `CustomNpcData = True`).

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\config\{GeneralConfig,NpcConfig}.java`, `config\custom\FakePlayersConfig.java` (the keys and defaults)

## Reference implementation to copy
- crates/l2-data/src/lib.rs (`Datapack::load`), crates/l2-config game config (generated fields + game_manual.rs)

## Files
- `crates/l2-data/src/lib.rs` (`DatapackOptions`, `Datapack::load_with`; `load` keeps Java's code defaults)
- `crates/game-server/src/lib.rs` (build the options from `GameConfig`)
- `crates/l2-admin-gui/src/views/data_browser.rs` only if its loader call signature changes
- tests

## Requirements
1. One options struct, every field documented with its ini key and Java default; built from `GameConfig` in one function in game-server.
2. No behaviour change for callers of `Datapack::load` (defaults).

## Out of scope
- New loaders.

## Tests (must exist and pass)
- Options from an empty config = Java defaults; from the shipped config = custom items/NPCs on; a small datapack with a custom NPC/item is loaded or not accordingly.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass
- [ ] committed as `M3-CFG-02: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read the CLAUDE.md safety rules first (never touch a local database or real ports).
