# T-RM-01: Remove the Java tools

| Field | Value |
|---|---|
| Milestone | Tools |
| Model | sonnet |
| Review | spot |
| Reference | D-58 |
| Depends on | — |
| Size | M |

## Goal
1. Delete `java-vectors/src`, `build.sh`, `build.ps1` and `java-vectors/README.md`.
2. Move `java-vectors/out/` (with `java_facts/`) to `crates/l2-testkit/vectors/`. Update `l2-testkit` and every test that names the old path. Tests keep their asserts; the values are the same files.
3. Delete the `crates/codegen` crate and its CI step in `.github/workflows/ci.yml`. Check the rest of `ci.yml`, including the check that forbids `l2jmobius/` paths.
4. Generated code becomes normal code: remove the `@generated` headers. The folders may stay named `src/generated/`, or rename them to something plainer if that's a small change (`generated` -> `tables` or similar); report which.
5. `crates/l2-testkit/src/java_facts.rs` and its tests stay, reading from the new path. Rename the module and the JSON folder if the name says Java and the rename is small.
6. `tools/`: delete `tools/analysis` scripts that need the Java checkout, and make `tools/sync-dist.py` a note-only leftover or delete it (`dist/` is now our own data, changed by hand). Update `dist/README.md`.
7. Docs: `CLAUDE.md`, `README.md`, `docs/MODULE_MAP.md`, `dist/README.md`: no more `C:\l2-rust\l2jmobius`, codegen commands or the "reference material" section; the rule "`src/generated` is never edited by hand" goes. `dist/` may be edited directly. `docs/PLAN.md` and task cards are history and stay.

## Done when
- [ ] fmt, clippy -D warnings and test pass (`CARGO_INCREMENTAL=0`, `-j 4`); `git merge main` before the final checks
- [ ] committed as `T-RM-01: <summary>`; report: what is done, what is left and why, every guess

## Notes from the orchestrator
- Read CLAUDE.md (safety rules) and DECISIONS D-58 first. D-58 replaces the older rules where they differ.
- **Never connect to a database on this machine.** Never bind real ports. Never touch `C:\l2-rust\run` or `C:\l2-rust\release`. Never kill processes by name.
- Never weaken a test to pass. A test that only checks an off-switch goes together with the switch; a test of the on-behaviour stays.
- T-RM-01 and T-RM-02 run at the same time; T-RM-03 comes after both. Don't touch comments for T-RM-03's sake.
