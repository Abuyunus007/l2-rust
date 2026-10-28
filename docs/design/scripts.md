# Design: the script engine and how to port a script (S-API-01)

Binding for every script card (S-Q-*, S-AI-*, S-BOSS-*, S-INST-*, S-EVENT-*, ...). The engine is
`crates/l2-world/src/script/`; the scripts live in `crates/l2-scripts/`. Reference ports:
`l2-scripts/src/quests/q00001_letters_of_love.rs` (a quest) and
`l2-scripts/src/ai/others/asamah.rs` (an NPC script). Copy them.

## The model in one page

| Java | Rust |
|---|---|
| `class Q00001_LettersOfLove extends Quest` | `pub struct Q00001LettersOfLove;` + `impl Script for Q00001LettersOfLove` |
| `class Asamah extends Script` (`super(-1)`) | the same, without `fn id` (default -1) |
| `getName()` (class simple name) | `fn name() -> "Q00001_LettersOfLove"` (exactly the Java class name: it is the `character_quests.name` and the bypass name) |
| `getPath()` (package as folder) | `fn path() -> "quests/Q00001_LettersOfLove"` (the html folder below `data/scripts`) |
| `super(1)` | `fn id() -> 1` |
| constructor: `addStartNpc(..)`, `addTalkId(..)`, `registerQuestItems(..)` | `fn setup(&self, s: &mut ScriptSetup)`: `s.add_start_npc(&[..])`, `s.add_talk_id(&[..])`, `s.register_quest_items(&[..])` |
| `public String onTalk(Npc npc, Player player)` | `fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String>` |
| `return null` / `return "x.htm"` | `None` / `Some("x.htm".to_owned())` |
| `npc.getId()`, `npc.getObjectId()` | `npc.id`, `npc.object_id` |
| `player` (object) | `player: ObjectId`; read through `ctx` (`ctx.player_level(player)`, `ctx.world().player(player)`) |
| `getQuestState(player, true)` | `ctx.get_quest_state(player, true)` -> `Option<Qs>` |
| `qs.setCond(2, true)` | `qs.set_cond(ctx, 2, true)` |
| `qs.startQuest()` / `qs.exitQuest(false, true)` | `qs.start_quest(ctx)` / `qs.exit_quest(ctx, false, true)` |
| `giveItems(player, id, n)`, `takeItems(..)`, `hasQuestItems(..)`, `getQuestItemsCount(..)` | `ctx.give_items(..)`, `ctx.take_items(..)`, `ctx.has_quest_items(..)`, `ctx.get_quest_items_count(..)` |
| `getRandom(100)` | `ctx.get_random(100)` (the world RNG, D-13) |
| `startQuestTimer("x", 5000, npc, player)` | `ctx.start_quest_timer("x", 5000, Some(npc), Some(player), false)` |
| fields of the script class (state) | `ctx.state::<MyState>()` (a `Default` struct of the script, owned by the world) |
| `ScriptManager.getScript("Other")` | `ctx.quest_state_of("Other", player, init)`, `ctx.notify_event_of("Other", ..)` |
| `player.getQuestState("Other")` | `ctx.player_quest_state(player, "Other")` (read only) |
| `static final int DARIN = 30048;` | `const DARIN: i32 = 30048;` |

Rules that follow from the architecture (D-10, D-11, D-13, D-15):
- A script holds **no state** (`&self` hooks, the struct is a unit struct). Anything a Java script
  keeps in fields goes to `ctx.state::<T>()`; global persistent values to the global variables.
- Never keep a `Qs` or a `ScriptNpc` between calls: they are ids. Use timers or `ctx.state`.
- No threads, no sleeps: `ThreadPool.schedule` in a script becomes `ctx.start_quest_timer`.
- No DB: `Qs` methods write `character_quests` through the outbox in Java's statement order.
- Anything the helpers do not cover: `ctx.world_mut()` and the `pub` world API. If a script needs
  world behaviour that is not `pub` yet, add a small `pub` method in the world module that owns it
  (your card must list that file) or report it.

## Writing a script, step by step

