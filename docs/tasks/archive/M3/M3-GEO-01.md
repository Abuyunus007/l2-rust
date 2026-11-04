# M3-GEO-01: Geoengine (height, line of sight, movement checks)

| Field | Value |
|---|---|
| Milestone | M3 |
| Model | sonnet |
| Review | full |
| Reference | yes (Java vectors) |
| Depends on | M1 l2-geo (parser, region model, block queries) |
| Size | L |

## Goal
Port `GeoEngine` on top of the existing `l2-geo` region model: loading the whole world from
`GeoDataPath` with a **per-region override list** (D-26), world <-> geo coordinates, `getHeight`,
`hasGeoPos`, `canSeeTarget` (line of sight), `canMoveToTarget` / `getValidLocation` (movement
check with NSWE), and the hook for dynamic obstacles (doors, fences). Every query is checked
against vectors from the real Java `GeoEngine` on **both** geodata sets.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\geoengine\GeoEngine.java` (all of it)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\geoengine\util\{GridLineIterator2D,GridLineIterator3D}.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\geoengine\geodata\**` (already ported in M1; extend only if a query needs it)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\config\GeoEngineConfig.java`, `dist\game\config\GeoEngine.ini`
- How doors and fences enter the checks: `DoorData`, `FenceData` calls inside GeoEngine (only the call sites; doors/fences themselves come later)

## Reference implementation to copy
- crates/l2-geo (M1: `GeoSet`, `Region`, block queries), crates/l2-geotool (loading both sets)
- java-vectors/src/l2rust/vectors (the M1 geo generator) and `java-vectors/out/geo_*.json`

## Files
- `crates/l2-geo/src/{engine.rs,line.rs,coords.rs,obstacles.rs}` (new) + `lib.rs` exports
- `crates/l2-geo/tests/engine_vectors.rs`
- `java-vectors/src/l2rust/vectors/GeoEngineVectors.java` (new generator) + `java-vectors/out/geo_engine_*.json`
- `crates/l2-config` only if a GeoEngine.ini field is missing or needs a manual port

## Requirements
1. Same results as Java for every ported query, both sets (`C:\l2-rust\geodata-download`, `C:\l2-rust\geodata-converted`), at the regions of `docs/analysis/05-geodata.md` (12_24, 13_21, 14_23, 16_10, 22_22, 17_10) incl. edge cases: region borders, missing regions (null region = flat/no geo), multilayer cells, z far above/below.
2. Per-region override (D-26): a config value (new Rust-only key, documented, default empty) listing regions to load from a second path. Unknown/malformed entries are logged and ignored.
3. The engine is immutable after loading and `Send + Sync` (D-14: pathfinding and LOS may run on worker threads). Doors/fences go through a trait (`DynamicObstacles`) with a no-op implementation for now.
4. Loading the whole world must not take minutes: measure and report load time and memory for both sets.

## Out of scope
- Pathfinding (`PathFinding`, `NodeBuffer`) — M3-GEO-02.
- Doors and fences themselves, movement in the world loop, zones.

## Tests (must exist and pass)
- Vector tests per query and set (skip with a message when the geodata folders are missing; CI has none — the committed vectors still run their pure parts).
- Unit tests for coordinate conversion and the line iterators (incl. Java's rounding at negative coordinates).

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass
- [ ] committed as `M3-GEO-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read CLAUDE.md safety rules first. No database, no ports in this card.
- The Java harness must stay deterministic (no clock, no unseeded randomness); seed any random sample points with a fixed seed and write the points into the JSON.
