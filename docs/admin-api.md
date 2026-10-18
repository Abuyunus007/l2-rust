# Admin API

Protocol between the admin GUI, the supervisor and the servers (D-30, D-31).
Code: `crates/l2-admin-api` (`messages.rs` = source of truth; its JSON snapshot test shows every shape).

## Topology

```
 l2-admin-gui ──ws──► l2-supervisor ──ws──► login-server  (127.0.0.1:7901)
 (any PC)            (127.0.0.1:7900        game-server   (later)
                      or a LAN/VPS address)
```
- The **GUI only talks to the supervisor**. The supervisor starts and stops the processes, collects their stdout/stderr as log lines, and relays server-specific requests (`GetLoginStatus`) to the server's own admin endpoint.
- Server admin endpoints bind to `127.0.0.1` by default and are meant for the supervisor only.
- The packet proxy is **not** part of this API: it runs inside the GUI (D-35).
- Remote use (supervisor on a Linux VPS): put it behind an SSH tunnel or a TLS reverse proxy. The protocol itself has no TLS yet.

## Connection

1. WebSocket connect (`ws://host:port`). One JSON message per text frame. Binary frames are an error.
2. The client sends `{"type":"hello","version":1,"token":"<secret>"}` within 10 s.
3. The listener answers `{"type":"welcome","version":1,"name":"supervisor"}`, or `{"type":"rejected","reason":"..."}` and closes. Reasons: wrong token, wrong version, no hello.
4. After that, any number of requests/responses and events, in both directions:
   - `{"type":"request","id":1,"request":{"kind":"list_servers"}}`
   - `{"type":"response","id":1,"response":{"kind":"servers","servers":[...]}}`
   - `{"type":"event","event":{"kind":"log_line","line":{...}}}`

Requests may be answered out of order. Match responses by `id`. An empty configured token rejects everyone.

## Messages (protocol version 1)

### Requests
| kind | Fields | Answered by | Response |
|---|---|---|---|
| `list_servers` | | supervisor | `servers` |
| `start_server` / `stop_server` / `restart_server` | `name` | supervisor | `ok` or `error` |
| `subscribe_logs` | `server` (null = all), `since` (last seq the client has, or null) | supervisor | `ok`, then `log_line` events: backlog after `since` first, then live |
| `unsubscribe_logs` | | supervisor | `ok` |
| `get_login_status` | `server` (supervisor: which LS to ask; LS itself: null) | supervisor → LS | `login_status` |

### Responses
| kind | Fields |
|---|---|
| `ok` | |
| `error` | `message` |
| `servers` | `servers`: list of ServerStatus |
| `login_status` | `status`: LoginStatus |

### Events
| kind | Fields | Sent when |
|---|---|---|
| `log_line` | `line`: LogLine | after `subscribe_logs` |
| `server_state_changed` | `status`: ServerStatus | a process starts, stops, crashes, restarts (to every client) |
| `login_status_changed` | `server`, `status`: LoginStatus | a GS registers/disconnects or its status changes |

### Types
- **ServerStatus:** `name`, `state` (`stopped` / `starting` / `running` / `stopping` / `crashed`), `pid`, `uptime_secs`, `last_exit_code`
- **LogLine:** `server`, `seq` (per server, increasing), `ts_ms`, `level` (`error` / `warn` / `info` / `debug` / `trace` / null), `target`, `message`
- **LoginStatus:** `game_servers` (list of `id`, `name`, `authed`, `online`, `max_players`, `status`), `authed_clients`

## Settings (Rust-only files, D-36)

**Servers:** `config/Admin.ini` next to the server's Java-style configs:
```ini
# Enable the local admin endpoint (used by the supervisor).
Enabled = False
Bind = 127.0.0.1:7901
# Shared secret; empty = endpoint refuses every connection.
Token =
```

**Supervisor:** `supervisor.ini` (format defined by M1-ADM-02):
```ini
Listen = 127.0.0.1:7900
Token =
```

## Changing the protocol
Add a variant in `messages.rs`, extend the snapshot test, document it here. Breaking changes (renamed or removed fields, changed meaning) bump `PROTOCOL_VERSION`.
