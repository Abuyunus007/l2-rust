# M1-GUI-01: Admin GUI shell

| Field | Value |
|---|---|
| Milestone | M1 |
| Model | sonnet |
| Review | full |
| Reference | yes (view structure every later GUI card copies) |
| Depends on | M1-ADM-01 |
| Size | M |

## Goal
`l2-admin-gui` (egui/eframe, D-31): app window, settings, connection to the supervisor, navigation between views, and a background task bridging the async admin client to the UI.

## Java source (the spec)
- Feature inspiration only: `C:\l2-rust\l2jmobius\java\org\l2jmobius\loginserver\ui\Gui.java`, `gameserver\ui\Gui.java`

## Reference implementation to copy
- `crates/l2-admin-api/src/transport.rs` (client side)

## Files
Create:
- `crates/l2-admin-gui/{Cargo.toml,src/main.rs,src/app.rs,src/bridge.rs,src/settings.rs,src/views/mod.rs,src/views/connection.rs}`

## Requirements
1. Layout: a left navigation (Servers, Logs, Packets, placeholders greyed out for later views), a top status bar (connected/disconnected, supervisor address), a central view.
2. `bridge.rs`: a tokio runtime on a background thread, channels to/from the UI, `ctx.request_repaint()` when events arrive. The UI thread never blocks.
3. A `View` trait (`fn ui(&mut self, ui, &mut AppCtx)`). Each view in its own file. This is the pattern later cards copy.
4. Settings persisted with eframe storage: supervisor URL, token (stored as-is locally; show a warning that it's stored in plain text), theme.
5. Auto-reconnect with backoff, shown in the status bar.
6. Windows and Linux builds. No platform-specific code.

## Out of scope
- The views' contents (GUI-02/03/04).

## Tests
- Unit tests for the settings and the bridge's message handling (no UI rendering tests).
- Manual: the report includes a screenshot of the running app connected to a supervisor.

## Done when
- [ ] builds and runs on Windows, CI builds on Linux
- [ ] committed as `M1-GUI-01: <summary>`
