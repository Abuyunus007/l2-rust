//! NPCs in the world: spawning what the spawn files describe, `NpcInfo` for the players who see
//! them, and taking them out again. No AI, no respawn, no combat (M4).
// Java: org/l2jmobius/gameserver/data/xml/SpawnData.java (the startup part: counts, logs)
// Java: org/l2jmobius/gameserver/entity/spawns/{Spawn,SpawnGroup,SpawnGroupEntry}.java
// Java: org/l2jmobius/gameserver/managers/DayNightSpawnManager.java
// Java: org/l2jmobius/gameserver/entity/actor/holders/npc/MinionList.java (first spawn only)
// Java: org/l2jmobius/gameserver/entity/actor/Npc.java (constructor, sendInfo, deleteMe)
// Java: org/l2jmobius/gameserver/network/serverpackets/AbstractNpcInfo.java (NpcInfo)
//
// What is read where: the spawn *description* (`SpawnData`: fixed spawns, groups, territories)
// and the NPC templates come from `l2-data` and are handed over with `World::set_npc_data`;
// the loader already did Java's file parsing. This module does what Java does when the loaded
// spawns are started: roll the groups, pick territory points with the world RNG, correct Z with
// the geodata, create the NPCs and put them into the regions.
//
// Not done here, with the reason:
// - respawn after death, `Spawn.decreaseCount`, group replacement rolls, `RespawnTaskManager`:
//   `respawn.rs` (M4-DEATH-01). This module creates the NPCs; `respawn.rs` counts them per spawn.
// - AI, walking routes (`WalkingManager`), `NpcPersonalAIData` (spawn `AIData`), champions
//   (`ChampionMonstersConfig`, off by default): M4.
// - Raid and grand bosses: Java's `RaidBossSpawnManager` / `GrandBossManager` spawn them from
//   their own tables, the spawn files skip them (`SpawnData.checkTemplate`).
// - the day/night switch while the server runs: `World::start_game_clock` (world.rs) calls
//   `set_night` when the game hour crosses 6 or 0.
// - castle owner crests in `NpcInfo`: castles are not loaded, so nobody owns one.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use l2_data::spawn::{NpcSpawn, PeriodOfDay, SpawnSelection};
use l2_data::{NpcTemplate, NpcTemplates, SpawnData};
use l2_net::Encode;
use l2_protocol_game::server::NpcInfo;
use l2_util::ObjectId;
use tracing::{debug, info, warn};

use crate::ai::idle::AnimationState;
use crate::ids::IdFactory;
use crate::objects::{Npc, ObjectKind};
use crate::player::CreatureCore;
use crate::world::{World, game_time_minutes, unix_ms_now};

/// Which `entity/actor/instance/*` class a template type names, as far as it changes what the
/// world does with the NPC (D-17: a kind enum instead of the class tree).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NpcClass {
    /// Folk, merchants, teachers, guards, gatekeepers...: not attackable by a player.
    Npc,
    /// `Monster` and the subclasses that add nothing visible: `Chest`, `EventMonster`,
    /// `FeedableBeast`, `RiftInvader`, `FestivalMonster`. Also what every minion is.
    Monster,
    /// `RaidBoss` (`isRaid()`).
    RaidBoss,
    /// `GrandBoss` (`isRaid()`).
    GrandBoss,
    /// `Block` (Block Checker): attackable only for players inside an arena.
    Block,
    /// `FriendlyMob`: attackable only for players with karma.
    FriendlyMob,
}

impl NpcClass {
    /// The class Java instantiates for a template type (`Class.forName("...instance." + type)`
    /// then `getConstructor(NpcTemplate.class)`). `None` when Java's `Spawn` constructor throws:
    /// there is no such class, or its constructor takes more than the template (pets,
    /// servitors, decoys, traps, effect points, tamed beasts, UC towers).
    // NOTE(java-quirk): that makes `Spawn.doSpawn`'s own Pet/Decoy/Trap check dead code; such
    // spawns are never created or registered, and never counted.
    pub fn from_type(npc_type: &str) -> Option<Self> {
        Some(match npc_type {
            "Monster" | "Chest" | "EventMonster" | "FeedableBeast" | "RiftInvader" | "FestivalMonster" => {
                Self::Monster
            }
            "RaidBoss" => Self::RaidBoss,
            "GrandBoss" => Self::GrandBoss,
            "Block" => Self::Block,
            "FriendlyMob" => Self::FriendlyMob,
            "Adventurer"
            | "Artefact"
            | "Auctioneer"
            | "BroadcastingTower"
            | "CastleDoorman"
            | "ClanHallDoorman"
            | "ClanHallManager"
            | "ControlTower"
            | "DawnPriest"
            | "Defender"
            | "Doorman"
            | "DungeonGatekeeper"
            | "DuskPriest"
            | "FestivalGuide"
            | "Fisherman"
            | "FlameTower"
            | "FlyTerrainObject"
            | "Folk"
            | "FortCommander"
            | "FortDoorman"
            | "FortLogistics"
            | "FortManager"
            | "Guard"
            | "KrateisCubeManager"
            | "KrateisMatchManager"
            | "Merchant"
            | "OlympiadManager"
            | "PetManager"
            | "QuestGuard"
            | "RaceManager"
            | "SchemeBuffer"
            | "SignsPriest"
            | "Teleporter"
            | "TerrainObject"
            | "TerritoryWard"
            | "Trainer"
            | "UCHelper"
            | "UCManager"
            | "VillageMaster"
            | "VillageMasterDElf"
            | "VillageMasterDwarf"
            | "VillageMasterFighter"
            | "VillageMasterKamael"
            | "VillageMasterMystic"
            | "VillageMasterOrc"
            | "VillageMasterPriest"
            | "Warehouse" => Self::Npc,
            _ => return None,
        })
    }

