# Runbook: quests, AIs, bosses, instances and events on the Rust GS (script milestone)

Setup as in `docs/runbooks/m4-combat-test.md` (Rust GS "Kain" next to the Java GS "Sieghardt" on
the same DB). Several steps need a GM character, a fresh character, a level 76+ character and a
party.

**Side by side (D-54):** quest states (`character_quests`) are per-player rows: Kain writes them as
Java does, so a quest started on one server continues on the other after a relog (not while the
character is online on both). The `global_variables` some scripts keep (boss and spawn timers,
event states, Q00604/Q00610 respawns, Castle Pailaka re-entry) are written only with
`CycleDatabaseWrites = True` in `config/SideBySide.ini`; with the switch off (the default) Kain
keeps them in memory and re-reads them where Java reads them. Grand boss AIs run on Kain only while
Sieghardt's boss row says alive. The birthday mail is sent only with `DailyResetDatabaseWrites`
on, so a character never gets it twice. Long-time events delete their items from offline
characters only with `CycleDatabaseWrites` on.

## 1. Checkpoints

### CP-Sa: a new character
1. Create a character of each race: the tutorial (Newbie Helper, the question marks, the first
   quest Q00255 Tutorial), the newbie guide buffs.
2. The first quests of the starting village: a talk quest, a kill quest with drops, the reward
   (adena, exp, items). The quest list (Alt+U) shows the right step.
3. Abort a quest from the quest list: the items are taken, the row is gone.
4. A repeatable quest (daily or `REPEATABLE`): finish it twice; a daily one refuses until 6:30.

### CP-Sb: class change and subclasses
1. 1st and 2nd class change at a village master with the path quest items (e.g. Q00401..Q00418
   proofs), the class master if `ClassMaster.xml` allows it.
2. Subclass: add at a village master (Q00234/Q00235 Mimir's Elixir for the noble path), switch
   (skills, hennas, effects follow), cancel. Subclass certification at the certification NPC.
3. Noble: Q00247 Possessor of a Precious Soul 4 makes the character noble.
4. Transfer skills: a cardinal/Eva's saint/shillien saint learns at the healer trainer, then
   cleanses (fee once, items back).
5. Soul crystals: Q00350 at Wesley; absorb a raid or a monster with the crystal skill, the crystal
   levels (with the party rules).

### CP-Sc: instances, NPC scripts, bosses
1. A Kamaloka run (party, enter, kill the boss, leave, the re-entry time); a Pailaka quest
   (Q00128, Q00129, Q00144): the instance, the item buffs, the reward.
2. Area NPC scripts: a buffer (NpcBuffers, the Seven Signs CabaleBuffer), a teleporter (Oracle,
   Strongholds, Hunting Grounds), the Monument of Heroes, the Manor Manager, Fame Manager.
3. Beast Farm: feed a beast, tame it (the tamed beast follows, buffs and heals the owner).
4. A raid boss with its AI (minions, skills, the leash); a grand boss (Queen Ant, Core, Orfen,
   Baium) only when its row says alive. `//grandboss` shows the states.
5. Traps and decoys: a decoy skill (the copy of the owner draws aggro), a trap skill, detect and
   remove a trap; the Crystal Caverns door trap.
6. The community board drop search (`_bbs_search_item`): find an item, the NPC list, the trace.

### CP-Sd: milestone
1. A character with quests in progress on Sieghardt logs into Kain and continues them: the same
   step, items and memo; finishing on Kain shows as completed on Sieghardt after a relog.
2. The shift-click windows (NPC info, drops) and the community board work.
3. A seasonal event that is active in the dates of `events/*.xml` shows its NPCs and drops.

## 2. Admin commands (new in the script milestone)

| Command | What it does |
|---|---|
| `//show_quests`, `//charquestmenu`, `//setcharquest` | a character's quest states: list, edit a variable, set state, delete |
| `//quest_info <name>` | a script's events, timers and NPC ids |
| `//quest_reload`, `//script_unload`, `//script_load`, `//reload quest [name]` | unload and reinstall a compiled-in script (the Rust scripts are compiled in, D-16: no file is re-read) |
| `//hellbound`, `.hellbound`, `//gracia_seeds`, `kill_tiat`, `set_sodstate`, `set_soistage` | Hellbound trust and level, Gracia seed states |

## 3. Known limits
- `//script_load`, `//quest_reload` cannot load changed code: a code change needs a rebuild and
  a restart (D-16).
- Not modelled yet: the `EffectPoint` creature of the signet `SummonNpc` skills; decoy messages
  forwarded to the owner; trap damage messages and owner-based trap hostility; traps are not
  removed by an instance teardown.
- NPCs get no zone effects (damage, peace); only the zones scripts listen to track NPC enter/exit.
- GM skill trees are loaded but `is_skill_allowed` still ignores them.
- The Olympiad quests (Q00551..Q00553) get the match result by character id only; the manager's
  noble page has no quest link (as in Java), use the quest bypass.
- Customs (`scripts/custom/`) come with M9.
