# S-HAND-04: Decoys, traps and the drop search board

| Field | Value |
|---|---|
| Milestone | Scripts |
| Model | sonnet |
| Review | full |
| Reference | `docs/design/scripts.md`, `crates/l2-scripts/src/quests/q00001_letters_of_love.rs`, `crates/l2-scripts/src/ai/others/asamah.rs` |
| Depends on | HAND-03 |
| Size | L |

## Goal
What S-HAND-03 left: the `Decoy` and `Trap` creature kinds (the SummonNpc "Decoy" branch and the SummonTrap effects, their AI, CharInfo for the decoy, targeting, `DecoyClick`/`TrapClick`, `TrapDetect`/`TrapRemove`, firing `ON_TRAP_ACTION` from S-API-04's event), and `DropSearchBoard` (`_bbs_search_item`, `_bbs_search_drop`, `_bbs_npc_trace`: a drop index over all NPC templates, the item-name search, the board registry entry) reusing `rewards::drop` and HAND-03's NpcViewMod rate code. Update `handler_registry.rs`. This card is world code: the l2-scripts file rules do not apply.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\instance\Decoy.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\instance\Trap.java`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\bypass\communityboard\DropSearchBoard.java`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\actions\click\{DecoyClick,TrapClick}.java`

## Files
- `crates/l2-world/src/*` (new files per part) + the registries
- tests

## Requirements
1. Follow `docs/design/scripts.md` exactly: one Rust file per Java script under `crates/l2-scripts/src/<java folder>/`, one `pub mod` and one `r.add(..)` line in that folder's `mod.rs`, tests at the end of the same file with `crate::testing::ScriptTest`.
2. Java behaviour, messages, html (the same page files), drops, rewards and rows exactly (D-39); `global_variables` only under D-54.
3. Scripts hold no state (`ctx.state::<T>()`); timers on the wheel; world RNG; no DB, no threads.
4. If a script needs world behaviour that is not `pub` yet, add a small `pub` method in the world module that owns it (add your lines only) and list it. If the engine lacks a hook, list it and port the rest.
5. Keep Java quirks with `// NOTE(java-quirk):`.

## Tests (must exist and pass)
- A decoy summoned and attacked; a trap set, detected, triggered and removed; the drop search pages.

## Done when
- [ ] fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`, `-j 4`); `git merge main` before the final checks
- [ ] committed as `S-HAND-04: <summary>`
- [ ] report delivered: every script ported, every script or part left and why, every world method added, every guess

## Notes from the orchestrator
- Read CLAUDE.md (safety rules), DECISIONS (D-05, D-15, D-16, D-39, D-53, D-54) and `docs/design/scripts.md` first.
- Up to eight script workers run at the same time, each in its own folder. Shared files (`l2-scripts/src/lib.rs`, the world modules) take only your added lines; run `git merge main` every hour or so.
- Added after S-API-05 (its trap item was left for this card): the script trap API too — `ctx.add_trap(..)` (Java `AbstractScript.addTrap`), the `ON_TRAP_ACTION` listener with `TrapAction` (triggered, disarmed, detected, ...), and the trap scripts that wait on it: the Crystal Caverns Dolph door trap (`instances/CrystalCaverns`) and the Gracia corridor traps (`gracia/...`, grep the Java for `addTrap`/`onTrapAction`). Read the "Added by S-API-05" section of scripts.md first; it already has the tamed beast and summon-spawn pieces you can follow.
