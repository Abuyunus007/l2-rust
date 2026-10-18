# M1-ADM-02: Supervisor

| Field | Value |
|---|---|
| Milestone | M1 |
| Model | sonnet |
| Review | full |
| Reference | no |
| Depends on | M1-ADM-01 |
| Size | M |

## Goal
`l2-supervisor` binary: starts, stops and restarts the configured server processes (the Rust LS now, the Java GS too, later the Rust GS), restarts on exit code 2, collects their output as log lines, and serves the admin API to the GUI.

## Java source (the spec)
- Start scripts: `C:\l2-rust\l2jmobius\dist\login\LoginServer.vbs`, `LoginServerTask.sh`, `dist\game\GameServer.*` (restart loop on exit code 2, working directory, java options in `java.cfg`)

## Reference implementation to copy
- `crates/l2-admin-api/src/transport.rs` (server side)

## Files
Create:
- `crates/l2-supervisor/{Cargo.toml,src/main.rs,src/config.rs,src/process.rs,src/logs.rs,src/api.rs}`
- `crates/l2-supervisor/supervisor.example.ini`

## Requirements
1. `supervisor.ini`: listen address (default `127.0.0.1:7900`), token, and a list of servers. Each server has `name`, `command`, `args`, `working_dir`, `admin_endpoint` (optional), `autostart`, `restart_on_exit_code = 2`. The example config has entries for the Rust LS and for the **Java GS** (`java @java.cfg -jar ../libs/GameServer.jar` style, taken from the Java start scripts).
2. Process control with `tokio::process`: start, graceful stop (on Windows: CTRL_BREAK / process group; on Linux: SIGTERM; kill after a timeout), restart. State events go to subscribers.
3. Logs: capture stdout/stderr line by line into a ring buffer per server (configurable size, default 10,000 lines), parse the level when the line has a known format, stream to subscribers, `since` support.
4. When a server has `admin_endpoint`, the supervisor connects to it and relays its status messages (`LoginStatus`).
5. The supervisor itself logs with `tracing` and never crashes when a child does.

## Out of scope
- Service installation (Windows service / systemd unit): a later card.

## Tests
- Process tests with a tiny test child binary (`tests/bin/child.rs`): exits 0, 2 (→ restarted), 1 (→ crashed), prints lines (captured), ignores the graceful stop (→ killed).
- API loopback: ListServers, Start/Stop, SubscribeLogs.

## Done when
- [ ] tests pass on Windows and Linux
- [ ] committed as `M1-ADM-02: <summary>`
