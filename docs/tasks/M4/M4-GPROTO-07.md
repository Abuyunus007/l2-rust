# M4-GPROTO-07: Combat, death and item packets

| Field | Value |
|---|---|
| Milestone | M4 |
| Model | haiku |
| Review | spot |
| Reference | no (copy M3-GPROTO-06: `server/interaction.rs`, `client/interaction.rs`) |
| Depends on | none |
| Size | M |

## Goal
Packet structs for fighting, dying, sitting/running and handling items.
- Server: `Attack` (with its hit list), `AutoAttackStart`, `AutoAttackStop`, `Die`, `Revive`, `ChangeMoveType`, `ChangeWaitType`, `SpawnItem`, `DropItem`, `GetItem`, `ExUseSharedGroupItem` (if `UseItem` needs it), `StopRotation`/`StartRotation` only if Java sends them on attack (check).
- Client: `AttackRequest`, `RequestActionUse`, `RequestRestartPoint`, `UseItem`, `RequestUnEquipItem`, `RequestDropItem`, `RequestDestroyItem`, `RequestItemList`.
Packets that already exist (`StatusUpdate`, `InventoryUpdate`, `ItemList`, `SocialAction`, `SystemMessage`, `MoveToPawn`) are not touched.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\serverpackets\{Attack,AutoAttackStart,AutoAttackStop,Die,Revive,ChangeMoveType,ChangeWaitType,SpawnItem,DropItem,GetItem,ExUseSharedGroupItem}.java` (+ `Hit` holder used by `Attack`)
- `...\network\clientpackets\{AttackRequest,RequestActionUse,RequestRestartPoint,UseItem,RequestUnEquipItem,RequestDropItem,RequestDestroyItem,RequestItemList}.java` (read part only)
- Opcodes: `crates/l2-protocol-game/src/generated/opcodes.rs` (generated: never edit; if an opcode is missing, report)

## Files
- `crates/l2-protocol-game/src/server/combat.rs`, `server/ground_item.rs` (new)
- `crates/l2-protocol-game/src/client/combat.rs`, `client/item.rs` (new)
- `server/mod.rs`, `client/mod.rs`, client dispatch lines
- `crates/l2-protocol-game/tests/combat.rs` (new)

## Requirements
1. Byte layout exactly as Java writes it (`Die` flags for the restart points; `Attack` flags per hit: soulshot grade, crit, shield, miss).
2. `Die` and `Attack` take plain values; how the flags are computed is the world's job, but provide the Java flag constants.
3. Byte checks against `fixtures/captures/java-gs-*.jsonl` for any packet the fixtures contain; local recordings via `L2_CAPTURE_DIR` (never commit them).

## Out of scope
- Handlers (COMBAT-01, INV-01, DEATH-01, DROP-01).

## Tests (must exist and pass)
- Round trip + layout per packet; fixture comparison where recorded.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`)
- [ ] committed as `M4-GPROTO-07: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read `docs/design/m4-combat.md` first, then CLAUDE.md safety rules.
