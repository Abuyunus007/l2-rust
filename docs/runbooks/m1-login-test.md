# M1 login test: Rust login server + Java game server + real client

Step by step, for Windows. You do not need to know Rust. Every command is meant to be copied
as it is. Each part ends with **Pass** (what you must see) and **If not** (what to do).

What this proves (milestone 1): the Rust login server (LS) does what the Java one does, so
the **Java game server (GS)** and the **real H5 client** work with it unchanged.

Your setup (paths used below; change them if yours differ):

| What | Where |
|---|---|
| Java L2JMobius install | `C:\L2J_Mobius_CT_2.6_HighFive\` (Java GS is started from its `game\` folder) |
| Rust repo | `C:\l2-rust\` |
| Rust LS run folder | `C:\l2-rust\run\login\` (has its own `config\` with `Database.ini`; not in git) |
| Supervisor settings | `C:\l2-rust\run\supervisor.ini` (not in git) |
| Programs (after building) | `C:\l2-rust\target\release\login-server.exe`, `l2-proxy.exe`, `l2-admin-gui.exe`, `l2-supervisor.exe` |

Never put passwords, account names or your public IP into anything you send me or commit.
This guide never asks you to print the contents of `run\`.

---

## 0. Rules for the day

1. **Java LS and Rust LS never run at the same time.** They use the same ports (2106 for the
   client, 9014 for the game server) and the same database tables.
2. **Back up the database once** before the first test (HeidiSQL: right click the database,
   *Export database as SQL*, or `mysqldump`). The Rust LS writes to the same tables as the Java
   one (`accounts`, `gameservers`), in the same format.
3. **Never run `cargo test` with `L2_TEST_DATABASE_URL` pointing at your real database.** The
   automated database tests are run by GitHub (CI) in a throw-away database. You do not run them.
4. Close a program with **Ctrl+C** in its window (the Rust LS stops cleanly on that), not with
   the X button.

## 1. Build everything (once, and after every update)

Open **PowerShell**:

```powershell
cd C:\l2-rust
git pull
cargo build --release -p login-server -p l2-proxy -p l2-admin-gui -p l2-supervisor
```

**Pass:** the last line says `Finished release` and these four files exist:

```powershell
Get-ChildItem target\release\login-server.exe, target\release\l2-proxy.exe, target\release\l2-admin-gui.exe, target\release\l2-supervisor.exe
```

**If not:** copy the last 20 lines of the output to me. (`git pull` complaining about local
changes: tell me, do not force it.)

## 2. Check the Rust LS run folder

The folder `C:\l2-rust\run\login\` must look like a Java `login` folder:

```powershell
Get-ChildItem C:\l2-rust\run\login, C:\l2-rust\run\login\config, C:\l2-rust\run\login\data | Select-Object FullName
```

You need: `config\Database.ini` (points at your database `l2jmobiush5`, same keys as Java),
`config\Server.ini` and `data\servername.xml` (copy it from
`C:\L2J_Mobius_CT_2.6_HighFive\login\data\servername.xml` if it is missing). `banned_ip.cfg` is
optional. The `.ini` files use exactly the Java key names. In `config\Server.ini` check these
three (open it in Notepad, do not paste it anywhere):

| Key | Must be | Why |
|---|---|---|
| `LoginserverPort` | `2106` | the port the client connects to |
| `LoginPort` | `9014` | the port the Java GS connects to (its `config\Server.ini` has `LoginPort = 9014`) |
| `AutoCreateAccounts` | your choice | `True`: a new login name creates an account (what the Java LS does by default) |

The Java GS keeps its normal config. Leave `LoginHost = 127.0.0.1` and `LoginPort = 9014` in
`C:\L2J_Mobius_CT_2.6_HighFive\game\config\Server.ini`.

## 3. Start the Rust LS and the Java GS

**Step 3.1: stop the Java LS** if it runs (Ctrl+C in its window, or close it the way you
normally do). Make sure nothing listens on the two ports:

```powershell
Get-NetTCPConnection -State Listen -LocalPort 2106, 9014 -ErrorAction SilentlyContinue
```

**Pass:** no output. **If not:** something still runs; find it in Task Manager (java.exe / login-server.exe) and stop it.

**Step 3.2: start the Rust LS.** In a new PowerShell window:

```powershell
cd C:\l2-rust\run\login
& C:\l2-rust\target\release\login-server.exe
```

**Pass:** within a few seconds you see lines like these (the time stamps and exact wording of the
first lines can differ; these must all be there, in this order, and no `ERROR` line):

```
Configuration loaded from ...\config.
Loading LoginController...
Cached 10 KeyPairs for RSA communication.
Stored 20 keys for Blowfish communication.
Loaded 3 server names.                         <- the number of servers in servername.xml
Loaded 1 registered Game Servers.              <- the rows of the gameservers table
Cached 10 RSA keys for Game Server communication.
Loaded 0 IP Bans.
Game server listener is listening on 127.0.0.1:9014.
Login client listener started on 0.0.0.0:2106.
Login server is ready.
```

The window stays open and quiet. That is normal. A `log\` folder appears next to it.

**If not:**
- `cannot connect to the database` -> MariaDB is not running, or `config\Database.ini` has a wrong
  URL/user/password. Start MariaDB (HeidiSQL opens the same database, so test with that).
- `listener could not be started on ...` -> the port is in use (see step 3.1).
- `Loaded 0 registered Game Servers` is fine on a brand new database: the GS then registers itself in step 3.3 (the LS accepts new servers, `AcceptNewGameServer = True` like Java).

**Step 3.3: start the Java GS** the way you always start it, from
`C:\L2J_Mobius_CT_2.6_HighFive\game\` (your start script there). Wait until it has loaded
(it takes minutes; the last lines say the server is ready for connections).

**Pass, in the GS window**, shortly after it connects:

```
LoginServerThread: Registered on login as Server 1: <server name>
```

**Pass, in the Rust LS window** at the same moment:

```
Updated Gameserver [1] <server name> IP's:
 ... (one line per host/subnet the GS announced)
