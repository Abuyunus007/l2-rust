//! Territory War (M7-TW-01): the nine territories and their castles and fortresses, the
//! registration of clans and mercenaries, the war period on the timer wheel, the territory wards
//! and the players who carry them, the outposts and flags, the badges and rewards, the Territory
//! Managers, `//territory_war` and the territory side of the castle, fortress and player code.
// Java: managers/TerritoryWarManager.java
// Java: mechanics/siege/TerritoryWard.java
// Java: entity/actor/instance/TerritoryWard.java
// Java: network/clientpackets/{RequestJoinDominionWar,RequestDominionInfo}.java
// Java: network/serverpackets/{ExShowDominionRegistry,ExReplyDominionInfo,ExShowOwnthingPos}.java
// Java: dist/game/data/scripts/ai/others/TerritoryManagers/TerritoryManagers.java
// Java: dist/game/data/scripts/quests/TerritoryWarScripts/TerritoryWarSuperClass.java (the part
//       of the super class instance: the next war date, the outpost and ward transport skills)
// Java: dist/game/data/scripts/handlers/skill/effects/{OutpostCreate,OutpostDestroy,
//       TakeTerritoryFlag}.java
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminTerritoryWar.java
// Java: dist/game/data/scripts/handlers/{items/Disguise,chat/channels/ChatBattlefield,
//       bypass/npc/TerritoryStatus}.java
// Design: docs/design/m7-residences.md
//
// # Shape
// Everything Java keeps in the `TerritoryWarManager` singleton lives in [`TerritoryWorld`]
// (`World::territory`). A [`Territory`] is keyed by its castle id (Java's `_territoryList`); it
// refers to its owner by clan id and to its NPCs, headquarters and wards by object id (D-11).
// The registrations are clan ids and character object ids. A [`TerritoryWard`] is Java's ward
// mechanic: the NPC on the ground, the player who carries it (and the worn item), the castle that
// owns it. Every `ThreadPool.schedule` is a [`TerritoryTimer`] on the wheel (D-12). The clock is
// the siege clock (`SiegeSettings::clock_override_unix_ms` in tests).
//
// A ward place keeps the object id of the ward NPC it shows; Java keeps a decayed NPC there
// (`decayMe`), here the id stays and the NPC is gone from the world: "spawned" is whether the
// world still has it.
//
// # Side by side with the Java game server (D-39, D-49, D-50, D-51)
// Sieghardt runs the real Territory War while both servers share the database. So:
// - every row the war writes (the registrations, the owned wards of a territory, the next war
//   date) goes out through [`World::siege_write`] (`SiegeDatabaseWrites`, default off); the world
//   changes its own state either way;
// - the schedule runs here too (the registration period, the countdown messages, the battlefield
//   channel), but the start of the war by the schedule happens only with the switch on. Off, the
//   territories, the registrations and the next date are read again once Sieghardt's war and its
//   channel are over ([`TerritoryTimer::SideBySideReread`]), and the schedule starts again from
//   the date read. A GM start (`//territory_war_start`) always runs the war in this world;
// - a registration (`RequestJoinDominionWar`) reads the registrations again first (D-50); a
//   registration this world holds without a row (written with the switch off) survives the read.
//
// # Left to other cards (D-05: quests and scripts come later)
// - The Territory War quests (`TerritoryWarSuperClass` and its children: "For the Sake of the
//   Territory", "Protect the ...", "Kill the ..."), so the quest points (`giveTWQuestPoint`), the
//   kill points of `onDeath` and the catapult and leader points of `onKill` have no caller yet:
//   [`World::territory_give_quest_point`], [`World::territory_give_point`] and
//   [`World::territory_catapult_destroyed`] are the API they will call. `setOnEnterWorld` of the
//   quests and their `onEnterWorld` are marked `// D-05:` where Java calls them.
// - `MakeMeNoble` of the Territory Managers completes the noblesse quests (D-05) and needs the
//   `nobless` column to be stored (the character store keeps the row's value): marked.
// - The olympiad check of the ward skills is in (`ward.rs`, M8-OLY-01).

