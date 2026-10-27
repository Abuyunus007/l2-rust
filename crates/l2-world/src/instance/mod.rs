//! Instance zones: copies of a place (doors, NPCs) that only the objects of the same instance id
//! see (M8-INST-01).
// Java: org/l2jmobius/gameserver/managers/InstanceManager.java
// Java: org/l2jmobius/gameserver/entity/instancezone/{Instance,InstanceWorld,InstanceReenterType,
//       InstanceRemoveBuffType}.java
// Java: org/l2jmobius/gameserver/entity/WorldObject.java (setInstanceId, sendInstanceUpdate)
//
// # The instance id on every object
// Every object carries its instance id in its table entry ([`crate::objects::ObjectEntry::
// instance_id`], Java `_location.getInstanceId()`); 0 is the normal world. Everything that asks
// "who is around" ([`World::visible_objects`] and the broadcasts built on it, so the knownlists,
// the AI scans, chat, targets and attacks) only finds objects of the *same* instance id, like
// Java's `World.forEachVisibleObject` (`wo.getInstanceId() != object.getInstanceId()`). The
// geodata checks get the instance id too: an instance's doors only block inside it (`door.rs`).
//
// Moving between instances is [`World::set_instance_id`] (Java `WorldObject.setInstanceId`): it
// keeps the instance's player and NPC lists, the instance timer of the client and the summon
// in step. Teleports call it (`teleport.rs`), so a player who enters or leaves an instance is
// taken out of view (`decayMe`), gets the new id and comes back into view of the objects of the
// new instance (`spawnMe` at `Appearing`).
//
// # Ownership (D-10, D-11)
// [`InstanceManager`] is the world's: the instances by id, the script worlds, the dynamic id
// counter, the re-entry times of the online characters. An [`Instance`] holds ids only (players,
// NPCs with their spawn index, doors by door id); the objects themselves stay in the world's
// tables. Java's `ThreadPool` tasks (`CheckTimeUp`, `TimeUp`, the eject of a dead player) are
// [`InstanceTimer`]s on the timer wheel.
//
// # Clocks
// Java keeps the start, end and "last left" times of an instance in `System.currentTimeMillis()`
// and does int arithmetic on them (`setDuration((int) (_instanceEndTime - now - 500))`), so the
// wall clock is used here too ([`World::instance_now_ms`]); tests pin it with
// [`InstanceSettings::clock_override_unix_ms`].
//
// # Files
// - `lifecycle.rs`: create from a template, the doors and spawn groups, the duration and the
//   empty-destroy timers, destroy.
// - `membership.rs`: `setInstanceId`, enter world, logout, death and revive, the restart point.
// - `script.rs`: Java `InstanceScript` (`enterInstance`, `finishInstance`, re-entry times, buff
//   removal), the API the instance scripts of the script milestone call (D-05).
// - `times.rs`: `character_instance_time` (Java `getInstanceTime`, `setInstanceTime`, ...), the
//   `/instancezone` user command.
// - `admin/instance.rs`: the admin words.

mod lifecycle;
mod membership;
mod script;
mod times;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use l2_data::{
    InstanceReenterTime, InstanceReenterType, InstanceRemoveBuffType, InstanceSpawn, InstanceTemplates,
};
use l2_util::ObjectId;

use crate::timer::TimerId;
use crate::world::{World, unix_ms_now};

pub use script::EnterOutcome;
pub use times::{INSTANCE_ZONE_COMMAND, InstanceTimeWrite};

/// Java `InstanceManager._dynamic`'s first value: ids from here on are dynamic instances.
pub const FIRST_DYNAMIC_INSTANCE_ID: i32 = 300_000;

/// The `General.ini` values the instances read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceSettings {
    /// `EjectDeadPlayerTime` (seconds, default 60) times 1000 (`GeneralConfig.EJECT_DEAD_PLAYER_TIME`).
    pub eject_dead_player_time_ms: i32,
    /// `DefaultFinishTime` (seconds, default 300) times 1000 (`GeneralConfig.INSTANCE_FINISH_TIME`).
    pub instance_finish_time_ms: i32,
    /// `RestorePlayerInstance` (default false).
    pub restore_player_instance: bool,
    /// `AllowSummonInInstance` (default false).
    pub allow_summon_in_instance: bool,
    /// Tests only: the wall clock at world tick 0 (the instance clock then runs with the ticks).
    pub clock_override_unix_ms: Option<i64>,
}