```

**If not:**
- GS says `Login server not found` / keeps retrying -> the Rust LS is not running or the port is not 9014.
- GS says `Wrong hexid` or `Rejected` -> the LS and GS disagree about the server id/hex id. Tell me which message; do not delete anything yet.
- Nothing in the LS window at all -> compare `LoginPort` of both sides (step 2).

## 4. Log in with the real client

Start the client the way you always do (it connects to `127.0.0.1:2106`).

| Do | Pass | LS window shows |
|---|---|---|
| Log in with a **new** login name and a password (needs `AutoCreateAccounts = True`) | The server list appears. Your server is in it, shown as up (not grey). | `Auto created account 'name'.` then `Account 'name' logged in from 127.0.0.1.` |
| Look at the server list | Server name, the player count, the Normal/Good state, and **the number of characters** you have on it (a small number next to the server) | nothing |
| Pick the server | The character selection screen appears with your characters. | nothing |
| Enter the game with a character | You stand in the world and can move. | nothing |

**If not:**
- *"Wrong user name or password"* with a **new** name -> `AutoCreateAccounts` is `False`; create the account in the database or set it to `True` and restart the LS.
- Client sits on "connecting" -> the LS is not listening on 2106 or the client's `l2.ini` points elsewhere.
- Server list is empty -> the GS did not register (step 3.3), or the server is GM-only (see part 5).
- Server shown grey/down -> the GS has not sent its status yet (wait a minute) or it lost the link; see part 5.
- Character selection hangs after picking the server -> copy the lines of the LS window and the GS window around that moment to me.

Leave the client for now. Quit the game with *Exit* in the client menu (this tells the GS and then the LS the player left).

## 5. The behaviours to check (all must match Java)

Do these one after another. After each, the **Pass** column is what the real Java setup does too.

| # | Do | Pass |
|---|---|---|
| 5.1 | Log in with a **wrong password** for an existing account | Message "Wrong user name or password". LS: `Login failed for 'name' from 127.0.0.1: wrong password (N characters received, attempt 1 of 5).` (the password itself is never logged) |
| 5.2 | Log in correctly right afterwards | Works; the failure counter is reset |
| 5.3 | Wrong password **5 times in a row** (`LoginTryBeforeBan`) | After the 5th: LS prints `Added banned address 127.0.0.1! Too many login attempts.` The client now cannot connect at all (refused immediately). **To get out: Ctrl+C the Rust LS and start it again** (the ban lives in memory; `LoginBlockAfterBan` is 900 s by default). |
| 5.4 | **Account in use:** log in with the client and enter the game, then start a **second client** and log in with the same account | Second client: "account already in use". First client: kicked from the game. LS (when the GS told it the player is in game): no extra log line needed; the GS console shows the kick. |
| 5.5 | **GS goes away:** stop the Java GS (close it the way you normally do) and look at a client's server list (log in again, or reopen the list) | The server is shown as **down**. You cannot enter it. |
| 5.6 | Start the Java GS again | After it registered again (step 3.3) the server is up again; new logins can enter. |
| 5.7 | **GM only:** in the Java GS file `game\config\General.ini` set `ServerGMOnly = True`, restart the GS, log in with a normal account and with an account whose `accessLevel` is 1 or higher | Normal account: the server is **not in the list**. GM account: it is there and can be entered. Set it back to `False` afterwards. |
| 5.8 | **Banned account:** in the database set `accessLevel = -1` for a test account, log in with it, then set it back to `0` | The client shows "account banned" (the permanent ban message). |

If any row differs from what the Java LS does, **that is a bug I want to know about**:
write down the row number, what you saw and the lines of the LS window.

## 6. Recording packets (proxy) and the Java comparison file

The proxy sits between the client and a login server, decrypts the packets and writes them to a
JSON-lines file. Credentials are never written (they are stored as `<redacted>`).

### 6.1 Record a session of the Rust LS (practice, and a file for me to look at)

The proxy takes the client's port 2106, so the Rust LS must listen on another port for this:
edit `C:\l2-rust\run\login\config\Server.ini`, set `LoginserverPort = 2107`, restart the Rust LS
(keep `LoginPort = 9014`). Then in a second PowerShell window:

```powershell
cd C:\l2-rust
New-Item -ItemType Directory -Force run\captures | Out-Null
& .\target\release\l2-proxy.exe --listen 127.0.0.1:2106 --upstream 127.0.0.1:2107 --no-rewrite --capture run\captures\rust-ls-login.jsonl
```

**Pass:** it prints `Proxy ready: client -> 127.0.0.1:2106  =>  login server 127.0.0.1:2107`.
Log in with the client; for every packet the proxy window prints a line like
`[conn 1] server -> client: Init (opcode 0x00)`, `... client -> server: AuthGameGuard`, `... LoginOk`,
`... ServerList`. Stop it with Ctrl+C. Open `run\captures\rust-ls-login.jsonl` in Notepad: one
packet per line, with the decoded `fields`.

(`--no-rewrite` records only the login part; the client then connects to the GS directly.
Without it the proxy also relays the game connection and the capture holds game packets too.)

Put `LoginserverPort` back to `2106` when you are done.

### 6.2 Record the **Java** LS session (the comparison file, "CP1")

This is the file the automated compare test needs (`fixtures/captures/java-ls-login.jsonl`).
It checks that the Rust LS answers the same session with the same packet types and fields.

1. Stop the Rust LS and the Java GS. (With the GS stopped the server list is the same every time: server shown as down.)
2. In `C:\L2J_Mobius_CT_2.6_HighFive\login\config\Server.ini` set `LoginserverPort = 2107`
   (write down the old value, normally 2106). Start the **Java LS** as usual.
3. Start the proxy:

   ```powershell
   cd C:\l2-rust
   & .\target\release\l2-proxy.exe --listen 127.0.0.1:2106 --upstream 127.0.0.1:2107 --no-rewrite --capture run\captures\java-ls-login.jsonl
   ```
   Do **not** add `--log-accounts`.
4. In the client, exactly this, one after the other:
   1. Log in with an **existing** account and a **wrong** password -> "wrong password". (Closes the connection; the client may show the login screen again.)
   2. Log in with the **same account and the right password** -> the server list appears (the server is shown as down).
   3. Do not pick the server. Close the client.
5. Ctrl+C the proxy. Stop the Java LS. **Restore `LoginserverPort` to `2106`.**
6. Check the file `run\captures\java-ls-login.jsonl` (see `fixtures\captures\README.md`): open it in
   Notepad, search for your public IP address. `127.0.0.1` or `192.168.x.x` in the ServerList line
   is fine. If you find a public IP, do not share the file; tell me.
7. Give me the file (or copy it to `C:\l2-rust\fixtures\captures\java-ls-login.jsonl`). I commit
   it; from then on the automated test `the_rust_login_server_answers_like_the_java_capture`
   (run by CI) replays that session against the Rust LS.

**Pass:** the file has about 12 lines (a header line, then per connection `Init`, `AuthGameGuard`,
`GGAuth`, `RequestAuthLogin`, and `LoginFail` or `LoginOk`; the second connection continues with
`RequestServerList` and `ServerList`).

**If not:** the client never reached the proxy -> the Java LS must listen on 2107 and nothing else
on 2106 (step 3.1 command). Client "cannot connect" -> the proxy window shows nothing: it is not running.

## 7. Same checks with the GUI and the supervisor (milestone test, "CP3")

The supervisor starts and stops the Rust LS for you and shows its log; the GUI talks to the
supervisor. (The Java GS is **not** managed by it, start it yourself as in step 3.3.)

1. Stop the Rust LS window from part 3 (Ctrl+C).
2. `C:\l2-rust\run\supervisor.ini` lists the Rust LS: `Server.login.Command` is the full path to
   `login-server.exe`, `Server.login.WorkingDir` is `C:\l2-rust\run\login`, and `Token` is a secret
   you chose (the GUI needs the same one). Set `Server.login.Autostart = True` if you want it to
   start with the supervisor.
3. Start the supervisor and the GUI (two windows):

   ```powershell
   cd C:\l2-rust\run
   & C:\l2-rust\target\release\l2-supervisor.exe
   ```
   ```powershell
   & C:\l2-rust\target\release\l2-admin-gui.exe
   ```
4. In the GUI, tab **Connection**: Address `ws://127.0.0.1:7900`, Token = the same secret, press **Connect**.

   **Pass:** the status line says *Connected to supervisor*. *Rejected: invalid token* -> the token differs.
