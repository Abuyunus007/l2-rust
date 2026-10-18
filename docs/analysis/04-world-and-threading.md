# World, entities, threading and the script API

This is where earlier Rust attempts got stuck (borrow checker / design). This doc describes how Java does it and why a direct translation fails, then lays out the options for the decision we still have to make together.

## Entity model in Java

```
ListenersContainer (event listeners: every object can have script hooks)
└─ WorldObject (objectId, x/y/z, heading, instance id, world region)
   ├─ Item (when lying on the ground; also lives in inventories)
   └─ Creature (7.6k lines: stats, status (HP/MP/CP), effects, target, AI, movement, casting, attack)
      ├─ Playable
      │  ├─ Player (15.4k lines, ~20 ScheduledFuture fields, references to party, clan, summon, requester, last NPC...)
      │  └─ Summon → Pet, Servitor
      ├─ Npc (2.2k)
      │  ├─ Folk → Merchant, Warehouse, Teleporter, VillageMaster, Doorman, ...
      │  └─ Attackable → Monster → RaidBoss, GrandBoss, Chest, FeedableBeast ...; Guard, Defender, ...
      ├─ Door, Vehicle (Boat, AirShip), StaticObject, Decoy
```
- **73 "instance" subclasses** (`entity/actor/instance`), mostly NPC types that differ in bypass/talk behaviour.
- **Parallel hierarchies:** `CreatureStat` → `PlayableStat`/`NpcStat`/..., `CreatureStatus` → ..., `CreatureTemplate` → `NpcTemplate`/`PlayerTemplate`/..., `CreatureAI` → `PlayableAI`/`AttackableAI`/...
- **Object graph with cycles everywhere:** `Creature._target`, `_summoner`, `_ai` (the AI points back at its actor); `Player._summon`, `_party`, `_clan`, `_activeRequester`; `Party` and `Clan` hold lists of `Player`. Inventories hold `Item` objects that are also `WorldObject`s.

## World and visibility

- `World` (2.6k lines) holds all objects by id, players by name, and a **3D grid of `WorldRegion`s**: 2048 world units per side (`SHIFT_BY = 11`), Z slices of 2000.
- Each region has a concurrent set of visible objects and precomputed surrounding regions. Regions activate and deactivate depending on whether players are nearby (NPC AI only runs in active regions).
- Visibility ("known list") is computed by iterating the surrounding regions (`World.forEachVisibleObject...`). Entering or leaving sight sends spawn/delete packets.

## Time and concurrency

- `ThreadPool`: a high-priority scheduled pool, a normal scheduled pool and an instant pool (`Threads.ini`).
- **20 task managers** tick shared sets: movement every **50 ms for players, 100 ms for NPCs** (`MovementTaskManager`), AI think (`AttackableThinkTaskManager`), attack stance, decay, respawn, auto-save, PvP flag, item lifetime, game time, ...
- ~350 `ThreadPool.schedule*` calls in the core, 188 in scripts, and **962 `startQuestTimer` calls** in scripts.
- Incoming packets run on a separate executor pool, concurrently even for the same client (see 02).
- So **any thread can touch any object at any time.** Java survives this with 288 `synchronized`, ~500 concurrent collections, `ReentrantLock`s and a `DeadlockWatcher` (it exists for a reason).

## Why "translate the classes" fails in Rust

1. Inheritance with overridden behaviour 4–5 levels deep doesn't map onto Rust structs. Traits + downcasting turn into a mess quickly.
2. Cyclic references (`Player ↔ Party ↔ Player`, `Creature ↔ AI`, target pointers) mean `Rc<RefCell>`/`Arc<Mutex>` everywhere. That brings runtime borrow panics, deadlocks, and lifetimes the compiler fights at every step.
3. "Any thread touches any object" plus `Arc<Mutex<Player>>` means lock-order deadlocks, which are harder to debug than in Java.
4. Scripts call into all of this freely (`player.getParty().getMembers().forEach(m -> m.sendPacket(...))`). If the core model fights the borrow checker, every one of the ~1,300 script files does too.

## Architecture options (for `DECISIONS.md`)