    /// Java `isMonster()`.
    pub fn is_monster(self) -> bool {
        matches!(
            self,
            Self::Monster | Self::RaidBoss | Self::GrandBoss | Self::Block
        )
    }

    /// Java `isRaid()`.
    pub fn is_raid(self) -> bool {
        matches!(self, Self::RaidBoss | Self::GrandBoss)
    }

    /// Java `isAutoAttackable(player)` for a player outside any siege, arena or faction war.
    // Siege NPCs (`Defender`, `FortCommander`, `TerritoryWard`) are only attackable while a siege
    // runs, `Guard` only with the faction system on: all false here until sieges exist.
    pub(crate) fn auto_attackable_by(self, viewer_karma: i32) -> bool {
        match self {
            Self::Monster | Self::RaidBoss | Self::GrandBoss => true,
            Self::FriendlyMob => viewer_karma > 0,
            Self::Block | Self::Npc => false,
        }
    }
}

/// The configuration values spawning and `NpcInfo` read (Java `NpcConfig`,
/// `RandomSpawnsConfig`). `Default` is the Java default of each key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NpcSettings {
    /// `NpcConfig.SHOW_NPC_LEVEL` (`ShowNpcLevel`).
    pub show_npc_level: bool,
    /// `NpcConfig.SHOW_NPC_AGGRESSION` (`ShowNpcAggression`).
    pub show_npc_aggression: bool,
    /// `NpcConfig.ENABLE_RANDOM_ENCHANT_EFFECT` (`EnableRandomEnchantEffect`).
    pub enable_random_enchant_effect: bool,
    /// `RandomSpawnsConfig.ENABLE_RANDOM_MONSTER_SPAWNS` (`EnableRandomMonsterSpawns`).
    pub random_monster_spawns: bool,
    /// `RandomSpawnsConfig.MOB_MAX_SPAWN_RANGE` (`MaxSpawnMobRange`); the minimum is its negative.
    pub mob_max_spawn_range: i32,
    /// `RandomSpawnsConfig.MOBS_LIST_NOT_RANDOM` (`MobsSpawnNotRandom`).
    pub mobs_not_random: HashSet<i32>,
    /// `GeneralConfig.MIN_NPC_ANIMATION` (`MinNpcAnimation`): the shortest wait, in seconds,
    /// before an NPC's random animation check.
    pub min_npc_animation: i32,
    /// `GeneralConfig.MAX_NPC_ANIMATION` (`MaxNpcAnimation`): the longest wait; 0 turns the random
    /// animation of NPCs off.
    pub max_npc_animation: i32,
    /// `GeneralConfig.MIN_MONSTER_ANIMATION` (`MinMonsterAnimation`).
    pub min_monster_animation: i32,
    /// `GeneralConfig.MAX_MONSTER_ANIMATION` (`MaxMonsterAnimation`); 0 turns it off for attackables.
    pub max_monster_animation: i32,
}

impl Default for NpcSettings {
    fn default() -> Self {
        Self {
            show_npc_level: false,
            show_npc_aggression: false,
            enable_random_enchant_effect: false,
            random_monster_spawns: false,
            mob_max_spawn_range: 150,
            mobs_not_random: HashSet::new(),
            min_npc_animation: 5,
            max_npc_animation: 60,
            min_monster_animation: 5,
            max_monster_animation: 60,
        }
    }
}

/// `ZoneManager.getZone(x, y, z, WaterZone.class) != null`. The world has no zones yet (M3 zone
/// work), so the game server hands it this over the datapack's zones.
pub type WaterCheck = Arc<dyn Fn(i32, i32, i32) -> bool + Send + Sync>;

/// Everything the NPC code keeps in the world besides the NPCs themselves.
#[derive(Default)]
pub(crate) struct NpcWorld {
    templates: Option<Arc<NpcTemplates>>,
    spawns: Option<Arc<SpawnData>>,
    pub(crate) settings: NpcSettings,
    water: Option<WaterCheck>,
    pub(crate) ids: Option<Arc<IdFactory>>,
    /// Java `DayNightSpawnManager._dayCreatures` / `_nightCreatures`: indices into the spawns.
    day: Vec<usize>,
    night: Vec<usize>,
    /// Java `RandomAnimationTaskManager`'s entries and `Npc._lastSocialBroadcast`, see `ai/idle.rs`.
    pub(crate) animations: HashMap<ObjectId, AnimationState>,
}

impl NpcWorld {
    /// The template of an NPC id, if the templates are loaded and know it.
    pub(crate) fn template(&self, npc_id: i32) -> Option<&NpcTemplate> {
        self.templates.as_ref()?.get(npc_id)
    }

    /// The spawn list (`SpawnData`), if one was handed over.
    pub(crate) fn spawn_data(&self) -> Option<&Arc<SpawnData>> {
        self.spawns.as_ref()
    }

    /// Java `NpcData.getTemplateByName(name)`.
    pub(crate) fn template_by_name(&self, name: &str) -> Option<&NpcTemplate> {
        self.templates.as_ref()?.get_by_name(name)
    }