mod admin;
mod flags;
mod load;
mod managers;
mod registration;
mod rewards;
mod war;
mod ward;

use std::collections::{BTreeMap, BTreeSet};

use l2_util::ObjectId;

use crate::timer::{TimerId, TimerKind, TimerWheel};
use crate::world::World;

pub(crate) use admin::register as register_admin;
pub(crate) use flags::{OutpostCreate, OutpostDestroy, TakeTerritoryFlag};
pub use load::{
    TerritoryLoad, TerritoryRecord, TerritoryRegistrationRecord, TerritoryRows, TerritorySpawnRecord,
};
pub(crate) use managers::territory_status_handler;

/// Java adds this to a castle id to get its territory id (81 to 89).
pub const TERRITORY_ID_BASE: i32 = 80;
/// The ward NPC of a territory is `36491 + territoryId` (36572 to 36580).
pub const WARD_NPC_BASE: i32 = 36491;
/// The ward item of a territory is `13479 + territoryId` (13560 to 13568).
pub const WARD_ITEM_BASE: i32 = 13479;
/// `OutpostCreate.HQ_NPC_ID`: the outpost (Territory Headquarters).
pub const OUTPOST_NPC_ID: i32 = 36590;
/// `TakeTerritoryFlag.FLAG_NPC_ID`.
pub const TERRITORY_FLAG_NPC_ID: i32 = 35062;
/// Java `Territory._territoryWardSpawnPlaces = new TerritoryNPCSpawn[9]`.
pub const WARD_PLACES: usize = 9;

/// Java `TerritoryWarManager.TERRITORY_ITEM_IDS`: the badge of a territory (13757 to 13765).
pub fn territory_badge(territory_id: i32) -> Option<i32> {
    (81..=89)
        .contains(&territory_id)
        .then(|| 13757 + (territory_id - 81))
}

/// Whether an item is a territory ward (`item.getId() >= 13560 && item.getId() <= 13568`).
pub fn is_ward_item(item_id: i32) -> bool {
    (13560..=13568).contains(&item_id)
}

/// The `TerritoryWar.ini` values (Java `TerritoryWarManager.load`'s statics). `Default` is Java's
/// default of each key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerritorySettings {
    /// `DefenderMaxClans` (Java reads it and never uses it).
    pub defender_max_clans: i32,
    /// `DefenderMaxPlayers` (Java reads it and never uses it).
    pub defender_max_players: i32,
    /// `ClanMinLevel` (Java reads it and never uses it).
    pub clan_min_level: i32,
    /// `PlayerMinLevel`.
    pub player_min_level: i32,
    /// `WarLength` (minutes) times 60000.
    pub war_length_ms: i64,
    /// `PlayerWithWardCanBeKilledInPeaceZone`.
    pub player_with_ward_can_be_killed_in_peace_zone: bool,
    /// `SpawnWardsWhenTWIsNotInProgress`.
    pub spawn_wards_when_tw_is_not_in_progress: bool,
    /// `ReturnWardsWhenTWStarts`.
    pub return_wards_when_tw_starts: bool,
    /// `MinTerritoryBadgeForNobless`.
    pub min_tw_badge_for_nobless: i32,
    /// `MinTerritoryBadgeForStriders`.
    pub min_tw_badge_for_striders: i32,
    /// `MinTerritoryBadgeForBigStrider`.
    pub min_tw_badge_for_big_strider: i32,
}

impl Default for TerritorySettings {
    fn default() -> Self {
        Self {
            defender_max_clans: 500,
            defender_max_players: 500,
            clan_min_level: 0,
            player_min_level: 40,
            war_length_ms: 120 * 60_000,
            player_with_ward_can_be_killed_in_peace_zone: false,
            spawn_wards_when_tw_is_not_in_progress: false,
            return_wards_when_tw_starts: false,
            min_tw_badge_for_nobless: 100,
            min_tw_badge_for_striders: 50,
            min_tw_badge_for_big_strider: 80,
        }
    }
}