5. Tab **Servers**: the login server row shows **Stopped** or **Running**. Press **Start**.

   **Pass:** State **Running**; a *Login server* section lists the game servers with *Authed* **Yes** once the
   Java GS has registered (start it now, step 3.3).
6. Tab **Logs**: the same lines as in part 3.2 scroll by.
7. Tab **Packets** (card GUI-04; may not be in your build yet): enter listen `127.0.0.1:2106` and the
   LS address, press start. For the proxy to take port 2106, the LS must listen on 2107 (as in 6.1).
   Log in with the client: the packets appear live, click one to see the decoded fields and the hex.
8. Press **Restart** on the login server row.

   **Pass:** the state goes Stopping -> Starting -> Running; the Java GS re-registers by itself after a
   moment (it keeps retrying); the client can log in again.
9. Close the GUI. In the supervisor window press Ctrl+C: it asks the login server to stop and waits (up to
   `StopTimeoutSecs`) before it kills it.

## 8. What to send me when something fails

- Which part and which step, what you saw instead of **Pass**.
- The last ~30 lines of the LS window (they contain no passwords) and, if it is about the game
  server, the matching lines of the GS console.
- The capture file from part 6.1 if it is about what the client sees.

Nothing from `run\config` (database password!) and no account names.

## Appendix: what the automated tests cover

GitHub runs these on every change (you do not run them):

- `crates/login-server/tests/e2e.rs`: auth success/failure, auto-create (on and off), IP ban after
  `LoginTryBeforeBan` failures, banned account, account in use (two clients, and a player on a game
  server with the game server told to kick it), game server registration (wrong hex id, second link),
  GM-only server, `RequestServerLogin`/`PlayOk`, the game server's `PlayerAuthRequest`, game server
  disconnect shows the server as down, and the capture comparison (a recorded session replayed
  through an in-process proxy gives the same packet types and fields).
- The fake client and fake game server those tests use live in `crates/l2-testkit`
  (`fake_login_client.rs`, `fake_game_server.rs`); the compare/replay code is `capture_compare.rs`.
