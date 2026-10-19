# M4-HENNA-01: Load the character's dyes and show them

| Field | Value |
|---|---|
| Milestone | M4 |
| Model | haiku |
| Review | spot |
| Reference | no (copy how the session loader reads `character_skills` / shortcuts) |
| Depends on | M4-STAT-01, M4-DATA-12 |
| Size | S |

## Goal
Java `Player.restoreHenna`: read `character_hennas` (`charId`, `class_index`) at enter world, fill
`PlayerProfile.hennas` (slot 1..3 → `symbol_id`, Java's skips for bad slots and 0), and send the
real `HennaInfo` (and the henna numbers `UserInfo` prints) instead of the empty one. The stat engine
(M4-STAT-01) already applies `FuncHenna` from `PlayerProfile.hennas`.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\Player.java` (`restoreHenna`, `RESTORE_CHAR_HENNAS`, `recalcHennaStats`, `getHennaStat*`, `getHennaEmptySlots`)
- `...\network\serverpackets\HennaInfo.java`, `UserInfo.java` (henna fields)
- `C:\l2-rust\PACKET_DOCS\03_server_to_client.md` (HennaInfo layout, quick check)

## Files
- `crates/l2-db/src/game/` (a read query for `character_hennas`, next to the existing character queries)
- `crates/game-server/src/client/enter.rs` or the session loader that builds `PlayerProfile` (whichever loads skills today)
- `crates/l2-world/src/enter.rs` (send the real `HennaInfo`)
- tests next to the files you change

## Requirements
1. Same SQL as Java's `RESTORE_CHAR_HENNAS`, same skips, active class index.
2. `HennaInfo` bytes exactly as Java for 0..3 dyes; `UserInfo` henna stat fields from the engine.
3. DB tests skip without `L2_TEST_DATABASE_URL`.

## Out of scope
- Adding/removing dyes at the symbol maker (later).

## Tests (must exist and pass)
- Loader test (DB, skips locally), packet test with 0/1/3 dyes, a stat test: a dye with STR+1 CON-3 changes STR/CON in `UserInfo`.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`)
- [ ] committed as `M4-HENNA-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read CLAUDE.md first (safety rules: no local DB, never kill processes by name).
