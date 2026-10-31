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
| D-28 | **New dependencies** need approval by the orchestrator (named in the card or here). Keeps the dependency tree small and reviewed. Approved so far: `regex` (M3-FIX-01, the Java name templates in `Server.ini`, matched with whole-string semantics). |

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
| D-39 | **Side by side:** the Java GS and the Rust GS run at the same time on the same DB, both registered with the Rust LS under **different server ids and ports** (Java: id 2 / 7777, Rust: its own id / 7778). The LS keeps an account on one GS at a time. The Rust GS allocates new object ids from a **separate high range** so it never collides with Java's IdManager: **`0x60000000..=0x7FFFFFFF`** (1610612736..=2147483647, 536,870,912 ids; `l2_world::IdFactory`, M2-IDF-01). Java allocates upwards from its `FirstObjectId` 0x10000000 and would need ~1.3 billion ids to reach it. At startup the Rust GS only *reads* the id columns Java scans (`characters`, `items`, `clan_data`, `itemsonground`, `messages`) and refuses to start if the range is exhausted. It never runs Java's startup cleanup (`DatabaseIdManager.cleanDatabase`/`cleanCharacterStatus`/`cleanTimestamps`: orphan DELETEs, `UPDATE characters SET online=0`, expired-timestamp DELETEs), because they would modify rows the running Java GS owns. The supervisor runs the proxy permanently in front of the LS, so every session is recorded to `run/captures/` (local, git-ignored) as reference material. Java-side cost, checked in `IdManager.java`: at startup Java calls `BitSet.set(id - FirstObjectId)` for every used id with no upper check (`LastObjectId` does not limit it), so once Rust-created rows exist the Java BitSet grows to about 0x50000000 bits (~170 MB, up to ~235 MB near the top of the range). Accepted: the Java GS runs with `-Xmx4g`, and it keeps allocating its own low ids (`nextClearBit`). |

## Process and quality

