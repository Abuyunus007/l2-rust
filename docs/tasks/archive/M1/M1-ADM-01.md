# M1-ADM-01: Admin API protocol

| Field | Value |
|---|---|
| Milestone | M1 |
| Model | opus (orchestrator) |
| Review | full |
| Reference | yes (every later admin feature adds messages here) |
| Depends on | M1-WS-01 |
| Size | S |

## Goal
`l2-admin-api`: the message types and transport helpers for the GUI ↔ supervisor ↔ server admin protocol (D-30): WebSocket + JSON, token auth, request/response + event streams.

## Java source (the spec)
None for the protocol (new). For the feature set, see the Java Swing UIs: `C:\l2-rust\l2jmobius\java\org\l2jmobius\loginserver\ui\Gui.java`, `gameserver\ui\*.java` (what admins could do from the GUI).

## Reference implementation to copy
none (this card creates it)

## Files
Create:
- `crates/l2-admin-api/{Cargo.toml,src/lib.rs,src/messages.rs,src/transport.rs,src/auth.rs}`
- `docs/admin-api.md` (protocol description: connect, auth, every message with a JSON example)

## Requirements
1. Topology (record it in `docs/admin-api.md`): the **GUI connects to the supervisor** (one endpoint, local or remote). The **supervisor connects to each server's local admin endpoint** (127.0.0.1) and relays. Servers never listen publicly for admin traffic.
2. Envelope: `{ "id": u64?, "type": "...", ... }` with serde-tagged enums `Request`, `Response`, `Event`. Version field in the hello handshake.
3. Auth: the first message is `Hello { version, token }`. The token is compared in constant time against a configured secret. A wrong token closes the connection.
4. M1 messages: `ListServers`, `ServerStatus { name, state (stopped/starting/running/stopping/crashed), pid, uptime, exit_code }`, `StartServer/StopServer/RestartServer { name }`, `SubscribeLogs { name, since }`, `Event::LogLine { server, ts, level, target, message }`, `Event::ServerStateChanged`, and from servers: `LoginStatus { game_servers: [{id, name, authed, online, max, status}], authed_clients }`.
5. Rust-only settings go into **new files** (`config/Admin.ini` for servers, `supervisor.ini`), never into the Java `.ini` files (D-02). Document the keys.
6. Transport helpers over `tokio-tungstenite`: `serve(addr, auth, handler)`, `connect(url, token)`, both returning typed message streams.

## Out of scope
- Supervisor logic (ADM-02), the LS endpoint (ADM-03), the GUI.

## Tests
- JSON round trip of every message (snapshot strings in the test, so protocol changes are visible in review).
- Loopback: auth ok / wrong token rejected / request-response matching by id.

## Done when
- [ ] tests pass, `docs/admin-api.md` written
- [ ] committed as `M1-ADM-01: <summary>`
