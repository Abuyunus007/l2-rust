# Script milestone draft: quests, AI scripts, instances, class change and subclasses (for the owner's review — no worker starts before sign-off)

**Done when:** the script engine runs Java's `Quest`/`AbstractScript` model (listeners, quest
states, timers, quest pages) and the scripts of `dist/game/data/scripts` work like on Sieghardt:
the 505 quests, the area/other AIs, the grand and raid boss AIs, the instance scripts, the class
change NPCs, subclasses and certifications, the seasonal events, the shift-click GM windows and the
community board. Everything stays readable for Sieghardt on the same DB (D-39).

Java size, for scale (`dist/game/data/scripts`):

| Part | Files | Lines | Notes |
|---|---|---|---|
| `quests/` | 513 (505 quests) | 158,800 | very templated: talk, kill, collect, reward |
| `ai/areas`, `ai/others` | 224 | 49,500 | area NPCs, monster AIs, teleporters, buffers |
| `ai/bosses` | 15 | 8,600 | Antharas, Valakas, Baium, Beleth, Core, Orfen, Queen Ant, Frintezza, Sailren, ... |
| `instances/` | 31 | 13,400 | Kamaloka, Pailaka, Crystal Caverns, Freya, SSQ instances, ... |
| `village_master/` | 18 | 4,100 | class change 1 and 2 (ClanMaster, AllianceMaster are built in since M6) |
| `handlers/` (left) | ~25 | ~4,000 | shift-click GM windows, community board, a few bypass/voiced handlers |
| `events/` | 10 | 2,800 | seasonal events on `LongTimeEvent` |
| `custom/` | 19 | 7,500 | customs: **M9**, not here |
| Engine (`mechanics/script/Quest.java`, `QuestState.java`, listeners) | | 6,700 | core |

What earlier milestones left for the scripts: the ad hoc "first talk" chains (ClanMaster,
ManorManager, SymbolMaker, the wedding priest, the hero monument, the conquerable hall scripts,
Kratei's Cube, the boat routes), `FOLLOW-UP(D-05)` markers (boss events of `//grandboss_*`, the
Territory War quests, `MakeMeNoble`, quest 635 and `OracleTeleport`, the block checker's
`FlipBlock`, event drops), the missing client packets `RequestQuestList`, `RequestQuestAbort`,
`RequestTutorial*`, the class change and the subclass switch (`Player.setActiveClass`).

## Proposed cards