/// Java `TerritoryNPCSpawn`: an NPC of the territory's spawn list, or a ward place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerritoryNpcSpawn {
    pub castle_id: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: i32,
    /// `_npcId`: the NPC template of a spawn; for a ward place the territory id of the ward that
    /// stands there (0 for a free place).
    pub npc_id: i32,
    /// `_type`: 0 town NPCs, 1 fortress NPCs, 2 castle NPCs, 4 a ward place.
    pub spawn_type: i32,
    /// `_npc`.
    pub npc: Option<ObjectId>,
}

/// Java `TerritoryWarManager.Territory`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Territory {
    /// `castleId + 80`.
    pub territory_id: i32,
    pub castle_id: i32,
    /// `_fortId` (from the `territories` row).
    pub fort_id: i32,
    /// `_ownerClan`.
    pub owner_clan: Option<i32>,
    /// `_spawnList` (types 0, 1 and 2).
    pub spawn_list: Vec<TerritoryNpcSpawn>,
    /// `_territoryWardSpawnPlaces`: up to nine places, in the order of the spawn list.
    pub ward_places: Vec<TerritoryNpcSpawn>,
    /// `_territoryHQ`: the outpost of the owner clan.
    pub hq: Option<ObjectId>,
    /// `_questDone`: `[0]` NPCs killed, `[1]` wards captured.
    pub quest_done: [i32; 2],
}

impl Territory {
    pub(crate) fn new(castle_id: i32) -> Self {
        Self {
            territory_id: castle_id + TERRITORY_ID_BASE,
            castle_id,
            fort_id: 0,
            owner_clan: None,
            spawn_list: Vec::new(),
            ward_places: Vec::new(),
            hq: None,
            quest_done: [0; 2],
        }
    }

    /// Java `getOwnedWardIds()`: the territory ids of the wards on the places, in place order.
    pub fn owned_ward_ids(&self) -> Vec<i32> {
        self.ward_places
            .iter()
            .filter(|p| p.npc_id > 0)
            .map(|p| p.npc_id)
            .collect()
    }
}

/// Java `mechanics/siege/TerritoryWard`: one ward of the war.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerritoryWard {
    /// `_territoryId`: the territory the ward stands for.
    pub territory_id: i32,
    /// `_itemId`: the ward item (`territoryId + 13479`).
    pub item_id: i32,
    /// `_ownerCastleId`: the castle that holds the ward now.
    pub owner_castle_id: i32,
    /// `_location`: where the ward goes when it is dropped (the carrier's last place).
    pub location: (i32, i32, i32, i32),
    /// `_oldLocation`: where the NPC stood when it was picked up.
    pub old_location: Option<(i32, i32, i32, i32)>,
    /// `_player` / `playerId`.
    pub player: Option<ObjectId>,
    /// `_item`: the worn item.
    pub item: Option<ObjectId>,
    /// `_npc`.
    pub npc: Option<ObjectId>,
}

/// One registration (a clan id or a character id) and whether it has no row (written while
/// `SiegeDatabaseWrites` was off, D-51).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Registration {
    pub(crate) id: i32,
    pub(crate) unsaved: bool,
}

/// The Territory War timers (one `TimerKind::Territory`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerritoryTimer {
    /// Java `ScheduleStartTWTask`.
    StartTw,
    /// Java `ScheduleEndTWTask`.
    EndTw,
    /// Java `closeTerritoryChannelTask`.
    CloseChannel,
    /// Java `RewardOnlineParticipants` (`scheduleAtFixedRate`, every minute).
    RewardOnline,
    /// Side by side (D-51): Sieghardt's war and channel are over; read the rows again and start
    /// the schedule from the date read.
    SideBySideReread,
    /// `MercenaryCaptain`'s `startQuestTimer("say", DELAY, npc, null, true)` (every hour).
    CaptainSay(ObjectId),
}

