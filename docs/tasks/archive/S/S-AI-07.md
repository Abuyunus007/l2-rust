# S-AI-07: Gracia instances and the seed managers

| Field | Value |
|---|---|
| Milestone | Scripts |
| Model | opus |
| Review | full |
| Reference | `docs/design/scripts.md`, `crates/l2-scripts/src/quests/q00001_letters_of_love.rs`, `crates/l2-scripts/src/ai/others/asamah.rs` |
| Depends on | AI-01 |
| Size | L |

## Goal
What S-AI-01 left: `instances/SeedOfDestruction/SeedOfDestruction.java`, `HallOfErosionAttack`, `HallOfErosionDefence`, `HallOfSufferingAttack`, `HallOfSufferingDefence`, `HeartInfinityAttack`, `HeartInfinityDefence`, plus Java's `SeedOfDestructionManager` / `SeedOfInfinityManager` logic (stage state, openSeed/closeSeed timers, door 14240102, zone 60010 exit) on the global variables (D-54, `ctx.reread_global_variables` with the switch off), the `GraciaSeeds.ini` values exposed to scripts, and the effect zones that apply skills (`EffectZone.addSkill/clearSkills`) for the Seed of Annihilation buffs. Build on S-AI-01's `gracia/seeds.rs`, `util.rs` and `instances/`.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\ai\areas\Gracia\instances\**`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\managers\ (grep SeedOfDestruction, SeedOfInfinity)`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\zone\type\EffectZone.java`

## Files
- `crates/l2-scripts/src/ai/areas/gracia/*`
- `crates/l2-world/src/zones.rs` (effect zone skills), the script settings
- tests

## Requirements
1. Follow `docs/design/scripts.md` exactly: one Rust file per Java script under `crates/l2-scripts/src/<java folder>/`, one `pub mod` and one `r.add(..)` line in that folder's `mod.rs`, tests at the end of the same file with `crate::testing::ScriptTest`.
2. Java behaviour, messages, html (the same page files), drops, rewards and rows exactly (D-39); `global_variables` only under D-54.
3. Scripts hold no state (`ctx.state::<T>()`); timers on the wheel; world RNG; no DB, no threads.
4. If a script needs world behaviour that is not `pub` yet, add a small `pub` method in the world module that owns it (add your lines only) and list it. If the engine lacks a hook, list it and port the rest.
5. Keep Java quirks with `// NOTE(java-quirk):`.

## Tests (must exist and pass)
- Each instance: enter, a stage, finish; the seed state change with the switch on and off; an effect zone buff.

## Done when
- [ ] fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`, `-j 4`); `git merge main` before the final checks
- [ ] committed as `S-AI-07: <summary>`
- [ ] report delivered: every script ported, every script or part left and why, every world method added, every guess

## Notes from the orchestrator
- Read CLAUDE.md (safety rules), DECISIONS (D-05, D-15, D-16, D-39, D-53, D-54) and `docs/design/scripts.md` first.
- Up to eight script workers run at the same time, each in its own folder. Shared files (`l2-scripts/src/lib.rs`, the world modules) take only your added lines; run `git merge main` every hour or so.
