//! The script engine (S-API-01): Java's `Quest` / `Script` model for the compiled-in scripts of
//! `l2-scripts` (D-05, D-16). Porting guide: `docs/design/scripts.md`.
// Java: mechanics/script/{Quest,QuestState,QuestTimer,QuestSound,State,QuestType,Script}.java
// Java: mechanics/events/{EventDispatcher,ListenersContainer,EventType}.java
// Java: managers/{ScriptManager,GlobalVariablesManager}.java
//
// # Pieces
// - [`Script`] (`api.rs`): the trait a ported script implements; [`ScriptSetup`] (`setup.rs`)
//   is what its constructor registers.
// - [`ScriptRegistry`]: the list of scripts the game server hands the world at startup
//   ([`World::install_scripts`]); the order is the registration order of the listeners.
// - [`ScriptCtx`] (`ctx.rs`): the `&mut World` a hook runs with, plus the helpers of Java's
//   `Quest` (`giveItems`, `takeItems`, `showHtmlFile`, ...).
// - [`Qs`] (`qs.rs`): a handle on a player's quest state (Java `QuestState`); its methods
//   write `character_quests` through the outbox (D-15) and send `QuestList`.
// - `notify.rs`: the world hooks call `World::script_*`; the events Java sends with
//   `notifyEventAsync` wait in a queue that runs after the current command or tick.
// - `timers.rs`: `startQuestTimer` on the timer wheel (D-12).
// - `link.rs`: the `Script` bypass, the quest window and the quest chooser.
// - `packets.rs`: `QuestList`, `RequestQuestList`, `RequestQuestAbort`, the tutorial packets.
// - `globals.rs`: `GlobalVariablesManager` with the D-54 switch.
//
// # Left out, with the card that brings it
// - `TimerExecutor` / `onTimerEvent` (the newer timers of seven scripts): later script cards.
// - `ON_PLAYER_BYPASS`, `ON_PLAYER_PRESS_TUTORIAL_MARK` (script listeners of the player): the
//   tutorial card.
// - Annotation listeners (`@RegisterEvent`), `registerConsumer` with custom callbacks, the
//   castle / fortress / olympiad listener types: the cards of the scripts that use them.
// - Premium rates (`PremiumSystemConfig`) and the multilingual `NpcStringId` chooser: M9.

pub mod api;
pub mod ctx;
pub mod globals;
pub mod link;
pub mod notify;
pub mod packets;
pub mod qs;
pub mod quest_helpers;
pub mod setup;
pub mod sound;
mod spawn;
pub mod state;
pub mod timers;

use std::any::Any;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use l2_util::ObjectId;
use tracing::{info, warn};

pub use api::{Script, ScriptItem, ScriptNpc};
pub use ctx::ScriptCtx;
pub use globals::{GlobalVariables, GlobalVariablesWrite};
pub use packets::QuestInput;
pub use qs::Qs;
pub use quest_helpers::{class_race, player_class, race};
pub use setup::{EventKind, ScriptId, ScriptSetup, StartCondition};
pub use sound::QuestSound;
pub use state::{COMPLETED, CREATED, PlayerQuests, QuestLoad, QuestState, QuestType, QuestWrite, STARTED};

use crate::world::World;

/// The scripts the server runs, in registration order (Java's load order of
/// `data/scripts/**`). Built by the game server (`l2_scripts::register_all`) or a test, then
/// handed to [`World::install_scripts`].
#[derive(Default)]
pub struct ScriptRegistry {
    scripts: Vec<Arc<dyn Script>>,
}

impl ScriptRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a script (Java: the script's `main` runs `new Script()`).
    pub fn add(&mut self, script: impl Script) {
        self.scripts.push(Arc::new(script));
    }

    pub fn len(&self) -> usize {
        self.scripts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.scripts.is_empty()
    }
}

