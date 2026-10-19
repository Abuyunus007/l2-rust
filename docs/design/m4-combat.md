# Design: stats, combat, NPC AI, death and rewards (M4)

Builds on `docs/design/world-loop.md` and `docs/design/m3-world.md`. The Java source stays the
spec for behaviour; this note fixes *where* things live in Rust and the shared data model the M4
cards build on. Owner decisions for M4 (2026-10-19): both servers read the same `Rates.ini`;
ground items follow `SaveDroppedItem` & co. of `General.ini` exactly like Java (default `False`:
memory only); the stat engine comes first (CP-M4a), combat after it.

## The Java model being ported (H5 / L2JMobius CT 2.6)
- `mechanics/stats/Stat.java`: ~150 stats with an XML name (`pAtk`, `maxHp`, ...).
- `Calculator`: per creature and stat, an ordered list of `AbstractFunction`s
  (`FuncSet/Add/Sub/Mul/Div/Share/Enchant/EnchantHp` from templates, plus the "standard" formula
  functions `FuncPAtkMod`, `FuncMaxHpMul`, `FuncHenna`, `FuncArmorSet`, ...). `calc` folds the
  value through the functions in `order`; every function has an `owner` (item, skill, effect) and
  an optional apply condition.
- `FuncTemplate` (+ attach condition) lives on item templates and on skill effect templates; when
  an item is equipped or a passive skill/effect starts, the templates make functions owned by it,
  and they are removed by owner.
- `Condition` trees (`mechanics/conditions/*`, parsed by `util/DocumentBase.parseCondition`):
  `and/or/not`, `player ...`, `target ...`, `using ...`, `game ...`.
- `CreatureStat` / `PlayerStat` / `NpcStat` read base values from the template and call
  `calcStat(stat, base, target, skill)`; `Formulas` holds hit/crit/damage/regen.

## Where it lives in Rust
| Piece | Crate / module | Notes |
|---|---|---|
| `Stat` enum, `FuncTemplate`, `FuncKind`, `Condition` tree (data only), item funcs/conditions/skills | `l2-data` `stats/`, `item` | parse only, no evaluation |
| Skill templates (per id+level), effects as templates (name, funcs, params raw), skill conditions | `l2-data` `skill/` | M4 evaluates only the stat funcs of passive effects; effect handlers are M5 |
| Armor sets, enchant HP bonus, hit condition bonus | `l2-data` | small loaders |
| Stat engine: per-creature calculators, standard formula funcs, condition evaluation, `calc_stat` | `l2-world` `stats/` | read-only on `&World` |
| `Formulas` (hit, crit, shield, damage, attack speed, regen, ...) | `l2-world` `formulas.rs` | pure functions over numbers + the stat engine; RNG = world RNG |
| Combat, AI, death, rewards, drops, ground items | `l2-world` | world thread, timer wheel |
| Persistence of inventory changes, ground items (when `SaveDroppedItem`) | `game-server` `persist.rs` via the world's outbox | never awaited in a tick |

## Stat engine rules (M4-STAT-01)
- One `Calculators` value per creature (in `CreatureCore`), Java's slot layout: an array indexed
  by `Stat as usize` of ordered function lists. NPCs without skills share the standard NPC set
  (`Formulas.getStdNPCCalculators`) through an `Arc` and copy on first write, like Java.
- A function = `{ stat, order, kind, value, owner: FuncOwner, cond: Option<Arc<Condition>> }`.
  `FuncOwner` is an enum of ids (`Item(ObjectId)`, `Skill { id, level }`, `ArmorSet(id)`,
  `Std`, ...). `remove_owner` returns the touched stats like Java.
- The standard formula functions are enum variants (`StdFunc::PAtkMod`, ...) whose `calc` reads
  the creature through a read-only view (`StatView`) built from `&World`; calling another stat
  (`getSTR()` inside `FuncPAtkMod`) is a recursive `calc_stat`, exactly as Java recomputes it.
- Conditions are evaluated against the same read-only view (`CondEnv { world, actor, target,
  skill }`). A condition kind not ported yet evaluates to `false` and is logged once (never
  silently true).
- `DerivedStats` (M2) is replaced by calls into the engine; the trait method `derive_stats` goes
  away once `UserInfo`, `CharInfo`, `NpcInfo`, `StatusUpdate` read the engine.
- Every number must match Java vectors (M4-JV-01) and the side-by-side `UserInfo` of the same
  character on the Java GS (CP-M4a).

## Combat and AI rules
- Attacks are timer-wheel events: `Creature.doAttack` schedules the hit (`HitTask`) and the
  attack end at Java's times (attack speed formulas); nothing sleeps.