| Card | Title | Model | Depends on | Notes |
|---|---|---|---|---|
| S-API-01 | Script engine | **opus** | — | `Quest`/`AbstractScript` as a Rust trait plus a compiled-in registry (D-16): listeners by NPC/item/zone/skill/event, `onTalk`/`onFirstTalk`/`onKill`/`onAttack`/`onSpawn`/`onSkillSee`/`onAggroRangeEnter`/`onEnterZone`/`onAdvEvent`/`onItemTalk`/... with Java's dispatch order; `QuestState` with `character_quests` (read at CharacterSelect, written through the outbox, D-15) and `global_variables`; quest timers on the wheel; the helpers (`giveItems`, `takeItems`, `rewardItems` with rates, `getQuestItemsCount`, `addSpawn`, `startQuestTimer`, `getRandomPartyMember`, `playSound`, `showHtmlFile`, NpcSay); the quest pages (`Quest` bypass, the quest list chooser, `noquest`); `QuestList`, `RequestQuestList`, `RequestQuestAbort`, the tutorial packets |
| S-API-02 | Move the built-ins onto the engine | sonnet | API-01 | ClanMaster, AllianceMaster, ManorManager, SymbolMaker, wedding priest, MonumentOfHeroes, conquerable halls, Kratei's Cube, Block Checker event, boat scripts: same behaviour, now as scripts; removes the first-talk chains |
| S-PILOT-01 | Ten reference quests | **opus** | API-01 | hand-picked patterns for the batches: a kill-count quest, a delivery chain, a collect-with-drop-chance, a party quest, a repeatable, a one-time with exp/adena/items, a class-specific quest, a quest with a timer, a quest that spawns a monster, a `AbstractSagaQuest` saga; plus the porting guide `docs/design/scripts.md` the batches follow |
| S-VM-01 | Class change and subclasses | **opus** | API-01 | the 16 class change scripts and `FirstClassTransferTalk`, core `VillageMaster*` (add, change, cancel a subclass), `Player.addSubClass`/`modifySubClass`/`setActiveClass` with `character_subclasses` and every per-class reload (skills, hennas, effects, shortcuts, recipe book, stats), certification skills, the third class (`ClassMaster` options as Java), the `MakeMeNoble` and `//setclass` markers |
| S-Q-01 .. S-Q-20 | Quests in batches of ~25 by id | haiku (sonnet review spot) | PILOT-01 | ~8,000 Java lines each; follow `docs/design/scripts.md` and the pilot quests; each batch gets a test per quest (start, a step, the reward) |
| S-AI-01 .. S-AI-08 | Area and other AIs in batches | sonnet | API-01 | `ai/areas/*` by region, `ai/others/*` (teleporters, buffers, NPC behaviour, monster specials) |
| S-BOSS-01 .. S-BOSS-04 | Grand and raid boss AIs | **opus** (01), sonnet | API-01 | Antharas, Valakas, Baium, Beleth (01); Core, Orfen, Queen Ant, Zaken (02); Frintezza, Sailren, Seer Ugoros, Dr. Chaos, Anais, Queen Shyeed (03/04); the `//grandboss_*` events; D-53: a boss AI runs on Kain only when the boss state allows it and writes no state with `CycleDatabaseWrites` off |
| S-INST-01 .. S-INST-04 | Instance scripts | sonnet | API-01 | Kamaloka/Rim Kamaloka, Pailakas, Crystal Caverns, Ice Queen's Castle, Nornil's Garden, Dark Cloud Mansion, the SSQ instances, Jinia hideouts, Castle Dungeon, Chambers of Delusion, Mithril Mine, Cavern of the Pirate Captain |
| S-TW-01 | Territory War quests | sonnet | API-01 | `TerritoryWarScripts` and the TW point markers of M7-TW-01 |
| S-HAND-01 | Shift-click windows and community board | sonnet | API-01 | `handlers/actions/shiftclick/*` (npcinfo, door/player/summon/item/static object), `handlers/bypass/communityboard/*` (home, region, favorites, clan, mail, memo, friends, drop search), the voiced and bypass handlers still missing |
| S-EVENT-01 | Seasonal events | sonnet | API-01 | `LongTimeEvent` and the 10 events (event drops through `add_event_drops`) |
| S-ADMIN-08 | Admin words of the script systems | haiku | API-01, VM-01 | `//quest_*`, `//show_quests`, `//reload quest` answer (restart, D-16), `//setclass` follow-ups |
| S-E2E-01 | End to end: a quest chain, a class change, a subclass switch, a boss fight start, an instance run | sonnet | Q batches, VM-01, BOSS-01, INST-01 | |
| S-RUN-01 | Runbook and milestone checks | opus | E2E-01 | |

About 46 cards: 5 opus (API-01, PILOT-01, VM-01, BOSS-01, RUN-01), ~20 haiku quest batches,
~20 sonnet.

## Checkpoints

| Checkpoint | You test |
|---|---|
| CP-Sa | A new character: the tutorial, the first quests in the starting village, a kill quest and its reward; abort a quest. |
| CP-Sb | Class change 1 and 2 at the village master, a subclass: add, switch (skills, hennas, effects follow), cancel; noble quest. |
| CP-Sc | A Kamaloka or Pailaka run; an area NPC with a script (a buffer, a teleporter); a raid boss with its AI. |
| CP-Sd | Milestone: a character with quests in progress on Sieghardt logs into Kain and continues them; the shift-click windows and the community board. |

## Open questions for the owner

1. **Scope:** all 505 quests, all AIs, all instances and the 10 events, so Kain is retail-like?
   Recommended: **yes**; the customs in `scripts/custom/` stay for M9.
2. **Side by side:** quest states (`character_quests`) are per-player rows and are written as
   usual (D-39). The `global_variables` some scripts use (boss timers, event states) count as
   global: written only with `CycleDatabaseWrites` (D-53). The grand boss AIs run on Kain only
   when Sieghardt's boss row says the boss is alive and never write the state with the switch off.
   Recommended: yes.
3. **Order:** engine, pilot and class change first (they set the patterns), then quests and AIs in
   parallel (up to seven workers), bosses and instances after the pilot, handlers and events last.
   Recommended: yes.
4. **Models:** the quest batches go to Haiku after the pilot shows the pattern, with Sonnet spot
   reviews on a sample per batch; the engine, the pilot, subclasses and the first boss batch go to
   Opus. Recommended: yes.