**A. Single-owner world loop (recommended).** One world task owns all game state. Network tasks parse packets and send `Command`s through a channel. The world applies them in order, runs a fixed tick (50 ms: matches player movement), and runs a **timer wheel** for everything Java schedules (`ThreadPool.schedule`, quest timers, task managers). Entities live in **arenas/slotmaps keyed by ObjectId**, and every reference becomes an id (`target: Option<ObjectId>`). Outgoing packets are queued per connection. DB writes go to an async persistence task.
- ➕ Basically no locks, no deadlocks, deterministic (testable with replayed captures), borrow-checker friendly (`&mut World` is passed down). Scripts become simple: `fn on_kill(&mut self, ctx: &mut Ctx, npc: NpcId, killer: PlayerId)`.
- ➖ The world logic uses one core. For a private server with tens to a few hundred players this is easily enough. L2's heavy parts (pathfinding, geodata line-of-sight) are pure functions on immutable geodata and can go to worker threads, with results coming back as commands.
- Behaviour difference: Java has races. Rust is strictly ordered. That's acceptable (and strictly better), but replayed captures may differ in timing details.

**B. Region-sharded actors** (each region or zone is its own task). Scales across cores, but cross-region interactions (party, clan, chat, targets across a boundary, teleports) become message protocols. Too complex as a starting point. Option A can be split up later if it ever becomes necessary.

**C. Full ECS** (`bevy_ecs`, `hecs`). It solves ownership well. But the Java logic is written in OOP style, so porting 300k lines of scripts to ECS queries is a big conceptual jump for every task card, and cheap models are much worse at it. A compromise is possible: A + plain structs for components inside the arenas.

**D. One-to-one translation with `Arc<Mutex<…>>`.** This is the path that failed before. Not recommended.

## Mapping the hierarchy (draft for option A)

- `Creature` data → a `CreatureCore` struct (position, stats, status, effects, target, ai state) embedded in `Player`, `Npc`, `Summon`, `Door`...
- Object kind → enum `ObjectKind { Player, Npc, Summon, Door, Item, Vehicle, StaticObject, ... }`. Lookup: `world.creature(id) -> CreatureRef` (an enum dispatching to the right arena).
- NPC "instance" subclasses → `NpcKind` enum + data-driven behaviour (template type string from XML), not 73 structs.
- Stat/Status/AI hierarchies → plain functions over `CreatureCore` + kind-specific extensions.
- Party/Clan → their own arenas holding member ids. A player holds `party: Option<PartyId>`.
- Item → an `items` arena. Inventory = list of item ids. A ground item = an item with a world position.

## Script API (what has to be designed before scripts are ported)

Most-used core types in scripts: `Player` (1051 imports), `Npc` (803), `QuestState` (536), `Quest` (499), `Creature` (296), `Skill` (286), `SystemMessageId`, `Location`, `NpcStringId`, configs, effects/conditions (skill handlers).

- Java events: `ListenersContainer` on objects, templates and zones; **89 `EventType`s**; registered via annotations (`@RegisterEvent`, `@Id`, `@Range`...) or `Quest` helper methods (`addKillId`, `addTalkId`, `addStartNpc`, `startQuestTimer`...). `Quest.java` (5.7k lines) is the de facto script base class.
- In Rust: a `Script` trait with default-empty callbacks (on_talk, on_first_talk, on_kill, on_adv_event, on_spawn, on_skill_see, on_enter_zone, ...), registration by NPC/zone id at startup, a `Ctx` handle with the API methods (give_items, take_items, show_html, start_timer, play_sound, npc_say...), and `QuestState` stored per player.
- Skill effect handlers (199 files) and conditions: same pattern, an `Effect` trait + registry by name, matching the names in the skill XML.
- Once this API is stable and has 10–20 hand-ported reference scripts, most quests become very mechanical → **this is where Haiku pays off**.

## Open questions for the decision session
1. Option A as the base: agreed?
2. Tick rate: 50 ms global, or 50 ms players + 100 ms NPCs like Java?
3. Deterministic RNG (seedable) so replay tests are reproducible?
4. Hot-reload of scripts (Java recompiles scripts at runtime) is **lost** with compiled Rust scripts unless we add plugins/dylibs. Acceptable (restart to reload)?
