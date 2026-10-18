# M3-GEO-02: Pathfinding on worker threads

| Field | Value |
|---|---|
| Milestone | M3 |
| Model | sonnet |
| Review | full |
| Reference | yes (Java vectors) |
| Depends on | M3-GEO-01 |
| Size | M |

## Goal
Port `PathFinding` + `NodeBuffer` (+ `GeoLocation`) on the immutable `Arc<GeoEngine>` and a path worker pool (plain threads) that answers requests through a channel, as described in `docs/design/m3-world.md`.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\geoengine\pathfinding\{PathFinding,NodeBuffer,GeoLocation}.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\geoengine\GeoEngine.java (findPath and its callers)`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\config\GeoEngineConfig.java (PathFinding, buffers, costs)`

## Reference implementation to copy
- crates/l2-geo (M3-GEO-01 engine) and its vector tests

## Files
- `crates/l2-geo/src/pathfinding/{mod.rs,node_buffer.rs,pool.rs}`
- `crates/l2-geo/tests/path_vectors.rs`
- `java-vectors/src/l2rust/vectors/GeoPathVectors.java + java-vectors/out/geo_path_*.json`

## Requirements
1. Same paths as Java (node lists) for fixed start/end pairs on both geodata sets, including 'no path' and buffer-exhausted cases.
2. `PathPool::new(threads, engine)` + `request(id, from, to, obstacles: Arc<dyn DynamicObstacles>) ` -> result delivered to a caller-provided sink (no tokio). Bounded queue; a newer request for the same requester may supersede an older one (cancel by request id).
3. Java's buffer sizes/limits from config; report time per path for long distances.

## Out of scope
- Using the paths in the world (M3-MOVE-01).

## Tests (must exist and pass)
- Vector tests per set (skip without geodata), pool tests (ordering, cancel, shutdown).

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass
- [ ] committed as `M3-GEO-02: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read `docs/design/m3-world.md` (M3 rules) and `docs/design/world-loop.md` first. Then the CLAUDE.md safety rules (never touch a local database or real ports).
- The owner's real Java GS and DB run side by side with our servers (D-39): anything written to the DB must look exactly like Java writes it.
