//! Fortresses and fortress sieges (M7-FORT-01): the fortress siege (registration, commanders, the
//! combat flag, the fort spawns, the owner change), `FortUpdater`, the fortress NPCs and the
//! `//fortsiege` commands. The residence part of a fortress (owner, functions, doors, flag pole,
//! supply level) is `residence/fort.rs` (M7-RES-01).
// Java: managers/{FortManager,FortSiegeManager,FortSiegeGuardManager}.java
// Java: mechanics/siege/{Fort,FortSiege,CombatFlag,FortSpawnHolder,FortTeleportWhoType,FortUpdater,
//       FortUpdaterType}.java
// Java: entity/actor/instance/{FortCommander,FortDoorman,FortLogistics,FortManager}.java
// Java: entity/zone/type/SiegeZone.java (the fortress's siege zone)
// Java: dist/game/data/scripts/ai/others/{FortressSiegeManager,FortressArcherCaptain}
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminFortSiege.java
// Design: docs/design/m7-residences.md
//
// # Shape
// A fortress's siege state lives in [`FortState`] (`World::fort`), by [`FortId`]: Java's
// `FortSiege` (registered clans, commanders, the three tasks), the spawn lists of `Fort`
// (`_siegeNpcs`, `_npcCommanders`, `_specialEnvoys`) and the two `FortUpdater` tasks. Spawns are
// ids into the spawn table (`respawn.rs`: runtime spawns), clans are ids, flags and players are
// object ids (D-11). Every Java `ThreadPool.schedule` is a [`FortTimer`] on the wheel (D-12).
//
// # Side by side with the Java game server (D-39, D-49, D-51)
// Sieghardt runs the real fortress sieges. So:
// - every row the siege writes (the registrations, the siege date) goes out through
//   [`World::siege_write`] (`SiegeDatabaseWrites`); the owner change goes through the residence
//   code (`residence/fort.rs`), which does the same;
// - the automatic start of a siege (Java's `ScheduleStartSiegeTask` chain: the announcements and
//   `startSiege()`) runs only with `SiegeDatabaseWrites`; without it the chain's first task is
//   armed (Java's `_siegeStartTask != null` is read by the registration code) and does nothing.
//   A GM starts a test siege (`//startfortsiege`), which runs to its end in this world;
// - the periodic jobs of `FortUpdater` that charge the clan warehouse and the castle treasury
//   skip those parts without the switch (as the function fees of RES-01 do);
// - a registration is read again before it is acted on (D-50): the Fortress Siege Manager asks
//   [`World::request_residence_reread`] with [`ResidenceReadThen::FortSiegeRegister`] and so on,
//   and a registration this world holds without a row (written with the switch off) survives it.
//
// # Left to other cards
// - Castle sieges, `SiegeFlag` (the headquarters), the siege skills and the siege clan states of
//   castles: M7-SIEGE-01. A fortress siege keeps its own list of the flags of its attackers
//   ([`FortSiegeClan::flags`]) for it to fill.
// - The castle ambassador's dialog (`CastleAmbassador`: the choice between a contract with the
//   castle and independence): M7-NPC-01. The fortress side of it (`setFortState`, the envoys
//   and the castle each speaks for) is here.
// - `FortSiegeGuardAI` (what a `Defender` does inside a fortress), `TerritoryWarManager`'s
//   registration windows (M7-TW-01), the control room.

mod admin;
mod combat_flag;
mod info;
mod load;
mod manager;
mod npc;
mod script;
mod settings;
mod siege;
mod spawns;
mod updater;
mod zone;

use std::collections::{BTreeMap, BTreeSet};

use l2_util::ObjectId;

pub(crate) use admin::register as register_admin;
pub use combat_flag::CombatFlag;
pub(crate) use combat_flag::{TakeFort, TakeFortStart};
pub use load::{FortSiegeFortLoad, FortSiegeGuardRecord, FortSiegeLoad, FortSiegeRows, FortSpawnRecord};
pub use settings::{
    FlagSpawn, FortSettings, FortSiegeIni, FortSpawnHolder, commander_spawn_list, flag_list,
    java_integer_decode,
};
pub use siege::{FortSiegeClan, FortSiegeWrite, FortTeleportWhoType};
pub use updater::FortUpdaterType;

use crate::residence::FortId;

