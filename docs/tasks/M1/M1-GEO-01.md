# M1-GEO-01: Geodata file parser and region model

| Field | Value |
|---|---|
| Milestone | M1 |
| Model | sonnet |
| Review | full |
| Reference | no |
| Depends on | M1-WS-01, M1-JV-03 |
| Size | M |

## Goal
`l2-geo` parses `.l2j` region files into an in-memory model with the block-level queries Java has, matching the Java vectors on both of your geodata sets.

## Java source (the spec)
Base: `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\geoengine\geodata\`
- `regions\Region.java` (block order, `getBlock` indexing), `regions\NullRegion.java` (what a missing region returns)
- `blocks\FlatBlock.java`, `ComplexBlock.java` (height = signed `(v & 0xFFF0) >> 1`), `MultilayerBlock.java` (layer search: nearest, next lower, next higher, ties)
- `IBlock.java`, `IRegion.java`, `Cell.java`
- `docs/analysis/05-geodata.md`

## Reference implementation to copy
- `tools/analysis/geocmp.py` (a working parser prototype, for orientation only)

## Files
Create:
- `crates/l2-geo/{Cargo.toml,src/lib.rs,src/region.rs,src/block.rs,src/cell.rs,src/error.rs}`
- `crates/l2-geo/tests/vectors.rs`

## Requirements
1. `Region::parse(bytes: &[u8]) -> Result<Region, GeoError>`: validates block types, layer counts (1..=125, like Java), exact length (trailing bytes = error).
2. Compact memory: flat blocks store one i16; complex blocks 64 × u16; multilayer blocks one byte buffer + per-cell offsets (like Java's `_data` approach). One region of the converted set must stay under ~3× its file size in memory.
3. Queries on `Region` with Java semantics: `nearest_z(geo_x, geo_y, world_z)`, `next_lower_z`, `next_higher_z`, `check_nearest_nswe(…, nswe)`, `nearest_nswe`, `block_type`. `NullRegion` behaviour for absent regions.
4. Runtime NSWE edits (`set_nearest_nswe`/`unset_nearest_nswe`, incl. flat→complex conversion) are **not** needed now. Leave a `// TODO(M3)` with the Java reference.
5. A loader for a whole directory: `GeoSet::load_dir(path)` (memory-mapping is optional; reading is fine), with region coordinates from file names `X_Y.l2j`.

## Out of scope
- World↔geo coordinate conversion, LOS, movement, pathfinding, per-region override (all M3).

## Tests
- All `geo_*.json` vector samples match (both sets, all 5 regions).
- Block type counts match `docs/analysis/05-geodata-comparison.csv` for those regions.
- When `C:\l2-rust\geodata-*` exist, every file in both sets parses (gated, `#[ignore]` by default + run in the report).

## Done when
- [ ] vector tests pass
- [ ] committed as `M1-GEO-01: <summary>`
