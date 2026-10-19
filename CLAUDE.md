# l2-rust: port of L2JMobius High Five 2.6 to Rust

A Lineage 2 H5 private server (login + game server + admin tools) ported from Java to Rust.
**The Java source is the spec.** When unsure how something behaves, read the Java code. Never guess.

## Read first (every agent, every session)
1. `docs/DECISIONS.md`: binding rules. Never break them. If a card seems to require it, stop and report.
2. `docs/MODULE_MAP.md`: which crate owns what, and the dependency layers.
3. Your task card in `docs/tasks/` (if you have one). Do exactly that card, nothing more.
4. The relevant `docs/analysis/*.md` for background.

## Safety rules (absolute)
- **Never connect to any database on this machine.** The owner's real game database runs locally. DB tests only run when `L2_TEST_DATABASE_URL` is set (CI's throwaway MariaDB). Don't probe local ports or try credentials.
- Never start servers that bind the owner's real ports (2106, 9013/9014, 7777) except in tests that use port 0.
- Never kill processes by name (`Stop-Process -Name java`, `taskkill /IM java.exe`, `pkill java`, ...): the owner's Java game server is a `java.exe`. Stop only processes you started, by their PID, after checking the command line.
- Never put credentials, account names or IPs into committed files or captures.

## Reference material (read-only, never modify, never copy into the repo wholesale)
- Java core: `C:\l2-rust\l2jmobius\java\org\l2jmobius\`
- Datapack, configs, scripts, SQL: `C:\l2-rust\l2jmobius\dist\`
- Geodata: `C:\l2-rust\geodata-download\`, `C:\l2-rust\geodata-converted\`
- Packet index (generated from the Java sources by the owner): `C:\l2-rust\PACKET_DOCS\` (`README.md` = opcode index; layouts per packet with offsets, connection states, users). A quick map only: when it and the Java code disagree, the Java code wins (its "Server handling"/"Used in" parts are regex summaries).
Use these absolute paths. They work from any git worktree.

## Commands
```
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p codegen -- <target>      # regenerate generated code
```
A task is done only when all three of fmt, clippy and test pass locally. CI runs them on Windows and Linux.

## Architecture rules (short version of DECISIONS)
- **Game state is owned by the world loop.** No `Arc<Mutex<_>>`, `RwLock`, `Rc<RefCell<_>>` or globals around game objects. Mutate through `&mut World` / the `Ctx` handle.
- **Refer to other objects by id** (`ObjectId`, `PartyId`, ...), never by reference or smart pointer.
- `l2-world` must not depend on tokio, sqlx or sockets. No DB calls or awaiting inside a tick.
- Scheduling = the world's timer wheel. Never spawn threads or tokio tasks from game logic.
- Randomness only via the world's RNG (seedable). Never `rand::thread_rng()` in game logic.
- Don't recreate the Java class hierarchy. Shared data structs + kind enums + small traits.
- Same DB schema, same `.ini` keys and defaults as Java. Don't rename or "improve" them.
- Files in `src/generated/` are produced by `codegen`. Never edit them by hand. Change the generator instead.

## Code conventions
- Rust 2024 edition, stable toolchain. Little-endian everywhere in the protocol.
- Errors: `thiserror` in libraries, `anyhow` only in binaries. No `unwrap()`/`expect()` on data from the network, files or the DB. Tests may unwrap.
- Logging: `tracing` (`info!`, `warn!`, ...). Never `println!` outside CLI tools.
- Names: Rust style (`snake_case`), but keep Java names recognizable (`RequestAuthLogin` → `RequestAuthLogin` struct, `SystemMessageId::YOU_HAVE_BEEN_DISCONNECTED...` stays as is).
- Add a short `// Java: path/To/Class.java` comment at the top of a module or type ported from Java, so reviewers can compare.
- Comments explain *why* (L2 quirks, retail behaviour), not what the code does.
- Cross-platform: no OS-specific APIs, `std::path` for paths, files with LF line endings.
- New dependencies: only ones the card allows or `docs/DECISIONS.md` names. Ask the orchestrator otherwise.

## Tests
- Byte-level code (crypto, packets, geodata, formulas) must be checked against Java vectors in `java-vectors/out/` through `l2-testkit`.
- Every packet struct gets a round-trip test (decode(encode(x)) == x) plus a vector/capture test when one exists.
- Don't weaken, skip or delete a failing test to make a card pass. Report it instead.
- Integration test files (`tests/*.rs`) start with `#![allow(clippy::unwrap_used, clippy::expect_used)]`. Clippy's test exemption only covers `#[test]` functions, not helpers.
- Tests that need the reference folders (`C:\l2-rust\l2jmobius`, geodata) or a database skip with a printed message when they're missing. CI has neither, except the MariaDB service on Linux.

## Working on a task card (worker agents)
1. You work in your own git worktree/branch. Touch only the files your card lists (plus new files in the paths it names).
2. Port behaviour faithfully from the Java files named in the card. If the Java code looks buggy, port it as is and add a `// NOTE(java-quirk):` comment. Don't fix it silently.
3. Run fmt, clippy and test with `CARGO_INCREMENTAL=0` set (the owner's disk is shared by several worktrees; the incremental cache costs gigabytes). Commit on your branch with a message `<card-id>: <summary>`.
4. Final report (your last message): what you did, files changed, test results (copy the summary lines), anything you couldn't do, and every guess you made.
5. **When stuck** (unclear Java behaviour, the card contradicts DECISIONS, a needed change is outside your files, tests you can't make pass): stop and report. Don't improvise architecture.

## Git
- `main` is protected by the review: only the orchestrator merges.
- **The repo is public.** Never commit personal data, real account names, IPs, passwords or emails.
- Never commit anything from the reference folders, geodata, captures with real account data, or secrets (`*.local.ini`, `.env`).
