# Runbook: the world on the Rust GS (M3)

Setup as in `docs/runbooks/m2-game-test.md` (Rust GS "Kain" next to the Java GS on the same DB).
M3 adds geodata, NPC spawns, doors, teleports, zones, NPC dialogs, gatekeepers and the core admin
commands.

## 1. Geodata

The Rust GS reads `GeoEngine.ini` from `run/game/config`. `GeoDataPath` is resolved against
`DatapackRoot`; an absolute path is used as it is. Two sets exist on the owner's machine
(`docs/analysis/05-geodata.md`): *download* and *converted*. Until the decision below,
`run/game/config/GeoEngine.ini` points at the converted set:

```
GeoDataPath = C:/l2-rust/geodata-converted/
PathFinding = 2
```

Optional, Rust-only `run/game/config/GeoOverride.ini` (D-26, M3-GEO-03): read some regions from
another folder:

```
GeoDataOverridePath = C:/l2-rust/geodata-download/
GeoDataOverrideRegions = 12_24,17_10
PathWorkerThreads = 4
```

Startup logs `GeoEngine: Loaded N regions.` and `Path finding is on (...)`. Memory: about 1.6 GB
for the converted set.

Note: the owner's Java GS currently runs **without** geodata (its `data/geodata` holds only a
Readme). Copy one set into `<java>/game/data/geodata/` to give it walls and terrain too.

## 2. What starts with the world

| Log line | Meaning |
|---|---|
| `DoorData: Loaded 1301 doors.` / `FenceData` / `StaticObjectData` | doors, fences, town maps etc. |
| `SpawnData: 40379 spawns have been initialized!` | Java's count rule |
| `NPC spawns: ~42,400 NPCs in the world (~2,700 minions)` | including minions and day spawns |
| `Cache[HTML]: ... on 21229 files loaded.` | all dialogs preloaded |
| `ZoneManager: Loaded 32 zone classes and 2860 zones.` | towns, peace zones, ... |

NPCs have **no AI** in M3 (they stand at their spawn) and nothing can be attacked yet (M4).

## 3. Checkpoints

### CP-M3a — walk around, see NPCs
1. Enter the world on Kain (second row).
2. NPCs and monsters stand where they stand on the Java GS (compare with Sieghardt).
3. Walk: click on the ground — the character runs there; it stops at walls (geodata) and at
   closed doors; clicking behind a wall makes it path around (pathfinding).
4. Town/peace zone: the compass icon next to the minimap shows the zone; entering a town gives the
   zone message.
5. **Geodata decision (D-26):** walk at a few spots where the two sets differ (regions 12_24,
   17_10, 16_10, 14_23 — e.g. use the admin GUI's Geodata view to see them side by side) and tell
   which set is right there. That decides the default set and the override list.

### CP-M3b — talk and teleport, two players
1. Click an NPC: it is targeted (a second click talks). A gatekeeper shows its dialog; choose a
   destination: adena are taken, the character teleports (loading screen) and arrives there.
2. Two clients (two accounts) on Kain: each sees the other walking, chatting (normal chat only
   within ~1250 range), and disappearing on logout.

### CP-M3c — milestone
Walk the world with geodata, NPCs and doors, a second player, gatekeepers, and the admin commands
(a GM account, `accesslevel` > 0 in `characters`): `//admin`, `//spawn <npcId>`,
`//teleport <x> <y> <z>`, `//loc`, `//kill`, `//heal`.

## 4. Known limits in M3
- No NPC AI, no combat, no death, no respawn, no drops (M4).
- P.Atk/P.Def in the character window are base values without gear (stat engine, M4).
- No quests, shops or skills beyond what the player owns (M5+).
