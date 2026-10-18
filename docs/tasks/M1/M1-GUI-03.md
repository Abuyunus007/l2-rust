# M1-GUI-03: Logs view

| Field | Value |
|---|---|
| Milestone | M1 |
| Model | haiku |
| Review | spot |
| Reference | no |
| Depends on | M1-GUI-01, M1-ADM-02 |
| Size | S |

## Goal
A live log viewer for each server: tail, level/text filter, pause, and copy.

## Java source (the spec)
- none

## Reference implementation to copy
- `crates/l2-admin-gui/src/views/servers.rs` (or `connection.rs` if GUI-02 isn't merged yet)

## Files
Create:
- `crates/l2-admin-gui/src/views/logs.rs`
Modify:
- `crates/l2-admin-gui/src/views/mod.rs` (register the view)

## Requirements
1. A server selector (or "all"), level checkboxes (error/warn/info/debug), a text filter (case-insensitive substring), pause/resume, auto-scroll that stops when you scroll up, a "copy selected" action.
2. Keep at most 20,000 lines in memory per server (drop the oldest). Render with egui's virtual scrolling (`ScrollArea::show_rows`) so it stays fast.
3. Errors in red, warnings in yellow.

## Out of scope
- Saving logs to file (the servers already write log files).

## Tests
- Unit tests for the filter logic and the line cap.

## Done when
- [ ] works against a running supervisor (screenshot in the report)
- [ ] committed as `M1-GUI-03: <summary>`