impl Default for InstanceSettings {
    fn default() -> Self {
        Self {
            eject_dead_player_time_ms: 60_000,
            instance_finish_time_ms: 300_000,
            restore_player_instance: false,
            allow_summon_in_instance: false,
            clock_override_unix_ms: None,
        }
    }
}

/// One instance (Java `Instance`). Ids only: the objects are in the world's tables.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instance {
    /// `_id`.
    pub id: i32,
    /// `_templateId`: -1 for an instance made without a template.
    pub template_id: i32,
    /// `_ejectTime`: how long a dead player may stay (ms).
    pub eject_time_ms: i32,
    /// `_allowRandomWalk`.
    pub allow_random_walk: bool,
    /// `_players`: the object ids of the players inside.
    pub players: BTreeSet<ObjectId>,
    /// `_npcs` with the spawn each came from. Like Java, an NPC that died or was deleted stays
    /// in the list until the instance goes or the NPC moves to another instance.
    pub npcs: BTreeMap<ObjectId, Option<usize>>,
    /// `_doorTemplates`: the door ids of the template, in file order.
    pub door_templates: Vec<i32>,
    /// `_doors`: door id -> the door object of this instance.
    pub doors: BTreeMap<i32, ObjectId>,
    /// `_spawnTemplates`.
    pub spawn_templates: Vec<InstanceSpawn>,
    /// `_enterLocations`.
    pub enter_locations: Vec<(i32, i32, i32)>,
    /// `_exitLocation`.
    pub exit_location: Option<(i32, i32, i32)>,
    /// `_allowSummon`.
    pub allow_summon: bool,
    /// `_emptyDestroyTime` (ms; -1: never destroyed for being empty).
    pub empty_destroy_time_ms: i64,
    /// `_lastLeft`, `_instanceStartTime`, `_instanceEndTime` (wall clock ms; -1 unset).
    pub last_left_ms: i64,
    pub start_time_ms: i64,
    pub end_time_ms: i64,
    /// `_isPvP`.
    pub pvp: bool,
    /// `_showTimer`, `_isTimerIncrease`, `_timerText`.
    pub show_timer: bool,
    pub timer_increase: bool,
    pub timer_text: String,
    /// `_type`, `_resetData`.
    pub reenter_type: InstanceReenterType,
    pub reenter_data: Vec<InstanceReenterTime>,
    /// `_removeBuffType`, `_exceptionList`.
    pub remove_buff_type: InstanceRemoveBuffType,
    pub buff_exceptions: Vec<i32>,
    /// `_checkTimeUpTask`.
    pub(crate) check_time_up: Option<TimerId>,
    /// `_ejectDeadTasks`.
    pub(crate) eject_dead: HashMap<ObjectId, TimerId>,
}

impl Instance {
    /// Java `new Instance(id)`: the field initialisers and `_instanceStartTime = now`.
    pub(crate) fn new(id: i32, eject_time_ms: i32, now_ms: i64) -> Self {
        Self {
            id,
            template_id: -1,
            eject_time_ms,
            allow_random_walk: true,
            players: BTreeSet::new(),
            npcs: BTreeMap::new(),
            door_templates: Vec::new(),
            doors: BTreeMap::new(),
            spawn_templates: Vec::new(),
            enter_locations: Vec::new(),
            exit_location: None,
            allow_summon: true,
            empty_destroy_time_ms: -1,
            last_left_ms: -1,
            start_time_ms: now_ms,
            end_time_ms: -1,
            pvp: false,
            show_timer: false,
            timer_increase: true,
            timer_text: String::new(),
            reenter_type: InstanceReenterType::None,
            reenter_data: Vec::new(),
            remove_buff_type: InstanceRemoveBuffType::None,
            buff_exceptions: Vec::new(),
            check_time_up: None,
            eject_dead: HashMap::new(),
        }
    }

    /// Java `containsPlayer`.
    pub fn contains_player(&self, id: ObjectId) -> bool {
        self.players.contains(&id)
    }

    /// Java `isRemoveBuffEnabled`.
    pub fn is_remove_buff_enabled(&self) -> bool {
        self.remove_buff_type != InstanceRemoveBuffType::None
    }
}

