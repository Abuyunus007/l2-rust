# M1-GUI-02: Servers view

| Field | Value |
|---|---|
| Milestone | M1 |
| Model | haiku |
| Review | spot |
| Reference | no |
| Depends on | M1-GUI-01, M1-ADM-02 |
| Size | S |

## Goal
A view listing the supervisor's servers with state, PID, uptime and last exit code, Start/Stop/Restart buttons, and, for the login server, its game server table (from `LoginStatus`).

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\loginserver\ui\Gui.java` (game server list columns)

## Reference implementation to copy
- `crates/l2-admin-gui/src/views/connection.rs` and the `View` trait

## Files
Create:
- `crates/l2-admin-gui/src/views/servers.rs`
Modify:
- `crates/l2-admin-gui/src/views/mod.rs` (register the view)

## Requirements
1. A table: name, state (coloured), PID, uptime, exit code, actions. Buttons are disabled when the action makes no sense (Start while running).
2. Stop/Restart ask for confirmation.
3. Login server details: GS id, name, authed, online/max, status; authed client count.
4. Updates live from events, no manual refresh needed.

## Out of scope
- Editing the supervisor config.

## Tests
- Unit test for the view model (state → enabled buttons).

## Done when
- [ ] works against a running supervisor (screenshot in the report)
- [ ] committed as `M1-GUI-02: <summary>`
