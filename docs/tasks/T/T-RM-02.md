# T-RM-02: Remove the side-by-side mode

| Field | Value |
|---|---|
| Milestone | Tools |
| Model | sonnet |
| Review | full |
| Reference | D-58, and D-39, D-49..D-54 for what is being removed |
| Depends on | — |
| Size | L |

## Goal
The Rust servers are the only servers on their database. Every behaviour that was switched off for a Java game server's sake now always runs. Remove the code paths that existed only for that.

1. **Switches:** remove `SideBySide.ini` and every `*DatabaseWrites` key: `DailyResetDatabaseWrites`, `ClanMaintenanceDatabaseWrites`, `MailExpiryDatabaseWrites`, `FishingChampionshipDatabaseWrites`, `CycleDatabaseWrites`, `OfflineTradeDatabaseWrites`, `SiegeDatabaseWrites`, and any other side-by-side key (`git grep SideBySide`, `git grep DatabaseWrites`). Keep the **on** branch of each `if`; delete the **off** branch, the setting, the loader and the plumbing.
2. **Other side-by-side behaviour:**
   - D-50 fresh re-reads of shared rows: remove them where they exist only because another server could change a row (also `reread_global_variables`).
   - D-52 offline trade: the account hold and "LS kick releases the trader" go; offline traders and offline play follow the on-behaviour.
   - D-54: grand boss AIs follow their own state; the "runs only while the row says alive" gate goes; bosses spawn and write as the on-behaviour does.
   - D-51 sieges: results always written.
   - Crest ids and other "side by side" special cases: `git grep -i 'side by side'` in code and handle each.
   - The object id range from `0x60000000` stays (it works and existing rows use it); report it.
3. **GUI:** remove the Java game server entry and the SideBySide switches from the setup wizard; remove the "Switch to Rust" assistant (`views/switch*.rs`), the Side-by-side topic in the settings editor, and any Java hint. The `BackupNow`/backup views stay.
4. **Supervisor:** remove the Java server block from `supervisor.example.ini`.
5. **dist and packaging:** delete `dist/game/config/SideBySide.ini`, `crates/game-server/config-examples/SideBySide.ini`, and the mentions in `tools/package.py` and `tools/sync-dist.py`.
6. **Docs:** `README.md` (remove the "Running next to a Java game server" section and the rows about it), the runbooks that describe side by side: delete `docs/runbooks/m9-switch.md`, and add a short note at the top of the older runbooks that side-by-side parts are history. `docs/design/*.md`: remove side-by-side rules.
7. **Tests:** tests of the off-behaviour go; the tests of the on-behaviour become plain tests without the switch. E2E `side_by_side_with_the_switch_*` keep the on-path as a plain test.

## Report
List every behaviour that changed for a running server compared with the old default (switch off): those are what the owner gets on the next start.

## Done when
- [ ] fmt, clippy -D warnings and test pass (`CARGO_INCREMENTAL=0`, `-j 4`); `git merge main` before the final checks
- [ ] committed as `T-RM-02: <summary>`; report: what is done, what is left and why, every guess

## Notes from the orchestrator
- Read CLAUDE.md (safety rules) and DECISIONS D-58 first. D-58 replaces the older rules where they differ.
- **Never connect to a database on this machine.** Never bind real ports. Never touch `C:\l2-rust\run` or `C:\l2-rust\release`. Never kill processes by name.
- Never weaken a test to pass. A test that only checks an off-switch goes together with the switch; a test of the on-behaviour stays.
- T-RM-01 and T-RM-02 run at the same time; T-RM-03 comes after both. Don't touch comments for T-RM-03's sake.
