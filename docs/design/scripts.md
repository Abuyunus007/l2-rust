# Design: the script engine and how to port a script (S-API-01)

Binding for every script card (S-Q-*, S-AI-*, S-BOSS-*, S-INST-*, S-EVENT-*, ...). The engine is
`crates/l2-world/src/script/`; the scripts live in `crates/l2-scripts/`. Reference ports:
`l2-scripts/src/quests/q00001_letters_of_love.rs` (a quest) and
`l2-scripts/src/ai/others/asamah.rs` (an NPC script). Copy them. Quest ports start from the
"Quest patterns" section below: ten reference quests, one per pattern, with snippets and pitfalls.

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

Player, NPC and family calls (`quest_helpers.rs`, S-PILOT-01): `player_race` (against
`script::race::*`), `player_class_id` (against `script::player_class::*`, Java's `PlayerClass`
names), `player_base_class`, `player_class_level` (`getPlayerClass().level()`),
`player_class_race`, `is_mage_class`, `is_dead`, `is_in_party`, `is_inside_radius_2d`,
`alt_party_range`, `check_if_in_range`, `script_settings`; NPCs: `get/set/is_script_value`,
`is_npc_busy`, `set_npc_busy`, `npc_move_to`, `delete_npc`, `heading_of`, `drop_item`,
`show_chat_window`, `set_last_quest_npc_object`; class change: `set_player_class`,
`set_base_class`, `is_sub_class_active`, `broadcast_user_info`, `broadcast_magic_skill_use`;
Java `static` fields shared by several scripts: `shared_state::<T>()`.

## Quest patterns (S-PILOT-01): which file to copy

Ten quests cover what the ~500 quests do. Find the pattern of your quest below, open the
reference file next to its Java file, and copy its shape. All are in `crates/l2-scripts/src/quests/`.

| Pattern | Reference | Java calls it shows |
|---|---|---|
| Delivery chain, start NPC hands out, others swap | `q00001_letters_of_love.rs`, `q00153_deliver_goods.rs` | `giveItems`/`takeItems`/`hasQuestItems`, `setCond(n, true)`, `exitQuest(false)` |
| Kill count with a drop chance up to a limit | `q00295_dreaming_of_the_skies.rs` | `giveItemRandomly(.., limit, chance, true)`, `checkIfInRange(ALT_PARTY_RANGE, ..)` |
| Party kill, a random member at cond N | `q00303_collect_arrowheads.rs` | `getRandomPartyMember(player, 1)` |
| Repeatable with "go on / quit" | `q00317_catch_the_wind.rs` | `getRandomPartyMemberState(killer, -1, 3, npc)`, `exitQuest(true, true)` in `onEvent` |
| One-time, items + exp + adena, a monster that talks | `q00158_seed_of_evil.rs` | `exitQuest(false, true)`, `onAttack` + `isScriptValue`/`setScriptValue`, `NpcSay` |
| Race / class / level-range restricted, reward choice | `q00182_new_recruits.rs` | `getRace()`, `getPlayerClass().level()` |
| Timer, busy NPC, NPC turns into a monster, daily | `q00450_grave_robber_rescue.rs` | `startQuestTimer("despawn", ..)`, `setBusy`, `deleteMe`, `addSpawn` + `addDamageHate`, `exitQuest(QuestType.DAILY)` |
| Spawns a monster at the killer, race-only | `q00275_dark_winged_spies.rs` | `addSpawn(npcId, killer)`, `onCreatureSee` |
| Saga (third class), the shared base | `abstract_saga_quest.rs` + `q00070_saga_of_the_phoenix_knight.rs` | `AbstractSagaQuest`, `static SPAWN_LIST`, `setPlayerClass` |
| Tutorial (no journal page, packets as events) | `q00255_tutorial.rs` | `setOnEnterWorld`, `TL:`/`TC:`/`QM:`/`CE:` events, memo bit fields, player listeners |

### Java to Rust, the lines every quest has

