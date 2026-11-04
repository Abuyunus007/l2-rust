# M3-ADMIN-01: Core admin commands

| Field | Value |
|---|---|
| Milestone | M3 |
| Model | haiku |
| Review | spot |
| Reference | no |
| Depends on | M3-TALK-01 |
| Size | S |

## Goal
Admin command framework (access levels from AccessLevels.xml/AdminCommands.xml, `//cmd` from chat) and the core commands for testing: //admin (panel html), //spawn, //teleport //teleportto //recall //move_to, //loc, //kill, //heal, //goto.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\chat\commands\admin\{AdminAdmin,AdminSpawn,AdminTeleport,AdminKill,AdminHeal}.java (+ //loc's handler)`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\data\xml\{AdminData,AccessLevels}*.java`
- `C:\l2-rust\l2jmobius\dist\game\config\{AccessLevels,AdminCommands}.xml`
- `C:\l2-rust\l2jmobius\dist\game\data\html\admin\`

## Reference implementation to copy
- crates/l2-world (bypass registry of M3-TALK-01)

## Files
- `crates/l2-world/src/admin/{mod.rs,commands.rs}`
- `crates/l2-data access level loader`
- `crates/l2-world/tests/admin.rs`

## Requirements
1. Same access checks and messages as Java; GM audit log line per command (D-25).
2. //kill and //heal only touch HP/MP as far as M3 models them (death = M4: log it).

## Out of scope
- Other admin commands.

## Tests (must exist and pass)
- Each command with a fake GM and a fake normal player (refused).

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass
- [ ] committed as `M3-ADMIN-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read `docs/design/m3-world.md` (M3 rules) and `docs/design/world-loop.md` first. Then the CLAUDE.md safety rules (never touch a local database or real ports).
- The owner's real Java GS and DB run side by side with our servers (D-39): anything written to the DB must look exactly like Java writes it.
