//! The Seven Signs (M8-SS-01): the weekly cycle of the two cabals (Lords of Dawn, Revolutionaries
//! of Dusk), the registration at the priests, the seal stones and their contribution, the three
//! seals and what they change, Ancient Adena, the catacomb and necropolis gatekeepers, the
//! `SSQStatus` record and the auto spawns of the Seven Signs NPCs.
// Java: mechanics/sevensigns/SevenSigns.java
// Java: entity/actor/instance/{SignsPriest,DawnPriest,DuskPriest,DungeonGatekeeper}.java
// Java: entity/spawns/AutoSpawnHandler.java
// Java: network/clientpackets/RequestSSQStatus.java, network/serverpackets/{SSQStatus,SSQInfo}.java
// Java: managers/CastleManager.java (validateTaxes)
//
// # Shape
// - `mod.rs`: the constants, the state (`SevenSignsWorld`), the rows and writes, the questions the
//   other modules ask (`seal owner`, `period`, `cabal of a player`, ...).
// - `cycle.rs`: the restore at start, the period change on the timer wheel (competition, results,
//   seal validation, a new cycle), the new seal owners, the saves, the 30 minute global save
//   (`DailyResetManager.onSave`), the shutdown save, `isDateInSealValidPeriod`, the parts of
//   `EnterWorld` and `onPlayerEnter`, and the D-50 re-reads.
// - `priest.rs`: the dawn and dusk priests (`SignsPriest`, `DawnPriest`, `DuskPriest`): joining a
//   cabal, the seal stones, Ancient Adena, the record of the Seven Signs.
// - `gatekeeper.rs`: `DungeonGatekeeper` (the catacombs and necropolises).
// - `status.rs`: `RequestSSQStatus` and the `SSQStatus` pages.
// - `auto_spawn.rs`: `AutoSpawnHandler` (`random_spawn`) and `spawnSevenSignsNPC`.
//
// # Side by side with the Java game server (D-39, D-50, D-53)
// Sieghardt (the Java GS) keeps running the cycle while both servers share the database. So:
// - The global rows (`seven_signs_status`, the whole-table `seven_signs` save of a period change
//   and of the shutdown, the castle tax and ticket rows a cycle change sets) are written only with
//   `CycleDatabaseWrites = True` (`SevenSignsSettings::cycle_database_writes`). Off, the cycle runs
//   in this world's memory only.
// - The rows Java writes per player are written as usual: the `INSERT` of a new member, the
//   `UPDATE` of one member (`Player.store`, and with `AltSevenSignsLazyUpdate = False` after a
//   contribution or a reward).
// - With the switch off the state is read again (D-50): at start, at `CharacterSelect` (the
//   member's row and the status row, `PlayerProfile::seven_signs_refresh`; the `SSQInfo` sky of
//   the select comes from that status row) and before the `SSQStatus` record is shown (every
//   member row and the status row, `PersistCommand::SevenSignsReadRequest`). A read replaces the
//   global state; the rows of the members who are online here stay as this world has them (this
//   server writes them). A failed read shows nothing (the record) or keeps the memory (select).
// - Kain's own contributions add to the cabal scores in its memory only while the switch is off;
//   Sieghardt never sees them. That is D-53's price.
//
// - `festival.rs` (M8-FEST-01): the Festival of Darkness (`SevenSignsFestival`, the festival guides
//   and monsters, `seven_signs_festival`). `rift.rs` (M8-FEST-01): `DimensionalRiftManager`,
//   `DimensionalRift`, `DimensionalRiftRoom`. Their global rows follow the same switch.
//
// # Left out, with the card that brings it
// - `//mammon_find`, `//mammon_respawn` (`AdminMammon`): the admin card of M8.

mod auto_spawn;
mod cycle;
mod festival;
mod festival_data;
mod gatekeeper;
mod priest;
mod rift;
mod status;

use std::collections::BTreeMap;

use l2_util::ObjectId;

use crate::timer::TimerId;
use crate::world::World;

