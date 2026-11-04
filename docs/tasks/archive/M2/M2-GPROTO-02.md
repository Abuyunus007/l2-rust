# M2-GPROTO-02: UserInfo and ItemList

| Field | Value |
|---|---|
| Milestone | M2 |
| Model | sonnet |
| Review | full |
| Reference | no |
| Depends on | M2-GPROTO-01 |
| Size | M |

## Goal
The two big enter-world packets, with a shared item-info writer reused by later inventory packets.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\serverpackets\{UserInfo,ItemList,AbstractItemPacket,AbstractInventoryUpdate}.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\item\instance\Item.java (what ItemInfo reads)`

## Reference implementation to copy
- crates/l2-protocol-game/src/server/charscreen.rs

## Files
- `crates/l2-protocol-game/src/server/{userinfo.rs,items.rs}`
- `crates/l2-protocol-game/tests/enterworld_big.rs`

## Requirements
1. Plain data structs (no world types): every field UserInfo writes, in order, with comments naming the Java getter.
2. Reference captures of the owner's Java GS sessions: `C:\l2-rust\run\captures\*.jsonl` (local only, never commit them raw; scrubbed copies go to `fixtures/captures/` via M2-CAP-01). Byte-exact against captured UserInfo/ItemList.

## Out of scope
- nothing beyond the files above

## Tests (must exist and pass)
- Capture comparison (skip if absent); round trips.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass
- [ ] committed as `M2-GPROTO-02: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read `docs/design/world-loop.md` and the CLAUDE.md safety rules first (never touch a local database or real ports).
- The owner's real Java GS and DB run side by side with our servers (D-39): anything written to the DB must look exactly like Java writes it.
