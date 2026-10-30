# S-API-05: Engine gaps the script batches found

| Field | Value |
|---|---|
| Milestone | Scripts |
| Model | opus |
| Review | full |
| Reference | `docs/design/scripts.md`, `crates/l2-world/src/script/*` |
| Depends on | Q batches |
| Size | L |

## Goal
Close the engine and data gaps the quest, AI and instance batches reported (grep `FOLLOW-UP(engine)` in `crates/l2-scripts` first and fix every one you can):
- **Bug:** `Qs::is_cond(ctx, 0)` is false for a fresh state; Java's `isCond(0)` is true (`script/qs.rs`). Fix it and the call sites that worked around it (Q00710, Q00713).
- **Bug check:** a click on Chamberlain 35274 in the script test world gives ActionFailed / MoveToPawn without a page (S-Q-18). Find out whether the real server has the same problem and fix it.
- `is_inside_radius_2d` must be strict `<` like Java `isInsideRadius2D`; switch the local strict helpers of Q00423/Q00457 to it.
- **Soul crystals (priority):** a `LevelUpCrystalData.xml` loader in l2-data and the full Q00350 `EnhanceYourWeapon` leveling (kill and skill-see listeners for the table's NPCs, absorb, `levelCrystal`, `exchangeCrystal`, the party leveling rules).
- NPC traps: the Trap creature (with S-HAND-04 if it lands first: reuse it), `addTrap`, `add_trap_action_id`, firing `ON_TRAP_ACTION`; wire Crystal Caverns' Dolph door trap and the Gracia corridor traps.
- `TamedBeast`: the creature Java's BeastFarm/FeedableBeasts create at the last growth step (follow, buffs, the food timer), wired into those two scripts.
- NPC display effect broadcast (`setDisplayEffect` / `ExChangeNpcState`) for scripts (Q10294 books, Sel Mahum), `setWalking`, `getPointInRange`, a pub `isRandomWalkingEnabled`, `isInSurroundingRegion`, `getRightHandItem`, `SpawnTable.getSpawns(id)` / `getLastSpawn`, NPC zone enter/exit, an EffectZone enable switch where scripts still lack one.
- Small pub APIs: raw `RadarControl(show, type, x, y, z)` (Q00307/308/309), NpcSay to one player (Q00403), `broadcast_clan_info_update` (Q00508-510), `is_my_lord` for castle/fort NPCs (Q00512), timers that do not merge by name (Q00311), the summon-spawn hook (`addSummonSpawnId`, DivineBeast, SinEater).
- Switch the Q00224/Q00225/Q00226 local stubs to `ctx.killing_blow_weapon` / `summoned_npc_count` / `add_spawn_summoned`; let `clan/academy.rs`, `summon/baby_pet.rs` and `stats/conditions.rs` (`Category`) use the loaded CategoryData; add the transform tree to `is_skill_allowed`.
- `ScriptTest` helpers the batches missed: a clan with a castle or fortress, a summon, a skill table, so the untested clan/castle/summon branches get tests.
- `ExShowSeedMapInfo` with the SoD/SoI states, the `.hellbound` voiced command, `//gracia_seeds` admin words (AdminGraciaSeeds).
- Recheck the SpecialCamera argument order in the Antharas and Pailaka Injured Dragon ports (Java's 11-argument constructor swaps range and duration; Crystal Caverns has it right).

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\` (the classes named above), `C:\l2-rust\l2jmobius\dist\game\data\scripts\quests\Q00350_EnhanceYourWeapon\`, `C:\l2-rust\l2jmobius\dist\game\data\LevelUpCrystalData.xml`

## Files
- `crates/l2-world/src/*`, `crates/l2-data/src/*`, `crates/l2-scripts/src/*` (the call sites), `crates/game-server/src/*` where needed; tests

## Requirements
1. Java behaviour and data exactly (D-39, D-49..D-54); no unwrap/expect on external data; no DB call in a tick.
2. Only add to the script API; keep existing signatures.

## Tests (must exist and pass)
- A test per fix; Q00350 from a kill to a leveled crystal.

## Done when
- [ ] fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`, `-j 4`); `git merge main` before the final checks
- [ ] committed as `S-API-05: <summary>`
- [ ] report delivered (every item done or left, and why)

## Notes from the orchestrator
- Disk space is tight: set CARGO_PROFILE_DEV_DEBUG=0 and CARGO_PROFILE_TEST_DEBUG=0. Other workers run at the same time: only add your lines in shared files.