/// A row write of the war, with Java's statement (`ResidenceWrite::Territory`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerritoryWrite {
    /// `changeRegistration(castleId, objId, false)`: `INSERT INTO territory_registrations
    /// (castleId, registeredId) values (?, ?)`.
    RegistrationInsert { castle_id: i32, registered_id: i32 },
    /// `changeRegistration(castleId, objId, true)`: `DELETE FROM territory_registrations WHERE
    /// castleId = ? and registeredId = ?`.
    RegistrationDelete { castle_id: i32, registered_id: i32 },
    /// `updateTerritoryData`: `UPDATE territories SET ownedWardIds=? WHERE territoryId=?`.
    OwnedWardIds { territory_id: i32, ward_ids: String },
    /// `GlobalVariablesManager.set(GLOBAL_VARIABLE, time)`: the `nextTWStartDate` row.
    NextTwStartDate(i64),
}

/// The client packets of the war.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerritoryInput {
    /// Java `RequestJoinDominionWar`.
    JoinDominionWar {
        territory_id: i32,
        is_clan: i32,
        is_joining: i32,
    },
    /// Java `RequestDominionInfo`.
    DominionInfo,
}

/// What the war does once its rows were read again (`ResidenceReadThen::Territory`, D-50).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerritoryThen {
    /// `RequestJoinDominionWar` on the fresh registrations.
    Join {
        player: ObjectId,
        territory_id: i32,
        is_clan: i32,
        is_joining: i32,
    },
    /// Side by side: Sieghardt's war is over; start the schedule from the date read.
    RestartSchedule,
}

/// The Territory War state of the world (Java's `TerritoryWarManager` fields).
#[derive(Debug)]
pub(crate) struct TerritoryWorld {
    pub(crate) settings: TerritorySettings,
    /// `_territoryList`, by castle id.
    pub(crate) territories: BTreeMap<i32, Territory>,
    /// `_registeredClans`, by castle id, in registration order.
    pub(crate) registered_clans: BTreeMap<i32, Vec<Registration>>,
    /// `_registeredMercenaries`, by castle id, in registration order.
    pub(crate) registered_mercenaries: BTreeMap<i32, Vec<Registration>>,
    /// `_disguisedPlayers`.
    pub(crate) disguised: BTreeSet<ObjectId>,
    /// `_territoryWards`, in the order they were made.
    pub(crate) wards: Vec<TerritoryWard>,
    /// `_clanFlags`: the flag (`TakeTerritoryFlag`) of each clan.
    pub(crate) clan_flags: BTreeMap<i32, ObjectId>,
    /// `_participantPoints`: `[side, kills, quests, npcs, catapults, wards, online minutes]`.
    pub(crate) participant_points: BTreeMap<ObjectId, [i32; 7]>,
    /// `_startTWDate` (epoch milliseconds).
    pub(crate) start_tw_date: i64,
    /// `_isRegistrationOver`.
    pub(crate) registration_over: bool,
    /// `_isTWChannelOpen`.
    pub(crate) channel_open: bool,
    /// `_isTWInProgress`.
    pub(crate) in_progress: bool,
    /// `_scheduledStartTWTask`.
    pub(crate) start_task: Option<TimerId>,
    /// `_scheduledEndTWTask`.
    pub(crate) end_task: Option<TimerId>,
    /// `_scheduledRewardOnlineTask`.
    pub(crate) reward_task: Option<TimerId>,
    /// `GlobalVariablesManager.getLong(GLOBAL_VARIABLE, 0)` as this world knows it.
    pub(crate) next_tw_start_date: i64,
    /// D-51: a GM started the war (`//territory_war_start`); the start task starts it whatever
    /// `SiegeDatabaseWrites` says.
    pub(crate) gm_start: bool,
    /// The fortresses whose siege zone has the war as its siege instance (`setSiegeInstance`).
    pub(crate) fort_zones: BTreeSet<i32>,
    /// The hourly shout of each mercenary captain.
    pub(crate) captain_timers: BTreeMap<ObjectId, TimerId>,
}

