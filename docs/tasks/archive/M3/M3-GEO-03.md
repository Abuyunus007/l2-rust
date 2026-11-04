# M3-GEO-03: Geoengine and path pool in the game server

| Field | Value |
|---|---|
| Milestone | M3 |
| Model | sonnet |
| Review | full |
| Reference | no |
| Depends on | M3-GEO-01, M3-GEO-02 |
| Size | S |

## Goal
The game server loads the geodata at startup exactly where Java does (GeoEngine.ini `GeoDataPath` resolved against `DatapackRoot`, Rust-only per-region override in a new `config/GeoOverride.ini`, D-26/D-36), and starts the path worker pool. Both are reachable from the world (`Arc<GeoEngine>`) and from the movement card (path requests, results back as `WorldCommand`s).

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\GameServer.java` (where GeoEngine/PathFinding are initialised and what is logged)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\config\GeoEngineConfig.java`

## Reference implementation to copy
- crates/game-server/src/lib.rs `start_with` (datapack loading on `spawn_blocking`), crates/l2-geo (engine `GeoEngine::load`, `GeoOverride::from_ini`, `PathPool`)

## Files
- `crates/game-server/src/geo.rs` (new: load + pool wiring)
- `crates/game-server/src/{lib.rs,state.rs}` (startup step, `GameState.geo`)
- `crates/l2-world` (only: a way to hand the world the `Arc<GeoEngine>`, e.g. `World::set_geo` + a field; and `WorldCommand::PathFound` carrying `l2_geo::PathResult`)
- `config/GeoOverride.ini` documentation in `docs/runbooks` or the module docs (no Java ini file is changed, D-36)
- tests

## Requirements
1. Startup log lines like Java's (loaded regions, path finding on/off); a failed or empty geodata load is not fatal (Java runs without geodata).
2. PathFinding = 0: no pool. Pool size: Rust-only setting in GeoOverride.ini (or a new GeoEngineRust.ini), default = number of CPUs / 2, at least 1.
3. Results of the pool become `WorldCommand::PathFound { requester, request_id, result }` on the world channel (the world ignores them until M3-MOVE-01).
4. Shutdown stops the pool cleanly before the world stops.

## Out of scope
- Using geodata or paths in the world (M3-MOVE-01, M3-NPC-01).

## Tests (must exist and pass)
- Config resolution (relative/absolute GeoDataPath, override file present/absent/malformed), startup without geodata folder, pool wiring with a synthetic engine.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass
- [ ] committed as `M3-GEO-03: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read `docs/design/m3-world.md` and `docs/design/world-loop.md` first. Then the CLAUDE.md safety rules (never touch a local database or real ports).
- Keep edits to l2-world minimal: M3-WREG-01 (the orchestrator) is changing world.rs at the same time.
