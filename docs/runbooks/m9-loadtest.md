# M9 load test: 2000 bots against a test setup

`l2-loadtest` logs in N bot clients through the real login server and game server, lets them
play a simple loop and measures the server. The target is 2000 players (D-57).

## Rules (read first)

- **Test setup only.** The test needs its own database, its own folders and its own ports. Never
  aim it at the owner's real login server (port 2106), game server (7777) or database. Bots create
  accounts and characters; a real database would be full of `lt0001`... rows afterwards.
- The tool refuses `--login host:2106` unless you add `--i-know-this-is-a-test-server`. Do not add
  it for the real server. It is only for a test setup that happens to use that port.
- Nothing here starts, stops or touches the real servers or the folders `run/` and `release/`.
- The bots use a throwaway password (`--password`, default `loadtest`). Never use a real one.

## Prepare the test setup

1. A separate MariaDB database (for example `l2_loadtest`) with the login and game schema
   (`dist/db_installer/sql`). Use a throwaway database user. Do not use the real game database.
2. A separate folder for the login server and the game server, copied from `dist/` (D-55), with:
   - `Database.ini` pointing at the test database;
   - login `Server.ini`: `LoginPort`, `LoginserverPort` on free ports (not 2106 and 9014),
     `AutoCreateAccounts = True`, `EnableFloodProtection = False` (2000 bots log in from one
     address);
   - game `Server.ini`: free `GameserverPort` (not 7777), `MaximumOnlineUsers = 2500`, and the
     link port of the test login server;
   - game `FloodProtector.ini`: no flood protection (the bots chat and move fast);
   - `Admin.ini`: `Enabled = True`, `Bind = 127.0.0.1:<free port>`, `Token = <something>`.
3. Start both servers from that folder (by hand or with a supervisor of that folder) and wait until
   the game server shows as up in the login server's list.

## Run

```
cargo run --release -p l2-loadtest -- ^
  --login 127.0.0.1:<test login port> ^
  --admin 127.0.0.1:<test admin port> --admin-token <token> ^
  --count 2000 --ramp 20 --duration 900 ^
  --accounts lt --report loadtest-report.txt
```

| Option | Meaning |
|---|---|
| `--login host:port` | the test login server |
| `--count N` | bots (default 20) |
| `--ramp per-second` | bots started per second (default 10; 2000 at 20/s takes 100 s) |
| `--duration secs` | length of the run, counted from the first bot, ramp included (default 60) |
| `--accounts prefix` | accounts `prefix0001`..: letters and digits, at most 10 (default `lt`) |
| `--password` | the bot password (default `loadtest`) |
| `--admin host:port` | the game server's admin endpoint; without it the tick time is not measured |
| `--admin-token` | its token (falls back to `L2_ADMIN_TOKEN`; empty is fine for a loopback supervisor endpoint, D-56) |
| `--sample-interval secs` | seconds between samples (default 5) |
| `--seed` | seed of the bots' random choices (default 1) |
| `--report file` | summary table to `file`, samples to `file.csv` |

Start small: `--count 20 --duration 30` first, then 200, 1000, 2000. The first run creates the
accounts and characters (class 0, a human fighter), so it is slower than later runs on the same
database.

## What a bot does

Full login (GameGuard, RSA login, server list, server login), game server handshake and
`AuthLogin`, character creation when the account has none, `EnterWorld`. Then about two actions a
second: walk to a random spot within 300 units of the start (58 %), chat in the "all" channel
(12 %), click and attack an attackable NPC it has been told about (22 %; walks instead when there
is none, as in a town), idle, and now and then (0.5 %) restart to the character screen and come
back. A dead bot picks "to the village". At the end every bot logs out.

## What it measures

- **Times** (min, avg, p50, p99, max): login server connect, login until `PlayOk`, game server
  connect and auth, enter world.
- **Counts**: bots started, logged in, in the world (peak), restarts, errors (with causes),
  disconnects the server caused, packets received and sent.
- **Samples** every `--sample-interval` seconds (CSV): bots in the world, packets per second, errors,
  disconnects, and from the game server's admin endpoint (`GameInfo`): online count, memory, and
  the measured tick time (average and p99 of the last 1200 ticks, a minute at 20 ticks per second).

CSV columns: `t_secs,in_world,packets_per_s,errors,disconnects,online,memory_mb,tick_avg_ms,tick_p99_ms`
(the last four are empty for a sample without admin data).

## What the server must pass

- **Tick p99 under 50 ms** in every sample (the world ticks every 50 ms; a longer tick means it
  falls behind).
- **No disconnects** and **no errors** during the run.

The summary ends with `verdict: PASS`, `FAIL` or `UNCHECKED` (no admin samples). The exit code is
1 on `FAIL`, 2 when the options or the report file are wrong.

## Limits to know

- The bots are fake clients from `l2-testkit` and only know what the server tells them. They do
  not run geodata, so they walk where the server lets them.
- A bot that hits a protocol surprise panics inside its own task; the run counts it as an error
  with the message and goes on.
- Reading uses short timeouts. On a very overloaded machine a half-read packet can show up as a
  disconnect; run the tool on a different machine than the servers for the 2000-bot run.
- The tool and 2000 bots need many sockets. Raise the open file limit (Linux `ulimit -n`) first.
