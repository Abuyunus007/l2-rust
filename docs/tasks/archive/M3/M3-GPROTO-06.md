# M3-GPROTO-06: Target, action and NPC dialog packets

| Field | Value |
|---|---|
| Milestone | M3 |
| Model | haiku |
| Review | spot |
| Reference | no (copy M2-GPROTO-03) |
| Depends on | none |
| Size | S |

## Goal
Packet structs for clicking and talking: server `MyTargetSelected`, `TargetSelected`, `TargetUnselected`, `ActionFailed`, `NpcHtmlMessage`, `DoorInfo`, `DoorStatusUpdate`, `StaticObjectInfo`; client `Action`, `RequestTargetCanceld`, `RequestBypassToServer`, `RequestLinkHtml`.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\serverpackets\{MyTargetSelected,TargetSelected,TargetUnselected,ActionFailed,NpcHtmlMessage,DoorInfo,DoorStatusUpdate,StaticObjectInfo}.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\clientpackets\{Action,RequestTargetCanceld,RequestBypassToServer,RequestLinkHtml}.java (read part only)`

## Reference implementation to copy
- crates/l2-protocol-game/src/server/enterworld.rs, tests/enterworld_small.rs

## Files
- `crates/l2-protocol-game/src/server/interaction.rs`
- `crates/l2-protocol-game/src/client/interaction.rs`
- `dispatch.rs + mod.rs lines`
- `crates/l2-protocol-game/tests/interaction.rs`

## Requirements
1. NpcHtmlMessage carries the final html string (replacements are the caller's job); keep Java's item id field and length behaviour.
2. RequestBypassToServer keeps the raw command string (parsing is the world's job).
3. Byte checks against the committed Java fixtures `fixtures/captures/java-gs-*.jsonl` (M2-CAP-01) for every packet the fixtures contain; the owner's unscrubbed recordings in `C:\l2-rust\run\captures` via `L2_CAPTURE_DIR` (local only, never commit them).

## Out of scope
- Handlers (M3-TALK-01).

## Tests (must exist and pass)
- Round trip + layout per packet, fixture comparison where recorded.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass
- [ ] committed as `M3-GPROTO-06: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read `docs/design/m3-world.md` (M3 rules) and `docs/design/world-loop.md` first. Then the CLAUDE.md safety rules (never touch a local database or real ports).
- The owner's real Java GS and DB run side by side with our servers (D-39): anything written to the DB must look exactly like Java writes it.
