# Runbook: the Rust game server next to the Java game server (M2)

How to run the Rust GS beside an existing L2JMobius H5 Java GS on the **same database** (D-39),
both behind the Rust login server, and how to check that characters move between them.

Paths below use placeholders: `<repo>` is this checkout, `<java>` is the Java server's install
folder (the one with `game/` and `login/`). Never commit anything from `run/` (git-ignored).

## 1. Layout

| Process | Port | Server id | Folder |
|---|---|---|---|
| Rust login server | 2107 (clients), 9014 (game servers), 7901 (admin) | — | `run/login` |
| Proxy in front of the LS (records sessions) | 2106 (clients) | — | `run/` |
| Java game server | 7777 | its `hexid.txt` (e.g. 2) | `<java>/game` |
| Rust game server | **7778** | asks for **3** (`RequestServerID`) | `run/game` |
| Supervisor | 7900 (admin GUI) | — | `run/supervisor.ini` |

The client keeps connecting to `127.0.0.1:2106`. The server list shows both game servers; the
names come from the client's own `ServerName-e.dat` (a custom client may show the same name for
both — the list is sorted by id).

## 2. Set up `run/game` once

1. Copy `<java>/game/config/*` (including `Custom/`) to `run/game/config/`, **without**
   `hexid.txt` (the Rust GS registers itself and writes its own).
2. In `run/game/config/Server.ini` change only:
   - `GameserverPort = 7778`
   - `RequestServerID = 3` (`AcceptAlternateID = True` lets the LS pick another free id)
   - `DatapackRoot = <java>/game` (absolute, forward slashes): the Rust GS reads the Java
     server's own `data/` (and `data/geodata`), so both servers use the same datapack.
3. Add `run/game/config/Admin.ini` (Rust-only, D-36):
   ```
   Enabled = True
   Bind = 127.0.0.1:7902
   Token = <random string>
   ```
4. Optional, Rust-only `run/game/config/GeoOverride.ini` (M3-GEO-03): `GeoDataOverridePath`,
   `GeoDataOverrideRegions = 12_24,17_10`, `PathWorkerThreads`.
5. Add the game server to `run/supervisor.ini`:
   ```
   Servers = login, proxy, game
   Server.game.Command = ../target/release/game-server.exe
   Server.game.Args = --root .
   Server.game.WorkingDir = game
   Server.game.AdminEndpoint = 127.0.0.1:7902
   Server.game.AdminToken = <the token from Admin.ini>
   Server.game.Autostart = False
   Server.game.RestartOnExitCode = 2
   ```

## 3. Build and start

```
cargo build --release -p login-server -p l2-proxy -p l2-supervisor -p l2-admin-gui -p game-server
```

Start the supervisor (`target/release/l2-supervisor.exe run/supervisor.ini`) and the admin GUI,
then start `game` from the GUI's Servers view. The Rust GS log ends with
`Registered on login as Server 3: <name>` and `Game server is ready.` The Java GS registers on
its own (it reconnects every 5 s when the LS restarts).

Startup of the Rust GS loads the datapack and, since M3, the geodata (about 1–1.6 GB of RAM for
the whole world).

A running `.exe` cannot be overwritten on Windows. To update one server without stopping the
others: rename the running exe (`game-server.exe` -> `game-server.old.exe`), build, then restart
that server from the GUI, and delete the `.old.exe` afterwards.

## 4. Things to know when both run on one database

- **Object ids:** the Rust GS creates characters and items with ids from `0x60000000` up (D-39).
  The next Java GS start then needs about 170 MB more heap for its id table (fine with
  `-Xmx4g`).
- **Never** let the Rust GS run Java's startup cleanup: it doesn't (D-39), and nothing here
  should change that.
- **One game server per account at a time** — the login server enforces it.
- **Character deletion:** `DeleteCharAfterDays` (Player.ini) applies on both servers; a
  character marked on one can be restored on either.
- **Account rows with padding:** the Rust LS trims NUL bytes around account names read from the
  DB (an account created with padding could not enter any game server before).

## 5. Checkpoint CP-M2d: characters move between both servers

1. **Rust -> Java.** On the Rust GS (second row): create a new character, enter the world, log
   out. On the Java GS (first row): the character is in the list; select it, enter the world:
   gear on, skills in the skill window, shortcuts on the bar, position as created. Log out.
2. **Java -> Rust.** Take a character you played on the Java GS (with some gear and skills):
   enter it on the Rust GS. Same checks: gear, inventory, skills, shortcuts, position. Log out.
3. **Back again.** Enter the same character on the Java GS again: nothing lost or duplicated.

If something is off, the session is recorded in `run/captures/` (proxy) and the Rust GS log is in
`run/game/log/`. Say which step and which character; don't paste captures (they contain names).