/// What a player asks about the fortresses (`RequestAllFortressInfo`, `RequestFortressSiegeInfo`,
/// `RequestFortressMapInfo`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FortInput {
    /// Java `RequestAllFortressInfo`.
    AllFortressInfo,
    /// Java `RequestFortressSiegeInfo`.
    FortressSiegeInfo,
    /// Java `RequestFortressMapInfo`.
    FortressMapInfo { fortress_id: i32 },
}

/// A `FortLogistics` command that hands something out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogisticsCommand {
    /// `rewards`: how many blood oaths the clan has.
    Rewards,
    /// `blood`: the blood oaths become items.
    Blood,
    /// `supplylvl`: the supply level of a contracted fortress.
    SupplyLevel,
    /// `supply`: the supply box.
    Supply,
}

/// What the fortress code does once a fortress was read again (`ResidenceReadThen::Fort`, D-50).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FortThen {
    /// `FortressSiegeManager` "register".
    Register { player: ObjectId, npc: ObjectId },
    /// `FortressSiegeManager` "cancel".
    Cancel { player: ObjectId, npc: ObjectId },
    /// `FortressSiegeManager` "warInfo".
    WarInfo { player: ObjectId, npc: ObjectId },
    /// A command of the logistics officer.
    Logistics {
        player: ObjectId,
        npc: ObjectId,
        kind: LogisticsCommand,
    },
}

/// The fortress timers (one `TimerKind::Fort`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FortTimer {
    /// Java `FortUpdater` (`scheduleAtFixedRate`, or once for the late `MAX_OWN_TIME`).
    Updater { fort: FortId, kind: FortUpdaterType },
    /// Java `ScheduleStartSiegeTask(time)`: the countdown before a siege (`time` in seconds).
    StartSiege { fort: FortId, time: i32 },
    /// Java `ScheduleEndSiegeTask`.
    EndSiege(FortId),
    /// Java `ScheduleSiegeRestore`: the barracks come back.
    SiegeRestore(FortId),
    /// Java `ScheduleSuspiciousMerchantSpawn`.
    SuspiciousMerchantSpawn(FortId),
    /// Java `FortCommander.ScheduleTalkTask`: the commander may speak again.
    CommanderCanTalk(ObjectId),
}

/// One fortress's siege state and spawns (see the module comment).
#[derive(Debug, Default)]
pub struct FortState {
    pub(crate) siege: siege::FortSiege,
    pub(crate) spawns: spawns::FortSpawns,
    pub(crate) updaters: [Option<updater::UpdaterTask>; 2],
}

/// The fortress state of the world.
#[derive(Debug, Default)]
pub(crate) struct FortWorld {
    pub(crate) settings: FortSettings,
    /// Java `FortSiegeManager._commanderSpawnList`.
    pub(crate) commander_lists: BTreeMap<FortId, Vec<FortSpawnHolder>>,
    /// Java `FortSiegeManager._flagList`.
    pub(crate) flags: BTreeMap<FortId, Vec<CombatFlag>>,
    pub(crate) forts: BTreeMap<FortId, FortState>,
    /// The siege zones (zone ids) that are active.
    pub(crate) active_zones: BTreeSet<i32>,
    /// The commanders that must not speak (Java `FortCommander._canTalk == false`).
    pub(crate) silenced_commanders: BTreeSet<ObjectId>,
}

impl FortWorld {
    pub(crate) fn state_mut(&mut self, id: FortId) -> &mut FortState {
        self.forts.entry(id).or_default()
    }
}

impl crate::world::World {
    /// Asks the persistence task for the fortress's row, its registrations and the blood oaths of
    /// its owner; the answer comes back as `WorldCommand::ResidenceReread` and goes on with
    /// `then` (D-50, D-15).
    pub fn fort_request_siege_reread(&mut self, fort: FortId, then: FortThen) {
        self.outbox
            .persist(crate::outbox::PersistCommand::ResidenceRereadRequest {
                residence_id: fort.0,
                kind: crate::residence::ResidenceReadKind::FortSiege,
                then: crate::residence::ResidenceReadThen::Fort(then),
            });
    }

    /// The follow-up of [`Self::fort_request_siege_reread`].
    pub(crate) fn on_fort_reread(&mut self, then: FortThen) {
        self.fort_script_after_reread(then);
    }
}
