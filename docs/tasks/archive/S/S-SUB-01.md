# S-SUB-01: Subclasses and the class switch

| Field | Value |
|---|---|
| Milestone | Scripts |
| Model | opus |
| Review | full |
| Reference | no (copy the merged M0..M7 code around it) |
| Depends on | none |
| Size | L |

## Goal
The core of subclasses (no quest engine needed): `Player.addSubClass`, `modifySubClass`, `removeSubClass`, `setActiveClass` with `character_subclasses` rows and **every per-class reload Java does** (skills, hennas, effects saved/restored per class index, shortcuts, recipe book, stats and templates, the hero/noble rules, summons, the class index of `character_quests` once S-API-01 lands); `VillageMaster*` core NPCs (`Subclass` bypass: add, change, cancel, the race/class rules `SubclassAllowRace`, the ClassId parents, `MaxSubclass`, level 75 and the quest-free config keys), certification skills (`character_subclasses` certification, `SubClassSkills`), and the markers others left (`setActiveClass` in M6-FU-12, M8-HERO-01, L-01's hennas, `//setclass`). The class change *quests* (village_master scripts) come later with the engine; `ClassMaster` (the config option) belongs here.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\Player.java (addSubClass, modifySubClass, setActiveClass, the subclass parts of restore/store)`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\instance\VillageMaster*.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\holders\player\SubClassHolder.java (grep SubClass)`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\instance\ClassMaster.java (grep)`

## Files
- `crates/l2-world/src/subclass/*` (new) + the reloads (skills, hennas, effects, shortcuts, recipes, stats)
- crates/game-server (load/store per class index), crates/l2-db (character_subclasses)
- tests

## Requirements
1. Java behaviour, messages, packets, HTML and DB rows exactly (D-39: the Java GS shares the DB; D-49..D-54 for global writes and fresh reads).
2. Single-owner world, ids not references, timers on the wheel, world RNG; no unwrap/expect on external data; no DB call or await in a tick (D-15: request/answer through the outbox).
3. Port the packets you need yourself in `crates/l2-protocol-game/src/{client,server}/<area>.rs` with round-trip tests, dispatch arms and proxy decoders (unless M8-GPROTO-11 already has them: check main first).
4. Small wire-ins in shared files are allowed: add your lines only. List every marker you resolve and every one you leave.

## Out of scope
- The customs in `scripts/custom/`, premium, item mall (M9).

## Tests (must exist and pass)
- Add, switch (skills, hennas, effects follow), cancel a subclass; the rules; certification; the rows.

## Done when
- [ ] fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`, `-j 4`); `git merge main` before the final checks
- [ ] committed as `S-SUB-01: <summary>`
- [ ] report delivered in the agent's report format (what you left and why, every guess)

## Notes from the orchestrator
- Read CLAUDE.md (safety rules), DECISIONS (D-15, D-39, D-49..D-54), the `docs/design/*.md` of the area you touch first.
- Up to seven workers run at the same time: in shared files (`lib.rs`, `world.rs`, `command.rs`, `outbox.rs`, `timer.rs`, `dispatch.rs`, `persist.rs`, `world_data.rs`) only add your lines; after `git merge main` watch for doubled `mod`/`use` lines.
- DB tests use `l2_db::GAME_SCHEMA`; add a schema copy to `crates/l2-db/sql/game/` for every new table.