/// Java `InstanceWorld`: the data a script keeps for an instance it made (who may enter, the
/// status and the parameters).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceWorld {
    /// `getInstanceId()`.
    pub instance_id: i32,
    /// `getTemplateId()` (copied from the instance when the world was made).
    pub template_id: i32,
    /// `_allowed`, in the order they were added.
    pub allowed: Vec<ObjectId>,
    /// `_parameters` (a `StatSet` of strings in Java); `INSTANCE_STATUS` is the status.
    pub parameters: BTreeMap<String, String>,
}

impl InstanceWorld {
    pub fn new(instance_id: i32, template_id: i32) -> Self {
        Self {
            instance_id,
            template_id,
            allowed: Vec::new(),
            parameters: BTreeMap::new(),
        }
    }

    /// Java `addAllowed`.
    pub fn add_allowed(&mut self, player: ObjectId) {
        if !self.allowed.contains(&player) {
            self.allowed.push(player);
        }
    }

    /// Java `removeAllowed`.
    pub fn remove_allowed(&mut self, player: ObjectId) {
        self.allowed.retain(|p| *p != player);
    }

    /// Java `isAllowed`.
    pub fn is_allowed(&self, player: ObjectId) -> bool {
        self.allowed.contains(&player)
    }

    /// Java `getStatus()`: `INSTANCE_STATUS`, 0 when unset.
    pub fn status(&self) -> i32 {
        self.parameters
            .get("INSTANCE_STATUS")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0)
    }

    /// Java `setStatus`.
    pub fn set_status(&mut self, value: i32) {
        self.parameters
            .insert("INSTANCE_STATUS".to_owned(), value.to_string());
    }

    /// Java `incStatus`.
    pub fn inc_status(&mut self) -> i32 {
        let status = self.status() + 1;
        self.set_status(status);
        status
    }
}

/// What a spawn made by an instance carries that `NpcSpawn` has no field for (Java `Spawn.
/// setInstanceId`, `setRandomWalking`, and `Attackable.setOnKillDelay` of the NPC).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct InstanceSpawnInfo {
    pub(crate) instance_id: i32,
    pub(crate) random_walk: bool,
    /// `onKillDelay` (-1: none). Only the kill listeners of scripts read it (D-05).
    pub(crate) on_kill_delay: i32,
}

/// Java `InstanceManager` (a singleton there, the world's here).
#[derive(Debug)]
pub struct InstanceManager {
    /// `INSTANCES`: -1 (the multiverse), 0 (the universe) and every instance made.
    pub(crate) instances: BTreeMap<i32, Instance>,
    /// `_instanceWorlds`.
    pub(crate) worlds: BTreeMap<i32, InstanceWorld>,
    /// `_dynamic`.
    pub(crate) dynamic: i32,
    /// `_instanceIdNames`, `_instanceTemplateNames`, `_instanceTemplates`.
    pub(crate) templates: Arc<InstanceTemplates>,
    /// `_playerTimes` of the characters in the world: template id -> time (ms).
    pub(crate) player_times: HashMap<ObjectId, BTreeMap<i32, i64>>,
    /// The spawns instances made, by spawn index.
    pub(crate) spawns: HashMap<usize, InstanceSpawnInfo>,
    pub(crate) settings: InstanceSettings,
}

impl Default for InstanceManager {
    /// Java's constructor: "Creates the multiverse" (-1) and "the universe" (0).
    fn default() -> Self {
        let settings = InstanceSettings::default();
        let mut instances = BTreeMap::new();
        for id in [-1, 0] {
            instances.insert(id, Instance::new(id, settings.eject_dead_player_time_ms, 0));
        }
        Self {
            instances,
            worlds: BTreeMap::new(),
            dynamic: FIRST_DYNAMIC_INSTANCE_ID,
            templates: Arc::new(InstanceTemplates::default()),
            player_times: HashMap::new(),
            spawns: HashMap::new(),
            settings,
        }
    }
}

impl World {
    /// Installs the instance names and templates (Java `InstanceManager.load`).
    pub fn set_instance_data(&mut self, templates: Arc<InstanceTemplates>) {
        self.instances.templates = templates;
    }

