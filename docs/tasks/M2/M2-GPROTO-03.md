# M2-GPROTO-03: Enter-world small packets and SystemMessage

| Field | Value |
|---|---|
| Milestone | M2 |
| Model | haiku |
| Review | spot |
| Reference | no |
| Depends on | M2-GPROTO-01, M2-GEN-01 |
| Size | M |

## Goal
The remaining packets EnterWorld sends for a plain character, and SystemMessage with parameters.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\clientpackets\EnterWorld.java (the list of packets it sends)`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\serverpackets\{SkillList,ShortcutInit,QuestList,HennaInfo,FriendList,EtcStatusUpdate,ExStorageMaxCount,ExBasicActionList,ExVoteSystemInfo,ExGetBookMarkInfoPacket,ValidateLocation,SystemMessage,ExShowScreenMessage,SkillCoolTime,ExUISetting}.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\clientpackets\RequestKeyMapping.java (client asks for it during enter)`

## Reference implementation to copy
- crates/l2-protocol-game/src/server/charscreen.rs and tests/charscreen.rs

## Files
- `crates/l2-protocol-game/src/server/enterworld.rs`
- `crates/l2-protocol-game/src/server/system_message.rs`
- `crates/l2-protocol-game/src/client/enterworld.rs (EnterWorld, RequestKeyMapping, other client packets sent during entering)`
- `crates/l2-protocol-game/tests/enterworld_small.rs`

## Requirements
1. SystemMessage: id + typed params (string, number, item name, skill name, …) exactly like Java's SystemMessage param encoding.
2. Reference captures of the owner's Java GS sessions: `C:\l2-rust\run\captures\*.jsonl` (local only, never commit them raw; scrubbed copies go to `fixtures/captures/` via M2-CAP-01). Byte-exact against captures.

## Out of scope
- Packets for features not in M2 (clan, pets, …): list them in the report.

## Tests (must exist and pass)
- Capture comparison; round trips.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass
- [ ] committed as `M2-GPROTO-03: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read `docs/design/world-loop.md` and the CLAUDE.md safety rules first (never touch a local database or real ports).
- The owner's real Java GS and DB run side by side with our servers (D-39): anything written to the DB must look exactly like Java writes it.