pub use auto_spawn::{AutoSpawnGroup, AutoSpawnTimer};
pub use cycle::{
    calc_ancient_adena_reward, calc_contribution_score, is_next_period_change_in_past, next_period_change_ms,
    ssq_info_state,
};
pub use festival::{
    FESTIVAL_COUNT, FESTIVAL_LEVEL_MAX_31, FESTIVAL_LEVEL_MAX_42, FESTIVAL_LEVEL_MAX_53,
    FESTIVAL_LEVEL_MAX_64, FESTIVAL_LEVEL_MAX_NONE, FESTIVAL_LEVEL_SCORES, FESTIVAL_OFFERING_ID,
    FESTIVAL_OFFERING_VALUE, FestivalData, FestivalSettings, FestivalStage, FestivalTimer, GuideInfo,
    festival_bypass_handler, festival_name, max_level_for_festival,
};
pub use festival_data::{
    FESTIVAL_DAWN_PLAYER_SPAWNS, FESTIVAL_DAWN_WITCH_SPAWNS, FESTIVAL_DUSK_PLAYER_SPAWNS,
    FESTIVAL_DUSK_WITCH_SPAWNS,
};
pub use rift::{
    DIMENSIONAL_FRAGMENT_ITEM_ID, RiftId, RiftRoomDef, RiftSettings, RiftTimer, rift_bypass_handler,
};

/// `SevenSigns.SEVEN_SIGNS_HTML_PATH`.
pub const SEVEN_SIGNS_HTML_PATH: &str = "data/html/seven_signs/";

/// `SevenSigns.CABAL_NULL`.
pub const CABAL_NULL: i32 = 0;
/// `SevenSigns.CABAL_DUSK`.
pub const CABAL_DUSK: i32 = 1;
/// `SevenSigns.CABAL_DAWN`.
pub const CABAL_DAWN: i32 = 2;

/// `SevenSigns.SEAL_NULL`.
pub const SEAL_NULL: i32 = 0;
/// `SevenSigns.SEAL_AVARICE`.
pub const SEAL_AVARICE: i32 = 1;
/// `SevenSigns.SEAL_GNOSIS`.
pub const SEAL_GNOSIS: i32 = 2;
/// `SevenSigns.SEAL_STRIFE`.
pub const SEAL_STRIFE: i32 = 3;

/// `SevenSigns.PERIOD_COMP_RECRUITING`.
pub const PERIOD_COMP_RECRUITING: i32 = 0;
/// `SevenSigns.PERIOD_COMPETITION`.
pub const PERIOD_COMPETITION: i32 = 1;
/// `SevenSigns.PERIOD_COMP_RESULTS`.
pub const PERIOD_COMP_RESULTS: i32 = 2;
/// `SevenSigns.PERIOD_SEAL_VALIDATION`.
pub const PERIOD_SEAL_VALIDATION: i32 = 3;

/// `SevenSigns.PERIOD_START_HOUR` / `PERIOD_START_MINS` (local time, Monday).
pub const PERIOD_START_HOUR: i64 = 18;
pub const PERIOD_START_MINS: i64 = 0;
/// `SevenSigns.PERIOD_MINOR_LENGTH`: the 15 minutes of the recruiting and the results periods.
pub const PERIOD_MINOR_LENGTH: i64 = 900_000;
/// `SevenSigns.PERIOD_MAJOR_LENGTH`.
pub const PERIOD_MAJOR_LENGTH: i64 = 604_800_000 - PERIOD_MINOR_LENGTH;

/// `SevenSigns.RECORD_SEVEN_SIGNS_ID` / `RECORD_SEVEN_SIGNS_COST`.
pub const RECORD_SEVEN_SIGNS_ID: i32 = 5707;
pub const RECORD_SEVEN_SIGNS_COST: i64 = 500;

/// The NPC ids of `SevenSigns`.
pub const ORATOR_NPC_ID: i32 = 31094;
pub const PREACHER_NPC_ID: i32 = 31093;
pub const MAMMON_MERCHANT_ID: i32 = 31113;
pub const MAMMON_BLACKSMITH_ID: i32 = 31126;
pub const MAMMON_MARKETEER_ID: i32 = 31092;
pub const LILITH_NPC_ID: i32 = 25283;
pub const ANAKIM_NPC_ID: i32 = 25286;
pub const CREST_OF_DAWN_ID: i32 = 31170;
pub const CREST_OF_DUSK_ID: i32 = 31171;

