# M1-ADM-03: Login server admin endpoint

| Field | Value |
|---|---|
| Milestone | M1 |
| Model | haiku |
| Review | spot |
| Reference | no |
| Depends on | M1-ADM-01, M1-LS-05 |
| Size | S |

## Goal
The Rust login server answers `LoginStatus` on a localhost admin endpoint so the supervisor and GUI can show the registered game servers and online counts.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\loginserver\ui\Gui.java` (what the Java GUI shows: game server list, status)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\loginserver\data\GameServerTable.java` (`GameServerInfo` fields)

## Reference implementation to copy
- `crates/l2-admin-api/src/transport.rs` (`serve`), the supervisor's API handler `crates/l2-supervisor/src/api.rs`

## Files
Create:
- `crates/login-server/src/admin.rs`
Modify:
- `crates/login-server/src/main.rs` (start the endpoint if enabled), `crates/login-server/Cargo.toml`

## Requirements
1. Config from the new `config/Admin.ini` (keys defined in `docs/admin-api.md`): `Enabled` (default false), `Bind` (default `127.0.0.1:7901`), `Token`.
2. `LoginStatus` built from the shared state: every known GS (id, name, authed, online count, max, status) + the number of authed LS clients.
3. Push `Event::ServerStateChanged`-style updates when a GS connects or disconnects (or a periodic status every 5 s if events are hard to wire; say which in the report).

## Out of scope
- Admin actions (kick, ban): M3+.

## Tests
- Loopback: connect with the token, request status, see a fake GS appear after it registers.

## Done when
- [ ] tests pass
- [ ] committed as `M1-ADM-03: <summary>`
