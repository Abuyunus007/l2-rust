# M1-CFG-02: Login server config structs

| Field | Value |
|---|---|
| Milestone | M1 |
| Model | sonnet |
| Review | full |
| Reference | yes (hand-written shape the M2 generator will reproduce for the 61 game config classes) |
| Depends on | M1-CFG-01 |
| Size | S |

## Goal
Typed, immutable config structs for the login server (and the shared database/thread configs), loaded from the same files with the same keys and defaults as Java.

## Java source (the spec)
Base: `C:\l2-rust\l2jmobius\java\org\l2jmobius\`
- `loginserver\config\LoginConfig.java` (reads `./config/Server.ini`)
- `commons\config\DatabaseConfig.java` (`Database.ini`), `commons\config\ThreadConfig.java` (`Threads.ini`), `commons\config\InterfaceConfig.java` (`Interface.ini`: only `EnableGUI`, ignored by Rust but parsed)
- `commons\network\ConnectionConfig.java` (`Network.ini` keys used by networking: `ShutdownWaitTime`, `DropPackets`, `DropPacketThreshold`, `ThreadPoolSize`. BufferPool keys are parsed and ignored)
- Files: `C:\l2-rust\l2jmobius\dist\login\config\*.ini`

## Reference implementation to copy
- `crates/l2-config/src/reader.rs`

## Files
Create:
- `crates/l2-config/src/login.rs`, `src/database.rs`, `src/threads.rs`, `src/network.rs`, `src/interface.rs`
- `crates/l2-config/tests/login_config.rs`
Modify:
- `crates/l2-config/src/lib.rs` (`mod` + re-export only)

## Requirements
1. One struct per Java config class, e.g. `LoginConfig { login_bind_address: String, port_login: u16, ... }`. Field names are snake_case of the Java constant names. Each field has a doc comment `/// Key: `LoginserverPort`, default 2106`.
2. `LoginConfig::load(config_dir: &Path) -> LoginConfigSet` loads all 5 files. Defaults are exactly the Java defaults, including derived values (e.g. `LOGIN_BLOCK_AFTER_BAN` seconds).
3. `Database.ini`: parse the JDBC `URL` (`jdbc:mysql://host[:port]/db?params`) into host/port/database + params, plus `Login`/`Password`/`MaximumDatabaseConnections`. Keep the raw URL too. Backup keys are parsed but unused for now.
4. Structs are `Clone + Debug`, used as `Arc<…>`. **Never log the password** (custom `Debug`).

## Out of scope
- Game server configs (M2). A generator (M2).

## Tests
- Loads `C:\l2-rust\l2jmobius\dist\login\config` (skipped when absent) and compares a handful of fields with the file values.
- An empty config dir gives all Java defaults (assert every field).

## Done when
- [ ] tests pass
- [ ] committed as `M1-CFG-02: <summary>`

## Notes from the orchestrator
From the CFG-01 report: `get_enum` needs a **case-sensitive** `FromStr` (Java `Enum.valueOf`). `get_bool` never falls back on bad values (anything but "true" is false). Trailing blanks in values are kept, so ints with a trailing space fall back to the default. Port the Java defaults with that in mind; don't trim.
