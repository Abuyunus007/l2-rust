# S-LEFT-01: Script milestone stragglers

| Field | Value |
|---|---|
| Milestone | Scripts |
| Model | sonnet |
| Review | full |
| Reference | `docs/design/scripts.md` (all "Added by" sections), `crates/l2-scripts/src/ai/others/healer_trainer.rs`, `crates/l2-scripts/src/ai/others/npc_buffers.rs` |
| Depends on | API-05 |
| Size | L |

## Goal
The small pieces the script cards left. Each is independent; do them in this order and report any you could not finish.

1. **Transfer skill trees.** Load `transferSkillTree` from `dist/game/data/skillTrees/` (the loader that owns the other trees in l2-data), add Java `SkillTreeData.getAvailableTransferSkills(player)` / `getTransferSkill(..)`, and finish the `FOLLOW-UP(engine)` loops in `ai/others/healer_trainer.rs` and the last part of `is_skill_allowed` (`skills/learn.rs` NOTE at the transfer trees).
2. **CabaleBuffer** (`ai/others/NpcBuffers/impl/CabaleBuffer.java`): one file next to `npc_buffers.rs`, registered in `ai/others/mod.rs`.
3. **Events:** check `events/character_birthday.rs` against `CharacterBirthday.java` (the daily-reset birthday mail) and finish what is missing. The `destroyItemsOnEnd` offline DELETE of the long-time events must run only when `CycleDatabaseWrites` is on (D-53: a global write) and through the outbox (D-15).
4. **ExChangeClientEffectInfo:** move the private packet out of `ice_queens_castle_battle.rs` into `l2-protocol-game` (with a round-trip test); the script uses it from there.
5. **ScriptTest fixtures:** a way to load a skill table (and the transformation table) into a test world, and a `set_spoiled` test hook. Use them for the untested paths: Q00367 Catherok storm, Q00417 spoiled kills, Q00365 poison, Q00501 FATAL_POISON hand-in, Sanctum of the Lords of Dawn transformation.
6. **Small ones:** Minigame burners through `spawns_of(BURNER)` instead of the NPC scan; Q00224 switched to `ctx.killing_blow_weapon` if a local stub is still there.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\data\xml\SkillTreeData.java`, `dist\game\data\skillTrees\*`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\ai\others\HealerTrainer\HealerTrainer.java`, `ai\others\NpcBuffers\impl\CabaleBuffer.java`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\events\CharacterBirthday\CharacterBirthday.java`, `java\org\l2jmobius\gameserver\model\quest\LongTimeEvent.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\serverpackets\ExChangeClientEffectInfo.java`

## Files
- l2-data skill tree loader, `crates/l2-world/src/skills/*`, `crates/l2-scripts/src/{ai/others,events,testing.rs}`, the quest files named above (tests only), `crates/l2-protocol-game/src/*` (the packet), `crates/l2-scripts/src/ai/areas/.../ice_queens_castle_battle.rs`, `minigame.rs`, `q00224*.rs`
- tests

## Requirements
1. Follow `docs/design/scripts.md`; Java behaviour, messages, pages and rows exactly (D-39); `global_variables` only under D-54.
2. No DB in a tick (D-15), timers on the wheel, world RNG, no threads.
3. Add only your lines to shared files. Keep Java quirks with `// NOTE(java-quirk):`.

## Tests (must exist and pass)
- Transfer skills listed and learned at a healer trainer; CabaleBuffer buff; the birthday mail; the event DELETE only with the switch on; packet round trip; each newly covered quest path above.

## Done when
- [ ] fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`, `-j 4`); `git merge main` before the final checks
- [ ] committed as `S-LEFT-01: <summary>`
- [ ] report delivered: every item done, every item left and why, every world method added, every guess

## Notes from the orchestrator
- Read CLAUDE.md (safety rules), DECISIONS (D-05, D-15, D-16, D-39, D-53, D-54) and `docs/design/scripts.md` first.
- S-HAND-04 (decoys, traps, drop search) and S-QR-11/13/15 (quests) run at the same time: stay out of their files.
