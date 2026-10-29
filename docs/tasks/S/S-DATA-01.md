# S-DATA-01: Category data, ClassMaster.xml, transformation tree, small script helpers

| Field | Value |
|---|---|
| Milestone | Scripts |
| Model | sonnet |
| Review | full |
| Reference | `crates/l2-world/src/subclass/categories.rs` (the hard-coded subset S-VM-01 wrote) |
| Depends on | VM-01 |
| Size | M |

## Goal
Data and helpers that the quest and AI batches found missing:
- A full `CategoryData` loader in l2-data (`data/CategoryData.xml`, every `CategoryType`), replacing the hard-coded subset in `subclass/categories.rs`. `is_in_category` answers every group Java has (HEAL_GROUP, KNIGHT_GROUP and all others).
- The `ClassMaster.xml` loader and the game-server construction of `ClassMaster::new(config)` (S-VM-01 left the default).
- The transformation skill tree in `SkillTrees` (AvantGarde's `LearnTransformationSkill`).
- `ScriptCtx` helpers: the sponsor and apprentice of a player (`getSponsor`, `getApprentice`, `getPledgeType`; Q00123), a social action sent to one player only (`player.sendPacket(new SocialAction(..))`; Q00062, Q00215), and a destroy of an item in a warehouse (AvantGarde's CancelCertification).
- Fix the call sites that keep a local category list marked `FOLLOW-UP(engine)` (grep `l2-scripts` for it) to use `is_in_category`.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\data\xml\CategoryData.java`, `C:\l2-rust\l2jmobius\dist\game\data\CategoryData.xml`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\ai\others\ClassMaster\ClassMaster.java` (its XML loader)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\data\xml\SkillTreeData.java` (transformSkillTree)

## Files
- `crates/l2-data/src/*`, `crates/l2-world/src/subclass/categories.rs`, `crates/l2-world/src/script/*` (helpers), `crates/game-server/src/lib.rs`, the l2-scripts call sites; tests

## Requirements
1. Java behaviour and data exactly; no unwrap/expect on file data.
2. Only add to the script API; quest workers use it right now.

## Tests (must exist and pass)
- The category loader against the shipped XML (skips without the reference folder); each helper.

## Done when
- [ ] fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`, `-j 4`); `git merge main` before the final checks
- [ ] committed as `S-DATA-01: <summary>`
- [ ] report delivered

## Notes from the orchestrator
- Read CLAUDE.md and DECISIONS first. Disk space is tight: set CARGO_PROFILE_DEV_DEBUG=0 and CARGO_PROFILE_TEST_DEBUG=0.