/// The seal stones.
pub const SEAL_STONE_BLUE_ID: i32 = 6360;
pub const SEAL_STONE_GREEN_ID: i32 = 6361;
pub const SEAL_STONE_RED_ID: i32 = 6362;
/// The Ancient Adena of one stone (`SEAL_STONE_*_VALUE`).
pub const SEAL_STONE_BLUE_VALUE: i64 = 3;
pub const SEAL_STONE_GREEN_VALUE: i64 = 5;
pub const SEAL_STONE_RED_VALUE: i64 = 10;
/// The contribution points of one stone (`*_CONTRIB_POINTS`).
pub const BLUE_CONTRIB_POINTS: i64 = 3;
pub const GREEN_CONTRIB_POINTS: i64 = 5;
pub const RED_CONTRIB_POINTS: i64 = 10;

/// `CommonSkill.THE_VICTOR_OF_WAR` / `THE_VANQUISHED_OF_WAR` (level 1): the Seal of Strife's
/// passive skills.
pub const THE_VICTOR_OF_WAR: i32 = 5074;
pub const THE_VANQUISHED_OF_WAR: i32 = 5075;

/// `Inventory.ANCIENT_ADENA_ID`.
pub const ANCIENT_ADENA_ID: i32 = l2_data::item::ANCIENT_ADENA_ID;

/// Java `getCabalShortName(cabal)`.
pub fn cabal_short_name(cabal: i32) -> &'static str {
    match cabal {
        CABAL_DAWN => "dawn",
        CABAL_DUSK => "dusk",
        _ => "No Cabal",
    }
}

/// Java `getCabalName(cabal)`.
pub fn cabal_name(cabal: i32) -> &'static str {
    match cabal {
        CABAL_DAWN => "Lords of Dawn",
        CABAL_DUSK => "Revolutionaries of Dusk",
        _ => "No Cabal",
    }
}

/// Java `getSealName(seal, shortName)`.
pub fn seal_name(seal: i32, short_name: bool) -> String {
    let mut name = if short_name {
        String::new()
    } else {
        "Seal of ".to_owned()
    };
    match seal {
        SEAL_AVARICE => name.push_str("Avarice"),
        SEAL_GNOSIS => name.push_str("Gnosis"),
        SEAL_STRIFE => name.push_str("Strife"),
        _ => {}
    }
    name
}

/// Java `getCurrentPeriodName()` (`None` is Java's `null` for an unknown period).
pub fn period_name(period: i32) -> Option<&'static str> {
    Some(match period {
        PERIOD_COMP_RECRUITING => "Quest Event Initialization",
        PERIOD_COMPETITION => "Competition (Quest Event)",
        PERIOD_COMP_RESULTS => "Quest Event Results",
        PERIOD_SEAL_VALIDATION => "Seal Validation",
        _ => return None,
    })
}

/// Java `Math.round(float)`.
pub(crate) fn java_round_f32(value: f32) -> i32 {
    (value + 0.5).floor() as i32
}

/// The configuration values the Seven Signs read (`Feature.ini`, `Npc.ini` and
/// `CycleDatabaseWrites` of `SideBySide.ini`). `Default` is the Java default of each key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SevenSignsSettings {
    /// `CycleDatabaseWrites` (D-53): the global rows are written only with it.
    pub cycle_database_writes: bool,
    /// `AltSevenSignsLazyUpdate`.
    pub lazy_update: bool,
    /// `AltMaxPlayerContrib`.
    pub max_player_contrib: i32,
    /// `StrictSevenSigns`.
    pub strict: bool,
    /// `AltOpenCatacumbs` / `AltOpenNecropolis`.
    pub open_catacombs: bool,
    pub open_necropolis: bool,
    /// `AltCastleForDawn` / `AltCastleForDusk`.
    pub castle_for_dawn: bool,
    pub castle_for_dusk: bool,
    /// `SevenSignsJoinDawnFee`.
    pub join_dawn_fee: i32,
    /// `SevenSignsManorsAgreementId`.
    pub manors_agreement_id: i32,
    /// `NpcConfig.ANNOUNCE_MAMMON_SPAWN` (`AnnounceMammonSpawn`).
    pub announce_mammon_spawn: bool,
    /// The keys of the Festival of Darkness (M8-FEST-01).
    pub festival: FestivalSettings,
}