    /// Java `NpcData.getGenericClanId()`.
    pub(crate) fn generic_clan_id(&self) -> i32 {
        self.templates.as_ref().map_or(0, |t| t.generic_clan_id())
    }
}

/// What [`World::spawn_all_npcs`] did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SpawnReport {
    /// Java `SpawnData._spawnCount`: the number in "N spawns have been initialized!". Fixed
    /// spawns that were created, units of groups that were created, and one for every day or
    /// night spawn (created or not).
    pub spawn_count: usize,
    /// Fixed spawns created at once (not day/night).
    pub fixed: usize,
    /// Units created by spawn groups.
    pub grouped: usize,
    /// Day and night spawns declared.
    pub day_night: usize,
    /// Day or night NPCs created by the start state.
    pub day_night_created: usize,
    /// NPC objects in the world afterwards, minions included.
    pub npcs: usize,
    /// Minions created for master monsters.
    pub minions: usize,
    /// Spawns Java would have dropped because the template type has no usable class.
    pub skipped_type: usize,
    /// Spawns that could not be placed (no location, territory without a shape, out of ids).
    pub failed: usize,
}

impl World {
    /// Hands the world the NPC templates and the spawn list. Call before [`Self::spawn_all_npcs`].
    pub fn set_npc_data(&mut self, templates: Arc<NpcTemplates>, spawns: Arc<SpawnData>) {
        self.npc_world.templates = Some(templates);
        self.npc_world.spawns = Some(spawns);
    }

    /// Installs the configuration values NPCs read.
    pub fn set_npc_settings(&mut self, settings: NpcSettings) {
        self.npc_world.settings = settings;
    }

    /// Installs the water zone lookup (NPCs in water use swim speeds and spawn without Z
    /// correction). Without it nothing is in water.
    pub fn set_water_check(&mut self, check: WaterCheck) {
        self.npc_world.water = Some(check);
    }

    /// The id factory the NPCs take their object ids from (the game server's `state.ids`).
    pub fn set_id_factory(&mut self, ids: Arc<IdFactory>) {
        self.npc_world.ids = Some(ids);
    }

    /// An NPC by object id.
    pub fn npc(&self, id: ObjectId) -> Option<&Npc> {
        self.npcs.get(&id)
    }

    /// An NPC by object id, to change it.
    pub fn npc_mut(&mut self, id: ObjectId) -> Option<&mut Npc> {
        self.npcs.get_mut(&id)
    }

    /// The object ids of all NPCs, in ascending order.
    pub fn npc_ids(&self) -> Vec<ObjectId> {
        let mut ids: Vec<ObjectId> = self.npcs.keys().copied().collect();
        ids.sort_unstable_by_key(|id| id.0);
        ids
    }

    /// Number of NPCs in the world.
    pub fn npc_count(&self) -> usize {
        self.npcs.len()
    }

    /// Java `SpawnData.load()` after the files are parsed: starts every fixed spawn, rolls and
    /// spawns the groups, registers the day and night spawns and applies the current period
    /// (`DayNightSpawnManager.notifyChangeMode()` from `GameServer`). `night` is
    /// `GameTimeTaskManager.isNight()`.
    ///
    /// Logs Java's "N spawns have been initialized!" line. Does nothing without
    /// [`Self::set_npc_data`] (`NoSpawns`).
    pub fn spawn_all_npcs(&mut self, night: bool) -> SpawnReport {
        let mut report = SpawnReport::default();
        let (Some(spawns), Some(_)) = (self.npc_world.spawns.clone(), self.npc_world.templates.clone())
        else {
            return report;
        };

        // Fixed spawns are started while Java parses the files, in file order...
        for (index, spawn) in spawns.spawns().iter().enumerate() {
            if spawn.group.is_some() {
                continue;
            }
            if !self.spawn_is_creatable(spawn, &mut report) {
                continue;
            }
            match spawn.period_of_day {
                None => {
                    // Spawn.init(): `while (_currentCount < _maximumCount) doSpawn()`.
                    let mut current = 0;
                    while current < spawn.amount {
                        if self.do_spawn(index, &mut report).is_none() {
                            break;
                        }
                        current += 1;
                        report.fixed += 1;
                        report.spawn_count += 1;
                    }
                }
                Some(period) => {
                    match period {
                        PeriodOfDay::Day => self.npc_world.day.push(index),
                        PeriodOfDay::Night => self.npc_world.night.push(index),
                    }
                    report.day_night += 1;
                    report.spawn_count += 1;
                }
            }
        }

        // ...the groups once all files are in.
        for group in 0..spawns.groups().len() {
            let spawned = self.spawn_group_initially(group, &mut report);
            report.grouped += spawned;
            report.spawn_count += spawned;
        }

        if report.skipped_type > 0 {
            warn!(
                "SpawnData: {} spawn(s) skipped: the template type has no class that Spawn can build from a template (Pet, Servitor, Decoy, ...).",
                report.skipped_type
            );
        }
        if report.failed > 0 {
            warn!("SpawnData: {} spawn(s) could not be placed.", report.failed);
        }
        info!("SpawnData: {} spawns have been initialized!", report.spawn_count);

        // GameServer: DayNightSpawnManager.getInstance().trim().notifyChangeMode()
        report.day_night_created = self.apply_day_night(night, &mut report);
        report.npcs = self.npcs.len();
        report
    }

