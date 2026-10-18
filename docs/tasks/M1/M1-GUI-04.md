# M1-GUI-04: Packet inspector

| Field | Value |
|---|---|
| Milestone | M1 |
| Model | sonnet |
| Review | full |
| Reference | yes (the inspector grows with every packet ported later) |
| Depends on | M1-GUI-01, M1-PROXY-02 |
| Size | M |

## Goal
The packet inspector you asked for: start/stop the proxy from the GUI (in-process, D-35), watch packets live, filter them, inspect decoded fields and hex, save and load captures.

## Java source (the spec)
- none (capture format: `crates/l2-proxy/src/capture.rs`)

## Reference implementation to copy
- `crates/l2-admin-gui/src/views/logs.rs` (virtual list, filters)

## Files
Create:
- `crates/l2-admin-gui/src/views/packets/{mod.rs,list.rs,detail.rs,proxy_control.rs}`
Modify:
- `crates/l2-admin-gui/src/views/mod.rs`, `crates/l2-admin-gui/Cargo.toml` (depend on `l2-proxy`)

## Requirements
1. Proxy control: listen address, upstream LS address, start/stop, status (connections, packet count). Settings persisted.
2. List: time, connection, direction (arrows, coloured), channel, opcode (hex), name, size. Virtual scrolling for 100k+ records.
3. Filters: direction, channel, opcode/name text, "hide noisy" (a user-editable list, e.g. MoveToLocation/ValidatePosition). Filters apply live.
4. Detail pane: decoded fields as a tree (when `fields` exists), plus a hex dump with offsets and ASCII and UTF-16 columns. Clicking a field highlights its bytes if offsets are available (optional; say in the report if skipped).
5. Save the current capture to JSONL, open a JSONL file (read-only mode, no proxy needed).

## Out of scope
- Editing or replaying packets.

## Tests
- Unit tests for filtering and the hex-dump formatter.

## Done when
- [ ] a real H5 login + enter world through the proxy shows up live (screenshot in the report)
- [ ] committed as `M1-GUI-04: <summary>`