| # | Decision |
|---|---|
| D-40 | **Agents:** Opus plans, writes task cards, builds core/architecture code and reviews. **Haiku does as much as possible** (templated work), and Sonnet takes cards that are too hard for Haiku. **Max throughput:** parallel agents. |
| D-41 | **Java oracle:** small Java harness programs call the real L2JMobius classes and dump test vectors as JSON. Rust must match byte for byte (crypto, packets, geodata queries, formulas). Captures from the real client cover end-to-end flows. |
| D-42 | **One git worktree/branch per task card.** Opus reviews and merges into `main`. |
| D-43 | **Public GitHub repo** (`Abuyunus007/l2-rust`, public by owner's choice) **+ GitHub Actions CI** on Windows and Linux: `cargo fmt --check`, `clippy -D warnings`, `cargo test`. CI must pass before a merge. |
| D-44 | **Risk-based review:** core/architecture code gets a full Opus review. Templated work is gated by automatic tests + spot checks. |
| D-45 | Planning state lives in the repo (`docs/`, `CLAUDE.md`, task cards), never only in a conversation. |
| D-46 | The Java reference (`C:\l2-rust\l2jmobius`) and the geodata folders are read-only and git-ignored. Agents reference them by absolute path. The one exception is `dist/` (D-55). |
| D-47 | The user (a Rust beginner) tests in-game at milestones. Opus does the code reviews. |
| D-48 | **Local time like Java:** where Java reads calendar fields in the JVM's default zone (`Calendar.getInstance()`, `java.sql.Date` written to `DATE` columns), Rust uses the machine's local zone through `l2_util::time::{local_offset_millis, local_now_millis}` (crate `chrono`, `clock` feature only). Epoch timestamps (`System.currentTimeMillis()`) stay UTC milliseconds. |
| D-49 | **No double global writes side by side:** scheduled jobs that change many rows at once (daily resets, clan/siege maintenance, ...) run on both GSs in Java and Rust. While the Java GS shares the DB (D-39), the Rust GS skips the global DB part of such jobs and only updates its own online players; each job gets a Rust-only switch in `config/SideBySide.ini` (D-36), default off (`DailyResetDatabaseWrites = False`, ...). Turned on the day the Rust GS runs alone. |
| D-50 | **Re-read shared rows before handing items out:** rows the Java GS can change while this server runs (D-39) are re-read before this server moves items out of them: a character's mails at CharacterSelect, a mail's attachments before take/reject/cancel, a clan warehouse at every window and withdraw (M6-SYNC-01), a pet collar's `pets` row before the first summon (M6-PET-01), a clan the table does not know at CharacterSelect (M6-CLAN-01). The read goes out through the outbox and comes back as a world command (D-15); a failed read refuses the action, never falls back to memory. An item missing after the re-read is skipped without the illegal-action punishment. New cards that hand out items from shared containers follow this rule. |
| D-51 | **Sieges side by side (owner, 2026-10-23):** while the Java GS shares the DB (D-39), **Sieghardt runs the real sieges** (castles, fortresses, conquerable halls, Territory War). Kain runs a siege only when a GM starts one (`//siege`, `//fortsiege`, ...) and writes its results (owner change, registrations, siege state) only with `SiegeDatabaseWrites = True` in `config/SideBySide.ini` (default `False`, D-49 pattern). Kain re-reads residence owners (castle, fortress, clan hall) at startup and with the clan of a character at CharacterSelect (D-50 pattern), so it shows what Sieghardt decided. Turned on the day Kain runs alone. |
| D-52 | **Offline trade side by side (2026-10-26):** while the Java GS shares the DB (D-39), the `character_offline_trade*` rows of Java's `OfflineTraderTable` would make Sieghardt restore Kain's traders at its next start, so the same character would sell on two servers. Kain keeps its offline traders (a player who logs out with a store open stays in the world without a client, `enteredOfflineMode`) in memory only: it writes, removes and restores no `character_offline_trade*` row (neither its own nor Sieghardt's), and a trader lives until Kain stops. Only with `OfflineTradeDatabaseWrites = True` in `config/SideBySide.ini` (default `False`, D-49 pattern) does Java's store/restore run (realtime `onTransaction`, `storeOffliners` at shutdown, `restoreOfflineTraders` with `RestoreOffliners`, `OfflineMaxDays` at restore only); set it only when Kain is alone on the DB. **Account hold:** with the switch True Kain tells the LS `PlayerLogout` at once like Java; with it False it does not, so the LS keeps the account on Kain and the character cannot be logged into Sieghardt while Kain's trader sells (both servers would write the same items). A login of that account anywhere makes the LS kick it on Kain; Kain stores and deletes the trader and its final store frees the account. The offline mode itself, `characters.online = 2` and the `OfflineTrade.ini` keys follow Java in both modes. |
| D-53 | **Global cycles side by side (owner, 2026-10-27):** while the Java GS shares the DB (D-39), Sieghardt keeps running the global cycles of M8: the Olympiad periods, validation and weekly points (`olympiad_data`, the whole-table `olympiad_nobles` saves), the hero choice (`heroes`), the Seven Signs cycle and its whole-table saves (`seven_signs_status`, `seven_signs`, `seven_signs_festival`), the grand boss states and raid boss respawn times (`grandboss_data`, `raidboss_spawnlist`), the item auction ends and the cursed weapons (`cursed_weapons`). Kain runs olympiad matches, Seven Signs registration and contributions, instances and boss fights for its own players, but writes those global rows only with `CycleDatabaseWrites = True` in `config/SideBySide.ini` (Rust-only, default `False`, D-49 pattern); with it off it keeps them in memory and re-reads them at start and when shown (D-50). A grand boss spawns on Kain only when Sieghardt's state row says it is alive; with the switch off Kain never drops a cursed weapon. Per-player rows Java writes per player (instance times, raid points, hero diary entries of the player's own deeds) are written as usual. The boss AIs, instance scripts and event scripts are scripts (D-05). |
| D-54 | **Scripts side by side (owner, 2026-10-27):** quest states (`character_quests`) are per-player rows and are written as Java writes them (D-39). The `global_variables` rows scripts use (boss timers, event states, script cycles) are global: Kain writes them only with `CycleDatabaseWrites = True` (D-53) and otherwise keeps them in memory and re-reads them at start and when they decide something (D-50). A grand boss AI runs on Kain only while Sieghardt's boss row says the boss is alive, and never writes the boss state with the switch off. Scripts are compiled in (D-16); `scripts/custom/` is M9. |
| D-55 | **Own server folders (owner, 2026-10-31):** `dist/` in the repo is a byte-exact snapshot of the L2JMobius CT 2.6 H5 release (MIT): `game/config`, `game/data` without `*.java` and geodata files, `login/{config,data,banned_ip.cfg}`, `db_installer/sql`, plus the Rust-only `Admin.ini`/`SideBySide.ini`. The servers run from a copy of it and need no L2JMobius install. Only `tools/sync-dist.py <release dist>` changes it (never by hand); real configs and secrets live in the git-ignored `run/`. The Java sources stay in the read-only reference (D-46). |