    /// `DayNightSpawnManager.notifyChangeMode()`: the day (or night) spawns appear, the others go.
    /// Returns how many NPCs were created. The game clock calls this when the period changes.
    pub fn set_night(&mut self, night: bool) -> usize {
        self.apply_day_night(night, &mut SpawnReport::default())
    }

    /// Whether it is night now: Java `GameTimeTaskManager._isNight`, game hours 0 to 5.
    // NOTE: the game clock counts from UTC midnight here (see `game_time_minutes`).
    pub fn is_night_now() -> bool {
        game_time_minutes(unix_ms_now()) < 6 * 60
    }

    fn apply_day_night(&mut self, night: bool, report: &mut SpawnReport) -> usize {
        if self.npc_world.day.is_empty() && self.npc_world.night.is_empty() {
            return 0;
        }
        let (unspawn, spawn, unspawn_name, spawn_name) = if night {
            (
                self.npc_world.day.clone(),
                self.npc_world.night.clone(),
                "day",
                "night",
            )
        } else {
            (
                self.npc_world.night.clone(),
                self.npc_world.day.clone(),
                "night",
                "day",
            )
        };
        if !unspawn.is_empty() {
            let mut removed = 0;
            for index in unspawn {
                // `spawn.stopRespawn()` first, so the NPC that goes does not come back.
                self.stop_respawn(index);
                if let Some(last) = self.last_spawn(index) {
                    self.delete_npc(last);
                    removed += 1;
                }
            }
            info!("DayNightSpawnManager: Removed {removed} {unspawn_name} creatures");
        }
        let mut created = 0;
        for index in &spawn {
            // `spawnDat.startRespawn(); spawnDat.doSpawn()`.
            self.start_respawn(*index);
            if self.do_spawn(*index, report).is_some() {
                created += 1;
            }
        }
        // Java counts the calls to doSpawn, not the NPCs that came out.
        info!(
            "DayNightSpawnManager: Spawned {} {spawn_name} creatures",
            spawn.len()
        );
        created
    }

    /// Java's `new Spawn(npcId)` can fail (no class): then the spawn does not exist.
    fn spawn_is_creatable(&self, spawn: &NpcSpawn, report: &mut SpawnReport) -> bool {
        let Some(templates) = &self.npc_world.templates else {
            return false;
        };
        let Some(template) = templates.get(spawn.npc_id) else {
            return false;
        };
        if NpcClass::from_type(&template.npc_type).is_none() {
            debug!(
                "Spawn could not be initialized: NPC {} class not found",
                spawn.npc_id
            );
            report.skipped_type += 1;
            return false;
        }
        true
    }

    /// Java `SpawnGroup.initialSpawn()`.
    fn spawn_group_initially(&mut self, group_index: usize, report: &mut SpawnReport) -> usize {
        let Some(spawns) = self.npc_world.spawns.clone() else {
            return 0;
        };
        let Some(group) = spawns.groups().get(group_index) else {
            return 0;
        };
        if group.entries.is_empty() {
            // The loader already warned ("has no valid entries"), as Java does here.
            return 0;
        }
        // The slots of an entry whose class Java cannot build do not exist.
        let entries: Vec<Vec<usize>> = group
            .entries
            .iter()
            .map(|entry| {
                entry
                    .slots
                    .iter()
                    .copied()
                    .filter(|slot| {
                        spawns
                            .spawns()
                            .get(*slot)
                            .is_some_and(|s| self.spawn_is_creatable(s, report))
                    })
                    .collect()
            })
            .collect();
        let mut state = GroupState {
            selection: group.selection,
            maximum: i64::from(group.maximum_npc),
            alive: 0,
            entry_alive: vec![0; group.entries.len()],
            entry_total: group.entries.iter().map(|e| i64::from(e.total)).collect(),
            slots: entries.clone(),
        };
        let mut spawned = 0;
        match group.selection {
            SpawnSelection::All => {
                for (i, entry) in group.entries.iter().enumerate() {
                    if state.try_reserve(i, i64::from(entry.total)) {
                        spawned += self.spawn_units(&mut state, &entries[i], i, entry.total, report);
                    }
                }
            }
            SpawnSelection::Random => {
                // A single roll over the entry index, the whole quota of the rolled entry spawns.
                let i = self.entry_roll(group.entries.len());
                let total = group.entries[i].total;
                if state.try_reserve(i, i64::from(total)) {
                    spawned += self.spawn_units(&mut state, &entries[i], i, total, report);
                }
            }
            SpawnSelection::RandomFill => {
                // The cap is the roll counter; rolls use replacement, a roll into a full entry is lost.
                for _ in 0..group.maximum_npc {
                    let i = self.entry_roll(group.entries.len());
                    if state.try_reserve(i, 1) {
                        spawned += self.spawn_units(&mut state, &entries[i], i, 1, report);
                    }
                }
            }
        }
        // The census goes on living: a unit that dies is replaced through it (`respawn.rs`).
        self.death.groups.insert(group_index, state);
        spawned
    }

    pub(crate) fn entry_roll(&mut self, len: usize) -> usize {
        let bound = i32::try_from(len).unwrap_or(i32::MAX);
        usize::try_from(self.rng().below(bound)).unwrap_or(0)
    }

