# S-API-02: Move the built-ins onto the engine

| Field | Value |
|---|---|
| Milestone | Scripts |
| Model | sonnet |
| Review | full |
| Reference | `docs/design/scripts.md`, `crates/l2-scripts/src/quests/q00001_letters_of_love.rs`, `crates/l2-scripts/src/ai/others/asamah.rs` |
| Depends on | API-01 |
| Size | L |

## Goal
The scripts that earlier cards built into the world by hand become real scripts in `l2-scripts` with the same behaviour: `village_master/ClanMaster`, `AllianceMaster`, `ai/others/ManorManager`, `SymbolMaker`, the wedding priest (`custom` Wedding as Java's script is), `MonumentOfHeroes`, the conquerable hall scripts (`conquerablehalls/*`), `KrateisCube`, `HandysBlockCheckerEvent`, the boat route scripts (`vehicles/*`), `NewbieGuide` (Q00001's guide block). Remove the first-talk and process-event chains from `manor/npc.rs`, `clan/master_script.rs` and the other built-in hooks once each script is in. Fix S-HAND-01's `tutorial_close_handler` to call `World::send_tutorial_close_html`.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\village_master\{ClanMaster,AllianceMaster}`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\ai\others\{ManorManager,SymbolMaker,MonumentOfHeroes,KrateisCube,HandysBlockCheckerEvent,NewbieGuide}`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\conquerablehalls\**`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\vehicles\**`

## Files
- `crates/l2-scripts/src/{village_master,ai/others,conquerablehalls,vehicles}/*`
- the built-in modules in `crates/l2-world/src/*` that shrink
- tests in the same files

## Requirements
1. Follow `docs/design/scripts.md` exactly: one Rust file per Java script under `crates/l2-scripts/src/<java folder>/`, one `pub mod` and one `r.add(..)` line in that folder's `mod.rs`, tests at the end of the same file with `crate::testing::ScriptTest`.
2. Java behaviour, messages, html (the same page files), drops, rewards and rows exactly (D-39); `global_variables` only under D-54.
3. Scripts hold no state (`ctx.state::<T>()`); timers on the wheel; world RNG; no DB, no threads.
4. If a script needs world behaviour that is not `pub` yet, add a small `pub` method in the world module that owns it (add your lines only) and list it. If the engine lacks a hook, list it and port the rest.
5. Keep Java quirks with `// NOTE(java-quirk):`.

## Tests (must exist and pass)
- The existing tests of those systems still pass through the scripts; one script test each.

## Done when
- [ ] fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`, `-j 4`); `git merge main` before the final checks
- [ ] committed as `S-API-02: <summary>`
- [ ] report delivered: every script ported, every script or part left and why, every world method added, every guess

## Notes from the orchestrator
- Read CLAUDE.md (safety rules), DECISIONS (D-05, D-15, D-16, D-39, D-53, D-54) and `docs/design/scripts.md` first.
- Up to eight script workers run at the same time, each in its own folder. Shared files (`l2-scripts/src/lib.rs`, the world modules) take only your added lines; run `git merge main` every hour or so.