    /// Installs the `General.ini` values of the instances. The multiverse and the universe get
    /// the eject time too (Java makes them after the config is loaded).
    pub fn set_instance_settings(&mut self, settings: InstanceSettings) {
        for id in [-1, 0] {
            if let Some(instance) = self.instances.instances.get_mut(&id) {
                instance.eject_time_ms = settings.eject_dead_player_time_ms;
            }
        }
        self.instances.settings = settings;
    }

    /// The instance clock (Java `System.currentTimeMillis()`), see the module notes.
    pub(crate) fn instance_now_ms(&self) -> i64 {
        match self.instances.settings.clock_override_unix_ms {
            Some(base) => base + self.world_ms(),
            None => unix_ms_now(),
        }
    }

    /// Java `InstanceManager.getInstance(id)`.
    pub fn instance(&self, id: i32) -> Option<&Instance> {
        self.instances.instances.get(&id)
    }

    /// Java `InstanceManager.getInstances()`, in id order.
    pub fn instances(&self) -> impl Iterator<Item = &Instance> {
        self.instances.instances.values()
    }

    /// Java `InstanceManager.getWorld(instanceId)`.
    pub fn instance_world(&self, instance_id: i32) -> Option<&InstanceWorld> {
        self.instances.worlds.get(&instance_id)
    }

    /// The script world of `instance_id`, to change (Java hands out the object).
    pub fn instance_world_mut(&mut self, instance_id: i32) -> Option<&mut InstanceWorld> {
        self.instances.worlds.get_mut(&instance_id)
    }

    /// Java `InstanceManager.addWorld`.
    pub fn add_instance_world(&mut self, world: InstanceWorld) {
        self.instances.worlds.insert(world.instance_id, world);
    }

    /// Java `InstanceManager.getWorld(object)`: the script world of the object's instance.
    pub fn instance_world_of(&self, id: ObjectId) -> Option<&InstanceWorld> {
        self.instances.worlds.get(&self.instance_id_of(id))
    }

    /// Java `InstanceManager.getPlayerWorld(player)`: the first world that allows the player.
    // NOTE(java-quirk): Java walks a ConcurrentHashMap (no defined order); this walks by id.
    pub fn player_instance_world(&self, player: ObjectId) -> Option<&InstanceWorld> {
        self.instances.worlds.values().find(|w| w.is_allowed(player))
    }

    /// Java `InstanceManager.getPlayer(objectId)`: the instance whose player list holds the id,
    /// 0 for none.
    pub fn instance_of_player(&self, player: ObjectId) -> i32 {
        self.instances
            .instances
            .values()
            .find(|i| i.contains_player(player))
            .map_or(0, |i| i.id)
    }

    /// Java `InstanceManager.getInstanceIdName(id)`.
    pub fn instance_name(&self, id: i32) -> String {
        self.instances.templates.name(id).to_owned()
    }

    /// Java `getInstanceId()` of any object (0 for one that is not in the world).
    pub fn instance_id_of(&self, id: ObjectId) -> i32 {
        self.objects.get(&id).map_or(0, |e| e.instance_id)
    }

    /// Java `isInInstance()`.
    pub fn is_in_instance(&self, id: ObjectId) -> bool {
        self.instance_id_of(id) > 0
    }

    /// Java `InstanceManager.getInstance(object.getInstanceId()).isPvP()`.
    pub(crate) fn is_in_pvp_instance(&self, id: ObjectId) -> bool {
        self.instances
            .instances
            .get(&self.instance_id_of(id))
            .is_some_and(|i| i.pvp)
    }

    /// The `Spawn.getRandomWalking()` of an instance spawn, `None` for any other NPC.
    pub(crate) fn instance_spawn_random_walk(&self, spawn: usize) -> Option<bool> {
        self.instances.spawns.get(&spawn).map(|s| s.random_walk)
    }
}

/// The payload of `TimerKind::Instance`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceTimer {
    pub instance_id: i32,
    pub task: InstanceTask,
}

/// Java `Instance`'s tasks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstanceTask {
    /// `CheckTimeUp(remaining)`.
    CheckTimeUp(i32),
    /// `TimeUp`: `destroyInstance(id)`.
    TimeUp,
    /// The `notifyDeath` task: a dead player is put out after the eject time. The exit point is
    /// the instance's when the task was made (the Java lambda holds the instance).
    EjectDead {
        player: ObjectId,
        exit: Option<(i32, i32, i32)>,
    },
}