    /// Java `SpawnGroup.spawnUnits`: a territory slot is rolled per unit.
    pub(crate) fn spawn_units(
        &mut self,
        state: &mut GroupState,
        slots: &[usize],
        entry: usize,
        count: i32,
        report: &mut SpawnReport,
    ) -> usize {
        let mut spawned = 0;
        for _ in 0..count {
            let slot = slots.get(self.entry_roll(slots.len())).copied();
            if slot.is_some_and(|slot| self.do_spawn(slot, report).is_some()) {
                spawned += 1;
            } else {
                state.release(entry, 1);
            }
        }
        spawned
    }

    /// Java `Spawn.doSpawn(false)` -> `initializeNpc`: creates one NPC for spawn `index`.
    pub(crate) fn do_spawn(&mut self, index: usize, report: &mut SpawnReport) -> Option<ObjectId> {
        self.initialize_npc(index, None, report)
    }

    /// Java `Spawn.initializeNpc(npc)`. `reuse` is `Spawn.respawnNpc(oldNpc)`: the NPC that died
    /// comes back with its object id (Java re-initialises the same object) and keeps the weapon
    /// enchant effect its constructor rolled.
    pub(crate) fn initialize_npc(
        &mut self,
        index: usize,
        reuse: Option<(ObjectId, i32)>,
        report: &mut SpawnReport,
    ) -> Option<ObjectId> {
        let spawns = self.npc_world.spawns.clone()?;
        let templates = self.npc_world.templates.clone()?;
        // A copy: the runtime spawns live in `self`, and the rest of this function borrows it.
        let spawn = self.spawn_def(index)?.clone();
        let template = templates.get(spawn.npc_id)?;
        let class = NpcClass::from_type(&template.npc_type)?;
        let settings = self.npc_world.settings.clone();

        // `new Npc(template)`: the random weapon enchant is drawn before the position. A
        // respawn does not run the constructor again.
        let enchant = match reuse {
            Some((_, enchant)) => enchant,
            None if settings.enable_random_enchant_effect => self.rng().get(4, 21),
            None => template.weapon_enchant,
        };

        let geo = self.geo.clone();
        let height = move |x: i32, y: i32, z: i32| geo.as_ref().map_or(z, |g| g.height(x, y, z));

        let (mut x, mut y, mut z);
        let mut heading = spawn.heading;
        let mut territory_high_z = None;
        if let Some(name) = &spawn.territory {
            let territory = spawns.territory(name)?;
            // None where Java's getRandomPoint throws; doSpawn catches it and returns null.
            let Some(point) = territory.random_point(self.rng(), &height) else {
                warn!(
                    "NPC {} class not found: its spawn territory {name} has no usable shape.",
                    spawn.npc_id
                );
                report.failed += 1;
                return None;
            };
            (x, y, z) = (point.x, point.y, point.z);
            // NOTE(java-quirk): a territory spawn forgets the heading of the XML (setHeading(-1)).
            heading = -1;
            territory_high_z = territory.shape().map(l2_data::spawn::TerritoryShape::high_z);
        } else if spawn.location.x == 0 && spawn.location.y == 0 {
            warn!("NPC {} doesn't have spawn location!", spawn.npc_id);
            report.failed += 1;
            return None;
        } else {
            (x, y, z) = (spawn.location.x, spawn.location.y, spawn.location.z);
        }

        let mut in_water = self.npc_world.water.as_ref().is_some_and(|f| f(x, y, z));

        // Random monster spawns (custom RandomSpawns.ini): a monster with a fixed heading
        // spawns up to MaxSpawnMobRange away, if it can walk and see there.
        // WalkingManager.isTargeted (walker routes) and instances: M4.
        if settings.random_monster_spawns
            && heading != -1
            && class.is_monster()
            && !template.quest_monster
            && !template.undying
            && !class.is_raid()
            && !template.flying
            && !in_water
            && !settings.mobs_not_random.contains(&template.id)
        {
            let range = settings.mob_max_spawn_range;
            let rand_x = x.wrapping_add(self.rng().get(-range, range));
            let rand_y = y.wrapping_add(self.rng().get(-range, range));
            let ok = self.geo.as_ref().is_none_or(|g| {
                let from = l2_geo::Location { x, y, z };
                let to = l2_geo::Location {
                    x: rand_x,
                    y: rand_y,
                    z,
                };
                g.can_move_to_target(from, to, 0) && g.can_see_target_in_instance(from, to, 0)
            });
            if ok {
                x = rand_x;
                y = rand_y;
                heading = -1;
                in_water = self.npc_world.water.as_ref().is_some_and(|f| f(x, y, z));
            }
        }

        // Correct Z of monsters.
        if class.is_monster() && !template.flying && !in_water {
            let geo_z = height(x, y, z);
            let dz = f64::from(z) - f64::from(geo_z);
            // NOTE(java-quirk): Java passes `squared = true` to calculateDistance, so the "300"
            // is a squared distance: Z is only corrected when it is less than ~17 off.
            if dz * dz < 300.0 {
                z = geo_z;
            }
            // Prevent new z exceeding spawn territory high z.
            if let Some(high_z) = territory_high_z
                && high_z < z
            {
                z = high_z;
            }
        }

        let heading = if heading == -1 {
            self.rng().below(61794)
        } else {
            heading
        };

        let id = self.create_npc_with_id(
            reuse.map(|(id, _)| id),
            template,
            class,
            Some(index),
            None,
            (x, y, z),
            heading,
            in_water,
            enchant,
        )?;
        // `_spawnedNpcs.add(npc)` and `_currentCount++`.
        self.spawn_registered(index, id);
        // A respawn is the same object, which keeps its random animation entry (`ai/idle.rs`).
        if reuse.is_some() {
            self.resume_random_animation(id);
        }

        // Minions of master monsters.
        if class.is_monster() && templates.master_monster_ids().contains(&template.id) {
            self.spawn_minions(id, template, &templates, report);
        }
        Some(id)
    }

