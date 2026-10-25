//! Conquerable clan hall sieges (M7-CHSIEGE-01): `CHSiegeManager` (the registration of a clan,
//! the lists), the base siege engine `ClanHallSiegeEngine` (the schedule, the start, the end, the
//! owner change) and the six hall scripts built on it: Fortress of Resistance, Devastated Castle,
//! Fortress of the Dead, Bandit Stronghold, Wild Beast Reserve and Rainbow Springs Chateau.
// Java: managers/CHSiegeManager.java
// Java: mechanics/siege/clanhalls/{ClanHallSiegeEngine,SiegableHall,SiegeStatus}.java
// Java: config/ConquerableHallSiegeConfig.java (`ConquerableHallSiege.ini`)
// Java: dist/game/data/scripts/conquerablehalls/{FortressOfResistance,DevastatedCastle,
//       FortressOfTheDead,BanditStronghold,WildBeastReserve,RainbowSpringsChateau}
// Java: entity/zone/type/{SiegableHallZone,ResidenceHallTeleportZone}.java
// Java: network/clientpackets/{RequestJoinSiege,RequestSiegeAttackerList}.java (the hall branches)
// Design: docs/design/m7-residences.md
//
// # Shape
// A conquerable hall is a [`ClanHall`](crate::residence::ClanHall) of kind `Siegable` (the owner,
// the functions, the doors and the zone are the residence code's, M7-RES-01). Everything a
// `SiegableHall` and its `ClanHallSiegeEngine` keep besides that is a [`HallSiege`] in
// [`HallSiegeWorld`] (`World::clanhall.siege`), by [`ClanHallId`]: the status, the schedule, the
// registered clans, the pending task and the state of the hall's script. Clans are ids, NPCs are
// object ids or indices into the runtime spawns (D-11). Every Java `ThreadPool.schedule` is a
// [`HallSiegeTimer`] on the wheel (D-12).
//
// # The scripts (D-05)
// The six scripts are built in like `ClanMaster`: the `Script` bypass hands the command to
// [`World::hall_script_event`], the first click on the NPC to [`World::hall_script_first_talk`],
// and the NPC events the scripts listen to (`ON_ATTACKABLE_ATTACK`, `ON_ATTACKABLE_KILL`,
// `ON_NPC_SPAWN`, `ON_NPC_SKILL_SEE`) to [`World::hall_script_on_attack`] and the others in
// `hooks.rs`. A script exists for a hall when the `siegable_clanhall` table has its row (Java
// `CHSiegeManager.getSiegableHall(id)` fails otherwise) and `NoQuests` is off.
//
// - `engine.rs`: `ClanHallSiegeEngine` (attackers, guards, `prepareOwner`, `startSiege`,
//   `endSiege`, the tasks) and the hooks the scripts override.
// - `manager.rs`: `CHSiegeManager` and the siege window (`RequestJoinSiege`,
//   `RequestSiegeAttackerList`, `SiegeInfo`, `SiegeAttackerList`).
// - `hooks.rs`: which NPC belongs to which script, and the script pages' common code.
// - `resistance.rs`, `devastated.rs`, `dead.rs`, `flag_war.rs` (Bandit Stronghold and Wild Beast
//   Reserve share one script), `rainbow.rs`: the scripts.
// - `zone.rs`: the siege zone of a hall, its teleport zones and its doors.
// - `load.rs`: the rows read at startup.
// - `schedule.rs`: `schedule_config` and `updateNextSiege`.
//
// # Side by side with the Java game server (D-39, D-49, D-50, D-51)
// Sieghardt (the Java GS) runs the real conquerable hall sieges while both servers share the
// database. So:
// - Every row the sieges write goes out through [`World::siege_write`] (`SiegeDatabaseWrites`,
//   default off): the new siege date, the attackers saved at shutdown, the flag war and Rainbow
//   Springs registrations, and the owner change (`residence/clan_hall.rs`, the same switch). Off,
//   the world still changes its own state, so a GM test siege works end to end.
// - The automatic chain (`PrepareOwner` one hour before the siege date, then `SiegeStarts`, then
//   `SiegeEnds`) is armed as in Java, but the first task does nothing without the switch (the
//   Java game server runs the real siege). A GM starts a test siege with the functions of
//   `World::hall_siege_*` (`//chsiege`, ADMIN-06): they run the chain to its end in this world.
// - A registration reads the hall's row again first (D-50: the owner and the next siege date
//   change on Sieghardt), and the flag war and Rainbow Springs registrations read their tables
//   again before the messenger acts on them. The attackers of the other three halls are saved by
//   Java only at its shutdown, so their rows are as old as that and are not read again: a
//   registration is as fresh as this world's memory.
// - The siege guards (`clanhall_siege_guards`) are read at startup, not at the first siege (D-15).
//
// # Java quirks kept (each marked `NOTE(java-quirk)` where it is)
// Bandit Stronghold and Wild Beast Reserve never start their battle: `startSiege()` of the script
// schedules itself again five minutes later and never reaches `ClanHallSiegeEngine.startSiege()`.
// Rainbow Springs never sets the status to running, fills its four arenas with the first
// registered clan, and so on. The scripts' other code is ported as it is.
//
// # Left to other cards
// - `SpecialSiegeGuardAI` (the AI of the flag war warriors: `SiegeGuardAI` that never attacks the
//   players of its clan's list, M7-SIEGE-02): the list is kept for it ([`World::hall_siege_allies_of`]).
// - `Defender.isAutoAttackable`'s hall branch (M7-SIEGE-02) and the hall branch of
//   `CastleDoorman` (M7-NPC-01; use [`World::hall_is_in_siege`] and [`World::hall_open_close_door`]).
// - `//chsiege` (`AdminCHSiege`, M7-ADMIN-06): the functions it calls are `World::hall_siege_*`.