impl Default for SevenSignsSettings {
    fn default() -> Self {
        Self {
            cycle_database_writes: false,
            lazy_update: true,
            max_player_contrib: 1_000_000,
            strict: true,
            open_catacombs: false,
            open_necropolis: false,
            castle_for_dawn: true,
            castle_for_dusk: true,
            join_dawn_fee: 50_000,
            manors_agreement_id: 6388,
            announce_mammon_spawn: false,
            festival: FestivalSettings::default(),
        }
    }
}

/// One member's data (Java's `StatSet` of `_signsPlayerData`, a `seven_signs` row). The numbers
/// are Java's longs; `getInt` narrows them.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SignsPlayer {
    pub char_id: i32,
    /// `""`, `"dawn"` or `"dusk"`.
    pub cabal: String,
    pub seal: i32,
    pub red_stones: i64,
    pub green_stones: i64,
    pub blue_stones: i64,
    pub ancient_adena_amount: i64,
    pub contribution_score: i64,
}

impl SignsPlayer {
    /// Java `getPlayerCabal`'s reading of the `cabal` text.
    pub fn cabal_id(&self) -> i32 {
        if self.cabal.eq_ignore_ascii_case("dawn") {
            CABAL_DAWN
        } else if self.cabal.eq_ignore_ascii_case("dusk") {
            CABAL_DUSK
        } else {
            CABAL_NULL
        }
    }
}

/// The global state of the cycle (the `seven_signs_status` row and the fields of `SevenSigns`).
/// Seals are indexed `1..=3` (index 0 is unused).
#[derive(Debug, Clone, PartialEq)]
pub struct SevenSignsStatus {
    pub current_cycle: i32,
    pub active_period: i32,
    pub previous_winner: i32,
    pub dawn_stone_score: f64,
    pub dawn_festival_score: i32,
    pub dusk_stone_score: f64,
    pub dusk_festival_score: i32,
    pub seal_owners: [i32; 4],
    pub dawn_seal_totals: [i32; 4],
    pub dusk_seal_totals: [i32; 4],
    /// `festival_cycle` and `accumulated_bonus0..4`: the festival's (not ported), written back as
    /// read.
    pub festival_cycle: i32,
    pub accumulated_bonus: [i32; 5],
    /// `_lastSave` (`date`): the milliseconds of the last status save.
    pub last_save: i64,
}

impl Default for SevenSignsStatus {
    /// Java's fields before `restoreSevenSignsData` (an empty status table).
    fn default() -> Self {
        Self {
            current_cycle: 0,
            active_period: 0,
            previous_winner: 0,
            dawn_stone_score: 0.0,
            dawn_festival_score: 0,
            dusk_stone_score: 0.0,
            dusk_festival_score: 0,
            seal_owners: [0; 4],
            dawn_seal_totals: [0; 4],
            dusk_seal_totals: [0; 4],
            festival_cycle: 0,
            accumulated_bonus: [0; 5],
            last_save: 0,
        }
    }
}

impl SevenSignsStatus {
    /// Java `getCurrentScore(cabal)`.
    pub fn current_score(&self, cabal: i32) -> i32 {
        let total = self.dawn_stone_score + self.dusk_stone_score;
        // `(float) totalStoneScore == 0 ? 1 : totalStoneScore`
        let divisor = if total as f32 == 0.0 { 1.0 } else { total };
        match cabal {
            CABAL_DAWN => java_round_f32((self.dawn_stone_score / divisor) as f32 * 500.0)
                .wrapping_add(self.dawn_festival_score),
            CABAL_DUSK => java_round_f32((self.dusk_stone_score / divisor) as f32 * 500.0)
                .wrapping_add(self.dusk_festival_score),
            _ => 0,
        }
    }

    /// Java `getCabalHighestScore()`.
    pub fn cabal_highest_score(&self) -> i32 {
        let (dusk, dawn) = (self.current_score(CABAL_DUSK), self.current_score(CABAL_DAWN));
        if dusk == dawn {
            CABAL_NULL
        } else if dusk > dawn {
            CABAL_DUSK
        } else {
            CABAL_DAWN
        }
    }

    /// Java `getSealOwner(seal)`.
    pub fn seal_owner(&self, seal: i32) -> i32 {
        usize::try_from(seal)
            .ok()
            .and_then(|i| self.seal_owners.get(i))
            .copied()
            .unwrap_or(CABAL_NULL)
    }

