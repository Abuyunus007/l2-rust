# M3-TELE-01: Teleports, zones in the world, map regions

| Field | Value |
|---|---|
| Milestone | M3 |
| Model | sonnet |
| Review | full |
| Reference | no |
| Depends on | M3-WREG-01, M3-DATA-07, M3-GPROTO-05 |
| Size | M |

## Goal
`teleToLocation` as Java (region leave/enter, TeleportToLocation, Appearing), zone membership updates when a player moves/teleports (ExSetCompassZoneCode, town/peace flags), nearest-town lookups for later restart.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\Creature.java + Player.java (teleToLocation, onTeleported)`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\clientpackets\Appearing.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\WorldRegion.java (revalidateZones)`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\managers\ZoneManager.java`

## Reference implementation to copy
- crates/l2-world (M3-WREG-01)

## Files
- `crates/l2-world/src/{teleport.rs,zones.rs}`
- `world.rs changes`
- `crates/l2-world/tests/teleport.rs`

## Requirements
1. Packet order of a teleport as in Java.
2. Zone checks only for the zone types M3 models; the rest are loaded but inert.

## Out of scope
- Zone effects (damage, no-landing ...).

## Tests (must exist and pass)
- Teleport sequence, zone enter/leave flags.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass
- [ ] committed as `M3-TELE-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read `docs/design/m3-world.md` (M3 rules) and `docs/design/world-loop.md` first. Then the CLAUDE.md safety rules (never touch a local database or real ports).
- The owner's real Java GS and DB run side by side with our servers (D-39): anything written to the DB must look exactly like Java writes it.