1. Create `crates/l2-scripts/src/<folder>/<snake_name>.rs` mirroring the Java folder:
   `quests/Q00123_Foo/Q00123_Foo.java` -> `quests/q00123_foo.rs`,
   `ai/others/Asamah/Asamah.java` -> `ai/others/asamah.rs`, `ai/areas/X/Y.java` -> `ai/areas/x/y.rs`.
2. Head: `//! one line`, then `// Java: dist/game/data/scripts/...`.
3. Constants first, in Java's order and with Java's names (`const DARIN: i32 = 30048;`).
4. `impl Script`: `name`, `path`, `id` (quests), `setup`, then the hooks the Java class overrides,
   **line by line**, keeping Java's `switch`es as `match` and Java's variable names (`htmltext`).
5. Register: one `pub mod` line and one `r.add(..)` line in the folder's `mod.rs`, sorted (quests by
   id, others by name). A new folder gets a `mod.rs` with a `register` fn and one line in its
   parent. Nothing else is shared.
6. Tests at the end of the same file (`#[cfg(test)] mod tests`) with `crate::testing::ScriptTest`
   (see the two reference ports): stand-in pages with the file name and the real page's links, NPC
   and item templates as XML strings, the players drive the world with clicks and bypasses. Test at
   least: the start, one step, the reward (items, `character_quests` writes), and every branch that
   writes something. Use another script's name with a stand-in `Script` when the port reads it.

Java quirks are ported as they are, with `// NOTE(java-quirk):` (D-17 does not change behaviour).
`l2-scripts` allows clippy's `collapsible_*` lints: keep Java's nesting rather than match guards.

## Hooks and when they run

Every hook has an empty default. Registration (`setup`) decides which NPCs/items/zones reach it.

| Hook | Registered with | Fired by (world) | Timing |
|---|---|---|---|
| `on_talk` | `add_talk_id` (+ `add_start_npc` for the quest list) | the `Script` bypass of an NPC page (quest window / chooser) | at once |
| `on_first_talk` | `add_first_talk_id` | NPC click, `Chat 0` | queued (Java async) |
| `on_event` | — | `bypass Script <name> <event>` (NPC in reach), quest timers, `notify_event` | at once |
| `on_kill` | `add_kill_id` | death of the NPC by a player/summon | timer: `onKillDelay` 2500 ms |
| `on_attack` | `add_attack_id` | damage by a player/summon | queued |
| `on_spawn` / `on_teleport` | `add_spawn_id` / `add_teleport_id` | the NPC enters the world | queued |
| `on_aggro_range_enter` | `add_aggro_range_enter_id` | hate 0 added by a player | queued |
| `on_faction_call` | `add_faction_call_id` | clan help (AI, death) | queued |
| `on_skill_see` | `add_skill_see_id` | a player casts within 1000 | queued |
| `on_spell_finished` | `add_spell_finished_id` | the NPC's cast on a playable ends | queued |
| `on_move_finished`, `on_node_arrived`, `on_route_finished` | `add_move_finished_id`, ... | movement / walking routes | queued |
| `on_enter_zone` / `on_exit_zone` | `add_enter_zone_id` / `add_exit_zone_id` | a player enters/leaves a zone | queued |
| `on_enter_world` | `set_on_enter_world()` | the end of `EnterWorld` | queued |
| `on_item_talk` / `on_item_event` | `add_item_talk_id` / `add_item_bypass_event_id` | `item_<id>_Script...` bypass | queued |
| `on_npc_hate` / `on_can_see_me` | `add_npc_hate_id` / `add_can_see_me_id` | monster aggro check | at once, first answer counts |
| `on_death` | `qs.add_notify_of_death(ctx, player)` | the player dies | at once |
| `on_event_received` | `add_event_received_id` | `World::script_send_event` | queued |
| `on_load` / `on_save` | — | after all scripts are installed / at shutdown | at once |

