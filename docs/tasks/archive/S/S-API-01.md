# S-API-01: Script engine

| Field | Value |
|---|---|
| Milestone | Scripts |
| Model | opus |
| Review | full |
| Reference | no (copy the merged M0..M7 code around it) |
| Depends on | none |
| Size | L |

## Goal
Port Java's script model as the API every later script card builds on: `Quest`/`AbstractScript` as a Rust trait (or a struct of hooks) plus a **compiled-in registry** (D-16: no hot reload). Listeners by NPC id, item id, zone id, skill and event kind with Java's dispatch order (`onTalk`, `onFirstTalk`, `onKill` incl. party kill handling, `onAttack`, `onSpawn`, `onSkillSee`, `onAggroRangeEnter`, `onSeeCreature`, `onEnterZone`/`onExitZone`, `onAdvEvent`, `onItemTalk`, `onItemUse`, `onFactionCall`, `onSpellFinished`, `onMoveFinished`, `onNodeArrived`/`onRouteFinished` (walking), `onEventReceived`, `onEnterWorld`, `onDeath`, ...). `QuestState` (states CREATED/STARTED/COMPLETED, cond, vars, `exitQuest(repeatable)`), `character_quests` read at CharacterSelect and written through the outbox (D-15), `global_variables` under D-54, quest timers on the timer wheel (`startQuestTimer`/`cancelQuestTimer`, per player/NPC), the helper set (`giveItems`, `takeItems`, `rewardItems` with quest rates, `getQuestItemsCount`, `hasQuestItems`, `giveAdena`, `addExpAndSp`, `addSpawn`, `getRandomPartyMember`, `getRandomPartyMemberState`, `playSound`, `showHtmlFile`/`getHtm` with the quest html folder, `NpcSay`, `broadcastNpcSay`, `teleportPlayer`, `addAttackDesire`, `castSkill`, `showOnScreenMsg`, `playMovie`), the `Quest` bypass and the quest chooser page, `noquest`/`alreadycompleted` pages, `QuestList`, `RequestQuestList`, `RequestQuestAbort`, `ExQuestNpcLogList`, the tutorial packets (`RequestTutorialLinkHtml`, `RequestTutorialPassCmdToServer`, `RequestTutorialQuestionMark`, `RequestTutorialClientEvent`, `TutorialShowHtml`, ...). Port **two real scripts** with it to prove the API: quest `Q00001_LettersOfLove` and one `ai/others` NPC script. Write `docs/design/scripts.md`: how a script is written in Rust (the porting guide for the batch workers).

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\mechanics\script\Quest.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\mechanics\script\QuestState.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\mechanics\events\*.java (ListenersContainer, EventDispatcher, the event types)`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\managers\ScriptManager.java`
- `C:\l2-rust\l2jmobius\dist\game\data\quests\Q00001_LettersOfLove\*`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\clientpackets\{RequestQuestList,RequestQuestAbort,RequestTutorial*}.java`

## Files
- `crates/l2-world/src/script/*` (new) + the world hooks (talk, kill, attack, spawn, zones, items, skills, enter world)
- `crates/l2-db` (character_quests, global_variables)
- crates/game-server
- `docs/design/scripts.md` (new)
- tests

## Requirements
1. Java behaviour, messages, packets, HTML and DB rows exactly (D-39: the Java GS shares the DB; D-49..D-54 for global writes and fresh reads).
2. Single-owner world, ids not references, timers on the wheel, world RNG; no unwrap/expect on external data; no DB call or await in a tick (D-15: request/answer through the outbox).
3. Port the packets you need yourself in `crates/l2-protocol-game/src/{client,server}/<area>.rs` with round-trip tests, dispatch arms and proxy decoders (unless M8-GPROTO-11 already has them: check main first).
4. Small wire-ins in shared files are allowed: add your lines only. List every marker you resolve and every one you leave.

## Out of scope
- The customs in `scripts/custom/`, premium, item mall (M9).

## Tests (must exist and pass)
- Dispatch order per event, quest state rows (switch on and off for global vars), timers, the quest chooser, the two scripts end to end.

## Done when
- [ ] fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`, `-j 4`); `git merge main` before the final checks
- [ ] committed as `S-API-01: <summary>`
- [ ] report delivered in the agent's report format (what you left and why, every guess)

## Notes from the orchestrator
- Read CLAUDE.md (safety rules), DECISIONS (D-15, D-39, D-49..D-54), the `docs/design/*.md` of the area you touch first.
- Up to seven workers run at the same time: in shared files (`lib.rs`, `world.rs`, `command.rs`, `outbox.rs`, `timer.rs`, `dispatch.rs`, `persist.rs`, `world_data.rs`) only add your lines; after `git merge main` watch for doubled `mod`/`use` lines.
- DB tests use `l2_db::GAME_SCHEMA`; add a schema copy to `crates/l2-db/sql/game/` for every new table.
