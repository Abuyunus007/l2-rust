# Runbook: combat, NPC AI and progression on the Rust GS (M4)

Setup as in `docs/runbooks/m3-world-test.md` (Rust GS "Kain" next to the Java GS "Sieghardt" on the
same DB). M4 adds the stat engine, inventory and equipping, regeneration, auto-attack, NPC AI,
death and respawn, exp/SP and levelling, vitality and Nevit, drops and items on the ground, and the
combat admin commands. Both servers read the same `Rates.ini`; ground items follow `SaveDroppedItem`
in `General.ini` like Java (shipped: off, items on the ground live in memory only).

## 1. What starts with the world (new in M4)

| Log line | Meaning |
|---|---|
| `ArmorSetData: Loaded 204 armor sets.`, `HennaData: Loaded 180 henna data.`, `EnchantItemHPBonusData`, `HitConditionBonusData` | stat data |
| `SkillData: Loaded 65965 Skill templates from XML files.` | all skills, all levels (effects run in M5) |
| `NPC spawns: ~42,400 NPCs in the world` | as in M3; now with AI where a player is near |

No NPC thinks until a player is near it (Java's active regions); with nobody online the world tick
costs nothing extra.

## 2. Checkpoints

### CP-M4a: stats and gear
1. Log in a geared character on Kain and on Sieghardt: P.Atk, P.Def, M.Atk, M.Def, accuracy,
   evasion, crit, speeds, max HP/MP/CP are the same (enchants, armor sets, dyes, passive skills).
2. Take gear off and put it back on, one piece at a time, incl. a full armor set: the numbers change
   exactly as on Sieghardt; the set bonus appears and goes with the last/first piece.
3. Weight: pick up / drop heavy items; the weight penalty icon appears at the same load as on Java.

### CP-M4b: hunting
1. Attack a monster: it fights back; aggressive monsters attack you when you come close; monsters
   chase you and give up and walk home when you run far enough (`MaxDriftRange`).
2. Kill it: exp and SP as on Sieghardt at the same level (vitality included: the vitality bar
   empties a little per kill); drops land around the corpse; others see them; for 15 s only you can
   loot; adena goes into your adena.
3. Level up: the level-up effect and message, HP/MP/CP full, new automatic skills.
4. Herbs drop and disappear on pickup **without** their effect (their skills come with M5).

### CP-M4c: death and regeneration
1. Let a monster kill you: the death dialog; "To village" brings you to the nearest town with 65 %
   HP (`RespawnRestoreHP`); you lose exp like on Java (none below level 10 with the Lucky skill).
2. The monster's corpse disappears after its corpse time and the monster respawns after its delay.
3. HP/MP/CP regenerate over time, faster sitting (`/sit`) as on Java.

### CP-M4d: milestone
A hunting session on Kain (levels, loot, a death), log out, log in the same character on Sieghardt:
level, exp, SP, inventory (the loot), position and vitality are what Kain left.

## 3. Admin commands (GM character, see the M3 runbook for the access level)

| Command | What it does |
|---|---|
| `//kill` / `//kill <radius>` | real death of the target (or everything around), corpse and respawn |
| `//res` / `//res_monster` | revive the target |
| `//invul`, `//setinvul` | invulnerable yourself / your target |
| `//spawn <id or name> [count] [respawn seconds]` | spawn with respawn (`//spawn_once` without) |
| `//create_item <id> [count]`, `//create_coin`, `//give_item_target` | items (confirmation dialogs are not ported yet: `//give_item_target` stops at the question) |
| `//set_level <level>`, `//add_level <n>` | level |
| `//add_exp_sp <exp> <sp>`, `//remove_exp_sp <exp> <sp>` | exp and SP |

`GMStartupInvulnerable` (General.ini) makes a GM with `admin_invul` invulnerable at login, as Java.

## 4. Known limits in M4
- No skills that are cast (M5): herbs, potions, scrolls and soulshots do nothing yet; spoil/sweep M5.
- No parties: every kill rewards the biggest damage dealer alone (M6).
- Guards do not run the attackable AI yet; raid-boss minion respawn tables are not ported.
- Daily resets (Nevit's daily reset, recommendations) are not ported.
