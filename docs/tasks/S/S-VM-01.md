# S-VM-01: Class change, ClassMaster, certifications, noble quest

| Field | Value |
|---|---|
| Milestone | Scripts |
| Model | opus |
| Review | full |
| Reference | `docs/design/scripts.md`, `crates/l2-scripts/src/quests/q00001_letters_of_love.rs`, `crates/l2-scripts/src/ai/others/asamah.rs` |
| Depends on | API-01, SUB-01 |
| Size | L |

## Goal
The 16 `village_master/*Change1/2` scripts and `FirstClassTransferTalk`, `ai/others/ClassMaster` (the config-driven class master), the third class change quests' rewards path, the subclass quest checks S-SUB-01 left (`checkQuests`: Q234, Q235, Kamael Q236, the `Q00422_RepentYourSins` exit in `setActiveClass`, the class index of `character_quests`), certification skills (`SubclassCertification`, `AvantGarde`, the subclass skill tree, `AcquireSkillType.SUBCLASS`, `getAvailableSubClassSkills`, the `SubClassSkills` validator), and the noble quest path (`MakeMeNoble` markers; the noble quest itself is ported in its quest batch: coordinate by id).

## Java source (the spec)
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\village_master\**`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\ai\others\{ClassMaster,SubclassCertification,AvantGarde}`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\clientpackets\RequestAcquireSkill.java`

## Files
- `crates/l2-scripts/src/{village_master,ai/others}/*`
- `crates/l2-world/src/subclass/*`, `skills/learn.rs`
- tests in the same files

## Requirements
1. Follow `docs/design/scripts.md` exactly: one Rust file per Java script under `crates/l2-scripts/src/<java folder>/`, one `pub mod` and one `r.add(..)` line in that folder's `mod.rs`, tests at the end of the same file with `crate::testing::ScriptTest`.
2. Java behaviour, messages, html (the same page files), drops, rewards and rows exactly (D-39); `global_variables` only under D-54.
3. Scripts hold no state (`ctx.state::<T>()`); timers on the wheel; world RNG; no DB, no threads.
4. If a script needs world behaviour that is not `pub` yet, add a small `pub` method in the world module that owns it (add your lines only) and list it. If the engine lacks a hook, list it and port the rest.
5. Keep Java quirks with `// NOTE(java-quirk):`.

## Tests (must exist and pass)
- 1st and 2nd class change for each race, ClassMaster with the config on, a certification, the subclass quest check.

## Done when
- [ ] fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`, `-j 4`); `git merge main` before the final checks
- [ ] committed as `S-VM-01: <summary>`
- [ ] report delivered: every script ported, every script or part left and why, every world method added, every guess

## Notes from the orchestrator
- Read CLAUDE.md (safety rules), DECISIONS (D-05, D-15, D-16, D-39, D-53, D-54) and `docs/design/scripts.md` first.
- Up to eight script workers run at the same time, each in its own folder. Shared files (`l2-scripts/src/lib.rs`, the world modules) take only your added lines; run `git merge main` every hour or so.
