# M2-GEN-02: Game config generator

| Field | Value |
|---|---|
| Milestone | M2 |
| Model | sonnet |
| Review | full |
| Reference | yes (all 61 game config classes) |
| Depends on | none |
| Size | M |

## Goal
Generate Rust config structs for all 61 game config classes from the Java sources: every simple `NAME = config.getX("Key", default);` line becomes a typed field with key and default. Everything else is listed for hand-porting.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\config\*.java and config\custom\*.java (61 classes)`
- `C:\l2-rust\l2jmobius\dist\game\config\*.ini and Custom\*.ini`

## Reference implementation to copy
- crates/l2-config/src/login.rs (hand-written shape the output must match: struct per class, `/// Key: X, default Y` docs, `from_ini`), crates/codegen opcodes target

## Files
- `crates/codegen/src/targets/config.rs`
- `crates/l2-config/src/generated/game/*.rs + mod.rs`
- `crates/l2-config/src/game.rs (GameConfig aggregate: `load(config_dir)`)`
- `docs/analysis/config-manual-fields.md (generated report: every non-simple field with its Java code line)`

## Requirements
1. ~1,030 simple lines are generated (check the real count and report it). Types from the getter (getInt→i32, getLong→i64, getBoolean→bool, getString→String, getFloat→f32, getDouble→f64, getByte/getShort).
2. Defaults that are expressions or constants (e.g. `Integer.MAX_VALUE`, other fields) are translated when trivial, otherwise the field goes to the manual list.
3. Non-simple fields get a `// TODO(M2-CFG-01): <java line>` placeholder field with the Java default if known, so the struct compiles.
4. The generator fails loudly on lines it can't classify (no silent drops).

## Out of scope
- Hand-porting the manual fields (M2-CFG-01).

## Tests (must exist and pass)
- Generated GameConfig loads all dist config files without errors; spot checks of 10 keys across files; an empty dir gives defaults.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass
- [ ] committed as `M2-GEN-02: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read `docs/design/world-loop.md` and the CLAUDE.md safety rules first (never touch a local database or real ports).
- The owner's real Java GS and DB run side by side with our servers (D-39): anything written to the DB must look exactly like Java writes it.