    /// Java `MinionList.spawnMinions(template.getParameters().getMinionList("Privates"))` for a
    /// master that has none yet.
    fn spawn_minions(
        &mut self,
        master: ObjectId,
        master_template: &NpcTemplate,
        templates: &NpcTemplates,
        report: &mut SpawnReport,
    ) {
        // `MinionList.countSpawnedMinionsById`: a master that comes back from its death (respawn)
        // still has the minions that survived it, so only the missing ones are made.
        // Through the master -> minions index, not a scan of every NPC (see `delete_npc`).
        let mut spawned_by_id: HashMap<i32, i32> = HashMap::new();
        for &minion in self.minions_of(master) {
            if let Some(npc) = self.npcs.get(&minion)
                && npc.leader == Some(master)
                && !self.is_dead(minion)
            {
                *spawned_by_id.entry(npc.npc_id).or_insert(0) += 1;
            }
        }
        for holder in master_template.minion_list("Privates") {
            let already = spawned_by_id.get(&holder.id).copied().unwrap_or(0);
            for _ in 0..(holder.count - already).max(0) {
                if holder.id == 0 {
                    continue;
                }
                let Some(minion_template) = templates.get(holder.id) else {
                    continue;
                };
                if self
                    .spawn_minion(master, master_template, minion_template)
                    .is_some()
                {
                    *spawned_by_id.entry(holder.id).or_insert(0) += 1;
                    report.minions += 1;
                }
            }
        }
    }

    /// Java `MinionList.spawnMinion` / `initializeNpc`: a `Monster` near its master.
    fn spawn_minion(
        &mut self,
        master: ObjectId,
        master_template: &NpcTemplate,
        template: &NpcTemplate,
    ) -> Option<ObjectId> {
        const SPAWN_OFFSET_RADIUS: i32 = 200;
        const MINIMUM_SPAWN_RADIUS: i32 = 30;
        let (mx, my, mz, heading) = {
            let m = self.npcs.get(&master)?;
            (m.core.x, m.core.y, m.core.z, m.core.heading)
        };
        let spawn_radius = SPAWN_OFFSET_RADIUS;
        // `(int) master.getCollisionRadius()`: the master's current radius (`Grow` changes it).
        let master_radius = self.npcs.get(&master).map_or_else(
            || f64::from(master_template.collision_radius),
            |m| m.collision_radius,
        );
        let minimum_radius = master_radius as i32 + MINIMUM_SPAWN_RADIUS;
        let mut new_x = self.rng().get(minimum_radius * 2, spawn_radius * 2);
        let mut new_y = self.rng().get(new_x, spawn_radius * 2);
        new_y = (f64::from(new_y * new_y - new_x * new_x)).sqrt() as i32;
        new_x = if new_x > spawn_radius + minimum_radius {
            (mx + new_x) - spawn_radius
        } else {
            (mx - new_x) + minimum_radius
        };
        new_y = if new_y > spawn_radius + minimum_radius {
            (my + new_y) - spawn_radius
        } else {
            (my - new_y) + minimum_radius
        };
        let in_water = self.npc_world.water.as_ref().is_some_and(|f| f(new_x, new_y, mz));
        let enchant = if self.npc_world.settings.enable_random_enchant_effect {
            self.rng().get(4, 21)
        } else {
            template.weapon_enchant
        };
        self.create_npc(
            template,
            NpcClass::Monster,
            None,
            Some(master),
            (new_x, new_y, mz),
            heading,
            in_water,
            enchant,
        )
    }

    /// The common end of every spawn: a new object id, full hit points, into the world.
    #[allow(clippy::too_many_arguments)]
    fn create_npc(
        &mut self,
        template: &NpcTemplate,
        class: NpcClass,
        spawn: Option<usize>,
        leader: Option<ObjectId>,
        position: (i32, i32, i32),
        heading: i32,
        in_water: bool,
        enchant: i32,
    ) -> Option<ObjectId> {
        self.create_npc_with_id(
            None, template, class, spawn, leader, position, heading, in_water, enchant,
        )
    }

