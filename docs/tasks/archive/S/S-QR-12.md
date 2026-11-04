# S-QR-12: Remaining quests Q00551..Q00692 (10 quests)

| Field | Value |
|---|---|
| Milestone | Scripts |
| Model | sonnet |
| Review | spot |
| Reference | `docs/design/scripts.md`, `crates/l2-scripts/src/quests/q00001_letters_of_love.rs`, `crates/l2-scripts/src/ai/others/asamah.rs` |
| Depends on | PILOT-01 |
| Size | L |

## Goal
The haiku batches left these quests (mostly the long ones). Port each as one file under `crates/l2-scripts/src/quests/`, following the **Quest patterns** section of `docs/design/scripts.md` and the ported quests around it (many have a sibling already: copy its shape). Use the helpers S-DATA-01 added (`killing_blow_weapon`, `summoned_npc_count`, `add_spawn_summoned`, `send_social_action`, `set_noble`, `player_sponsor`/`player_apprentice`, `world.is_in_category`). If a hook is really missing, port the rest, mark it `// FOLLOW-UP(engine):` and list it.

- `Q00551_OlympiadStarter`
- `Q00552_OlympiadVeteran`
- `Q00553_OlympiadUndefeated`
- `Q00604_DaimonTheWhiteEyedPart2`
- `Q00610_MagicalPowerOfWaterPart2`
- `Q00620_FourGoblets`
- `Q00660_AidingTheFloranVillage`
- `Q00662_AGameOfCards`
- `Q00663_SeductiveWhispers`
- `Q00692_HowtoOpposeEvil`

## Java source (the spec)
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\quests\<quest folder>\*.java` and the html pages next to them

## Files
- `crates/l2-scripts/src/quests/<one file per quest>.rs`, one `pub mod` and one `r.add(..)` line each in `quests/mod.rs`
- tests at the end of each quest file

## Requirements
1. Follow `docs/design/scripts.md` exactly: one Rust file per Java script under `crates/l2-scripts/src/<java folder>/`, one `pub mod` and one `r.add(..)` line in that folder's `mod.rs`, tests at the end of the same file with `crate::testing::ScriptTest`.
2. Java behaviour, messages, html (the same page files), drops, rewards and rows exactly (D-39); `global_variables` only under D-54.
3. Scripts hold no state (`ctx.state::<T>()`); timers on the wheel; world RNG; no DB, no threads.
4. If a script needs world behaviour that is not `pub` yet, add a small `pub` method in the world module that owns it (add your lines only) and list it. If the engine lacks a hook, list it and port the rest.
5. Keep Java quirks with `// NOTE(java-quirk):`.

## Tests (must exist and pass)
- Per quest: start (level/race/class gates), one middle step, the reward and the `character_quests` rows, as the pilot tests do.

## Done when
- [ ] fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`, `-j 4`); `git merge main` before the final checks
- [ ] committed as `S-QR-12: <summary>`
- [ ] report delivered: every script ported, every script or part left and why, every world method added, every guess

## Notes from the orchestrator
- Read CLAUDE.md (safety rules), DECISIONS (D-05, D-15, D-16, D-39, D-53, D-54) and `docs/design/scripts.md` first.
- Up to eight script workers run at the same time, each in its own folder. Shared files (`l2-scripts/src/lib.rs`, the world modules) take only your added lines; run `git merge main` every hour or so.