- NPC AI runs in the tick for creatures in active regions (`WorldRegion.setActive` already
  ported), Java's think interval (`AbstractAI` / `AttackableAI` 1 s `onEvtThink` + intention
  events). One `ai` state per NPC (enum of Java intentions), no threads.
- Damage, death, exp and drops are world events; the outbox carries packets and DB writes.

## Death, corpses and respawn (M4-DEATH-01)
- `World::do_die` (combat/damage.rs) hands over to `death.rs`, which follows Java's class chain
  (a player: `Playable.doDie` then `Player.doDie`; an NPC: `Creature.doDie`, `Npc.doDie`, and for
  an attackable `Attackable.doDie`). The `Die` packet is sent by `notify_action_death`
  (ai/mod.rs), as in Java, so an NPC without an AI dies silently. Rewards, drops and the exp
  penalty are empty hooks in `death.rs` marked `// M4-EXP-01:` / `// M4-DROP-01:`.
- `respawn.rs` is `Spawn.decreaseCount`, `DecayTaskManager`, `RespawnTaskManager` and
  `SpawnGroupTaskManager` on the timer wheel (`TimerKind::{Decay, Respawn, GroupSpawn}`). Java's
  managers poll once a second; here an entry fires on the tick its due time falls in.
- A dead NPC's entry leaves `World::npcs` when the corpse decays; its object id stays reserved and
  the respawn re-creates the NPC with the same id (Java re-initialises the same `Npc` object).
  Per-spawn counters (`_currentCount`, `_scheduledCount`, `_doRespawn`, `_spawnedNpcs`) and the
  spawn group census live in `DeathWorld`.
- `RequestRestartPoint` sets "pending revive" and calls `teleToLocation`, which revives first, as
  in Java. Clan hall, castle, fortress and siege buttons need clans and sieges (M5).

## Exp, SP and levels (M4-EXP-01)
- `rewards/exp.rs` follows Java's stat class chain: `Attackable.calculateRewards` (the exp half,
  called from `death.rs` after the drops) -> `PlayerStat.addExpAndSp` -> `addExp` ->
  `PlayableStat.addExp` -> `addLevel` (`rewardSkills`, the level up packets). The tables
  (`experience.xml`, `experienceLoss.xml`, `karmaLoss.xml`, the class skill trees) are `Arc`s
  installed with `World::set_exp_data`; the `.ini` values are `WorldSettings::exp`.
- A player's `exp`, `sp`, `level` and `expBeforeDeath` live in `PlayerData` / `PlayerProfile`; the
  `characters` row is written by the normal store. Skills a level teaches or takes are also sent
  at once as `PersistCommand::SkillChanges` (Java's `storeSkill` / `removeSkill`).
- Not yet (own cards): parties, pets/summons, the over-hit bonus, `AutoLearnSkills = True`.

## Vitality and Nevit (M4-VIT-01)
- `rewards/vitality.rs`: `PlayerStat`'s vitality (float points, level, multiplier), the points a
  kill uses (`Attackable.getVitalityPoints`, bosses override it), the 60 s `VitalityTask`
  (`TimerKind::Vitality`) and the recovery at login. The state is in `ExpWorld::vitality`; the
  whole number is mirrored into `PlayerProfile::vitality_points`, which `UserInfo` shows and the
  store writes to `characters.vitality_points` (Java stores `(int)` of the float).
- `rewards/nevit.rs`: `Nevit.java` (points, 30 s advent task, the 180 s blessing) and the
  hourglass of `Player.java` (`character_reco_bonus.time_left`). Timers: `NevitAdvent`,
  `NevitEffectEnd`, `NevitHourglassEnd`. The peace zone pauses them (`zones.rs`).
- Stored values (D-39): `hunting_points`, `hunting_time`, `nevit_b` are `character_variables`
  rows. The world records the names it set or removed in `PlayerProfile::variables_changed` /
  `variables_removed`; with every store the persistence task upserts and deletes exactly those
  (Java's `saveNowSync` statements). `character_reco_bonus` is written with every store of a
  Nevit-enabled player (`reco_store`), the hourglass time being what is left of the timer.
- Left out: the glow (`AbnormalVisualEffect.NAVIT_ADVENT`, effects are M5), `RecoGiveTask` and the
  recommendation votes, the daily reset (`DailyResetManager.resetNevit`), parties.

## Not in M4
- Active skills and their effects (cast, buffs, debuffs, heals): M5. Only passive skill stat
  funcs and the stat parts of item skills are applied in M4.
- Summons/pets, PvP flags/karma beyond what death needs, olympiad, sieges.
