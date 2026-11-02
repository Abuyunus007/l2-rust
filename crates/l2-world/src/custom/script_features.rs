//! The switches of the custom scripts of `data/scripts/custom` and the admin words of the Java
//! `Event` scripts (M9-C07).
// Java: config/custom/DelevelManagerConfig.java, NoblessMasterConfig.java, TransmogConfig.java
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminEvents.java
// Java: mechanics/script/Event.java
//
// The scripts (`l2-scripts/src/custom`) read these settings through `World::custom_script_settings`;
// the game server fills them from the ini files and from the `config.xml` of each event folder
// (`<schedule pattern="..."/>`) before the scripts are installed.

use std::collections::{BTreeMap, BTreeSet};

use l2_util::ObjectId;

use crate::admin::{AdminHandlers, JavaException};
use crate::minigame::cron::SchedulingPattern;
use crate::world::World;

/// `config/Custom/DelevelManager.ini` (Java `DelevelManagerConfig`).
#[derive(Debug, Clone, PartialEq)]
pub struct DelevelManagerSettings {
    /// `Enabled`.
    pub enabled: bool,
    /// `NpcId`.
    pub npc_id: i32,
    /// `RequiredItemId`.
    pub item_id: i32,
    /// `RequiredItemCount`.
    pub item_count: i32,
    /// `MimimumDelevel` (the key is misspelled in Java).
    pub minimum_delevel: i32,
}

impl Default for DelevelManagerSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            npc_id: 1_002_000,
            item_id: 4356,
            item_count: 2,
            minimum_delevel: 20,
        }
    }
}

/// `config/Custom/NoblessMaster.ini` (Java `NoblessMasterConfig`).
#[derive(Debug, Clone, PartialEq)]
pub struct NoblessMasterSettings {
    /// `Enabled`.
    pub enabled: bool,
    /// `NpcId`.
    pub npc_id: i32,
    /// `LevelRequirement`.
    pub level_requirement: i32,
    /// `ItemId`.
    pub item_id: i32,
    /// `ItemCount`.
    pub item_count: i64,
    /// `RewardTiara`.
    pub reward_tiara: bool,
}

impl Default for NoblessMasterSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            npc_id: 1_003_000,
            level_requirement: 80,
            item_id: 57,
            item_count: 0,
            reward_tiara: false,
        }
    }
}

/// `config/Custom/Transmog.ini` (Java `TransmogConfig`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TransmogSettings {
    /// `TransmogEnabled`.
    pub enabled: bool,
    /// `TransmogShareAccount`.
    pub share_account: bool,
    /// `TransmogApplyCost`.
    pub apply_cost: i32,
    /// `TransmogRemoveCost`.
    pub remove_cost: i32,
    /// `TransmogBannedItemIds`.
    pub banned_item_ids: BTreeSet<i32>,
}

/// Every switch of the custom scripts; `Default` is Java's default of each key (all off).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CustomScriptSettings {
    /// `DelevelManager.ini`.
    pub delevel: DelevelManagerSettings,
    /// `NoblessMaster.ini`.
    pub nobless: NoblessMasterSettings,
    /// `Transmog.ini`.
    pub transmog: TransmogSettings,
    /// The `<schedule pattern>` of the `config.xml` of each event, by event name (`Elpies`,
    /// `Rabbits`, `Race`, `Deathmatch`, `TvT`, `CtF`), in file order.
    pub event_schedules: BTreeMap<String, Vec<String>>,
    /// `GeneralConfig.CUSTOM_NPC_DATA` (`CustomNpcData`): Elpies and Rabbits use custom NPCs.
    pub custom_npc_data: bool,
}

/// What the world keeps for the custom scripts.
#[derive(Debug, Default)]
pub struct CustomScriptWorld {
    pub(crate) settings: CustomScriptSettings,
    pub(crate) transmog: super::transmog::TransmogWorld,
    /// The flags of the `EventMonster`s a script set.
    pub(crate) event_monsters: BTreeMap<ObjectId, super::events::EventMonsterFlags>,
}

impl World {
    /// Installs the switches of the custom scripts (before `install_scripts`).
    pub fn set_custom_script_settings(&mut self, settings: CustomScriptSettings) {
        self.custom_scripts.settings = settings;
    }

    /// The switches of the custom scripts.
    pub fn custom_script_settings(&self) -> &CustomScriptSettings {
        &self.custom_scripts.settings
    }

    /// Java `ExperienceData.getInstance().getExpForLevel(level)`; 0 without a table.
    pub fn experience_for_level(&self, level: i32) -> i64 {
        self.exp_for_level(level).unwrap_or(0)
    }

    /// Java `NpcData.getTemplate(npcId).getType().equals(type)` (`getAllNpcOfClassType`): the
    /// NPC template exists and is of this type.
    pub fn npc_template_is_type(&self, npc_id: i32, npc_type: &str) -> bool {
        self.npc_world
            .template(npc_id)
            .is_some_and(|t| t.is_type(npc_type))
    }