/// The values of `Rates.ini` and `General.ini` the script helpers read, and the D-54 switch.
/// `Default` is Java's default of each key.
#[derive(Debug, Clone, PartialEq)]
pub struct ScriptSettings {
    /// `RatesConfig.RATE_QUEST_REWARD` (`RateQuestReward`).
    pub rate_quest_reward: f32,
    /// `RatesConfig.RATE_QUEST_REWARD_XP` (`RateQuestRewardXP`).
    pub rate_quest_reward_xp: f32,
    /// `RatesConfig.RATE_QUEST_REWARD_SP` (`RateQuestRewardSP`).
    pub rate_quest_reward_sp: f32,
    /// `RatesConfig.RATE_QUEST_REWARD_ADENA` (`RateQuestRewardAdena`).
    pub rate_quest_reward_adena: f32,
    /// `RatesConfig.RATE_QUEST_REWARD_USE_MULTIPLIERS` (`UseQuestRewardMultipliers`).
    pub rate_quest_reward_use_multipliers: bool,
    /// `RatesConfig.RATE_QUEST_REWARD_POTION` (`RateQuestRewardPotion`).
    pub rate_quest_reward_potion: f32,
    /// `RatesConfig.RATE_QUEST_REWARD_SCROLL` (`RateQuestRewardScroll`).
    pub rate_quest_reward_scroll: f32,
    /// `RatesConfig.RATE_QUEST_REWARD_RECIPE` (`RateQuestRewardRecipe`).
    pub rate_quest_reward_recipe: f32,
    /// `RatesConfig.RATE_QUEST_REWARD_MATERIAL` (`RateQuestRewardMaterial`).
    pub rate_quest_reward_material: f32,
    /// `RatesConfig.QUEST_ITEM_DROP_AMOUNT_MULTIPLIER` (`QuestItemDropAmountMultiplier`).
    pub quest_item_drop_amount_multiplier: f32,
    /// `GeneralConfig.AUTODELETE_INVALID_QUEST_DATA` (`AutoDeleteInvalidQuestData`). Read, but
    /// see `World::script_player_enter`: rows of scripts this server does not have are never
    /// deleted (D-39).
    pub autodelete_invalid_quest_data: bool,
    /// `CycleDatabaseWrites` of `config/SideBySide.ini` (D-53, D-54): the global variables are
    /// written only when this is on.
    pub cycle_database_writes: bool,
}

impl Default for ScriptSettings {
    fn default() -> Self {
        Self {
            rate_quest_reward: 1.0,
            rate_quest_reward_xp: 1.0,
            rate_quest_reward_sp: 1.0,
            rate_quest_reward_adena: 1.0,
            rate_quest_reward_use_multipliers: false,
            rate_quest_reward_potion: 1.0,
            rate_quest_reward_scroll: 1.0,
            rate_quest_reward_recipe: 1.0,
            rate_quest_reward_material: 1.0,
            quest_item_drop_amount_multiplier: 1.0,
            autodelete_invalid_quest_data: false,
            cycle_database_writes: false,
        }
    }
}

/// One installed script and what its constructor registered.
struct Installed {
    script: Arc<dyn Script>,
    quest_items: Vec<i32>,
    start_conditions: Vec<(StartCondition, String)>,
}

/// Everything the script engine keeps in the world.
#[derive(Default)]
pub(crate) struct ScriptWorld {
    installed: Vec<Installed>,
    /// Java `ScriptManager._quests` + `_scripts` by name (case sensitive).
    by_name: HashMap<String, ScriptId>,
    pub(crate) listeners: setup::Listeners,
    /// The quest states of every online player (Java `Player._quests`).
    pub(crate) players: HashMap<ObjectId, PlayerQuests>,
    pub(crate) timers: timers::QuestTimers,
    /// The events Java sends with `notifyEventAsync`, waiting for the end of the command.
    pub(crate) pending: VecDeque<notify::PendingEvent>,
    /// True while the queue runs (an event that queues more is drained in the same loop).
    pub(crate) draining: bool,
    /// What a Java script keeps in its fields, per script ([`ScriptCtx::state`]).
    pub(crate) script_state: HashMap<ScriptId, Box<dyn Any + Send>>,
    pub(crate) globals: GlobalVariables,
    /// Java `Player.getRadar()` markers, per player.
    pub(crate) radar: HashMap<ObjectId, Vec<(i32, i32, i32)>>,
    pub(crate) settings: ScriptSettings,
    /// Java `Attackable._onKillDelay` where a script changed it.
    pub(crate) kill_delays: HashMap<ObjectId, i32>,
    /// Java `Npc._scriptValue` where it is not 0.
    pub(crate) npc_script_values: HashMap<ObjectId, i32>,
    /// Java `static` fields shared by a family of scripts ([`ScriptCtx::shared_state`]).
    pub(crate) shared_states: quest_helpers::SharedStates,
}

impl ScriptWorld {
    pub(crate) fn script(&self, id: ScriptId) -> Option<Arc<dyn Script>> {
        self.installed.get(id.0 as usize).map(|i| Arc::clone(&i.script))
    }