    /// Java `getSealProportion(seal, cabal)`.
    pub fn seal_proportion(&self, seal: i32, cabal: i32) -> i32 {
        let totals = match cabal {
            CABAL_NULL => return 0,
            CABAL_DUSK => &self.dusk_seal_totals,
            _ => &self.dawn_seal_totals,
        };
        usize::try_from(seal)
            .ok()
            .and_then(|i| totals.get(i))
            .copied()
            .unwrap_or(0)
    }
}

/// What the game server read at start or again (D-50): every `seven_signs` row and the status
/// row (`None`: the table has no `id = 0` row).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SevenSignsLoad {
    pub players: Vec<SignsPlayer>,
    pub status: Option<SevenSignsStatus>,
    /// Every `seven_signs_festival` row (M8-FEST-01).
    pub festival: Vec<FestivalData>,
}

/// What the session read at `CharacterSelect` (`PlayerProfile::seven_signs_refresh`): the member's
/// row (`None`: no row) and the status row (`None`: no row).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SevenSignsRefresh {
    pub player: Option<SignsPlayer>,
    pub status: Option<SevenSignsStatus>,
}

/// A Seven Signs row write with Java's statement (`PersistCommand::SevenSigns`).
#[derive(Debug, Clone, PartialEq)]
pub enum SevenSignsWrite {
    /// `INSERT_PLAYER`: a new member (per player, always written).
    InsertPlayer { char_id: i32, cabal: String, seal: i32 },
    /// `saveSevenSignsData(objectId)`: one member (per player, always written).
    UpdatePlayer(SignsPlayer),
    /// `saveSevenSignsData()`: every member (global, only with `CycleDatabaseWrites`).
    UpdateAllPlayers(Vec<SignsPlayer>),
    /// `saveSevenSignsStatus()` (global, only with `CycleDatabaseWrites`).
    UpdateStatus(SevenSignsStatus),
    /// `SevenSignsFestival.saveFestivalData()`: `REPLACE INTO seven_signs_festival` of every row
    /// (global, only with `CycleDatabaseWrites`, M8-FEST-01).
    ReplaceFestival(Vec<FestivalData>),
}

/// What the world does once the Seven Signs rows were read again (D-50).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SevenSignsThen {
    /// Only bring this world up to date.
    Nothing,
    /// `RequestSSQStatus`: show the page.
    Status { player: ObjectId, page: i8 },
    /// A festival guide's score page (4, 5 or 6): show it (M8-FEST-01).
    FestivalPage {
        player: ObjectId,
        npc: ObjectId,
        page: i32,
    },
}

/// The Seven Signs timers (one `TimerKind::SevenSigns`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SevenSignsTimer {
    /// `ThreadPool.schedule(new SevenSignsPeriodChange(), milliToChange)`.
    PeriodChange,
    /// `DailyResetManager`'s global save every 30 minutes (`saveSevenSignsStatus`).
    GlobalSave,
    /// The spawn and despawn tasks of `AutoSpawnHandler`.
    AutoSpawn(AutoSpawnTimer),
    /// The festival manager's round and its stages (M8-FEST-01).
    Festival(FestivalTimer),
    /// The tasks of the Dimensional Rift (M8-FEST-01).
    Rift(RiftTimer),
}

/// The Seven Signs state of the world.
#[derive(Debug)]
pub(crate) struct SevenSignsWorld {
    pub(crate) settings: SevenSignsSettings,
    pub(crate) status: SevenSignsStatus,
    /// `_signsPlayerData`, by character id.
    pub(crate) players: BTreeMap<i32, SignsPlayer>,
    /// `_nextPeriodChange` (epoch milliseconds).
    pub(crate) next_period_change: i64,
    pub(crate) period_timer: Option<TimerId>,
    pub(crate) save_timer: Option<TimerId>,
    /// `AutoSpawnHandler`.
    pub(crate) auto_spawn: auto_spawn::AutoSpawnWorld,
    /// `SevenSignsFestival` (M8-FEST-01).
    pub(crate) festival: festival::FestivalWorld,
    /// `DimensionalRiftManager` and the running rifts (M8-FEST-01).
    pub(crate) rift: rift::RiftWorld,
    /// `load_seven_signs` ran (the game server always loads; bare test worlds may not).
    pub(crate) loaded: bool,
}

