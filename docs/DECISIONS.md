# Decisions

Binding for every agent. Changing one needs a new entry that says what it replaces and why.
Background for each decision: `docs/analysis/`.

## Goals and scope

| # | Decision |
|---|---|
| D-01 | **Goal:** a playable private H5 server, behaving like retail H5 as L2JMobius does. **Done** means feature parity with L2JMobius H5 2.6 for normal play. |
| D-02 | **Configurable like Java:** same `.ini` files, same keys, same defaults, including the `Custom/` configs. Existing config files work unchanged. |
| D-03 | **Platforms:** Windows and Linux. No OS-specific APIs in server code. |
| D-04 | **Rollout:** the Rust login server comes first and must work with the **Java game server** (same LS↔GS protocol). The Rust game server grows feature by feature until it replaces Java. |
| D-05 | **Scripts** (quests, AI, handlers, instances) are ported to Rust, **last**, after the script API is stable. |

## Architecture

| # | Decision |
|---|---|
| D-10 | **Single-owner world loop** (option A, doc 04). One world task owns all game state. Network tasks parse packets and send commands over channels. No `Arc<Mutex<_>>` around game objects. |
| D-11 | **Entities in arenas keyed by ObjectId.** Every cross-reference (target, summon, party, clan, owner...) is an id, never a pointer or `Rc`. |
| D-12 | **Global 50 ms tick** for all movement, AI and periodic work (Java uses 50 ms for players and 100 ms for NPCs, a deliberate deviation). Everything Java does with `ThreadPool.schedule`, quest timers and task managers becomes a **timer wheel** inside the world. |
| D-13 | **Seedable RNG** owned by the world. Production seeds randomly. Tests use fixed seeds so replays are reproducible. |
| D-14 | Heavy pure work (pathfinding, line of sight on immutable geodata) may run on worker threads. Results come back to the world as commands. |
| D-15 | **Persistence:** game logic never awaits the DB inside a tick. Loads happen before an entity enters the world. Saves are sent to an async persistence task. |
| D-16 | **Script hot-reload: none.** Scripts are compiled in, so a reload means a restart. Data, HTML and config can still reload live. |
| D-17 | The Java inheritance tree is **not** reproduced. Use shared data structs (`CreatureCore`) plus kind enums (`ObjectKind`, `NpcKind`) plus traits only where behaviour really differs. |

## Tech stack

| # | Decision |
|---|---|
| D-20 | **Cargo workspace with many small crates** (draft: `l2-crypto`, `l2-net`, `l2-protocol`, `l2-config`, `l2-db`, `l2-geo`, `l2-data`, `login-server`, `game-server`, `admin-api`, `l2-admin-gui`, `codegen`, `java-vectors`). Final layout in `docs/MODULE_MAP.md`. |
| D-21 | **Database:** the **same MySQL/MariaDB schema as Java**, so Java and Rust can share a DB. `sqlx` (MySQL) with runtime-checked queries (no compile-time DB needed). |
| D-22 | **Async runtime:** tokio for networking, DB and admin API. The world loop is a single task/thread fed by channels. |
| D-23 | **Code generators** for repetitive code: `SystemMessageId`/`NpcStringId`, opcode tables, config structs (key, type, default), XML structs from the XSDs, packet layouts where possible. Agents write logic, not tables. |
| D-24 | Crypto: the `blowfish` crate in **little-endian** mode (`BlowfishLE`), RSA via the `rsa`/`num-bigint` crates with raw (no-padding) operations. Everything is checked against Java vectors (D-41). |
| D-25 | Logging: `tracing`, console + files, including separate chat/item/GM-audit logs like Java. |
| D-26 | Geodata: support both sets, with a **per-region override list** so sets can be mixed. Which set is the default is decided after the geotool render + an in-game check (doc 05). |
| D-27 | Rust **2024 edition**, stable toolchain. Errors: `thiserror` in libraries, `anyhow` only in binaries. |
| D-28 | **New dependencies** need approval by the orchestrator (named in the card or here). Keeps the dependency tree small and reviewed. |

## Tools and GUI

| # | Decision |
|---|---|
| D-30 | Servers run **headless** and expose an **admin API** (WebSocket + JSON, authenticated, bound to localhost by default) for control, logs, status and admin actions. |
| D-31 | **Admin GUI** in Rust with **egui/eframe**, a separate app that connects to the admin API locally or remotely (Linux host). |
| D-32 | GUI scope, eventually: server control + live logs, **packet inspector** (via a decrypting proxy: live list, decoded fields, hex, filter, save/load), admin tasks (accounts, bans, access levels, online players, kick/announce, GS registration, config editing), data browser and geodata viewer. |
| D-33 | The GUI **grows with milestones**. M1 = packet inspector + login server control/logs. Later milestones add views as features exist. |
| D-34 | Packet captures are saved as JSON lines and double as **test fixtures**. Credentials are always redacted (the repo is public). |
| D-35 | The **proxy runs inside the GUI process** (library `l2-proxy`) on the PC where the game client runs. A standalone `l2-proxy` CLI also exists. The supervisor doesn't relay captures. |
| D-36 | **Rust-only settings live in new files** (`config/Admin.ini` per server, `supervisor.ini`). The Java `.ini` files are never extended. |
| D-37 | **License:** MIT, like L2JMobius. The original `Copyright (c) 2013 L2jMobius` notice is kept in `LICENSE`. |
| D-38 | **Graceful stop:** Rust servers stop through an admin `shutdown` request (the supervisor sends it, kill only after `StopTimeoutSecs`). The Java GS is **not** managed by the supervisor on Windows (no safe Ctrl event without `unsafe`, and a kill loses player saves); it's temporary until the Rust GS replaces it. |
| D-39 | **Side by side:** the Java GS and the Rust GS run at the same time on the same DB, both registered with the Rust LS under **different server ids and ports** (Java: id 2 / 7777, Rust: its own id / 7778). The LS keeps an account on one GS at a time. The Rust GS allocates new object ids from a **separate high range** so it never collides with Java's IdManager. The supervisor runs the proxy permanently in front of the LS, so every session is recorded to `run/captures/` (local, git-ignored) as reference material. |

## Process and quality

| # | Decision |
|---|---|
| D-40 | **Agents:** Opus plans, writes task cards, builds core/architecture code and reviews. **Haiku does as much as possible** (templated work), and Sonnet takes cards that are too hard for Haiku. **Max throughput:** parallel agents. |
| D-41 | **Java oracle:** small Java harness programs call the real L2JMobius classes and dump test vectors as JSON. Rust must match byte for byte (crypto, packets, geodata queries, formulas). Captures from the real client cover end-to-end flows. |
| D-42 | **One git worktree/branch per task card.** Opus reviews and merges into `main`. |
| D-43 | **Public GitHub repo** (`Abuyunus007/l2-rust`, public by owner's choice) **+ GitHub Actions CI** on Windows and Linux: `cargo fmt --check`, `clippy -D warnings`, `cargo test`. CI must pass before a merge. |
| D-44 | **Risk-based review:** core/architecture code gets a full Opus review. Templated work is gated by automatic tests + spot checks. |
| D-45 | Planning state lives in the repo (`docs/`, `CLAUDE.md`, task cards), never only in a conversation. |
| D-46 | The Java reference (`C:\l2-rust\l2jmobius`) and the geodata folders are read-only and git-ignored. Agents reference them by absolute path. |
| D-47 | The user (a Rust beginner) tests in-game at milestones. Opus does the code reviews. |
