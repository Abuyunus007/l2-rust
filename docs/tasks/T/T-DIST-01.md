# T-DIST-01: Tests and the DB schema use the repo's own dist/

| Field | Value |
|---|---|
| Milestone | Tools |
| Model | sonnet |
| Review | spot |
| Reference | `dist/README.md`, D-55 |
| Depends on | — |
| Size | M |

## Goal
Since D-55 the repo has its own datapack, configs and SQL in `dist/`. Every test that still reads
`C:\l2-rust\l2jmobius\dist` (or `L2JMOBIUS_GAME_DIR` / `L2JMOBIUS_DIST` / `L2J_ROOT` + `dist`) must
read the repo's `dist/` instead, so the tests run everywhere, CI included.

1. Add one helper in `l2-testkit`: `dist_dir()` (the repo's `dist/`, found from
   `CARGO_MANIFEST_DIR`), `game_dir()` (`dist/game`), `login_dir()`, `sql_dir()`. Keep the env
   overrides (`L2JMOBIUS_GAME_DIR`, `L2JMOBIUS_DIST`) working as overrides.
2. Switch every test default (`crates/*/tests/*.rs` and the `#[cfg(test)]` modules) to the helper.
   Crates that cannot dev-depend on `l2-testkit` (cycles) use the same `CARGO_MANIFEST_DIR` path.
3. The `l2-db` test schema: load the tables from `dist/db_installer/sql/{login,game}` instead of the
   copies in `crates/l2-db/sql/`, and delete the copies that are byte-identical. Keep Rust-only
   tables (if any) in `crates/l2-db/sql/`.
4. The "skip when missing" paths stay (a checkout without `dist/` still skips with a message).
5. CI: the datapack tests now run on Windows and Linux. Fix what fails on Linux (path case,
   separators) in the test code; if a real bug shows up, report it.
6. Left as is (they need the Java sources, not the datapack): `codegen` (`L2J_ROOT`) and
   `java-vectors/`. Write that in `dist/README.md` under "What still needs L2JMobius".

## Files
- `crates/l2-testkit/src/*`, every test file and `#[cfg(test)]` module that names the L2JMobius
  dist, `crates/l2-db/src/*` (test schema loader), `crates/l2-db/sql/*`, `.github/workflows/ci.yml`
  (only if needed), `dist/README.md`

## Done when
- [ ] `grep -rn 'l2jmobius.dist\|L2JMOBIUS_GAME_DIR' crates` shows no default path outside the helper
- [ ] fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`, `-j 4`); `git merge main` first
- [ ] committed as `T-DIST-01: <summary>`; report: files, tests that now run and did not before, anything that failed
