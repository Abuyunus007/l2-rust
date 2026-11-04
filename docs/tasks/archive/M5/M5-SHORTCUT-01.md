# M5-SHORTCUT-01: Shortcut bar (register and delete)

| Field | Value |
|---|---|
| Milestone | M5 |
| Model | sonnet |
| Review | full |
| Reference | no (copy the M5 client packets and M4-INV-01's persistence path) |
| Depends on | M5-CAST-01 |
| Size | M |

## Goal
Players can put skills, items, actions, macros and recipes on the shortcut bar and remove them, like
Java: client `RequestShortCutReg`, `RequestShortCutDel` (and `RequestShortCutReg` for the auto-shot
icons if Java does), server `ShortCutRegister` (exists, skill form only: complete it), `ShortCutDelete`
(check `ShortCutInit` rules), `character_shortcuts` rows exactly as Java writes them (D-39), the
shortcut level update when a skill level changes (`Player.updateShortcuts`), and deleting the shortcut
of a removed skill (the NOTEs in `rewards/exp.rs`, `admin/skills.rs`).

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\clientpackets\{RequestShortCutReg,RequestShortCutDel}.java`
- `...\network\serverpackets\{ShortCutRegister,ShortCutDelete,ShortCutInit}.java`
- `...\entity\actor\holders\player\Shortcuts.java` (or wherever `Shortcuts` lives: grep), `Player.java` (`registerShortcut`, `deleteShortcut`, `updateShortcuts`)
- `C:\l2-rust\PACKET_DOCS\` (quick map)

## Files
- `crates/l2-protocol-game/src/{client,server}/` (packets, dispatch, proxy decoders)
- `crates/l2-world/src/shortcuts.rs` (new) + small wire-ins
- `crates/game-server/src/{client/enter.rs,persist.rs}`, `crates/l2-db` if a query is missing
- tests next to them

## Requirements
1. Exact Java layouts, rules and DB rows; byte checks against recorded fixtures where present.

## Out of scope
- Macros editing (later).

## Tests (must exist and pass)
- Register/delete per type, level update, removed skill, DB rows (DB test skips locally).

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`)
- [ ] committed as `M5-SHORTCUT-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read CLAUDE.md (safety rules) first.