mod dead;
mod devastated;
mod engine;
mod flag_war;
mod hooks;
mod load;
mod manager;
mod rainbow;
mod resistance;
pub(crate) mod schedule;
mod zone;

use std::collections::BTreeMap;

use l2_util::ObjectId;

pub use flag_war::FlagWarData;
pub use load::{
    FlagWarRecord, HallGuardRecord, HallSiegeHallLoad, HallSiegeLoad, HallSiegeRows, HallSiegeWrite,
};
pub use manager::HallThen;

use crate::residence::ClanHallId;
use crate::siege::SiegeClan;
use crate::timer::TimerId;

/// `ClanHallSiegeEngine.FORTRESS_RESSISTANCE`.
pub const FORTRESS_RESISTANCE: i32 = 21;
/// `ClanHallSiegeEngine.DEVASTATED_CASTLE`.
pub const DEVASTATED_CASTLE: i32 = 34;
/// `ClanHallSiegeEngine.BANDIT_STRONGHOLD`.
pub const BANDIT_STRONGHOLD: i32 = 35;
/// `ClanHallSiegeEngine.RAINBOW_SPRINGS`.
pub const RAINBOW_SPRINGS: i32 = 62;
/// `ClanHallSiegeEngine.BEAST_FARM`.
pub const BEAST_FARM: i32 = 63;
/// `ClanHallSiegeEngine.FORTRESS_OF_DEAD`.
pub const FORTRESS_OF_DEAD: i32 = 64;

/// Java `SiegeStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SiegeStatus {
    /// Clans may register.
    Registering,
    /// The registration is over; the battle starts in an hour.
    WaitingBattle,
    /// The battle runs.
    Running,
}

/// `ConquerableHallSiege.ini` (Java `ConquerableHallSiegeConfig.CHS_*`) and `NoQuests`.
///
/// `Default` is Java's default of each key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HallSiegeSettings {
    /// `MinClanLevel`: the level a clan needs to register.
    pub min_clan_level: i32,
    /// `MaxAttackers`.
    pub max_attackers: i32,
    /// `MaxFlagsPerClan`.
    pub max_flags_per_clan: i32,
    /// `EnableFame`.
    pub enable_fame: bool,
    /// `FameAmount`.
    pub fame_amount: i32,
    /// `FameFrequency` (seconds).
    pub fame_frequency: i32,
    /// `NoQuests` of `Development.ini`: no scripts are loaded, so no hall has a siege.
    pub no_quests: bool,
}