```rust
use l2_protocol_game::NpcStringId;              // NpcStringId.HOW_DARE_YOU -> NpcStringId::HowDareYou
use l2_protocol_game::server::ChatType;         // ChatType.NPC_GENERAL -> ChatType::NpcGeneral
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, QuestType, STARTED, player_class, race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// final QuestState qs = getQuestState(player, false); if (qs == null) return null;
let player = player?;
let qs = ctx.get_quest_state(player, false)?;

// if ((qs != null) && qs.isCreated() && event.equals("30536-03.htm")) { qs.startQuest(); return event; } return null;
let qs = ctx.get_quest_state(player, false);
if let Some(qs) = qs
    && qs.is_created(ctx)
    && event == "30536-03.htm"
{
    qs.start_quest(ctx);
    return Some(event.to_owned());
}
None

// String htmltext = event; ... return htmltext;   (a quest that echoes every event)
let mut htmltext = Some(event.to_owned());
// case "despawn": ... htmltext = null;
htmltext = None;

// switch (qs.getState()) { case State.CREATED: ... }
match qs.state(ctx) {
    CREATED => {}
    STARTED => match qs.get_cond(ctx) { 1 => {}, _ => {} },
    COMPLETED => htmltext = ctx.get_already_completed_msg(player),
    _ => {}
}

// case State.COMPLETED: if (!qs.isNowAvailable()) { ...; break; } qs.setState(State.CREATED); // fall through
let mut state = qs.state(ctx);
if state == COMPLETED {
    if !qs.is_now_available(ctx) {
        return Some("32650-03.html".to_owned());
    }
    qs.set_state(ctx, CREATED);
    state = CREATED;
}

// qs.setCond(2);  qs.setCond(2, true);
qs.set_cond(ctx, 2, false);
qs.set_cond(ctx, 2, true);
// qs.exitQuest(true, true);  qs.exitQuest(false);  qs.exitQuest(QuestType.DAILY, true);
qs.exit_quest(ctx, true, true);
qs.exit_quest(ctx, false, false);
qs.exit_quest_type(ctx, QuestType::Daily, true);

// player.getRace() == Race.ORC   getPlayerClass() == PlayerClass.ORC_MAGE   getPlayerClass().level() == 0
ctx.player_race(player) == race::ORC;
ctx.player_class_id(player) == player_class::ORC_MAGE;
ctx.player_class_level(player) == 0;
```

### Kills, drops and parties

```rust
// LocationUtil.checkIfInRange(PlayerConfig.ALT_PARTY_RANGE, npc, killer, true)
ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true)

// giveItemRandomly(killer, npc, ITEM, amount, limit, chance, true)   (one amount: min = max)
ctx.give_item_randomly(killer, Some(npc), ITEM, amount, amount, limit, chance, true)
// giveItemRandomly(killer, ITEM, 1, 70, 1, true)   (the overload without the NPC)
ctx.give_item_randomly(killer, None, ITEM, 1, 1, 70, 1.0, true)
// giveItemRandomly(.., minAmount, maxAmount, ..): the same with min and max; limit 0 = no limit.

// final Player member = getRandomPartyMember(player, 1);
let member = ctx.get_random_party_member_cond(player, 1);
// final QuestState qs = getRandomPartyMemberState(killer, -1, 3, npc);  (-1 = any started)
let qs = ctx.random_party_member_state(killer, -1, 3, Some(npc));   // qs.player() is the member

// final ItemHolder ITEM = new ItemHolder(9746, 80); hasItem(player, ITEM) / takeItem(player, ITEM)
ctx.get_quest_items_count(player, 9746) >= 80;
ctx.take_items(player, 9746, 80);
```

