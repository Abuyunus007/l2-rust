# S-API-03: Engine follow-ups

| Field | Value |
|---|---|
| Milestone | Scripts |
| Model | sonnet |
| Review | full |
| Reference | `docs/design/scripts.md`, `crates/l2-scripts/src/quests/q00001_letters_of_love.rs`, `crates/l2-scripts/src/ai/others/asamah.rs` |
| Depends on | API-01 |
| Size | L |

## Goal
The engine gaps S-API-01 listed: `TimerExecutor` / `onTimerEvent` (7 scripts use it), `CreatureSeeTaskManager` so `on_creature_see` fires, `ON_PLAYER_BYPASS` and `ON_PLAYER_PRESS_TUTORIAL_MARK`, the skill of a hit passed to `on_attack`, the movie holder (`ExStartScenePlayer`/`EndScenePlayer`, `playMovie` waits), `GmViewQuestInfo` contents for `//show_quests` (and the `//quest_*` admin words), the D-50 re-read of `global_variables` before a boss/event script decides, freeing the runtime spawns of `add_spawn` when the NPC is deleted, and the remaining event types other cards marked `FOLLOW-UP(D-05)` (ON_CREATURE_ATTACK, ON_PLAYER_* events, clan/siege/olympiad events) as engine hooks.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\mechanics\script\*.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\mechanics\events\**`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\taskmanagers\CreatureSeeTaskManager.java`

## Files
- `crates/l2-world/src/script/*` + the world hooks
- tests
- tests in the same files

## Requirements
1. Follow `docs/design/scripts.md` exactly: one Rust file per Java script under `crates/l2-scripts/src/<java folder>/`, one `pub mod` and one `r.add(..)` line in that folder's `mod.rs`, tests at the end of the same file with `crate::testing::ScriptTest`.
2. Java behaviour, messages, html (the same page files), drops, rewards and rows exactly (D-39); `global_variables` only under D-54.
3. Scripts hold no state (`ctx.state::<T>()`); timers on the wheel; world RNG; no DB, no threads.
4. If a script needs world behaviour that is not `pub` yet, add a small `pub` method in the world module that owns it (add your lines only) and list it. If the engine lacks a hook, list it and port the rest.
5. Keep Java quirks with `// NOTE(java-quirk):`.

## Tests (must exist and pass)
- Each hook fires like Java (a test per hook).

## Done when
- [ ] fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`, `-j 4`); `git merge main` before the final checks
- [ ] committed as `S-API-03: <summary>`
- [ ] report delivered: every script ported, every script or part left and why, every world method added, every guess

## Notes from the orchestrator
- Read CLAUDE.md (safety rules), DECISIONS (D-05, D-15, D-16, D-39, D-53, D-54) and `docs/design/scripts.md` first.
- Up to eight script workers run at the same time, each in its own folder. Shared files (`l2-scripts/src/lib.rs`, the world modules) take only your added lines; run `git merge main` every hour or so.
