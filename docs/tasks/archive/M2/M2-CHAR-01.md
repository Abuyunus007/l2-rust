# M2-CHAR-01: Character screen logic

| Field | Value |
|---|---|
| Milestone | M2 |
| Model | sonnet |
| Review | full |
| Reference | no |
| Depends on | M2-GS-03, M2-DATA-02, M2-DATA-03, M2-DATA-04, M2-DB-02, M2-CFG-01 |
| Size | M |

## Goal
List, NewCharacter templates, create (all Java validations, DB inserts, initial items/shortcuts/skills, start location), delete/restore with the deletion timer, select → CharSelected, ReplyCharacters to the LS.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\clientpackets\{NewCharacter,CharacterCreate,CharacterDelete,CharacterRestore,CharacterSelect,RequestGotoLobby}.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\GameClient.java (markToDeleteChar, restore, load)`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\Player.java (create, initial setup)`

## Reference implementation to copy
- crates/game-server/src/client/session.rs

## Files
- `crates/game-server/src/client/charscreen.rs`
- `tests (unit with fakes + DB-gated)`

## Requirements
1. Characters created here must be **playable on the Java GS** (shared DB, D-39): same rows, same initial items/skills/shortcuts as Java would create. The orchestrator will verify with a real Java GS.
2. Object ids from the IdFactory (Rust range).

## Out of scope
- Entering the world (ENTER-01).

## Tests (must exist and pass)
- DB-gated create/delete/restore/select in CI; unit tests for validations.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass
- [ ] committed as `M2-CHAR-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read `docs/design/world-loop.md` and the CLAUDE.md safety rules first (never touch a local database or real ports).
- The owner's real Java GS and DB run side by side with our servers (D-39): anything written to the DB must look exactly like Java writes it.
