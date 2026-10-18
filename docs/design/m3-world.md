# Design: world, movement and NPCs (M3)

Builds on `docs/design/world-loop.md` (single-owner world, D-10..D-15). Everything below lives in
the world thread unless it says otherwise. The Java source stays the spec for behaviour; this
note only fixes *where* things live in Rust and how the threads talk.

## Objects in the world
- One id space (`ObjectId`), one table per kind inside `World`: `players`, `npcs`, `doors`
  (fences, items on the ground, summons come later). A `WorldObjectRef` enum (`Player(id)`,
  `Npc(id)`, `Door(id)`) is what regions and targets store (D-11: ids, never references).
- Shared, immutable data comes from `Arc<Datapack>` (NPC templates, html, teleport lists,
  zones, map regions, doors) and `Arc<GeoEngine>`. The world never reads files or the DB.
- NPCs (M3: no AI) = `CreatureCore` + template id + spawn reference. Spawned once at startup
  from `SpawnData`; respawn after death is M4.

## Regions and visibility (port of `entity/World.java`, `entity/WorldRegion.java`)
- Same 3D region grid as Java: `SHIFT_BY = 11` (2048 units), `Z_REGION_SIZE = 2000`, the same
  world bounds and offsets, the same surrounding-region sets. Same "active region" bookkeeping
  (`setActive`, neighbour counters), even though nothing uses activity before NPC AI (M4).
- Visibility = Java's: objects in the surrounding regions (and the range checks of the call
  site). Adding, removing and region switching send exactly the packets Java sends
  (`CharInfo`/`NpcInfo`/`DoorInfo`.../`DeleteObject`) to exactly the players Java sends them to.
- Broadcast helpers (`broadcast_to_visible_players`, `..._in_range`) are the only way game logic
  sends packets to other players. Chat ranges (normal chat 1250, shout/trade per Java) use them.

## Movement (port of `Creature.moveToLocation`, `updatePosition`, `MovementTaskManager`)
- Position updates run in the tick, with Java's formula and Java's update interval.
- A move request is checked with the geoengine in the world thread (cheap, D-14 allows it).
  When Java would pathfind (`PathFinding` > 0 and the direct line is blocked), the request goes
  to a **path worker pool** (plain threads, not tokio) with `Arc<GeoEngine>` and a door snapshot;
  the result comes back as `WorldCommand::PathFound { object, request, path }`. A newer move
  request cancels an older pending one (request id).
- `ValidatePosition` from the client: Java's correction rules (`ValidateLocation`, Z correction,
  desync handling), nothing invented.

## Geodata and doors
- `GeoEngine` is built once at startup (blocking task) and shared as `Arc<GeoEngine>` (D-26:
  per-region override from M3-GEO-01).
- Doors/fences: the world owns their state (open/closed). The geoengine asks through the
  `DynamicObstacles` trait: in the world thread an implementation over the world's door table,
  for path workers an immutable `Arc<DoorSnapshot>` taken when the request is sent.

## NPC interaction
- `Action` (click): target, second click = talk/attack-intent as Java does; html via
  `NpcHtmlMessage` with Java's replacements (`%objectId%`, ...).
- Html is loaded into an in-memory cache at startup (`data/html/**`, Java `HtmCache` with
  `LazyCache=false`). If the owner's config says `LazyCache=true`, Rust still preloads (no file
  IO in the world) and logs it once; the result for the client is the same.
- `RequestBypassToServer` and admin commands (`//...`) go through **handler registries** in the
  world: `name -> fn(&mut World, ObjectId, &args)`. Handlers are ported from the Java script
  handlers. D-05 ports scripts last; M3 only ports the few handlers the PLAN names (bypasses for
  html links and gatekeepers, the core admin commands), so the handler API can still change. Access rights come from `AccessLevels.xml` /
  `AdminCommands.xml` exactly like Java.

## Teleports, zones, map regions
- `teleToLocation` as Java: leave old regions (DeleteObject to others), `TeleportToLocation`,
  wait for the client's `Appearing`, enter new regions.
- Zones (M3 basic): shapes and the zone types the client shows (town, peace, ...) for
  `ExSetCompassZoneCode` and for restart points; zone scripts/effects come later. Map regions
  for "nearest town".

## Testing
- Every new packet: round trip + byte check against the committed Java fixtures
  (`fixtures/captures/java-gs-*.jsonl`, M2-CAP-01) where the fixture has it.
- World logic: unit tests with a recording outbox and two fake players (visibility, movement
  steps, region switches).
- End to end: two fake clients against the in-process LS + GS (M3-E2E-02).