impl Default for TerritoryWorld {
    fn default() -> Self {
        Self {
            settings: TerritorySettings::default(),
            territories: BTreeMap::new(),
            registered_clans: BTreeMap::new(),
            registered_mercenaries: BTreeMap::new(),
            disguised: BTreeSet::new(),
            wards: Vec::new(),
            clan_flags: BTreeMap::new(),
            participant_points: BTreeMap::new(),
            start_tw_date: 0,
            // Java: `_isRegistrationOver = true`, `_isTWChannelOpen = false`.
            registration_over: true,
            channel_open: false,
            in_progress: false,
            start_task: None,
            end_task: None,
            reward_task: None,
            next_tw_start_date: 0,
            gm_start: false,
            fort_zones: BTreeSet::new(),
            captain_timers: BTreeMap::new(),
        }
    }
}

impl World {
    /// Installs the `TerritoryWar.ini` values. Call before [`Self::load_territories`].
    pub fn set_territory_settings(&mut self, settings: TerritorySettings) {
        self.territory.settings = settings;
    }

    /// The `TerritoryWar.ini` values.
    pub fn territory_settings(&self) -> &TerritorySettings {
        &self.territory.settings
    }

    /// Java `isTWInProgress()`.
    pub fn is_tw_in_progress(&self) -> bool {
        self.territory.in_progress
    }

    /// Java `isTWChannelOpen()`.
    pub fn is_tw_channel_open(&self) -> bool {
        self.territory.channel_open
    }

    /// Java `isRegistrationOver()`.
    pub fn is_tw_registration_over(&self) -> bool {
        self.territory.registration_over
    }

    /// Java `getTWStartTimeInMillis()`.
    pub fn tw_start_time_ms(&self) -> i64 {
        self.territory.start_tw_date
    }

    /// Java `getTerritory(castleId)`.
    pub fn territory(&self, castle_id: i32) -> Option<&Territory> {
        self.territory.territories.get(&castle_id)
    }

    /// Every territory, by castle id.
    pub fn territories(&self) -> impl Iterator<Item = &Territory> {
        self.territory.territories.values()
    }

    /// Java `getAllTerritories()`: the territories that have an owner clan.
    pub fn owned_territories(&self) -> Vec<&Territory> {
        self.territory
            .territories
            .values()
            .filter(|t| t.owner_clan.is_some())
            .collect()
    }

    /// Java `getAllTerritoryWards()`.
    pub fn territory_wards(&self) -> &[TerritoryWard] {
        &self.territory.wards
    }

    /// Java `isDisguised(playerObjId)`.
    pub fn is_disguised(&self, player: ObjectId) -> bool {
        self.territory.disguised.contains(&player)
    }

    /// Java `getRegisteredClans(castleId)` (`None` where Java's map has no list).
    pub fn territory_registered_clans(&self, castle_id: i32) -> Option<Vec<i32>> {
        self.territory
            .registered_clans
            .get(&castle_id)
            .map(|list| list.iter().map(|r| r.id).collect())
    }

    /// Java `getRegisteredMercenaries(castleId)`.
    pub fn territory_registered_mercenaries(&self, castle_id: i32) -> Option<Vec<i32>> {
        self.territory
            .registered_mercenaries
            .get(&castle_id)
            .map(|list| list.iter().map(|r| r.id).collect())
    }

    /// Java `getHQForClan(clan)`: the outpost of the clan's territory (a castle owner only).
    pub fn territory_hq_for_clan(&self, clan_id: i32) -> Option<ObjectId> {
        let castle = self.clan(clan_id)?.castle_id;
        if castle <= 0 {
            return None;
        }
        self.territory.territories.get(&castle)?.hq
    }