    /// Java `SpawnTable.getInstance().getAnySpawn(npcId)` as a place: the first spawn of the
    /// table, which holds the spawns of the datapack and, for a raid boss, the spawn the raid boss
    /// manager added.
    pub fn any_spawn_location(&self, npc_id: i32) -> Option<(i32, i32, i32)> {
        self.spawns_of(npc_id)
            .first()
            .map(|s| s.location)
            .or_else(|| self.raid_spawn_location(npc_id))
    }

    /// Java `schedulingPattern.getDelayToNextFromNow()`: milliseconds until the next match of the
    /// pattern, `None` for a pattern that never matches or does not parse.
    pub fn scheduling_delay_to_next(&self, pattern: &str) -> Option<i64> {
        let pattern = SchedulingPattern::parse(pattern).ok()?;
        let now = self.minigame_now_ms();
        let offset = l2_util::time::local_offset_millis(now);
        let next = pattern.next(now, offset);
        (next >= 0).then_some(next - now)
    }
}

type Outcome = Result<(), JavaException>;

/// Java `AdminEvents.ADMIN_COMMANDS`.
pub(crate) fn register(handlers: &mut AdminHandlers) {
    for word in [
        "admin_event_menu",
        "admin_event_start",
        "admin_event_stop",
        "admin_event_start_menu",
        "admin_event_stop_menu",
        "admin_event_bypass",
    ] {
        handlers.register(word, admin_events);
    }
}

/// Java `AdminEvents.onCommand`.
fn admin_events(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    // `new StringTokenizer(command, " ")`: the word, the event name, the bypass.
    let mut tokens = command.split(' ').filter(|t| !t.is_empty());
    tokens.next();
    let event_name = tokens.next().unwrap_or("");
    let event_bypass = tokens.next().unwrap_or("");

    if command.contains("_menu") {
        show_menu(world, gm);
    }

    // `(Event) ScriptManager.getScript(eventName)`: a script that is not an `Event` is a
    // `ClassCastException`, caught into the usage line.
    let found = world.scripts.by_name(event_name).and_then(|id| {
        world
            .scripts
            .script(id)
            .map(|script| (id, script.is_event(), script))
    });
    if command.starts_with("admin_event_start") {
        match found {
            Some((_, false, _)) => {
                world.send_sys_message(gm, "Usage: //event_start <eventname>");
            }
            Some((id, true, script)) => {
                let started = script.event_start(&mut crate::script::ScriptCtx::new(world, id), Some(gm));
                world.script_drain();
                if started {
                    world.send_sys_message(gm, &format!("Event {event_name} started."));
                } else {
                    world.send_sys_message(gm, &format!("There is problem starting {event_name} event."));
                }
            }
            None => {}
        }
    } else if command.starts_with("admin_event_stop") {
        match found {
            Some((_, false, _)) => {
                // NOTE(java-quirk): the stop word prints the usage of the start word.
                world.send_sys_message(gm, "Usage: //event_start <eventname>");
            }
            Some((id, true, script)) => {
                let stopped = script.event_stop(&mut crate::script::ScriptCtx::new(world, id));
                world.script_drain();
                if stopped {
                    world.send_sys_message(gm, &format!("Event {event_name} stopped."));
                } else {
                    world.send_sys_message(gm, &format!("There is problem with stoping {event_name} event."));
                }
            }
            None => {}
        }
    } else if command.starts_with("admin_event_bypass") {
        match found {
            Some((_, false, _)) => {
                world.send_sys_message(gm, "Usage: //event_bypass <eventname> <bypass>");
            }
            Some((id, true, script)) => {
                let result = script.event_bypass(&mut crate::script::ScriptCtx::new(world, id), gm, event_bypass);
                world.script_drain();
                if result.is_err() {
                    world.send_sys_message(gm, "Usage: //event_bypass <eventname> <bypass>");
                }
            }
            None => {}
        }
    }
    Ok(())
}

/// Java `AdminEvents.showMenu`.
fn show_menu(world: &mut World, gm: ObjectId) {
    // NOTE: Java walks a `ConcurrentHashMap<String, Quest>`; the list is sorted by name here.
    let mut names: Vec<&'static str> = world
        .scripts
        .ids()
        .filter_map(|id| world.scripts.script(id))
        .filter(|script| script.is_event())
        .map(|script| script.name())
        .collect();
    names.sort_unstable();
    let mut list = String::new();
    for name in names {
        list.push_str(&format!(
            "<tr><td><font color=\"LEVEL\">{name}:</font></td><br><td><button value=\"Start\" action=\"bypass -h admin_event_start_menu {name}\" width=80 height=21 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></td><td><button value=\"Stop\" action=\"bypass -h admin_event_stop_menu {name}\" width=80 height=21 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></td></tr>"
        ));
    }
    let mut page = world.admin_html_file(gm, "data/html/admin/gm_events.htm");
    page.replace("%LIST%", &list);
    world.send_admin_html(gm, &page);
}
