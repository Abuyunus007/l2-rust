# M2-GEN-01: SystemMessageId and NpcStringId generator

| Field | Value |
|---|---|
| Milestone | M2 |
| Model | sonnet |
| Review | spot |
| Reference | no (copy M1-GEN-01) |
| Depends on | none |
| Size | S |

## Goal
Generated `SystemMessageId` (3,236 entries) and `NpcStringId` (5,915) with id, Java name and message text, into `l2-protocol-game/src/generated/`.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\SystemMessageId.java (`@ClientString(id, message)` + field)`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\NpcStringId.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\ClientString.java`

## Reference implementation to copy
- crates/codegen/src/targets/opcodes.rs and the GAME_GENERATED_MODS rule in crates/codegen/src/targets/mod.rs

## Files
- `crates/codegen/src/targets/messages.rs`
- `crates/codegen/src/targets/mod.rs (register target)`
- `crates/codegen/src/main.rs (subcommand)`
- `crates/l2-protocol-game/src/generated/{system_message_id.rs,npc_string_id.rs,mod.rs}`

## Requirements
1. `SystemMessageId::id() -> i32`, `name()`, `message()`, `from_id()`; same for NpcStringId. Rust variant names CamelCase, `name()` returns the Java constant.
2. Message param count derived from `$s1`/`$c1`… placeholders if the Java code does that (check ClientString / SystemMessage).
3. `--check` support like opcodes; counts asserted in a test.

## Out of scope
- SystemMessage packet encoding (M2-GPROTO-03).

## Tests (must exist and pass)
- Counts, a few spot checks by id, round trip from_id(id()).

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass
- [ ] committed as `M2-GEN-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read `docs/design/world-loop.md` and the CLAUDE.md safety rules first (never touch a local database or real ports).
- The owner's real Java GS and DB run side by side with our servers (D-39): anything written to the DB must look exactly like Java writes it.
