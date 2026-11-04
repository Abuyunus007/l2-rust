# S-API-04: Engine: global variable re-read, per-object listeners, the remaining events

| Field | Value |
|---|---|
| Milestone | Scripts |
| Model | sonnet |
| Review | full |
| Reference | `docs/design/scripts.md`, `crates/l2-scripts/src/quests/q00001_letters_of_love.rs`, `crates/l2-scripts/src/ai/others/asamah.rs` |
| Depends on | API-03 |
| Size | L |

## Goal
What S-API-03 left: (1) the D-50 re-read of `global_variables` before a boss or event script decides with `CycleDatabaseWrites` off (a `PersistCommand` request and `WorldCommand` answer like `BossRereadRequest`, the l2-db read, a ctx helper that runs a continuation when the rows are back); (2) per-object dynamic listeners (`addListener`/`removeListener` on players and NPCs, Java's `ListenersContainer`) and the remaining `EventType`s: `ON_CREATURE_ATTACKED/ATTACK/ATTACK_AVOID/SKILL_USE/DAMAGE_DEALT/DAMAGE_RECEIVED/KILLED` (with the `TerminateReturn` answer of `Playable.doDie`), `ON_PLAYER_SIT/LEVEL_CHANGED/ITEM_ADD/ITEM_PICKUP/PROFESSION_CHANGE/SUMMON_TALK`, `ON_PLAYABLE_EXP_CHANGED`, `ON_NPC_MANOR_BYPASS`, `ON_TRAP_ACTION`, `ON_DAILY_RESET`, and the clan, castle, fort and olympiad finish events; fire each where Java fires it. Also: a config-gated registration (`Script::enabled(&World)` or a setup flag) so MonumentOfHeroes and HandysBlockCheckerEvent register only when Java does, and the `ON_NPC_MANOR_BYPASS` listener so ManorManager's `onNpcManorBypass` moves into its script. Only add to the API: script workers use it right now.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\mechanics\events\**`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\mechanics\script\*.java`

## Files
- `crates/l2-world/src/script/*` + the world hooks
- game-server persist and l2-db for the re-read
- tests

## Requirements
1. Follow `docs/design/scripts.md` exactly: one Rust file per Java script under `crates/l2-scripts/src/<java folder>/`, one `pub mod` and one `r.add(..)` line in that folder's `mod.rs`, tests at the end of the same file with `crate::testing::ScriptTest`.
2. Java behaviour, messages, html (the same page files), drops, rewards and rows exactly (D-39); `global_variables` only under D-54.
3. Scripts hold no state (`ctx.state::<T>()`); timers on the wheel; world RNG; no DB, no threads.
4. If a script needs world behaviour that is not `pub` yet, add a small `pub` method in the world module that owns it (add your lines only) and list it. If the engine lacks a hook, list it and port the rest.
5. Keep Java quirks with `// NOTE(java-quirk):`.

## Tests (must exist and pass)
- A test per new event type and for the re-read (switch off, the row changed by the other server).

## Done when
- [ ] fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`, `-j 4`); `git merge main` before the final checks
- [ ] committed as `S-API-04: <summary>`
- [ ] report delivered: every script ported, every script or part left and why, every world method added, every guess

## Notes from the orchestrator
- Read CLAUDE.md (safety rules), DECISIONS (D-05, D-15, D-16, D-39, D-53, D-54) and `docs/design/scripts.md` first.
- Up to eight script workers run at the same time, each in its own folder. Shared files (`l2-scripts/src/lib.rs`, the world modules) take only your added lines; run `git merge main` every hour or so.
