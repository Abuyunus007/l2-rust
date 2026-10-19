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

## Not in M4
- Active skills and their effects (cast, buffs, debuffs, heals): M5. Only passive skill stat
  funcs and the stat parts of item skills are applied in M4.
- Summons/pets, PvP flags/karma beyond what death needs, olympiad, sieges.
