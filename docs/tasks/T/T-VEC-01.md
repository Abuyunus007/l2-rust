# T-VEC-01: Tests that read Java sources use committed fact files

| Field | Value |
|---|---|
| Milestone | Tools |
| Model | sonnet |
| Review | spot |
| Reference | `crates/codegen` (its targets and `--check` mode), `java-vectors/out/` (committed JSON read through `l2-testkit`) |
| Depends on | T-DIST-01 |
| Size | M |

## Goal
A few tests still compare the Rust port with the **Java source text**. They read `.java` files
from `C:\l2-rust\l2jmobius\java\...` or from the `scripts/` folders. `dist/` has no `.java` files,
so these tests skip in CI and on every machine without the L2JMobius checkout.

Move the facts they need into committed JSON files, produced by a new `codegen` target. The tests
then read the JSON and run everywhere. Only regenerating the JSON needs the Java checkout.

Known tests (find any others: `grep -rn '\.java' crates --include=*.rs`, and every test that skips
because a `.java` file is missing):
- `l2-world/src/handler_registry.rs`: the handler audit. It covers the NPC bypass handlers, the
  community board words, the item handlers, and the user, voiced and click command lists.
- `l2-world/tests/castle_npcs.rs`: the Castle script links.
- `l2-world/tests/vehicle.rs`: the boat paths, the SoD controller and the Gludio-Gracia airship
  routes.
- `l2-data/tests/player_templates.rs`: the race of each class (`PlayerClass.java`).

## Steps
1. Add a `codegen` target `java-facts`. It reads the Java sources from `L2J_ROOT`, as the other
   targets do. It writes one JSON file per test topic to `java-vectors/out/java_facts/`, for
   example `handlers.json`, `castle_links.json`, `boat_routes.json` and `player_classes.json`.
   - Each file holds exactly what the test compared before: class names with their command words
     and bypass prefixes, the route points, and the class id to race map.
   - The JSON is sorted and stable, so a regenerate gives no diff.
   - `codegen -- all --check` covers the new target. In CI it skips with a notice, as the other
     targets do.
2. Switch each test to read its JSON through `l2-testkit` (add a small loader next to the vector
   loader). A missing JSON file is a test failure, because the files are committed.
3. Keep the same asserts. The test must still catch a Rust change that drifts from Java. Never
   weaken an assert.
4. Update `dist/README.md`, section "What still needs L2JMobius": only `codegen` and
   `java-vectors/build.*` remain.

## Files
- `crates/codegen/src/targets/java_facts.rs` (new) and the target list in `crates/codegen/src/main.rs`
- `java-vectors/out/java_facts/*.json` (new, generated)
- `crates/l2-testkit/src/*` (the loader)
- the test files above, and any other test you find that reads `.java` files
- `dist/README.md`

## Done when
- [ ] No test outside `codegen` opens a `.java` file. Check with `grep -rn '\.java' crates --include=*.rs`, which may still match comments and `script_load` strings.
- [ ] `cargo run -p codegen -- java-facts --check` is clean.
- [ ] fmt, clippy -D warnings and test pass (`CARGO_INCREMENTAL=0`, `-j 4`); run `git merge main` first.
- [ ] Committed as `T-VEC-01: <summary>`. The report lists the files, the tests that now run without the Java checkout, and every guess.