impl Default for SevenSignsWorld {
    /// A world that has not loaded the Seven Signs (the game server always does): the seal
    /// validation period with no seal owned, the neutral state the residence and siege code of M7
    /// was written against.
    fn default() -> Self {
        Self {
            settings: SevenSignsSettings::default(),
            status: SevenSignsStatus {
                active_period: PERIOD_SEAL_VALIDATION,
                ..SevenSignsStatus::default()
            },
            players: BTreeMap::new(),
            next_period_change: 0,
            period_timer: None,
            save_timer: None,
            auto_spawn: auto_spawn::AutoSpawnWorld::default(),
            festival: festival::FestivalWorld::default(),
            rift: rift::RiftWorld::default(),
            loaded: false,
        }
    }
}

impl World {
    /// Installs the Seven Signs configuration. Call before [`Self::load_seven_signs`].
    pub fn set_seven_signs_settings(&mut self, settings: SevenSignsSettings) {
        self.seven_signs.settings = settings;
    }

    /// The Seven Signs configuration.
    pub fn seven_signs_settings(&self) -> &SevenSignsSettings {
        &self.seven_signs.settings
    }

    /// The global state of the cycle (tests and the admin pages).
    pub fn seven_signs_status(&self) -> &SevenSignsStatus {
        &self.seven_signs.status
    }

    /// One member's data (tests).
    pub fn seven_signs_player(&self, char_id: i32) -> Option<&SignsPlayer> {
        self.seven_signs.players.get(&char_id)
    }

    /// Java `getCurrentPeriod()`.
    pub fn ss_current_period(&self) -> i32 {
        self.seven_signs.status.active_period
    }

    /// Java `getCurrentCycle()`.
    pub fn ss_current_cycle(&self) -> i32 {
        self.seven_signs.status.current_cycle
    }

    /// Java `isCompetitionPeriod()`.
    pub fn ss_is_competition_period(&self) -> bool {
        self.ss_current_period() == PERIOD_COMPETITION
    }

    /// Java `isSealValidationPeriod()`.
    pub fn ss_is_seal_validation_period(&self) -> bool {
        self.ss_current_period() == PERIOD_SEAL_VALIDATION
    }

    /// Java `isCompResultsPeriod()`.
    pub fn ss_is_comp_results_period(&self) -> bool {
        self.ss_current_period() == PERIOD_COMP_RESULTS
    }

    /// Java `getSealOwner(seal)`.
    pub fn ss_seal_owner(&self, seal: i32) -> i32 {
        self.seven_signs.status.seal_owner(seal)
    }

    /// Java `getCabalHighestScore()`.
    pub fn ss_cabal_highest_score(&self) -> i32 {
        self.seven_signs.status.cabal_highest_score()
    }

    /// Java `getCurrentScore(cabal)`.
    pub fn ss_current_score(&self, cabal: i32) -> i32 {
        self.seven_signs.status.current_score(cabal)
    }

    /// Java `getPlayerCabal(objectId)`.
    pub fn ss_player_cabal(&self, char_id: i32) -> i32 {
        self.seven_signs
            .players
            .get(&char_id)
            .map_or(CABAL_NULL, SignsPlayer::cabal_id)
    }

    /// Java `getPlayerSeal(objectId)`.
    pub fn ss_player_seal(&self, char_id: i32) -> i32 {
        self.seven_signs
            .players
            .get(&char_id)
            .map_or(SEAL_NULL, |p| p.seal)
    }

    /// Java `getPlayerContribScore(objectId)` (`getInt`).
    pub fn ss_player_contrib_score(&self, char_id: i32) -> i32 {
        self.seven_signs
            .players
            .get(&char_id)
            .map_or(0, |p| p.contribution_score as i32)
    }

    /// Java `getPlayerStoneContrib(objectId)`: the stones turned in (`getInt` of each).
    pub fn ss_player_stone_contrib(&self, char_id: i32) -> i32 {
        self.seven_signs.players.get(&char_id).map_or(0, |p| {
            (p.red_stones as i32)
                .wrapping_add(p.green_stones as i32)
                .wrapping_add(p.blue_stones as i32)
        })
    }

