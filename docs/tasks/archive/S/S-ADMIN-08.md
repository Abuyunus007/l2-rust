# S-ADMIN-08: Admin words of the script systems

| Field | Value |
|---|---|
| Milestone | Scripts |
| Model | sonnet |
| Review | spot |
| Reference | `crates/l2-world/src/script/admin.rs` (`admin_show_quests`, `admin_quest_info`), the other admin handlers in `crates/l2-world/src/admin/` |
| Depends on | API-05 |
| Size | M |

## Goal
The quest and script admin commands that are still missing, in `script/admin.rs` next to the two that exist:
- `AdminShowQuests`: `admin_charquestmenu` (the per-character quest menu and its sub-pages) and `admin_setcharquest` (set a quest variable/state on the target; written as a normal `character_quests` row change, D-39).
- `AdminQuest`: `admin_quest_reload`, `admin_script_load`, `admin_script_unload`. The Rust scripts are compiled in, so reload and load cannot re-read a Java file (D-16). Port `script_unload` / `quest_reload` as unload-then-reinstall of the registered script (or, where that is not possible, the same answer page with a clear "restart needed" system message). Document what you chose in `docs/design/scripts.md`.
- `AdminReload`: check the `quest`/`script`/`scripts` sub-words and answer the same way.
- Check `//setclass`, `//show_quests` and `//quest_info` against Java once more and fix what differs.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\chat\commands\admin\{AdminShowQuests,AdminQuest,AdminReload,AdminEditChar}.java`
- the admin html pages under `dist\game\data\html\admin\` they use

## Files
- `crates/l2-world/src/script/admin.rs`, `crates/l2-world/src/script/registry*.rs` (unload/reinstall only), `docs/design/scripts.md`
- tests

## Requirements
1. Java behaviour, pages and messages exactly; rows as Java writes them (D-39); no DB in a tick (D-15).
2. Add only your lines to shared files. Keep Java quirks with `// NOTE(java-quirk):`.

## Tests (must exist and pass)
- Each command: the page shown, the state change, the bad-argument answer; unload stops a script's events and reload brings them back.

## Done when
- [ ] fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`, `-j 4`); `git merge main` before the final checks
- [ ] committed as `S-ADMIN-08: <summary>`
- [ ] report delivered: every command done, every part left and why, every guess

## Notes from the orchestrator
- Read CLAUDE.md (safety rules), DECISIONS (D-15, D-16, D-39) first.
- S-HAND-04, S-QR-15 and S-LEFT-01 run at the same time: stay out of their files.
