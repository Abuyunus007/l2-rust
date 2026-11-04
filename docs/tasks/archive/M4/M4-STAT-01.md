# M4-STAT-01: Stat engine for players and NPCs

| Field | Value |
|---|---|
| Milestone | M4 |
| Model | opus |
| Review | full |
| Reference | yes (first instance) |
| Depends on | M4-DATA-10, M4-DATA-11, M4-DATA-12, M4-JV-01 |
| Size | XL |

## Goal
Java's stat system in the world: per-creature calculators, the standard formula functions, item
and passive-skill functions, condition evaluation, and the `CreatureStat`/`PlayerStat`/`NpcStat`
getters. Every number in `UserInfo`, `CharInfo`, `NpcInfo`, `StatusUpdate` comes from it.
Replaces `DerivedStats` / `StaticData::derive_stats` and the ad-hoc passive scan in
`game-server/src/world_data.rs`.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\mechanics\stats\{Calculator,Stat,BaseStat,Formulas (getStd*Calculators, addFuncsToNewCharacter)}.java`, `functions\*`, `functions\formulas\*`
- `...\mechanics\conditions\*` (evaluation of the kinds DATA-10 parses)
- `...\entity\actor\stat\{CreatureStat,PlayableStat,PlayerStat,NpcStat}.java`
- `...\entity\actor\{Creature (addStatFunc(s), removeStatsOwner, getLevelMod, _calculators), Player (getLevelMod, hennas, getChestArmorInstance, addSkill/removeSkill), Npc}.java`
- `...\entity\item\{ItemTemplate (getStatFuncs), instance\Item (getStatFuncs)}.java`, `...\entity\item\container\Inventory.java` (`StatsListener`, `ArmorSetListener`, `ItemSkillsListener`: what equip/unequip adds/removes)
- `...\mechanics\skill\Skill.java` (`applyEffects` for passives, `getStatFuncs`), `...\mechanics\effects\AbstractEffect.java` (`getStatFuncs`)

## Files
- `crates/l2-world/src/stats/{mod.rs,calculators.rs,std_funcs.rs,conditions.rs,creature_stat.rs,player_stat.rs,npc_stat.rs}` (new)
- `crates/l2-world/src/{world.rs,player.rs,objects.rs,player_view.rs,npc.rs,enter.rs,data.rs,lib.rs}` (wire-in)
- `crates/game-server/src/world_data.rs`, `crates/game-server/src/lib.rs` (hand the data to the world, drop `derive_stats`)
- `crates/l2-world/tests/stats_vectors.rs` (new)

## Requirements
1. Design per `docs/design/m4-combat.md` ("Stat engine rules"). `Calculators` live in `CreatureCore`; NPCs share the standard NPC set until they get their own funcs (copy on write); functions keep Java's `order` and insertion rule (`addFunc` inserts after equal orders).
2. Equipped items add their funcs with the item as owner (and enchant funcs), armor-set and item-skill effects as Java's inventory listeners do at enter world; passive skills apply their effect funcs at enter world (and NPC template skills at spawn). Inventory changes during play are M4-INV-01, but expose `on_equip`/`on_unequip` hooks it will call.
3. Condition evaluation for every kind DATA-10 parses that can be decided with world state today; anything else evaluates to `false` and warns once per kind.
4. Stat getters for players and NPCs with Java's clamps (`MaxPAtk` etc. from config), champion/raid multipliers, class balance multipliers, `getLevelMod`, transformation hooks left as `None`.
5. Vectors: every case of M4-JV-01's `stats_*.json` matches exactly (int casts and rounding like Java).
6. `UserInfo`/`CharInfo`/`NpcInfo`/`StatusUpdate` read the engine; M2/M3 tests keep passing (update expected numbers only where Java vectors prove the new value).

## Out of scope
- Equip/unequip packets and rules (INV-01), regen (REGEN-01), combat formulas (COMBAT-01), effects other than stat funcs (M5).

## Tests (must exist and pass)
- Vector tests (skip if `java-vectors/out` files are missing), unit tests per std func and calculator ordering, NPC copy-on-write.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`)
- [ ] committed as `M4-STAT-01: <summary>`
- [ ] report delivered

## Notes from the orchestrator
- Read `docs/design/m4-combat.md`, `docs/design/world-loop.md`, `docs/design/m3-world.md` first.
- The world is single-owner: no locks, no `Arc<Mutex>`; read-only views for `calc`.
