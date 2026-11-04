# S-AI-04: Area AIs part 2

| Field | Value |
|---|---|
| Milestone | Scripts |
| Model | sonnet |
| Review | spot |
| Reference | `docs/design/scripts.md`, `crates/l2-scripts/src/quests/q00001_letters_of_love.rs`, `crates/l2-scripts/src/ai/others/asamah.rs` |
| Depends on | API-01 |
| Size | L |

## Goal
Every script in the remaining `ai/areas` folders: CrumaTower, DenOfEvil, IsleOfPrayer, Rune, FrozenLabyrinth, StakatoNest, PlainsOfLizardman, VarkaSilenosBarracks, KetraOrcOutpust, SilentValley, Giran, TowerOfInsolence, MithrilMines, CryptsOfDisgrace, HotSprings, GiantsCave, PlainsOfDion, ForestOfTheDead, PavelArchaic.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\ai\areas\CrumaTower\**`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\ai\areas\DenOfEvil\**`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\ai\areas\IsleOfPrayer\**`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\ai\areas\Rune\**`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\ai\areas\FrozenLabyrinth\**`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\ai\areas\StakatoNest\**`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\ai\areas\PlainsOfLizardman\**`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\ai\areas\VarkaSilenosBarracks\**`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\ai\areas\KetraOrcOutpust\**`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\ai\areas\SilentValley\**`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\ai\areas\Giran\**`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\ai\areas\TowerOfInsolence\**`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\ai\areas\MithrilMines\**`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\ai\areas\CryptsOfDisgrace\**`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\ai\areas\HotSprings\**`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\ai\areas\GiantsCave\**`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\ai\areas\PlainsOfDion\**`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\ai\areas\ForestOfTheDead\**`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\ai\areas\PavelArchaic\**`

## Files
- `crates/l2-scripts/src/ai/areas/*`
- tests in the same files

## Requirements
1. Follow `docs/design/scripts.md` exactly: one Rust file per Java script under `crates/l2-scripts/src/<java folder>/`, one `pub mod` and one `r.add(..)` line in that folder's `mod.rs`, tests at the end of the same file with `crate::testing::ScriptTest`.
2. Java behaviour, messages, html (the same page files), drops, rewards and rows exactly (D-39); `global_variables` only under D-54.
3. Scripts hold no state (`ctx.state::<T>()`); timers on the wheel; world RNG; no DB, no threads.
4. If a script needs world behaviour that is not `pub` yet, add a small `pub` method in the world module that owns it (add your lines only) and list it. If the engine lacks a hook, list it and port the rest.
5. Keep Java quirks with `// NOTE(java-quirk):`.

## Tests (must exist and pass)
- One test per script.

## Done when
- [ ] fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`, `-j 4`); `git merge main` before the final checks
- [ ] committed as `S-AI-04: <summary>`
- [ ] report delivered: every script ported, every script or part left and why, every world method added, every guess

## Notes from the orchestrator
- Read CLAUDE.md (safety rules), DECISIONS (D-05, D-15, D-16, D-39, D-53, D-54) and `docs/design/scripts.md` first.
- Up to eight script workers run at the same time, each in its own folder. Shared files (`l2-scripts/src/lib.rs`, the world modules) take only your added lines; run `git merge main` every hour or so.
