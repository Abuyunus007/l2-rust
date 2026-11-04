# S-HAND-03: Lottery, NPC view and drop search, decoys and traps, spawn protection

| Field | Value |
|---|---|
| Milestone | Scripts |
| Model | sonnet |
| Review | full |
| Reference | `docs/design/scripts.md`, `crates/l2-scripts/src/quests/q00001_letters_of_love.rs`, `crates/l2-scripts/src/ai/others/asamah.rs` |
| Depends on | HAND-02 |
| Size | L |

## Goal
What S-HAND-02 left: `LotteryManager` with the `games` table, its timers and ticket items, and the `Loto` bypass; `NpcViewMod` (`AltGameViewNpc`) and `DropSearchBoard` with the shared drop-rate code (player bonuses; premium answers as off); the `Decoy` and `Trap` creatures (summon effects, AI, `DecoyClick`/`TrapClick`, trap detection); spawn protection (`Player.setSpawnProtection`, `isSpawnProtected`, used by teleport, FindPvP and the pet restore); the wedding voiced commands returning Java's results so `voice .engage` from pages works; `go_to_love`'s `forceIsCasting - 4`. This card is world code, not scripts: the l2-scripts rules of this template do not apply.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\managers\ (grep Lottery)`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\bypass\npc\Loto.java`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\bypass\npc\NpcViewMod.java`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\bypass\communityboard\DropSearchBoard.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\instance\Decoy.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\instance\Trap.java`

## Files
- `crates/l2-world/src/*` (new files per part) + the registries
- game-server, l2-db (games)
- tests

## Requirements
1. Follow `docs/design/scripts.md` exactly: one Rust file per Java script under `crates/l2-scripts/src/<java folder>/`, one `pub mod` and one `r.add(..)` line in that folder's `mod.rs`, tests at the end of the same file with `crate::testing::ScriptTest`.
2. Java behaviour, messages, html (the same page files), drops, rewards and rows exactly (D-39); `global_variables` only under D-54.
3. Scripts hold no state (`ctx.state::<T>()`); timers on the wheel; world RNG; no DB, no threads.
4. If a script needs world behaviour that is not `pub` yet, add a small `pub` method in the world module that owns it (add your lines only) and list it. If the engine lacks a hook, list it and port the rest.
5. Keep Java quirks with `// NOTE(java-quirk):`.

## Tests (must exist and pass)
- A lottery draw with a winning ticket; the NPC view pages; a decoy and a trap; spawn protection rules.

## Done when
- [ ] fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`, `-j 4`); `git merge main` before the final checks
- [ ] committed as `S-HAND-03: <summary>`
- [ ] report delivered: every script ported, every script or part left and why, every world method added, every guess

## Notes from the orchestrator
- Read CLAUDE.md (safety rules), DECISIONS (D-05, D-15, D-16, D-39, D-53, D-54) and `docs/design/scripts.md` first.
- Up to eight script workers run at the same time, each in its own folder. Shared files (`l2-scripts/src/lib.rs`, the world modules) take only your added lines; run `git merge main` every hour or so.
