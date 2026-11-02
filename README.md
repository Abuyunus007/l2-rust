# l2-rust

A Lineage 2 **High Five** server (login server, game server and admin tools) written in Rust: a
port of [L2JMobius](https://l2jmobius.org/) CT 2.6 High Five from Java.

The goal is a retail-like server that behaves like the Java one. It uses the same database
schema, the same `.ini` configuration files and the same datapack, and runs on Windows and
Linux. **The Java source is the spec:** behaviour, packets, messages, HTML pages and database
rows are ported as they are, Java quirks included (marked `NOTE(java-quirk)` in the code).

The Rust game server can run **next to** an existing Java game server on the same database, so
the two can be compared with the same characters and a player can move between them.

> Status: all planned milestones up to the scripts are ported and pass their automated tests
> (8185 tests). In-game testing of the later milestones is ongoing. Customs, premium features
> and the final switch from Java (M9) are still to come. See the [progress log](#progress-log).

---

## Contents

- [Features](#features)
- [Requirements](#requirements)
- [Installation](#installation)
- [Running next to a Java game server](#running-next-to-a-java-game-server)
- [Admin tools](#admin-tools)
- [Project layout](#project-layout)
- [Development](#development)
- [Progress log](#progress-log)
- [License](#license)

---

## Features

- **Login server**: accounts, server list, game server registration (the Java game server
  works with it unchanged), bans, auto-create accounts.
- **Game server**:
  - Character creation and selection, entering the world, geodata movement and pathfinding.
  - Combat, NPC AI, experience, skills and effects.
  - Items, trade, private shops, warehouses, mail.
  - Parties, PvP, clans, alliances, pets and summons.
  - Castles, sieges, fortresses, clan halls, Territory War.
  - Olympiad, heroes, Seven Signs, instances, boats, raid and grand bosses, cursed weapons.
  - All 509 quests, the area and NPC AIs, boss AIs, instance scripts, class change and
    subclasses, and the seasonal events.
  - GM commands (`//admin` and the admin panels).
- **Tools**:
  - **Supervisor**: starts, stops and restarts the servers and collects their logs.
  - **Admin GUI**: server control, live logs, packet inspector.
  - **Proxy**: a decrypting proxy that records client sessions.
  - **Geodata tool**: geodata statistics, diffs and region checks.

## Requirements

| What | Version / notes |
|---|---|
| Rust | stable, 1.85 or newer (edition 2024); `rustup` installs the right toolchain from `rust-toolchain.toml` |
| MariaDB or MySQL | a database with the **L2JMobius H5 schema** (created with the L2JMobius DB installer) |
| Geodata | L2J geodata (`.l2j` files) for movement and line of sight (a separate download, too big for the repo) |
| Client | a Lineage 2 High Five client |
| OS | Windows 10/11 or Linux (x86_64) |

The datapack, the default configuration and the database schema are in this repository, in
[`dist/`](dist/): a snapshot of the L2JMobius CT 2.6 High Five release (MIT licensed). No
L2JMobius install is needed. Only the geodata and the client come from elsewhere.

## Installation

### 1. Get the code and build

```bash
git clone https://github.com/Abuyunus007/l2-rust.git
cd l2-rust
cargo build --release -p login-server -p game-server -p l2-supervisor -p l2-admin-gui -p l2-proxy
```

The programs are in `target/release/`: `login-server`, `game-server`, `l2-supervisor`,
`l2-admin-gui`, `l2-proxy` (with `.exe` on Windows).

### 2. Database

Install the schema with the database installer (no Java needed). It creates the database when it
is missing, asks what to do when it exists (reset, keep or cancel), and runs the SQL files of
`dist/db_installer/sql/login` and `game`:

```bash
cargo build --release -p l2-db-installer
dist/db_installer/DatabaseInstaller.sh          # or DatabaseInstaller.cmd on Windows
```

It opens a window (host, port, user, password, database name, "Install Login", "Install Game");
`--console` (or a machine without a display) asks the same questions on the terminal, and
`--sql <dir>` selects another script folder. Nothing touches the database before you press Install.
The password is never logged or saved. The scripts `DROP` and recreate their tables, so even
"keep" replaces the L2JMobius tables it names: use an empty database for a new install.

The Rust servers use exactly the same tables and columns as L2JMobius, so an existing L2JMobius
database works too (then you do not need the installer).

**Back up the database before the first start.** The Rust servers write to the same tables as
the Java ones.

### 3. Set up the run folders

Copy `dist/login` and `dist/game/config` into a `run/` folder in the repository. `run/` is
git-ignored and holds your real configs and secrets. The datapack itself stays in `dist/game/data`:

```bash
mkdir -p run/game
cp -r dist/login run/login
cp -r dist/game/config run/game/config
```

```
run/
├── supervisor.ini
├── login/
│   ├── config/        <- Database.ini, Server.ini, Admin.ini, ...
│   ├── data/          <- servername.xml
│   └── banned_ip.cfg
└── game/
    └── config/        <- Database.ini, Server.ini, Admin.ini, SideBySide.ini, ... (Custom/ too)
```

Put the geodata files (`*.l2j`) into `dist/game/data/geodata/` (git-ignored), or point
`GeoOverride.ini` at another folder.

**Login server** (`run/login/config/`):
- `Database.ini`: your database URL, user and password (same keys as Java).
- `Server.ini`: `LoginserverPort` (client port, default 2106) and `LoginPort` (game server
  port, default 9014).
- `Admin.ini` (Rust only): `Enabled = True`, `Bind = 127.0.0.1:7901`, `Token = <random string>`.

**Game server** (`run/game/config/`):
- `Database.ini`: the same database as the login server.
- `Server.ini`:
  - `DatapackRoot = <repo>/dist/game` (absolute path, forward slashes). The Rust server reads
    `data/` from there. When it runs next to a Java game server, you can also point it at the
    Java server's `game` folder, so both use the very same datapack.
  - `GameserverPort` (7777 by default; use another port when the Java game server also runs).
  - `RequestServerID`.
- `Admin.ini` (Rust only): `Enabled = True`, `Bind = 127.0.0.1:7902`, `Token = <random string>`.
- `GeoOverride.ini` (Rust only, optional): `GeoDataOverridePath` to use geodata from another
  folder.
- `SideBySide.ini` (Rust only): only needed when a Java game server shares the database (see
  [below](#running-next-to-a-java-game-server)). The example is
  `crates/game-server/config-examples/SideBySide.ini`.

The `.ini` files use exactly the Java key names and defaults. The Rust-only settings live in
their own files (`Admin.ini`, `GeoOverride.ini`, `SideBySide.ini`). The Java files are never
extended.

### 4. Supervisor

Copy `crates/l2-supervisor/supervisor.example.ini` to `run/supervisor.ini` and adjust it:

```ini
Listen = 127.0.0.1:7900
Token = <random string for the admin GUI>
Servers = login, game

Server.login.Command = ../target/release/login-server.exe
Server.login.WorkingDir = login
Server.login.AdminEndpoint = 127.0.0.1:7901
Server.login.AdminToken = <Token of run/login/config/Admin.ini>
Server.login.Autostart = True
Server.login.RestartOnExitCode = 2

Server.game.Command = ../target/release/game-server.exe
Server.game.Args = --root .
Server.game.WorkingDir = game
Server.game.AdminEndpoint = 127.0.0.1:7902
Server.game.AdminToken = <Token of run/game/config/Admin.ini>
Server.game.Autostart = False
Server.game.RestartOnExitCode = 2
```

On Linux, leave out the `.exe`.

### 5. Start

```bash
target/release/l2-supervisor run/supervisor.ini
```

Or just start `target/release/l2-admin-gui`: it finds `run/supervisor.ini` (the repo's `run/`,
then `./run`, then its own folder), reads `Listen` and `Token` from it and, when nobody listens,
starts `l2-supervisor` (from the GUI's folder) detached, with its output in
`run/log/supervisor.log`. Closing the GUI leaves the supervisor and the servers running. The
**Connection** view has "Choose run folder" for other layouts and "Stop supervisor" (stops all
servers cleanly, then the supervisor exits). Address and token fields there are optional
overrides. The status bar says why the supervisor is not running (missing `supervisor.ini`, empty
`Token`, port used by another program, `l2-supervisor` missing). Without a `supervisor.ini` the
GUI points here: the setup wizard (a later card) will create it; until then copy the example as
shown above. A second supervisor is never started: the supervisor writes `run/supervisor.pid`
and the GUI checks it and the port first.

Start the servers from the **Servers** view (**Start all** in `Servers` order, **Stop all** in
reverse). You can also start the servers without
the supervisor: `login-server` from `run/login`, and `game-server --root .` from `run/game`.

The game server is ready when its log shows `Game server is ready.` It loads the datapack and
the geodata at start, which takes about 1 to 1.6 GB of RAM for the whole world.

Stop a server with **Ctrl+C** or from the GUI, so it saves cleanly.

> **Never commit `run/`** or anything with passwords, tokens, account names or IP addresses.
> `run/`, `*.local.ini` and `.env` are git-ignored.

## Running next to a Java game server

Both game servers register with the Rust login server and use the same database:

| Process | Port | Server id |
|---|---|---|
| Rust login server | 2106 (clients), 9014 (game servers) | — |
| Java game server | 7777 | its `hexid.txt` |
| Rust game server | e.g. 7778 | e.g. 3 (`RequestServerID`) |

The Java login server and the Rust login server must never run at the same time: they use the
same ports and tables.

Some jobs change rows of every character, or global state, and must only run once per
database. By default the Rust game server leaves these to the Java one and keeps the results in
memory. `config/SideBySide.ini` switches them on when the Rust server is the only one on the
database. All of them are False by default:

| Key | Covers |
|---|---|
| `DailyResetDatabaseWrites` | the 06:30 daily reset (recommendations, birthday mail) |
| `ClanMaintenanceDatabaseWrites` | clan dissolution, leader change, alliance checks |
| `MailExpiryDatabaseWrites` | expiry of mails |
| `FishingChampionshipDatabaseWrites` | the fishing championship ranking |
| `CycleDatabaseWrites` | Olympiad, heroes, Seven Signs, boss states, item auctions, cursed weapons, script `global_variables`, event item cleanup |
| `OfflineTradeDatabaseWrites` | offline private shops |
| `SiegeDatabaseWrites` | castle, fortress, clan hall and Territory War siege results |

Per-player rows (characters, items, quests, skills, ...) are written as usual, so a character
can play on either server (not on both at once). New objects of the Rust server get ids from
`0x60000000` up, so they never collide with the Java ids.

## Admin tools

- **`//admin`** in game: the GM panel, with the same commands and access levels as Java
  (`AccessLevels.xml`, `AdminCommands.xml`).
- **Admin GUI** (`l2-admin-gui`): server start and stop, live logs, status, packet inspector.
- **Proxy** (`l2-proxy`): put it in front of the login server to record decrypted client
  sessions as JSONL (only for testing; captures may hold personal data).
- **Test runbooks** for every milestone, step by step: [`docs/runbooks/`](docs/runbooks/).

## Project layout

```
crates/
  login-server       the login server (bin)
  game-server        network <-> world glue, login link, persistence, admin endpoint (bin)
  l2-world           the game world: one world loop, timers, entities, AI, combat, skills, ...
  l2-scripts         quests, AIs, instances, events, handlers (one file per Java script)
  l2-data            datapack loaders (XML, HTML)
  l2-geo             geodata, line of sight, pathfinding
  l2-db              database access (sqlx), same schema as Java
  l2-protocol-login  login protocol packets
  l2-protocol-game   game protocol packets, opcodes, system messages
  l2-net             framing, packet reader/writer, connections
  l2-crypto          Blowfish, RSA, the game cipher
  l2-config          .ini reader with Java ConfigReader semantics
  l2-util            ids, seedable RNG, time helpers
  l2-supervisor      process supervisor (bin)
  l2-admin-api       admin protocol types
  l2-admin-gui       egui admin app (bin)
  l2-proxy           decrypting recording proxy (bin)
  l2-geotool         geodata tool (bin)
  l2-testkit         test helpers, Java vectors, captures
  codegen            code generators (writes src/generated/)
dist/                the server folders: datapack, default configs, SQL schema (L2JMobius snapshot)
tools/               helper scripts (sync-dist.py, merge scripts, analysis)
docs/
  PLAN.md            milestones
  DECISIONS.md       binding design decisions (D-01 ...)
  MODULE_MAP.md      crates, layers and their Java sources
  design/            design notes (e.g. scripts.md)
  runbooks/          in-game test guides
  tasks/             task cards and the board
java-vectors/        byte-level test vectors produced by the Java code
fixtures/            scrubbed packet captures used as tests
```

### Design in short

- **One world loop owns the game state.** No shared locks around game objects. Objects refer to
  each other by id.
- **No database call inside a game tick.** Persistence goes through an outbox to a separate task.
- Scheduling uses the world's timer wheel. Randomness uses the world's seedable RNG, so tests are
  repeatable.
- No copy of the Java class hierarchy: data structs, kind enums and small traits.
- Details: [`docs/DECISIONS.md`](docs/DECISIONS.md) and [`docs/MODULE_MAP.md`](docs/MODULE_MAP.md).

## Development

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p codegen -- <target>     # regenerate generated code (needs the Java sources)
```

- A change is done only when fmt, clippy and the tests pass. CI runs them on Windows and Linux,
  plus database integration tests on a throwaway MariaDB.
- Database tests only run when `L2_TEST_DATABASE_URL` is set. **Never point it at a real
  database.**
- Tests that need the L2JMobius datapack or geodata skip with a message when those are missing.
- Byte-level code (crypto, packets, geodata, formulas) is checked against vectors from the Java
  code in `java-vectors/out/`.
- Files in `src/generated/` are produced by `codegen`. Never edit them by hand.
- `dist/` is changed only by `python tools/sync-dist.py <L2JMobius release dist folder>`, which
  refreshes it from a newer L2JMobius release and keeps the Rust-only files.
- Code style: Rust 2024, `thiserror` in libraries, `tracing` for logs, no `unwrap()` on network,
  file or database data. Ported types carry a `// Java: path/To/Class.java` comment.

## Progress log

| Milestone | Content | Finished | Status |
|---|---|---|---|
| M0 Planning | Analysis of the Java code, plan, decisions, module map | 2026-10-18 | ✅ |
| M1 Foundations | Crypto, networking, config, DB layer, Rust login server, supervisor, admin GUI, proxy, packet inspector | 2026-10-18 | ✅ tested in game: real client logs in, the Java GS registers and plays through the Rust LS |
| M2 Game server skeleton | Game server, character create/delete/restore, enter the world, side by side with the Java GS | 2026-10-19 | ✅ tested in game: characters move between the Java and the Rust GS with nothing lost |
| M3 World and movement | Geodata, movement, pathfinding, NPCs, spawns, doors, zones, teleports | 2026-10-19 | ✅ automated tests |
| M4 Combat, NPC AI, progression | Stats and formulas, attacks, NPC AI, death, experience, drops | 2026-10-20 | ✅ automated tests, stats checked in game |
| M5 Skills and effects | Skills, effects, buffs, conditions, cubics, transformations | 2026-10-20 | ✅ automated tests |
| M6 Parties, PvP, trade, clans, companions | Parties, PvP/karma, trade, shops, warehouses, mail, clans, alliances, pets, summons | 2026-10-23 | ✅ automated tests |
| M7 Castles, sieges, fortresses, clan halls | Residences, sieges, taxes, manor, Territory War basics | 2026-10-25 | ✅ automated tests |
| Leftovers of M0..M7 | Remaining GM commands and admin panels, open follow-ups | 2026-10-27 | ✅ |
| M8 Big systems | Olympiad, heroes, Seven Signs, festival, rift, instances, boats, raid and grand bosses, cursed weapons, mini games, weddings | 2026-10-27 | ✅ automated tests |
| Scripts | Script engine; all 509 quests; area, NPC and boss AIs; instances (Kamaloka, Pailaka, Crystal Caverns, Gracia, ...); class change and subclasses; Territory War quests; 10 seasonal events; decoys and traps; drop search board; quest admin commands | 2026-10-31 | ✅ automated tests, including end-to-end; in-game tests pending |
| M9 Customs, hardening, switch from Java | `scripts/custom/`, premium, hardening, replacing the Java server | — | ⏳ planned |

How each milestone is tested in game: [`docs/runbooks/`](docs/runbooks/). Task cards and
their status: [`docs/tasks/BOARD.md`](docs/tasks/BOARD.md).

### Known limits

- Rust scripts are compiled in: `//reload quest` reinstalls a script but cannot load changed
  code. A code change needs a rebuild and a restart.
- Not modelled yet:
  - zone effects on NPCs;
  - the effect points of signet skills;
  - a few decoy and trap details;
  - GM skill trees in the skill checks;
  - HWID checks, champion monsters, premium, the item mall.
- More in each runbook's "Known limits" section.

## License

[MIT](LICENSE). Lineage 2 is a trademark of NCSOFT. This project is not affiliated with NCSOFT
or the L2JMobius team; it is a port of the open-source L2JMobius server.
