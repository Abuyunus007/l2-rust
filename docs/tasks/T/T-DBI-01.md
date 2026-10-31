# T-DBI-01: Database installer (GUI and console)

| Field | Value |
|---|---|
| Milestone | Tools |
| Model | sonnet |
| Review | full |
| Reference | `crates/l2-admin-gui` (egui app setup), `crates/l2-db` (sqlx pool) |
| Depends on | — |
| Size | M |

## Goal
A Rust port of the L2JMobius `DatabaseInstaller` as a new binary crate `l2-db-installer`, so a
new install needs no Java. It installs the schema from the repo's `dist/db_installer/sql/`.

- **GUI** (egui, like `l2-admin-gui`): host, port, user, password, database name; "Install Login"
  and "Install Game" check boxes (both on by default); an Install button; a progress bar and a log.
- **Console mode** (`--console`, and the fallback when no display is available): the same
  questions on stdin, as Java's console mode.
- **Behaviour as Java:**
  - Create the database when it is missing.
  - When it exists: ask Reset (DROP + CREATE), Keep (install into it), or Cancel.
  - Run every `.sql` file of `sql/login` and/or `sql/game`.
  - On a script error: ask Continue or Abort.
  - Show a summary at the end.
  - Defaults come from `dist/db_installer/config/Interface.ini` (`EnableGUI`, `DarkTheme`).
  - The SQL folder defaults to `../db_installer/sql` next to the exe, or `dist/db_installer/sql` from
    the repo; `--sql <dir>` overrides it.
- **Safety:**
  - The password is never logged or written to a file.
  - The Reset question names the database and needs an explicit click or "yes".
  - No database is touched before the user presses Install.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\tools\DatabaseInstaller.java`
- `C:\l2-rust\l2jmobius\dist\db_installer\config\Interface.ini`

## Files
- `crates/l2-db-installer/` (new crate; add it to the workspace `members`)
- `dist/db_installer/` start scripts `DatabaseInstaller.cmd` / `DatabaseInstaller.sh` that run the exe (Rust-only files: add them to the `RUST_ONLY` list of `tools/sync-dist.py` so a sync keeps them)
- `README.md` (the "Database" install step uses the installer), `docs/MODULE_MAP.md` (one row)

## Requirements
1. Dependencies: only what the workspace already uses (`eframe`/`egui`, `sqlx`, `tokio`, `clap`,
   `anyhow`, `tracing`). Ask before adding anything else.
2. Split each SQL file into statements the way Java does (read the Java code: comments, `;` line
   ends, `DELIMITER` if Java handles it) and keep Java's file order. If Java's order is the
   unsorted `File.listFiles` order, sort by name and mark it `NOTE(java-quirk)`.
3. **Never connect to a database on this machine.** Tests use `L2_TEST_DATABASE_URL` only (CI's
   MariaDB): a test installs both schemas into a throwaway database, checks that the tables exist,
   and runs the Reset path. Unit tests cover the statement splitter against every file in
   `dist/db_installer/sql`.

## Done when
- [ ] fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`, `-j 4`); `git merge main` first
- [ ] committed as `T-DBI-01: <summary>`; report: files, what you ported, every guess