"Queued" = Java's `notifyEventAsync`: the event runs after the command or tick that fired it
(`World::script_drain`). Listeners of one NPC run in registration order, which is the order of
`l2_scripts::register_all` (Java's folder order: `ai`, ..., `quests`).

The return value of `on_talk`, `on_first_talk`, `on_event` and `on_item_talk` goes through Java's
`showResult`: `*.htm`/`*.html` = a page of the script folder (`.htm` of a quest id 1..19999 opens
the quest window), `<html...` = the text as a page, anything else = a chat message.

## Helpers (`ScriptCtx`, `ctx.rs`)

Items: `give_items`, `give_items_enchanted`, `reward_items` (quest rates), `give_adena(.., apply_rates)`,
`give_item_randomly`, `take_items` (`-1` = all), `take_items_many`, `has_quest_items`,
`has_all_quest_items`, `has_at_least_one_quest_item`, `get_quest_items_count(_of)`,
`get_enchant_level`. Exp: `add_exp_and_sp`. Sound: `play_sound(QuestSound::..)`,
`play_sound_named`. Html: `show_html_file`, `get_htm`, `get_no_quest_msg`,
`get_already_completed_msg`, `show_result`; tutorial: `world_mut().send_tutorial_html`.
Messages: `show_on_screen_msg`, `show_on_screen_npc_string`, `broadcast_npc_say(_text)`.
Spawns and NPCs: `add_spawn`, `add_spawn_at`, `add_attack_desire`, `cast_skill`, `open_door`,
`close_door`, `play_movie`, `teleport_player`. Radar: `add_radar`, `remove_radar`, `clear_radar`.
Party: `get_random_party_member(_cond/_var/_state)`, `random_party_member_for_npc`,
`random_party_member_state`, `execute_for_each_player` (calls `action_for_each_player`).
Quest states: `get_quest_state`, `quest_state_of`, `player_quest_state`, `has_quest_state`,
`have_memo`, the one-time flags and the NR memo helpers. Timers: `start_quest_timer`,
`cancel_quest_timer(s)`, `has_quest_timer`. Global variables: `global_variables()`,
`set_global_variable`, `remove_global_variable`. Random: `get_random`, `get_random_range`,
`get_random_boolean`, `get_random_entry`.

`Qs` (`qs.rs`): `state`, `is_created/started/completed`, `get`, `get_int`, `is_set`, `set`,
`set_int`, `unset`, `get_cond`, `is_cond`, `set_cond`, memo states, `start_quest`, `exit_quest`,
`exit_quest_type` (daily: `restartTime`), `set_state(_with)`, `show_question_mark`,
`add_notify_of_death`, `set_restart_time`, `is_now_available`.

## Persistence and side by side (D-15, D-39, D-54)

- `character_quests`: read at `CharacterSelect` (`PlayerProfile::quests`, the two queries of
  `Quest.playerEnter`), installed at `EnterWorld` before `QuestList`, written per statement
  (`PersistCommand::Quest`). Per-player rows: written as Java writes them. Rows of scripts this
  server does not have are never deleted (Java's `AutoDeleteInvalidQuestData` would delete them,
  and they belong to quests the Java GS still runs).
- `global_variables`: read at startup; scripts change the copy in memory; written only with
  `CycleDatabaseWrites = True` at shutdown, as `REPLACE`/`DELETE` of the changed names (not Java's
  whole-table rewrite, which would clobber the rows other systems of this server write).
- Boss and event scripts that decide on a global row follow D-53/D-54 (re-read, no writes with the
  switch off); their cards bring the re-read requests.

## Not in the engine yet (follow-ups)

`TimerExecutor` (`getTimers()`, seven scripts), annotation listeners, `ON_PLAYER_BYPASS` /
`ON_PLAYER_PRESS_TUTORIAL_MARK`, `CreatureSeeTaskManager` (so `on_creature_see` is never fired),
summon spawn/talk, trap actions, olympiad/castle/fortress listener types, the skill of a hit in
`on_attack` (always `None`), `GmViewQuestInfo` contents, the movie holder (`EndScenePlayer`),
champion bonuses in `give_item_randomly` (champions are not rolled), premium rates (M9), the
multilingual chooser strings (M9).