impl Default for HallSiegeSettings {
    fn default() -> Self {
        Self {
            min_clan_level: 4,
            max_attackers: 500,
            max_flags_per_clan: 1,
            enable_fame: false,
            fame_amount: 0,
            fame_frequency: 0,
            no_quests: false,
        }
    }
}

/// The scripts, one per hall.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HallScript {
    FortressOfResistance,
    DevastatedCastle,
    FortressOfTheDead,
    BanditStronghold,
    WildBeastReserve,
    RainbowSpringsChateau,
}

impl HallScript {
    /// Every script, in the order of their hall ids.
    pub const ALL: [HallScript; 6] = [
        Self::FortressOfResistance,
        Self::DevastatedCastle,
        Self::BanditStronghold,
        Self::RainbowSpringsChateau,
        Self::WildBeastReserve,
        Self::FortressOfTheDead,
    ];

    /// The hall the script is for.
    pub fn hall_id(self) -> i32 {
        match self {
            Self::FortressOfResistance => FORTRESS_RESISTANCE,
            Self::DevastatedCastle => DEVASTATED_CASTLE,
            Self::BanditStronghold => BANDIT_STRONGHOLD,
            Self::RainbowSpringsChateau => RAINBOW_SPRINGS,
            Self::WildBeastReserve => BEAST_FARM,
            Self::FortressOfTheDead => FORTRESS_OF_DEAD,
        }
    }

    /// The script of a hall.
    pub fn of_hall(hall_id: i32) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.hall_id() == hall_id)
    }

    /// The name of the script (its class, and the name in `bypass Script <name>`).
    pub fn name(self) -> &'static str {
        match self {
            Self::FortressOfResistance => "FortressOfResistance",
            Self::DevastatedCastle => "DevastatedCastle",
            Self::FortressOfTheDead => "FortressOfTheDead",
            Self::BanditStronghold => "BanditStronghold",
            Self::WildBeastReserve => "WildBeastReserve",
            Self::RainbowSpringsChateau => "RainbowSpringsChateau",
        }
    }

    /// The script of a `bypass Script <name>`.
    pub fn by_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.name() == name)
    }
}

/// What a script keeps besides what the engine does.
#[derive(Debug)]
pub(crate) enum ScriptState {
    Resistance(resistance::Resistance),
    Devastated(devastated::Devastated),
    Dead(dead::Dead),
    FlagWar(flag_war::FlagWar),
    Rainbow(Box<rainbow::Rainbow>),
}

/// A `SiegableHall`'s siege and its `ClanHallSiegeEngine` (Java `SiegableHall` besides what
/// `ClanHall` has, and the script).
#[derive(Debug)]
pub(crate) struct HallSiege {
    pub(crate) script: HallScript,
    /// `_status`.
    pub(crate) status: SiegeStatus,
    /// `_scheduleConfig`.
    pub(crate) schedule_config: [i32; 5],
    /// `_attackers`, in the order they registered (the lists show Java's map order).
    pub(crate) attackers: Vec<SiegeClan>,
    /// The most attackers the map ever held (it never shrinks, which decides its order).
    pub(crate) attackers_peak: usize,
    /// `_siegeTask`: the pending `PrepareOwner`, `SiegeStarts` or `SiegeEnds`.
    pub(crate) task: Option<TimerId>,
    /// The rows of `clanhall_siege_guards` read at startup.
    pub(crate) guard_rows: Vec<HallGuardRecord>,
    /// `_guards`: the spawns made from the rows, null until the first siege.
    pub(crate) guards: Option<Vec<(usize, i32)>>,
    /// `_missionAccomplished`.
    pub(crate) mission_accomplished: bool,
    /// The hall's `SiegeZone` (`SiegableHall.getSiegeZone()`).
    pub(crate) siege_zone: Option<i32>,
    pub(crate) state: ScriptState,
}

/// The state of the conquerable hall sieges in the world.
#[derive(Debug, Default)]
pub struct HallSiegeWorld {
    pub(crate) settings: HallSiegeSettings,
    pub(crate) halls: BTreeMap<ClanHallId, HallSiege>,
    /// `SpecialSiegeGuardAI.getAlly()`: the players a flag war warrior (by NPC) leaves alone.
    pub(crate) allies: BTreeMap<ObjectId, Vec<i32>>,
    /// `ResidenceHallTeleportZone._teleTask`: the pending teleport of a zone (by zone id).
    pub(crate) tele_tasks: BTreeMap<i32, TimerId>,
}