    /// [`Self::create_npc`] with the object id of an NPC that died and comes back (`reuse`), as
    /// Java's `respawnNpc` re-initialises the same `Npc` object.
    #[allow(clippy::too_many_arguments)]
    fn create_npc_with_id(
        &mut self,
        reuse: Option<ObjectId>,
        template: &NpcTemplate,
        class: NpcClass,
        spawn: Option<usize>,
        leader: Option<ObjectId>,
        position: (i32, i32, i32),
        heading: i32,
        in_water: bool,
        enchant: i32,
    ) -> Option<ObjectId> {
        let ids = self.npc_world.ids.clone();
        let Some(ids) = ids else {
            warn!(
                "Cannot spawn NPC {}: the world has no object id factory.",
                template.id
            );
            return None;
        };
        let id = match reuse {
            Some(id) => id,
            None => match ids.allocate() {
                Ok(id) => id,
                Err(e) => {
                    warn!("Cannot spawn NPC {}: {e}", template.id);
                    return None;
                }
            },
        };
        let (x, y, z) = position;
        // Java `Spawn.getLocation()`: the XML location, or for a territory spawn the point rolled
        // for the last NPC created (here: this NPC's own point).
        let spawn_point = spawn.and_then(|index| {
            let data = self.spawn_def(index)?;
            Some(if data.territory.is_some() {
                position
            } else {
                (data.location.x, data.location.y, data.location.z)
            })
        });
        let npc = Npc {
            core: CreatureCore {
                object_id: id,
                x,
                y,
                z,
                heading,
                level: i32::from(template.level),
                // Set from the stat engine right below (`init_npc_stats`).
                cur_hp: 0.0,
                max_hp: 0.0,
                cur_mp: 0.0,
                max_mp: 0.0,
                cur_cp: 0.0,
                max_cp: 0.0,
                // Java: `_calculators = NPC_STD_CALCULATOR`.
                calculators: self.npc_calculators(),
                skills: Vec::new(),
            },
            npc_id: template.id,
            class,
            spawn,
            leader,
            decayed: false,
            in_water,
            right_hand: template.rhand_id,
            left_hand: template.lhand_id,
            enchant_effect: enchant,
            display_effect: 0,
            collision_radius: f64::from(template.collision_radius),
            must_reward_exp_sp: true,
            special_drop: false,
            movement: crate::movement::NpcMovement::default(),
            spawn_point: spawn_point.unwrap_or_default(),
            spawn_time_ms: 0,
        };
        self.npcs.insert(id, npc);
        // Java `Attackable.onSpawn`: "Clear mob spoil, seed" and `_sweepItems.set(null)`. A respawn
        // reuses the object id, so the spoil table must not remember the last life.
        self.death.spoil.remove(&id);
        // A new Java object has a new effect list (a respawn re-creates the NPC with its old id).
        self.forget_effects(id);
        // Java's constructor: the template's skills (passive effects copy the calculators).
        self.add_npc_template_skills(id);
        // Java `MinionList.onMinionSpawn`: the minion joins its master's list.
        if let Some(master) = leader {
            self.register_minion(master, id);
        }
        // Java `onSpawn`: full HP and MP (`getMaxHp()`: the template value through the standard
        // NPC calculators, so times the CON / MEN bonus).
        self.init_npc_stats(id);
        self.register_object(id, ObjectKind::Npc);
        self.spawn_at(id, x, y, z);
        if !self.npcs.contains_key(&id) {
            // Out of the world: `spawn` removed it again.
            ids.release(id);
            return None;
        }
        Some(id)
    }

    /// A new NPC of template `npc_id` at an exact place, the way an admin spawn command makes
    /// one (`Spawn` with fixed coordinates and heading): no territory, no Z correction, no
    /// minions. `None` for an unknown template, a type Java cannot build, or no free object id.
    pub fn spawn_npc_at(&mut self, npc_id: i32, x: i32, y: i32, z: i32, heading: i32) -> Option<ObjectId> {
        let templates = self.npc_world.templates.clone()?;
        let template = templates.get(npc_id)?;
        let class = NpcClass::from_type(&template.npc_type)?;
        let in_water = self.npc_world.water.as_ref().is_some_and(|f| f(x, y, z));
        let enchant = if self.npc_world.settings.enable_random_enchant_effect {
            self.rng().get(4, 21)
        } else {
            template.weapon_enchant
        };
        self.create_npc(template, class, None, None, (x, y, z), heading, in_water, enchant)
    }

    /// Java `Npc.deleteMe()`: the NPC goes out of view for everybody and out of the world. Its
    /// minions go with it (`Monster.deleteMe` -> `MinionList.onMasterDie(true)`). Like Java it
    /// is `onDecay()`: the spawn counts one NPC less and, unless the spawn was told to stop
    /// (`Spawn.stopRespawn`), the NPC comes back after the respawn delay (see `respawn.rs`).
    ///
    /// The object id is not given back: Java's NPCs keep theirs for their next life.
    pub fn delete_npc(&mut self, id: ObjectId) -> bool {
        if !self.npcs.contains_key(&id) {
            return false;
        }
        // The master -> minions index (`MinionList`), not a scan of every NPC: the day/night
        // switch deletes hundreds of NPCs in one tick, and a scan made each delete cost ~0.5 ms.
        let minions: Vec<ObjectId> = self.minions_of(id).to_vec();
        for minion in minions {
            self.delete_npc(minion);
        }
        // A minion that is deleted tells its master (`onMinionDie(this, 0)`): no respawn of it,
        // and the master's list is derived from `leader`, so nothing to do.
        self.on_npc_decay(id);
        true
    }

    /// Java `Npc.sendInfo(player)`.
    pub(crate) fn send_npc_info(&mut self, subject: ObjectId, viewer: ObjectId) {
        if !self.is_visible_for(subject, viewer) {
            return;
        }
        if let Some(info) = self.npc_info(subject, viewer) {
            self.send(viewer, &info);
        }
    }

