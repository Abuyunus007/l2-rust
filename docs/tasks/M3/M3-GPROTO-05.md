# M3-GPROTO-05: Visibility and movement packets

| Field | Value |
|---|---|
| Milestone | M3 |
| Model | sonnet |
| Review | full |
| Reference | no (copy M2-GPROTO-02) |
| Depends on | none |
| Size | M |

## Goal
Packet structs for seeing and moving: server `CharInfo`, `NpcInfo` (AbstractNpcInfo), `MoveToLocation`, `StopMove`, `ValidateLocation` (exists, check), `TeleportToLocation`, `DeleteObject`, `StatusUpdate` (exists? check), `ExSetCompassZoneCode`, `RelationChanged`, `ExBrExtraUserInfo`, `MoveToPawn`; client `MoveToLocation` (MOVE_BACKWARD_TO_LOCATION), `ValidatePosition`, `CannotMoveAnymore`, `Appearing`.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\serverpackets\{CharInfo,AbstractNpcInfo,MoveToLocation,StopMove,TeleportToLocation,DeleteObject,StatusUpdate,ExSetCompassZoneCode,RelationChanged,ExBrExtraUserInfo,MoveToPawn}.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\clientpackets\{MoveToLocation,ValidatePosition,CannotMoveAnymore,Appearing}.java (read part only)`

## Reference implementation to copy
- crates/l2-protocol-game/src/server/userinfo.rs (big packet with plain fields), tests/enterworld_big.rs (fixture comparison)

## Files
- `crates/l2-protocol-game/src/server/{visibility.rs,movement.rs}`
- `crates/l2-protocol-game/src/client/movement.rs`
- `dispatch.rs + mod.rs lines`
- `crates/l2-protocol-game/tests/visibility.rs`

## Requirements
1. Plain data structs in wire order, each field documented with the Java getter it comes from; the caller computes values (no game logic in the crate).
2. NpcInfo: port the H5 `AbstractNpcInfo.NpcInfo` layout (also Summon variants only if trivial; otherwise leave for later and say so).
3. Byte checks against the committed Java fixtures `fixtures/captures/java-gs-*.jsonl` (M2-CAP-01) for every packet the fixtures contain; the owner's unscrubbed recordings in `C:\l2-rust\run\captures` via `L2_CAPTURE_DIR` (local only, never commit them).

## Out of scope
- Sending them (world cards).

## Tests (must exist and pass)
- Round trip per packet, hand-computed layout for CharInfo and NpcInfo, fixture comparison (NPC_INFO, CHAR_INFO if recorded, MOVE_TO_LOCATION, VALIDATE_LOCATION...).

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass
- [ ] committed as `M3-GPROTO-05: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read `docs/design/m3-world.md` (M3 rules) and `docs/design/world-loop.md` first. Then the CLAUDE.md safety rules (never touch a local database or real ports).
- The owner's real Java GS and DB run side by side with our servers (D-39): anything written to the DB must look exactly like Java writes it.