    pub(crate) fn name_of(&self, id: ScriptId) -> &'static str {
        self.installed.get(id.0 as usize).map_or("", |i| i.script.name())
    }

    pub(crate) fn quest_id_of(&self, id: ScriptId) -> i32 {
        self.installed.get(id.0 as usize).map_or(-1, |i| i.script.id())
    }

    /// Java `ScriptManager.getScript(name)`.
    pub(crate) fn by_name(&self, name: &str) -> Option<ScriptId> {
        self.by_name.get(name).copied()
    }

    /// Java `ScriptManager.getQuest(questId)`: the first quest with the id.
    pub(crate) fn by_quest_id(&self, quest_id: i32) -> Option<ScriptId> {
        self.installed
            .iter()
            .position(|i| i.script.id() > 0 && i.script.id() == quest_id)
            .map(|index| ScriptId(index as u32))
    }

    pub(crate) fn quest_items(&self, id: ScriptId) -> &[i32] {
        self.installed
            .get(id.0 as usize)
            .map_or(&[], |i| i.quest_items.as_slice())
    }

    pub(crate) fn start_conditions(&self, id: ScriptId) -> &[(StartCondition, String)] {
        self.installed
            .get(id.0 as usize)
            .map_or(&[], |i| i.start_conditions.as_slice())
    }

    pub(crate) fn ids(&self) -> impl Iterator<Item = ScriptId> + use<> {
        (0..self.installed.len() as u32).map(ScriptId)
    }
}

impl World {
    /// Installs the compiled-in scripts (D-16): runs every constructor ([`Script::setup`]) in
    /// registration order, then every `onLoad`. Call once, after the NPC templates are in (the
    /// listeners of unknown NPCs are dropped like Java's) and before players enter.
    pub fn install_scripts(&mut self, registry: ScriptRegistry) {
        let has_templates = self.npc_world.has_templates();
        let (mut quests, mut scripts) = (0, 0);
        for script in registry.scripts {
            let id = ScriptId(self.scripts.installed.len() as u32);
            let name = script.name();
            let mut setup = ScriptSetup::default();
            script.setup(&mut setup);
            let npc_world = &self.npc_world;
            let known = |npc_id: i32| npc_world.template(npc_id).is_some();
            self.scripts.listeners.register(
                id,
                name,
                &setup,
                has_templates.then_some(&known as &dyn Fn(i32) -> bool),
            );
            // Java `ScriptManager.addQuest` / `addScript`: a second script of the same name
            // replaces the first (`put`); the first one's listeners stay registered here, as no
            // two compiled-in scripts share a name.
            if self.scripts.by_name.insert(name.to_owned(), id).is_some() {
                warn!("ScriptManager: replaced the script {name}.");
            }
            if script.id() > 0 {
                quests += 1;
            } else {
                scripts += 1;
            }
            self.scripts.installed.push(Installed {
                script,
                quest_items: setup.quest_items,
                start_conditions: setup.start_conditions,
            });
        }
        info!("ScriptManager: Loaded {quests} quests.");
        info!("ScriptManager: Loaded {scripts} scripts.");
        for id in self.scripts.ids().collect::<Vec<_>>() {
            if let Some(script) = self.scripts.script(id) {
                script.on_load(&mut ScriptCtx::new(self, id));
            }
        }
        self.script_drain();
    }

    /// The script settings (`Rates.ini`, `General.ini`, `SideBySide.ini`).
    pub fn set_script_settings(&mut self, settings: ScriptSettings) {
        self.scripts.settings = settings;
    }

    /// Runs `f` with the context of the script `name` (`false` when there is none): what an
    /// admin command or a test does when it acts for a script. Queued events run after it.
    pub fn run_as_script(&mut self, name: &str, f: impl FnOnce(&mut ScriptCtx<'_>)) -> bool {
        let Some(id) = self.scripts.by_name(name) else {
            return false;
        };
        f(&mut ScriptCtx::new(self, id));
        self.script_drain();
        true
    }

    /// Java `ScriptManager.getScript(name) != null`.
    pub fn has_script(&self, name: &str) -> bool {
        self.scripts.by_name(name).is_some()
    }

    /// The quest states of an online player (Java `Player._quests`).
    pub fn player_quests(&self, player: ObjectId) -> Option<&PlayerQuests> {
        self.scripts.players.get(&player)
    }

    /// Java `ScriptManager.save()`: every script's `onSave`, then (with the D-54 switch) the
    /// global variables (`GlobalVariablesManager.storeMe` of `Shutdown`).
    pub(crate) fn scripts_shutdown(&mut self) {
        for id in self.scripts.ids().collect::<Vec<_>>() {
            if let Some(script) = self.scripts.script(id) {
                script.on_save(&mut ScriptCtx::new(self, id));
            }
        }
        self.store_global_variables();
    }
}