/// The conquerable hall timers (inside `ClanHallTimer::HallSiege`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HallSiegeTimer {
    /// Java `ClanHallSiegeEngine.PrepareOwner`: the registration ends, one hour before the siege.
    PrepareOwner(ClanHallId),
    /// Java `ClanHallSiegeEngine.SiegeStarts`.
    SiegeStarts(ClanHallId),
    /// Java `ClanHallSiegeEngine.SiegeEnds`.
    SiegeEnds(ClanHallId),
    /// Java `OnAttackableKill` after the NPC's `_onKillDelay` (2.5 s): the script's `onKill`.
    Kill {
        hall: ClanHallId,
        npc: ObjectId,
        npc_id: i32,
        spawn: Option<usize>,
        killer: ObjectId,
    },
    /// `OnAttackableAttack`, notified asynchronously: one tick after the hit, with the NPC's HP
    /// after it.
    Attacked {
        npc: ObjectId,
        player: ObjectId,
        damage: i32,
    },
    /// `OnNpcSpawn`, notified asynchronously: one tick after the NPC spawned, when the players
    /// around know it.
    NpcSpawned(ObjectId),
    /// The lambda `BanditStronghold.startSiege` schedules (5 minutes): the outer doors close, the
    /// zone is cleared and `startSiege()` runs again.
    FlagWarOuterDoors(ClanHallId),
    /// The lambda `BanditStronghold.onKill` schedules (5 minutes) after the first phase.
    FlagWarSecondPhase(ClanHallId),
    /// The lambda `RainbowSpringsChateau.onKill` schedules (2 minutes) after the win.
    RainbowArenaExit(ClanHallId),
    /// `RainbowSpringsChateau.GenerateTask` of one yeti (10 s, then every 5 minutes).
    RainbowGenerate { hall: ClanHallId, yeti: usize },
    /// `RainbowSpringsChateau.ChestsSpawn` (5 s, then every 5 s).
    RainbowChests(ClanHallId),
    /// `ResidenceHallTeleportZone.TeleportTask` (30 s) of the zone.
    TeleportZone(i32),
}

impl crate::world::World {
    /// Installs `ConquerableHallSiege.ini`. Call before [`Self::load_hall_sieges`].
    pub fn set_hall_siege_settings(&mut self, settings: HallSiegeSettings) {
        self.clanhall.siege.settings = settings;
    }

    /// The conquerable hall siege configuration.
    pub fn hall_siege_settings(&self) -> &HallSiegeSettings {
        &self.clanhall.siege.settings
    }

    /// The conquerable hall timers.
    pub(crate) fn on_hall_siege_timer(&mut self, timer: HallSiegeTimer) {
        match timer {
            HallSiegeTimer::PrepareOwner(hall) => self.on_hall_prepare_owner_timer(hall),
            HallSiegeTimer::SiegeStarts(hall) => self.hall_siege_start_task(hall),
            HallSiegeTimer::SiegeEnds(hall) => self.hall_siege_end_task(hall),
            HallSiegeTimer::Kill {
                hall,
                npc,
                npc_id,
                spawn,
                killer,
            } => self.hall_script_on_kill(hall, npc, npc_id, spawn, killer),
            HallSiegeTimer::Attacked { npc, player, damage } => {
                self.hall_script_on_attack_due(npc, player, damage);
            }
            HallSiegeTimer::NpcSpawned(npc) => self.hall_script_on_spawn_due(npc),
            HallSiegeTimer::FlagWarOuterDoors(hall) => self.flag_war_outer_doors_due(hall),
            HallSiegeTimer::FlagWarSecondPhase(hall) => self.flag_war_second_phase_due(hall),
            HallSiegeTimer::RainbowArenaExit(hall) => self.rainbow_arena_exit_due(hall),
            HallSiegeTimer::RainbowGenerate { hall, yeti } => self.rainbow_generate_due(hall, yeti),
            HallSiegeTimer::RainbowChests(hall) => self.rainbow_chests_due(hall),
            HallSiegeTimer::TeleportZone(zone) => self.hall_teleport_zone_due(zone),
        }
    }
}
