# S-PILOT-01: Ten reference quests and the porting guide

| Field | Value |
|---|---|
| Milestone | Scripts |
| Model | opus |
| Review | full |
| Reference | `docs/design/scripts.md`, `crates/l2-scripts/src/quests/q00001_letters_of_love.rs`, `crates/l2-scripts/src/ai/others/asamah.rs` |
| Depends on | API-01 |
| Size | L |

## Goal
Port ten quests that cover the patterns the 505 quests use, and extend `docs/design/scripts.md` with a 'quest patterns' section the haiku batches follow: a kill-count quest with drop chance (`giveItemRandomly`), a delivery chain over several NPCs, a party-kill quest (`getRandomPartyMember`), a repeatable quest, a one-time quest with exp/adena/items rewards, a class- or race-restricted quest, a quest with a timer, a quest that spawns a monster (`addSpawn`), one `AbstractSagaQuest` saga (and the saga base), and a tutorial quest (`Q00255_Tutorial`). Pick concrete ids from the Java folder and say which pattern each one is.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\quests\AbstractSagaQuest.java`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\quests\Q00255_Tutorial`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\quests\ (pick the rest)`

## Files
- `crates/l2-scripts/src/quests/*`
- `docs/design/scripts.md`
- tests in the same files

## Requirements
1. Follow `docs/design/scripts.md` exactly: one Rust file per Java script under `crates/l2-scripts/src/<java folder>/`, one `pub mod` and one `r.add(..)` line in that folder's `mod.rs`, tests at the end of the same file with `crate::testing::ScriptTest`.
2. Java behaviour, messages, html (the same page files), drops, rewards and rows exactly (D-39); `global_variables` only under D-54.
3. Scripts hold no state (`ctx.state::<T>()`); timers on the wheel; world RNG; no DB, no threads.
4. If a script needs world behaviour that is not `pub` yet, add a small `pub` method in the world module that owns it (add your lines only) and list it. If the engine lacks a hook, list it and port the rest.
5. Keep Java quirks with `// NOTE(java-quirk):`.

## Tests (must exist and pass)
- Each quest from start to reward with its rows; the saga base.

## Done when
- [ ] fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`, `-j 4`); `git merge main` before the final checks
- [ ] committed as `S-PILOT-01: <summary>`
- [ ] report delivered: every script ported, every script or part left and why, every world method added, every guess

## Notes from the orchestrator
- Read CLAUDE.md (safety rules), DECISIONS (D-05, D-15, D-16, D-39, D-53, D-54) and `docs/design/scripts.md` first.
- Up to eight script workers run at the same time, each in its own folder. Shared files (`l2-scripts/src/lib.rs`, the world modules) take only your added lines; run `git merge main` every hour or so.