    /// `new AbstractNpcInfo.NpcInfo(npc, viewer)`: what `viewer` is told when `subject` comes
    /// into view. `None` for a decayed NPC (Java writes nothing), an unknown template and a fake
    /// player (`FakePlayerInfo`, not ported).
    // Java: Npc.sendInfo sends ServerObjectInfo instead when getRunSpeed() == 0; the template
    // loader never lets a speed reach 0, so that packet is not ported.
    pub fn npc_info(&self, subject: ObjectId, viewer: ObjectId) -> Option<NpcInfo> {
        let npc = self.npcs.get(&subject)?;
        if npc.decayed {
            return None;
        }
        let template = self.npc_world.templates.as_ref()?.get(npc.npc_id)?;
        if template.fake_player {
            return None;
        }
        let settings = &self.npc_world.settings;
        let stats = self.stats(subject)?;
        // `getMovementSpeedMultiplier()` and the speeds divided by it, `(int) Math.round(...)`.
        let move_multiplier = stats.movement_speed_multiplier();
        let speed = |value: f64| crate::stats::java_round(value / move_multiplier) as i32;
        let (run_speed, walk_speed) = (speed(stats.run_speed()), speed(stats.walk_speed()));
        let viewer_karma = self.players.get(&viewer).map_or(0, |p| p.data.karma);

        let name = if template.using_server_side_name {
            template.name.clone()
        } else {
            String::new()
        };
        // Creature._title of an NPC is never set, so it is null (written as empty) unless the
        // template says to send its own.
        let mut title = if template.using_server_side_title {
            template.title.clone()
        } else {
            String::new()
        };
        if npc.class.is_monster() && (settings.show_npc_level || settings.show_npc_aggression) {
            let mut t1 = String::new();
            if settings.show_npc_level {
                t1 += &format!("Lv {}", npc.core.level);
            }
            let mut t2 = String::new();
            if settings.show_npc_aggression {
                if !t1.is_empty() {
                    t2 += " ";
                }
                if template.is_aggressive {
                    t2 += "[A]";
                }
                if !template.clans.is_empty() && template.clan_help_range > 0 {
                    t2 += "[G]";
                }
            }
            t1 += &t2;
            if !title.is_empty() {
                t1 += " ";
                t1 += &title;
            }
            title = t1;
        }

        Some(NpcInfo {
            object_id: subject.0,
            display_id: template.display_id,
            attackable: npc.class.auto_attackable_by(viewer_karma),
            x: npc.core.x,
            y: npc.core.y,
            z: npc.core.z,
            heading: npc.core.heading,
            m_atk_spd: stats.m_atk_spd(),
            p_atk_spd: stats.p_atk_spd() as i32,
            run_speed,
            walk_speed,
            swim_run_speed: speed(stats.swim_run_speed()),
            swim_walk_speed: speed(stats.swim_walk_speed()),
            fly_run_speed: if template.flying { run_speed } else { 0 },
            fly_walk_speed: if template.flying { walk_speed } else { 0 },
            move_multiplier,
            attack_speed_multiplier: stats.attack_speed_multiplier(),
            collision_radius: template.collision_radius,
            collision_height: template.collision_height,
            right_hand: npc.right_hand,
            left_hand: npc.left_hand,
            running: npc.movement.running,
            in_combat: self.is_in_combat(subject),
            alike_dead: self.is_dead(subject),
            show_summon_animation: false,
            name,
            title,
            // `isInvisible() ? effects | STEALTH : effects`.
            abnormal_visual_effects: if self.objects.get(&subject).is_some_and(|e| e.invisible) {
                self.abnormal_visual_effects(subject).0 | crate::player_view::STEALTH_MASK
            } else {
                self.abnormal_visual_effects(subject).0
            },
            clan_id: 0,
            clan_crest_id: 0,
            ally_id: 0,
            ally_crest_id: 0,
            water_or_flying: if npc.in_water {
                1
            } else if template.flying {
                2
            } else {
                0
            },
            team: 0,
            enchant_effect: npc.enchant_effect,
            flying: template.flying,
            color_effect: 0,
            targetable: template.targetable,
            show_name: template.show_name,
            abnormal_visual_effect_special: self.abnormal_visual_effects(subject).1,
            display_effect: npc.display_effect,
        })
    }

    /// `NpcInfo` as bytes, for tools and tests.
    pub fn npc_info_bytes(&self, subject: ObjectId, viewer: ObjectId) -> Option<Vec<u8>> {
        self.npc_info(subject, viewer).map(|info| info.to_bytes())
    }
}

/// The census of one group while it rolls its first units (Java `SpawnGroup._aliveOrReserved`
/// and `SpawnGroupEntry._aliveOrReserved`).
#[derive(Debug)]
pub(crate) struct GroupState {
    pub(crate) selection: SpawnSelection,
    pub(crate) maximum: i64,
    pub(crate) alive: i64,
    pub(crate) entry_alive: Vec<i64>,
    pub(crate) entry_total: Vec<i64>,
    /// The slots of each entry that Java can build (`SpawnGroupEntry.getSlots`).
    pub(crate) slots: Vec<Vec<usize>>,
}

impl GroupState {
    /// `SpawnGroup.tryReserve`. The cap is deliberately not read for `random` groups.
    pub(crate) fn try_reserve(&mut self, entry: usize, count: i64) -> bool {
        if self.selection != SpawnSelection::Random && self.maximum > 0 && self.alive + count > self.maximum {
            return false;
        }
        if self.entry_alive[entry] + count > self.entry_total[entry] {
            return false;
        }
        self.entry_alive[entry] += count;
        self.alive += count;
        true
    }

    /// `SpawnGroup.release`.
    pub(crate) fn release(&mut self, entry: usize, count: i64) {
        self.entry_alive[entry] -= count;
        self.alive -= count;
    }
}
