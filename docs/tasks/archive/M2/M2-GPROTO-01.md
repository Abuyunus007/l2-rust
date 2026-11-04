# M2-GPROTO-01: Game packet pattern + handshake and character screen packets

| Field | Value |
|---|---|
| Milestone | M2 |
| Model | sonnet |
| Review | full |
| Reference | yes (pattern for ~700 game packets) |
| Depends on | none |
| Size | M |

## Goal
First real game packet structs in `l2-protocol-game`, following the login packet pattern, plus a state-based client dispatcher using the generated opcode tables.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\clientpackets\{ProtocolVersion,AuthLogin,NewCharacter,CharacterCreate,CharacterDelete,CharacterRestore,CharacterSelect,RequestGotoLobby}.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\serverpackets\{KeyPacket,CharSelectionInfo,NewCharacterSuccess,CharCreateOk,CharCreateFail,CharDeleteSuccess,CharDeleteFail,CharSelected,ServerClose}.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\GamePacketHandler.java, ServerPackets.java (writeId incl. ex ids)`

## Reference implementation to copy
- crates/l2-protocol-login/src/{client.rs,server.rs,dispatch.rs} (the M1 pattern) and crates/l2-protocol-game/src/generated/opcodes.rs

## Files
- `crates/l2-protocol-game/src/{client/mod.rs,client/charscreen.rs,server/mod.rs,server/charscreen.rs,dispatch.rs,lib.rs}`
- `crates/l2-protocol-game/tests/charscreen.rs`

## Requirements
1. Server packets write their opcode via the generated ServerOpcode (incl. the EX_DUMMY quirk).
2. CharSelectionInfo takes a plain `Vec<CharSelectEntry>` (all fields Java writes, incl. paperdoll item ids, augmentation, delete timer) — no DB code here.
3. Reference captures of the owner's Java GS sessions: `C:\l2-rust\run\captures\*.jsonl` (local only, never commit them raw; scrubbed copies go to `fixtures/captures/` via M2-CAP-01). Every packet that appears in a capture must match it byte-for-byte (write a capture-driven test; skip when no capture is present).

## Out of scope
- Logic (M2-CHAR-01).

## Tests (must exist and pass)
- Round trips; hand-computed layouts; capture comparison where available.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass
- [ ] committed as `M2-GPROTO-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read `docs/design/world-loop.md` and the CLAUDE.md safety rules first (never touch a local database or real ports).
- The owner's real Java GS and DB run side by side with our servers (D-39): anything written to the DB must look exactly like Java writes it.