    /// Java `getHQForTerritory(territoryId)`.
    pub fn territory_hq(&self, territory_id: i32) -> Option<ObjectId> {
        self.territory
            .territories
            .get(&(territory_id - TERRITORY_ID_BASE))?
            .hq
    }

    /// Java `getFlagForClan(clan)`.
    pub fn territory_flag_for_clan(&self, clan_id: i32) -> Option<ObjectId> {
        self.territory.clan_flags.get(&clan_id).copied()
    }

    /// Java `System.currentTimeMillis()` of the war: the siege clock.
    pub(crate) fn tw_now_ms(&self) -> i64 {
        self.siege_now_ms()
    }

    /// Schedules a war timer `delay_ms` from now.
    pub(crate) fn schedule_territory_timer(&mut self, timer: TerritoryTimer, delay_ms: i64) -> TimerId {
        let delay = u64::try_from(delay_ms.max(0)).unwrap_or(0);
        let due = self.tick + TimerWheel::ms_to_ticks(delay);
        self.timers.schedule_at(due, TimerKind::Territory(timer))
    }

    /// The war timers.
    pub(crate) fn on_territory_timer(&mut self, timer: TerritoryTimer) {
        match timer {
            TerritoryTimer::StartTw => self.on_tw_start_task(),
            TerritoryTimer::EndTw => self.on_tw_end_task(),
            TerritoryTimer::CloseChannel => self.on_tw_close_channel(),
            TerritoryTimer::RewardOnline => self.on_tw_reward_online(),
            TerritoryTimer::SideBySideReread => self.on_tw_side_by_side_reread(),
            TerritoryTimer::CaptainSay(npc) => self.on_captain_say_timer(npc),
        }
    }

    /// Hands a write of the war to the persistence task, only with `SiegeDatabaseWrites` (D-51).
    /// Returns whether it went out.
    pub(crate) fn territory_write(&mut self, write: TerritoryWrite) -> bool {
        self.siege_write(crate::residence::ResidenceWrite::Territory(write))
    }

    /// `getTerritory(castleId).setOwnerClan(clan)` (`Castle.setOwner`; side by side also the
    /// owner a re-read of the castle brings, D-51).
    pub(crate) fn territory_set_owner_clan(&mut self, castle_id: i32, clan: Option<i32>) {
        match self.territory.territories.get_mut(&castle_id) {
            Some(t) => t.owner_clan = clan,
            // Java throws (`getTerritory` is null) when the datapack has no territory there.
            None => tracing::debug!("No territory for castle {castle_id}."),
        }
    }

    /// The war forgets an NPC that left the world (its decay or delete).
    pub(crate) fn territory_forget_npc(&mut self, npc: ObjectId) {
        if let Some(timer) = self.territory.captain_timers.remove(&npc) {
            self.timers.cancel(timer);
        }
    }

    /// `EnterWorld`'s Territory War block: a registered player is on his territory's side (an
    /// attacker while the war runs).
    pub(crate) fn territory_player_entered(&mut self, player: ObjectId) {
        let territory_id = self.registered_territory_id(player);
        if territory_id > 0 {
            if self.territory.in_progress {
                self.set_siege_state(player, 1);
            }
            self.set_siege_side(player, territory_id);
        }
        // D-05: `TerritoryWarSuperClass.onEnterWorld` (the territory quests and the "Kill the"
        // quest notified of the player's death).
    }

    /// The end of `Player.deleteMe` for the war: a ward carrier drops it (`dropCombatFlag(this,
    /// false, false)`), then the player's war state goes.
    pub(crate) fn territory_player_leaving(&mut self, player: ObjectId) {
        if self.territory_ward_of_player(player).is_some() {
            self.territory_drop_combat_flag(player, false, false);
        }
    }
}