    /// Java `getPlayerAdenaCollect(objectId)` (`getInt`).
    pub fn ss_player_adena_collect(&self, char_id: i32) -> i32 {
        self.seven_signs
            .players
            .get(&char_id)
            .map_or(0, |p| p.ancient_adena_amount as i32)
    }

    /// Java `getTotalMembers(cabal)`: the members whose `cabal` is the cabal's short name.
    pub fn ss_total_members(&self, cabal: i32) -> i32 {
        let name = cabal_short_name(cabal);
        let count = self
            .seven_signs
            .players
            .values()
            .filter(|p| p.cabal == name)
            .count();
        i32::try_from(count).unwrap_or(i32::MAX)
    }

    /// Java `checkSummonConditions(player)`: `true` (with the message) when a Dusk member may not
    /// summon a siege golem because Dawn owns the Seal of Strife in the seal validation period.
    pub fn ss_check_summon_conditions(&self, player: ObjectId) -> bool {
        if self.ss_is_seal_validation_period()
            && self.ss_seal_owner(SEAL_STRIFE) == CABAL_DAWN
            && self.ss_player_cabal(player.0) == CABAL_DUSK
        {
            self.send(
                player,
                &l2_protocol_game::server::SystemMessage::new(
                    l2_protocol_game::SystemMessageId::DueToTheAffectsOfTheSealOfStrifeItIsNotPossibleToSummonAtThisTime,
                ),
            );
            return true;
        }
        false
    }

    /// Java `Player.isIn7sDungeon()`.
    pub fn is_in_7s_dungeon(&self, player: ObjectId) -> bool {
        self.players
            .get(&player)
            .is_some_and(|p| p.data.profile.in_7s_dungeon)
    }

    /// Java `Player.setIn7sDungeon(value)`.
    pub fn set_in_7s_dungeon(&mut self, player: ObjectId, value: bool) {
        if let Some(p) = self.players.get_mut(&player) {
            p.data.profile.in_7s_dungeon = value;
        }
    }

    /// The row `Player.store` writes with every store (`saveSevenSignsData(objectId)`), `None` for
    /// a character that never registered.
    pub(crate) fn ss_row_to_store(&self, char_id: i32) -> Option<SignsPlayer> {
        self.seven_signs.players.get(&char_id).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_names_are_javas() {
        assert_eq!(cabal_short_name(CABAL_DAWN), "dawn");
        assert_eq!(cabal_short_name(CABAL_DUSK), "dusk");
        assert_eq!(cabal_short_name(CABAL_NULL), "No Cabal");
        assert_eq!(cabal_name(CABAL_DUSK), "Revolutionaries of Dusk");
        assert_eq!(seal_name(SEAL_STRIFE, false), "Seal of Strife");
        assert_eq!(seal_name(SEAL_GNOSIS, true), "Gnosis");
        assert_eq!(seal_name(0, false), "Seal of ");
        assert_eq!(period_name(PERIOD_SEAL_VALIDATION), Some("Seal Validation"));
        assert_eq!(period_name(9), None);
    }

    #[test]
    fn the_score_is_the_stone_share_of_500_plus_the_festival() {
        let mut s = SevenSignsStatus {
            dawn_stone_score: 300.0,
            dusk_stone_score: 100.0,
            dawn_festival_score: 10,
            dusk_festival_score: 0,
            ..SevenSignsStatus::default()
        };
        assert_eq!(s.current_score(CABAL_DAWN), 385);
        assert_eq!(s.current_score(CABAL_DUSK), 125);
        assert_eq!(s.current_score(CABAL_NULL), 0);
        assert_eq!(s.cabal_highest_score(), CABAL_DAWN);
        // No stones at all: both have their festival score only.
        s.dawn_stone_score = 0.0;
        s.dusk_stone_score = 0.0;
        s.dawn_festival_score = 0;
        assert_eq!(s.cabal_highest_score(), CABAL_NULL);
    }

    #[test]
    fn the_defaults_are_the_java_defaults() {
        let s = SevenSignsSettings::default();
        assert!(!s.cycle_database_writes && s.lazy_update && s.strict);
        assert!(!s.open_catacombs && !s.open_necropolis);
        assert!(s.castle_for_dawn && s.castle_for_dusk);
        assert_eq!(
            (s.max_player_contrib, s.join_dawn_fee, s.manors_agreement_id),
            (1_000_000, 50_000, 6388)
        );
    }
}