`on_kill` runs 2.5 s after the death (Java's `onKillDelay`); the NPC is dead but still in the
world, so its position is there. `killer` is the player also when a summon killed
(`is_summon`).

### Timers, spawns and NPCs

```rust
// startQuestTimer("despawn", 3000, npc, player);  (on_event gets "despawn" with that npc/player)
ctx.start_quest_timer("despawn", 3000, Some(npc), Some(player), false);
// case "despawn": npc.setBusy(false); npc.deleteMe();
if let Some(npc) = npc { ctx.set_npc_busy(npc, false); ctx.delete_npc(npc); }

// addSpawn(MOB, npc.getX(), npc.getY(), npc.getZ(), npc.getHeading(), true, 600000)
let at = ctx.world().position_of(npc.object_id);   // read BEFORE delete_npc: a deleted NPC has no place
let heading = ctx.heading_of(npc.object_id);
if let Some((x, y, z)) = at && let Some(mob) = ctx.add_spawn(MOB, x, y, z, heading, true, 600_000, 0) {
    // monster.setRunning(); monster.addDamageHate(player, 0, 999); monster.getAI().setIntentionAttack(player);
    ctx.add_attack_desire(mob, player, 999);
}
// addSpawn(MOB, killer)   (at the killer's place and heading, no offset, no despawn)
ctx.add_spawn_at(MOB, killer, false, 0);

// npc.broadcastPacket(new NpcSay(npc, ChatType.NPC_GENERAL, NpcStringId.HOW_DARE_YOU_CHALLENGE_ME));
ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, NpcStringId::HowDareYouChallengeMe, &[]);
// showOnScreenMsg(player, NpcStringId.X, 5, 5000);
ctx.show_on_screen_npc_string(player, NpcStringId::X, 5, 5000, &[]);
// if (npc.isScriptValue(0)) { ...; npc.setScriptValue(1); }   (back to 0 when the NPC decays)
if ctx.is_script_value(npc, 0) { ctx.set_script_value(npc, 1); }
```

### A family of scripts with a Java base class

Java's `abstract class X extends Quest` with protected fields becomes a data struct, free
functions that take it, and a `macro_rules!` that writes the `impl Script` (see
`abstract_saga_quest.rs`). A saga is then a constant and one line:

```rust
const DATA: SagaData = SagaData { quest_id: 70, npc: [..], items: [..], mob: [..], class_id: &[90],
    prev_class: &[0x05], npc_spawn_locations: [..], text: [..] };
saga_quest!(Q00070SagaOfThePhoenixKnight, "Q00070_SagaOfThePhoenixKnight", DATA);
```

A `static` field of the base (shared by every script of the family) is
`ctx.shared_state::<MyType>()`; a field of one script is `ctx.state::<MyType>()`.

### Testing a quest

Every quest gets, at the end of its file, a test from the start to the reward with the
`character_quests` rows, and one test per branch that writes something. Copy the `tests` module
of the closest reference. The harness (`crate::testing`):

```rust
let mut registry = ScriptRegistry::new();
registry.add(Q00295DreamingOfTheSkies);
let pages = Pages::of(&Q00295DreamingOfTheSkies)
    .page("30536-02.htm", &["30536-03.htm"])          // a page and the events its links send
    .plain(&["30536-01.htm", "30536-03.htm"])           // pages without links
    .npcs(&[ARIN]);                                    // NPC default pages + "already completed"
let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
let arin = t.spawn(ARIN, 20);                          // 20 steps east of the players
t.enter(1, 11, vec![]);                                // or t.enter_as(1, lvl, race::ORC, player_class::ORC_FIGHTER, items)
assert!(t.quest_page(1, arin).contains("30536-02.htm"));   // talk + "bypass Script"
assert!(t.event(1, NAME, "30536-03.htm").contains("30536-03.htm"));
assert_eq!(t.take_writes(), [rows::insert(NAME, "cond", "1"), rows::insert(NAME, "<state>", "Started")]);
let mob = t.spawn(MAGICAL_WEAVER, 60);
t.kill(1, mob);                                        // do_die + the 2.5 s kill delay
t.party(1, 2);                                         // 1 invites 2, 2 accepts
t.wait_ms(3_000);                                      // quest timers, despawns
assert!(!t.in_world(robber));                          // deleted NPCs keep their entry
t.npcs_of(MOB);                                        // the NPCs of an id in the world (add_spawn)
t.world.run_as_script(NAME, |ctx| { /* call a hook the world does not fire yet */ });
```

Stand-in pages carry the file name and the links of the real page (look at the real `.htm` in
`dist/game/data/scripts/quests/<Quest>/`): the world takes a `bypass Script <name> <event>` only
when the last page offered it. Item templates of quest items need `is_questitem`, stack counts
need `is_stackable`. The world RNG is seeded (7), so drops and rolls are the same every run: a
test loops "kill until the count is reached" instead of guessing the number of kills.

### Pitfalls (each one cost a test run)

- **Rows**: the first `set` of a variable is `InsertVar`, every later one `UpdateVar` (also across
  steps: the saga's `spawned` is inserted at step 6, updated at 15). `exit_quest(ctx, true, _)` is
  one `DeleteQuest { repeatable: true }`; `exit_quest(ctx, false, _)` is `DeleteQuest { repeatable:
  false }` then `UpdateVar <state> Completed`. Java's name of `CREATED` is `"Start"`.
- **`setCond` / `exitQuest` overloads**: `setCond(n)` has no sound (`false`), `exitQuest(b)` has no
  finish sound (`false`). `set_cond` does nothing on a state that is not started.
- **RNG order**: call `get_random` where and as often as Java does (`getRandom(100) > 25 ? 1 : 2`
  before `giveItemRandomly`; `getRandom(2) <= 1` is always true but still rolls).
- **`npc` in `on_event`** is `None` for a timer started without one; where Java calls
  `npc.getId()` without a check use `let npc = npc?;`, where it compares use
  `npc.is_some_and(|n| n.id == X)`.
- **Deleted NPCs**: `delete_npc` takes the NPC out of the world but keeps the entry (Java keeps
  the object for the next life), so `world.npc(id)` is still `Some`: test with `t.in_world`. Read
  position and heading before deleting.
- **Hostile spawns** keep attacking the test character during `wait_ms`/`kill` and can kill it
  (100 HP): delete them (`t.world.delete_npc`) once the test checked them.
- **Clicks**: `t.talk` clicks once when the NPC is the target already (two clicks would talk twice
  and fire `on_first_talk` twice).
- **Quirks**: port them and mark them (`// NOTE(java-quirk):`): Q00303 and Q00320 give the item to
  the killer, not to the member drawn; Q00275 shows page 05 at the reward; Q00153 and Q00182 echo
  any event; Q00255's listeners are added again on every call.
- **Hooks the world does not fire yet** (list below): port them anyway and test them by calling
  the hook through `t.world.run_as_script(NAME, |ctx| Script.on_x(ctx, ..))`.
- **Test HP**: the test world has no stat tables, so max HP is 0.
- **`Pages`**: an event not linked from the last page is refused silently (`t.event` returns
  an empty string).

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

## Added by S-API-03

- `getTimers()` (`TimerExecutor`): `ctx.add_timer(event, Some(TimerParams::new().set("Name", x)), ms, npc, player)`,
  `add_repeating_timer`, `has_timer`, `cancel_timer(s)`, `cancel_timers_of`, `cancel_all_timers`; hooks `on_timer_event`,
  `on_timer_cancel` (params: `TimerParams::get_string/get_int/get_long/get_bool`).
- `on_creature_see` fires (`CreatureSeeTaskManager`, once a second, aggro range or `AltPartyRange`).
- Global listeners: `s.set_on_player_bypass()`, `set_on_player_press_tutorial_mark()`, `set_on_player_logout()`,
  `set_on_day_night_change()` (hooks `on_player_bypass`, `on_player_press_tutorial_mark`, `on_player_logout`,
  `on_day_night_change`, all queued) and `s.add_creature_death_id(&[..])` (`on_creature_death`, at once).
- `on_attack` gets the skill of a skill hit; `ctx.play_movie` keeps the movie until `EndScenePlayer`
  (`ctx.play_movie_for(&players, id)`, `World::is_playing_movie`); `add_spawn` slots are reused once the NPC is gone;
  `//show_quests` and `//quest_info` exist (`script/admin.rs`).

## Not in the engine yet (follow-ups)

Annotation listeners with per-player registration (`player.addListener(..)` of the event scripts), the other
`EventType`s (`ON_CREATURE_ATTACKED/SKILL_USE/DAMAGE_*`, `ON_PLAYER_SIT/LEVEL_CHANGED/ITEM_*`, clan, siege, olympiad,
trap, `ON_DAILY_RESET`), the D-50 re-read of `global_variables` (needs the game server DB task), summon spawn/talk,
premium rates (M9), the multilingual chooser strings (M9).

Found by S-PILOT-01: the tutorial's player listeners (`ON_PLAYER_LEVEL_CHANGED`, `ON_PLAYER_ITEM_PICKUP`,
`ON_PLAYER_SIT`, `ON_CREATURE_ATTACKED`: kept in its state, with `pub` entry points for the engine); `setup` cannot read the
configuration (Java constructors do: `if (!PlayerConfig.DISABLE_TUTORIAL) addStartNpc(..)`), so such
scripts check `ctx.script_settings()` in their hooks; the client ground location of a player's
`MagicSkillUse` in `broadcast_magic_skill_use` (always none).
