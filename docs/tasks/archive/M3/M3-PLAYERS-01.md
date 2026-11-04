# M3-PLAYERS-01: Other players visible (two clients)

| Field | Value |
|---|---|
| Milestone | M3 |
| Model | opus (orchestrator) |
| Review | full |
| Reference | no |
| Depends on | M3-MOVE-01 |
| Size | M |

## Goal
Two clients see each other: CharInfo on entering view, movement broadcasts, UserInfo/CharInfo updates on changes, DeleteObject on leave, chat to players in range per Java chat handlers.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\Player.java (broadcastUserInfo, broadcastCharInfo)`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\chat\ (ChatGeneral, ChatShout, ChatTrade, ChatWhisper ranges)`

## Reference implementation to copy
- crates/l2-world (M3-WREG-01)

## Files
- `crates/l2-world/src/{player_view.rs,chat.rs changes}`
- `crates/l2-world/tests/two_players.rs`

## Requirements
1. Same recipients and packets as Java.

## Out of scope
- nothing beyond the files above

## Tests (must exist and pass)
- Two fake players: see, move, chat, leave.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass
- [ ] committed as `M3-PLAYERS-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read `docs/design/m3-world.md` (M3 rules) and `docs/design/world-loop.md` first. Then the CLAUDE.md safety rules (never touch a local database or real ports).
- The owner's real Java GS and DB run side by side with our servers (D-39): anything written to the DB must look exactly like Java writes it.
