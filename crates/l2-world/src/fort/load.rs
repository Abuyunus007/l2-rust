//! Loading the fortress siege data at startup, and reading it again on demand (D-50).
// Java: mechanics/siege/Fort.java (the constructor: initNpcs, initSiegeNpcs, initNpcCommanders,
//       spawnNpcCommanders, initSpecialEnvoys, spawnSpecialEnvoys)
// Java: managers/FortManager.java (loadInstances: `getSiege().getSiegeGuardManager()
//       .loadSiegeGuard()`)
// Java: mechanics/siege/FortSiege.java (constructor: checkAutoTask, loadSiegeClan)
// Java: managers/FortSiegeManager.java (load: the commander and flag lists)

use tracing::info;

use super::combat_flag::CombatFlag;
use super::settings::{FortSettings, commander_spawn_list, flag_list};
use crate::residence::{FortId, FortRecord};
use crate::world::World;

/// A `fort_spawnlist` row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FortSpawnRecord {
    pub id: i32,
    pub npc_id: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: i32,
    /// 0 always, 1 NPC commanders, 2 siege NPCs, 3 special envoys.
    pub spawn_type: i32,
    pub castle_id: i32,
}

/// A `fort_siege_guards` row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FortSiegeGuardRecord {
    pub npc_id: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: i32,
    /// Seconds.
    pub respawn_delay: i32,
}

/// What the game server read for one fortress.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FortSiegeFortLoad {
    pub fort_id: i32,
    /// `fort_spawnlist` (all types).
    pub spawns: Vec<FortSpawnRecord>,
    /// `fort_siege_guards`.
    pub siege_guards: Vec<FortSiegeGuardRecord>,
    /// `fortsiege_clans`: the registered clans.
    pub siege_clans: Vec<i32>,
}

/// Everything the game server read for the fortress sieges at startup (D-15: only reads, D-39).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FortSiegeLoad {
    pub forts: Vec<FortSiegeFortLoad>,
}

/// The rows of a fortress read again on demand for its siege (`ResidenceReadKind::FortSiege`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FortSiegeRows {
    /// The `fort` row with its functions.
    pub fort: FortRecord,
    /// `fortsiege_clans`.
    pub clans: Vec<i32>,
    /// `clan_data.blood_oath_count` of the fortress's owner, when it has one.
    pub owner_blood_oath_count: Option<i32>,
}

impl World {
    /// Installs the fortress configuration (`FortSiege.ini`, the fortress values of `Feature.ini`
    /// and `Player.ini`). Call before the residences are loaded; a fortress that exists already
    /// gets its commander and flag lists from it.
    pub fn set_fort_settings(&mut self, settings: FortSettings) {
        self.fort.settings = settings;
        self.fort_build_siege_lists();
    }

    /// The fortress configuration.
    pub fn fort_settings(&self) -> &FortSettings {
        &self.fort.settings
    }

    /// Java `FortSiegeManager.load()`: the commander and flag list of every fortress.
    fn fort_build_siege_lists(&mut self) {
        let forts: Vec<(FortId, String)> = self
            .residences
            .forts
            .iter()
            .map(|f| (f.id, f.name.clone()))
            .collect();
        for (id, name) in forts {
            let commanders = commander_spawn_list(&self.fort.settings.properties, id.0, &name);
            let flags = flag_list(&self.fort.settings.properties, &name)
                .into_iter()
                .map(|f| CombatFlag::new(f.x, f.y, f.z, f.item_id))
                .collect();
            self.fort.commander_lists.insert(id, commanders);
            self.fort.flags.insert(id, flags);
        }
    }

    /// Java's fortress constructors and `FortSiege` constructors at startup, in the order of
    /// `FortManager.loadInstances`: all spawns of every fortress, then the sieges (the
    /// countdown of a registered fortress and the siege guards). Call after
    /// [`World::load_residences`] and the NPC data.
    pub fn load_fort_data(&mut self, load: FortSiegeLoad) {
        self.fort_build_siege_lists();
        let forts: Vec<(FortId, i32, i32)> = self
            .residences
            .forts
            .iter()
            .map(|f| (f.id, f.owner_id, f.state))
            .collect();
        // The `Fort` constructor.
        for (id, owner, state) in &forts {
            let rows = load
                .forts
                .iter()
                .find(|f| f.fort_id == id.0)
                .map(|f| f.spawns.as_slice())
                .unwrap_or_default();
            // load and spawn npcs (Always spawned).
            self.fort_init_npcs(rows);
            // Suspicious merchants (despawned 10 minutes before the siege), NPC commanders
            // (not monsters, spawned outside of a siege) and the envoys from the castles.
            self.fort_init_spawn_lists(*id, rows);
            self.fort_spawn_npc_commanders(*id);
            if *owner != 0 && *state == 0 {
                self.fort_spawn_special_envoys(*id);
            }
        }
        // `FortManager.loadInstances`: `fort.getSiege()` creates the siege.
        for (id, _, _) in &forts {
            let fort_load = load.forts.iter().find(|f| f.fort_id == id.0);
            let clans = fort_load.map(|f| f.siege_clans.clone()).unwrap_or_default();
            self.fort_apply_siege_clan_rows(*id, &clans);
            self.fort_check_auto_task(*id);
            let guards = fort_load.map(|f| f.siege_guards.as_slice()).unwrap_or_default();
            self.fort_load_siege_guards(*id, guards);
        }
        info!("FortSiegeManager: {} fortress sieges.", forts.len());
    }

    /// The rows of a fortress read again for its siege (D-50): the fortress, the registrations
    /// and the blood oaths of the owner replace this world's copy; a registration this world
    /// holds without a row stays.
    pub(crate) fn apply_fort_siege_rows(&mut self, rows: &FortSiegeRows) {
        let id = FortId(rows.fort.id);
        if self.fort(id).is_none() {
            return;
        }
        self.apply_fort_record(&rows.fort);
        self.fort_apply_siege_clan_rows(id, &rows.clans);
        if let (Some(count), Some(owner)) = (rows.owner_blood_oath_count, self.fort(id).map(|f| f.owner_id))
            && let Some(clan) = self.clan_mut(owner)
        {
            clan.blood_oath_count = count;
        }
    }

    /// The updaters of a fortress whose owner was just read from the database.
    pub(crate) fn fort_refresh_updaters(&mut self, fort: FortId) {
        let owner = self.fort(fort).map_or(0, |f| f.owner_id);
        let running = self
            .fort
            .forts
            .get(&fort)
            .and_then(|s| s.updaters[0].or(s.updaters[1]))
            .map(|task| task.clan);
        if owner == 0 {
            if running.is_some() {
                self.fort_restart_updaters(fort, None);
            }
        } else if running != Some(owner) {
            self.fort_restart_updaters(fort, None);
            self.fort_start_updaters_on_load(fort);
        }
    }
}
