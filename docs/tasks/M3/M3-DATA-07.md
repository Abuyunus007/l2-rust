# M3-DATA-07: Zones and map regions

| Field | Value |
|---|---|
| Milestone | M3 |
| Model | sonnet |
| Review | full |
| Reference | no |
| Depends on | none |
| Size | M |

## Goal
Loaders for `data/zones/*.xml` (zone shapes: cuboid, npoly, cylinder, + the zone type and parameters) and `data/mapregion/*.xml` (map regions, restart points, banned races, `getMapRegionLocId`/nearest town). Shape queries (`isInsideZone(x,y,z)`), zone lookup per world region like Java's ZoneManager grid.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\managers\ZoneManager.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\zone\** (ZoneForm shapes, ZoneType + the type classes for their parameters)`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\data\xml\MapRegionData.java`
- `C:\l2-rust\l2jmobius\dist\game\data\zones\**`
- `C:\l2-rust\l2jmobius\dist\game\data\mapregion\**`

## Reference implementation to copy
- crates/l2-data/src/item.rs

## Files
- `crates/l2-data/src/zone/{mod.rs,shape.rs}`
- `crates/l2-data/src/map_region.rs`
- `lib.rs lines`
- `crates/l2-data/tests/zones.rs`

## Requirements
1. Exact shape math (Java's int/float behaviour at edges).
2. Zone types loaded with their raw parameters; the M3 world uses town/peace/... flags only, the rest are data for later.
3. Nearest-town and restart point lookups as Java.

## Out of scope
- Zone enter/exit effects and scripts.

## Tests (must exist and pass)
- Real data counts, inside/outside checks at known coordinates (towns), map region of a few known spots.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass
- [ ] committed as `M3-DATA-07: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read `docs/design/m3-world.md` (M3 rules) and `docs/design/world-loop.md` first. Then the CLAUDE.md safety rules (never touch a local database or real ports).
- The owner's real Java GS and DB run side by side with our servers (D-39): anything written to the DB must look exactly like Java writes it.
