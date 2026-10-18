# M4 draft: combat, NPC AI, progression (for the owner's review — no worker starts before sign-off)

**Done when (PLAN.md):** you hunt mobs, level up, loot, die and respawn. Formulas are checked
against Java vectors (D-41).

What M3 leaves for M4 (from the M3 reports): NPC AI and respawn, item/skill stat effects in
UserInfo/NpcInfo (P.Atk without gear today), attack, death, exp, drops, items on the ground,
inventory changes, HP/MP regen.

## Proposed cards

| Card | Title | Model | Depends on | Notes |
|---|---|---|---|---|
| M4-DATA-10 | Item stat functions and skill references from the item XML | sonnet | M2-DATA-04 | `<stats>` (FuncTemplate: add/mul/set/enchant...), `<skills>`, `<conditions>` kept raw today |
| M4-DATA-11 | Skill data, levels and passive stat functions (SkillData, minimum for passives) | sonnet | M4-DATA-10 | full skill effects are M5; here: the stat funcs of passive skills (masteries) and the skill templates NPCs carry |
| M4-DATA-12 | Armor sets, enchant HP bonus, hit condition bonus | haiku | M4-DATA-10 | small loaders the stat engine reads |
| M4-JV-01 | Java vectors for the stat engine and Formulas | sonnet | — | harness programs: base stat bonus, P.Atk/P.Def/M.Atk... from template + items, hit/miss/crit/damage, regen; both the owner's datapack and dist |
| M4-STAT-01 | Stat engine (Calculator, Stat, Func*) for players and NPCs | **opus** | DATA-10, DATA-11, DATA-12, JV-01 | core: every number in UserInfo/NpcInfo/CharInfo comes from here; fixes the "no gear in P.Atk" gap and the NPC 31033 attack-speed diff |
| M4-GPROTO-07 | Combat and item packets | haiku | — | Attack, AutoAttackStart/Stop, Die, Revive, RequestRestartPoint, InventoryUpdate, DropItem, SpawnItem, GetItem, SocialAction, ChangeMoveType, ChangeWaitType, UseItem/UnEquip/DropItem/DestroyItem client packets |
| M4-INV-01 | Inventory and equipping | sonnet | STAT-01, GPROTO-07 | UseItem / RequestUnEquipItem / paperdoll rules / weight / InventoryUpdate / ArmorSet effects / persistence of the changes |
| M4-REGEN-01 | HP/MP/CP regeneration | haiku | STAT-01 | Java formulas + StatusUpdate; tick-based |
| M4-COMBAT-01 | Auto-attack (players and NPCs) | **opus** | STAT-01, GPROTO-07 | AttackRequest, attack speed timing (timer wheel), hit/miss/crit/shield/damage formulas, AutoAttack stance, reduceCurrentHp |
| M4-AI-01 | NPC AI: attackable core | sonnet | COMBAT-01 | CreatureAI/AttackableAI: aggro list, attack intention, return home, give up, MoveToPawn chase; runs in the world tick (no AI threads) |
| M4-AI-02 | NPC AI: idle behaviour | haiku | AI-01 | random walk, random animation, social actions, active regions switching AI on/off (`switchAI`) |
| M4-DEATH-01 | Death, respawn, corpses | sonnet | COMBAT-01 | doDie, Die packet, RequestRestartPoint (to town / map region), NPC respawn timers (Spawn.decreaseCount / respawn delay), corpse decay |
| M4-EXP-01 | Exp/SP rewards and levelling | sonnet | DEATH-01 | Attackable reward calc (damage share), rates.ini, level up (stats, SocialAction, system messages), auto-learn skills on level up |
| M4-DROP-01 | Drops, spoils, items on the ground, pickup | sonnet | DEATH-01, INV-01 | drop/spoil calc with rates and the grouped drop lists, DropItem/SpawnItem, GetItem/pickup, auto-destroy, persistence of ground items (`itemsonground`, D-39 aware) |
| M4-ADMIN-02 | Admin commands for combat testing | haiku | DEATH-01 | //res, //setlevel, //create_item, //invul, //spawn with respawn |
| M4-E2E-01 | End to end: hunt, loot, level, die, respawn | sonnet | EXP-01, DROP-01 | two fake clients, CI MariaDB |
| M4-RUN-01 | Runbook and milestone checks | opus | E2E-01 | |

17 cards: 4 opus (STAT-01, COMBAT-01, RUN-01 + review), 9 sonnet, 4 haiku.

## Checkpoints

| Checkpoint | You test |
|---|---|
| CP-M4a | Equip and unequip gear on Kain: P.Atk/P.Def/M.Def change like on Sieghardt for the same character (side-by-side numbers). |
| CP-M4b | Hunt a monster: it fights back, dies, gives exp/SP, drops items you can pick up. |
| CP-M4c | Die, go to town, monsters respawn; HP/MP regenerate. |
| CP-M4d | Milestone: a hunting session on Kain; the character then plays on Sieghardt with the loot and levels. |

## Open questions for the owner

1. **Rates:** the Rust GS reads `Rates.ini` from `run/game/config` (copied from the Java server). Keep both servers on the same rates? (Recommended: yes, so the comparison is fair.)
2. **Java-side shared DB writes:** items dropped on Kain's ground are stored in `itemsonground` like Java. Fine, or keep ground items in memory only for now?
3. **Order inside M4:** stats first (CP-M4a) as planned, or combat first with base stats and the stat engine after?
