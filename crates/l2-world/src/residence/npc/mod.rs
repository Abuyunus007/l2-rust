//! The NPCs of a castle: the chamberlain, the siege manager, the mercenary manager and the
//! mercenary captain, the vault keeper, the mass gatekeeper, the blacksmith, the court magician,
//! the special envoys of the fortresses (`CastleAmbassador`) and the castle doormen
//! (M7-NPC-01).
// Java: dist/game/data/scripts/ai/others/{CastleChamberlain,CastleSiegeManager,
//       CastleMercenaryManager,CastleWarehouse,CastleTeleporter,CastleBlacksmith,
//       CastleCourtMagician,CastleAmbassador,MercenaryCaptain}/*.java (and their html pages)
// Java: entity/actor/instance/{CastleDoorman,Doorman}.java
// Java: mechanics/script/Quest.java (notifyFirstTalk, notifyEvent, showResult, showHtmlFile,
//       getHtm, startQuestTimer)
// Java: entity/actor/Player.java (processScriptEvent)
// Java: dist/game/data/scripts/handlers/actions/click/NpcClick.java, handlers/bypass/npc/
//       {ChatLink,ScriptLink}.java
//
// # Shape
// D-05: the script API comes last, so these scripts are built in like `ClanMaster`
// (`clan/master_script.rs`) and `ManorManager` (`manor/npc.rs`): a [`CastleScript`] names the
// script of an NPC id, the first click and `Chat 0` ask [`World::residence_npc_first_talk`], the
// `Script <Name> <event>` bypass asks [`World::residence_script_event`] (called from
// `process_script_event`), and what a Java `onEvent`/`onFirstTalk` returns goes through
// [`World::castle_script_result`], which is `Quest.showResult`. The pages are the scripts' own
// (`data/scripts/ai/others/<Name>/*.html`). The `CastleDoorman` is an NPC class, not a script
// (`doorman.rs`); its window is the click hook [`World::residence_npc_on_click`].
//
// Two ways of showing a page, as in Java:
// - the script returns a page name or a whole page (`return "chamberlain-04.html"`): `showResult`
//   replaces `%objectId%` and `%playername%` and follows with an `ActionFailed`;
// - the script builds an `NpcHtmlMessage` itself (`getHtmlPacket`, `html.replace(..)`,
//   `player.sendPacket(html)`): only the `replace` calls happen, no `ActionFailed`.
//
// # Side by side (D-39, D-50, D-51)
// The chamberlain acts on the castle row (owner, tax rate, treasury, functions), which Sieghardt
// changes while Kain runs: every chamberlain command that is not a static page asks for the
// castle to be read again first (`ResidenceReadThen::Npc`) and goes on with the fresh rows; a
// failed read refuses the command. The vault keeper's "Receive" and count page read the castle
// with its owner's `blood_alliance_count` (`CastleRecord::owner_blood_alliance_count`), which the
// Java game server adds to at the end of its sieges. What a player does himself (a tax rate, a deposit or a
// withdrawal, a function, a door or trap upgrade, the dawn tickets) is written; the fee tasks of
// the functions are RES-01's. The envoys' timer-driven decision (`DESPAWN`) is a fortress job
// (`Writes::Job`), the owner's choice at the envoy a player's action.
//
// # Left to other cards (each has its marker where it belongs)
// - Seven Signs (`seven_signs.rs`): the questions go to `sevensigns/` (M8-SS-01).
// - Territory War (`territory/`, M7-TW-01) owns `ExShowDominionRegistry`, `isTWInProgress`, the
//   captain's shouting timer and the badge prices of `TerritoryWar.ini`; the Territory Managers
//   are a script of this module whose pages are in `territory/managers.rs`.
// - `getConquerableHall().showSiegeInfo/isInSiege` of the siege manager: M7-CHSIEGE-01.

mod ambassador;
mod captain;
mod chamberlain;
mod court_magician;
mod doorman;
mod seven_signs;
mod simple;
mod teleporter;

use std::collections::{BTreeMap, BTreeSet};

use l2_protocol_game::server::{ActionFailed, NpcHtmlMessage};
use l2_util::ObjectId;
use tracing::{debug, warn};

pub use ambassador::AMBASSADORS;
pub use seven_signs::{CABAL_DAWN, CABAL_DUSK, CABAL_NULL};

use crate::dialog::INTERACTION_DISTANCE;
use crate::residence::CastleId;
use crate::target::distance_2d;
use crate::timer::TimerId;
use crate::world::World;

/// `CastleChamberlain.NPC`: Sayres, Crosby, Saul, Brasseur, Logan, Neurath, Alfred, Frederick,
/// August.
pub const CHAMBERLAINS: [i32; 9] = [35100, 35142, 35184, 35226, 35274, 35316, 35363, 35509, 35555];
/// `CastleSiegeManager.SIEGE_MANAGER`: the nine castles, the Fortress of the Dead and the
/// Devastated Castle.
pub const SIEGE_MANAGERS: [i32; 11] = [
    35104, 35146, 35188, 35232, 35278, 35320, 35367, 35513, 35559, 35639, 35420,
];
/// `CastleMercenaryManager.NPCS`.
pub const MERCENARY_MANAGERS: [i32; 9] = [35102, 35144, 35186, 35228, 35276, 35318, 35365, 35511, 35557];
/// `CastleWarehouse.NPCS`.
pub const WAREHOUSES: [i32; 9] = [35099, 35141, 35183, 35225, 35273, 35315, 35362, 35508, 35554];
/// `CastleTeleporter.NPCS`.
pub const TELEPORTERS: [i32; 9] = [35095, 35137, 35179, 35221, 35266, 35311, 35355, 35502, 35547];
/// `CastleBlacksmith.NPCS`.
pub const BLACKSMITHS: [i32; 9] = [35098, 35140, 35182, 35224, 35272, 35314, 35361, 35507, 35553];
/// `CastleCourtMagician.COURT_MAGICIAN`.
pub const COURT_MAGICIANS: [i32; 9] = [35648, 35649, 35650, 35651, 35652, 35653, 35654, 35655, 35656];
/// `MercenaryCaptain.NPCS` (the keys): the captain of each castle's Territory War mercenaries.
pub const MERCENARY_CAPTAINS: [i32; 9] = [36481, 36482, 36483, 36484, 36485, 36486, 36487, 36488, 36489];
/// `TerritoryManagers`: `addFirstTalkId(36490 + i)` for the nine territories (M7-TW-01).
pub const TERRITORY_MANAGERS: [i32; 9] = [36490, 36491, 36492, 36493, 36494, 36495, 36496, 36497, 36498];

/// The scripts of this module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CastleScript {
    Chamberlain,
    SiegeManager,
    MercenaryManager,
    Warehouse,
    Teleporter,
    Blacksmith,
    CourtMagician,
    Ambassador,
    MercenaryCaptain,
    /// `TerritoryManagers` (M7-TW-01, `territory/managers.rs`).
    TerritoryManagers,
}

impl CastleScript {
    /// Java `ScriptManager.getScript(name)`: the names are case sensitive.
    pub fn by_name(name: &str) -> Option<Self> {
        Some(match name {
            "CastleChamberlain" => Self::Chamberlain,
            "CastleSiegeManager" => Self::SiegeManager,
            "CastleMercenaryManager" => Self::MercenaryManager,
            "CastleWarehouse" => Self::Warehouse,
            "CastleTeleporter" => Self::Teleporter,
            "CastleBlacksmith" => Self::Blacksmith,
            "CastleCourtMagician" => Self::CourtMagician,
            "CastleAmbassador" => Self::Ambassador,
            "MercenaryCaptain" => Self::MercenaryCaptain,
            "TerritoryManagers" => Self::TerritoryManagers,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Chamberlain => "CastleChamberlain",
            Self::SiegeManager => "CastleSiegeManager",
            Self::MercenaryManager => "CastleMercenaryManager",
            Self::Warehouse => "CastleWarehouse",
            Self::Teleporter => "CastleTeleporter",
            Self::Blacksmith => "CastleBlacksmith",
            Self::CourtMagician => "CastleCourtMagician",
            Self::Ambassador => "CastleAmbassador",
            Self::MercenaryCaptain => "MercenaryCaptain",
            Self::TerritoryManagers => "TerritoryManagers",
        }
    }

    /// The script of an NPC template id (`addFirstTalkId`).
    pub fn of_npc_id(npc_id: i32) -> Option<Self> {
        let in_list = |list: &[i32]| list.contains(&npc_id);
        Some(if in_list(&CHAMBERLAINS) {
            Self::Chamberlain
        } else if in_list(&SIEGE_MANAGERS) {
            Self::SiegeManager
        } else if in_list(&MERCENARY_MANAGERS) {
            Self::MercenaryManager
        } else if in_list(&WAREHOUSES) {
            Self::Warehouse
        } else if in_list(&TELEPORTERS) {
            Self::Teleporter
        } else if in_list(&BLACKSMITHS) {
            Self::Blacksmith
        } else if in_list(&COURT_MAGICIANS) {
            Self::CourtMagician
        } else if in_list(&AMBASSADORS) {
            Self::Ambassador
        } else if in_list(&MERCENARY_CAPTAINS) {
            Self::MercenaryCaptain
        } else if in_list(&TERRITORY_MANAGERS) {
            Self::TerritoryManagers
        } else {
            return None;
        })
    }

    /// `addStartNpc`: the siege manager only listens for the first talk.
    fn is_start_npc(self) -> bool {
        self != Self::SiegeManager
    }

    /// `Quest.getPath()`: the folder below `data/scripts`.
    fn path(self) -> String {
        format!("ai/others/{}", self.name())
    }
}

/// The configuration values the castle NPCs read (`Feature.ini`, `General.ini`'s `AllowManor` is
/// the manor's).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResidenceNpcSettings {
    /// `CastleTeleportFunctionFeeRatio` (ms).
    pub cs_tele_fee_ratio: i64,
    pub cs_tele1_fee: i32,
    pub cs_tele2_fee: i32,
    /// `CastleSupportFunctionFeeRatio` (ms).
    pub cs_support_fee_ratio: i64,
    pub cs_support1_fee: i32,
    pub cs_support2_fee: i32,
    /// `CastleMpRegenerationFunctionFeeRatio` (ms).
    pub cs_mpreg_fee_ratio: i64,
    pub cs_mpreg1_fee: i32,
    pub cs_mpreg2_fee: i32,
    /// `CastleHpRegenerationFunctionFeeRatio` (ms).
    pub cs_hpreg_fee_ratio: i64,
    pub cs_hpreg1_fee: i32,
    pub cs_hpreg2_fee: i32,
    /// `CastleExpRegenerationFunctionFeeRatio` (ms).
    pub cs_expreg_fee_ratio: i64,
    pub cs_expreg1_fee: i32,
    pub cs_expreg2_fee: i32,
    pub outer_door_upgrade_price2: i32,
    pub outer_door_upgrade_price3: i32,
    pub outer_door_upgrade_price5: i32,
    pub inner_door_upgrade_price2: i32,
    pub inner_door_upgrade_price3: i32,
    pub inner_door_upgrade_price5: i32,
    pub wall_upgrade_price2: i32,
    pub wall_upgrade_price3: i32,
    pub wall_upgrade_price5: i32,
    pub trap_upgrade_price1: i32,
    pub trap_upgrade_price2: i32,
    pub trap_upgrade_price3: i32,
    pub trap_upgrade_price4: i32,
    /// `SevenSignsDawnTicketQuantity`.
    pub ssq_dawn_ticket_quantity: i32,
    /// `SevenSignsDawnTicketPrice`.
    pub ssq_dawn_ticket_price: i32,
    /// `SevenSignsDawnTicketBundle`.
    pub ssq_dawn_ticket_bundle: i32,
    /// `SevenSignsManorsAgreementId`.
    pub ssq_manors_agreement_id: i32,
}

impl Default for ResidenceNpcSettings {
    /// Java's default of each key.
    fn default() -> Self {
        Self {
            cs_tele_fee_ratio: 604_800_000,
            cs_tele1_fee: 1000,
            cs_tele2_fee: 10_000,
            cs_support_fee_ratio: 86_400_000,
            cs_support1_fee: 49_000,
            cs_support2_fee: 120_000,
            cs_mpreg_fee_ratio: 86_400_000,
            cs_mpreg1_fee: 45_000,
            cs_mpreg2_fee: 65_000,
            cs_hpreg_fee_ratio: 86_400_000,
            cs_hpreg1_fee: 12_000,
            cs_hpreg2_fee: 20_000,
            cs_expreg_fee_ratio: 86_400_000,
            cs_expreg1_fee: 63_000,
            cs_expreg2_fee: 70_000,
            outer_door_upgrade_price2: 3_000_000,
            outer_door_upgrade_price3: 4_000_000,
            outer_door_upgrade_price5: 5_000_000,
            inner_door_upgrade_price2: 750_000,
            inner_door_upgrade_price3: 900_000,
            inner_door_upgrade_price5: 1_000_000,
            wall_upgrade_price2: 1_600_000,
            wall_upgrade_price3: 1_800_000,
            wall_upgrade_price5: 2_000_000,
            trap_upgrade_price1: 3_000_000,
            trap_upgrade_price2: 4_000_000,
            trap_upgrade_price3: 5_000_000,
            trap_upgrade_price4: 6_000_000,
            ssq_dawn_ticket_quantity: 300,
            ssq_dawn_ticket_price: 1000,
            ssq_dawn_ticket_bundle: 10,
            ssq_manors_agreement_id: 6388,
        }
    }
}

/// The state of the castle NPCs in the world.
#[derive(Debug, Default)]
pub struct ResidenceNpcWorld {
    pub(crate) settings: ResidenceNpcSettings,
    /// The mass gatekeepers whose `isScriptValue(0)` is false: the teleport is announced and
    /// waits for its timer.
    pub(crate) teleporting: BTreeSet<ObjectId>,
    /// The pending `DESPAWN` quest timer of each envoy.
    pub(crate) envoy_timers: BTreeMap<ObjectId, TimerId>,
}

/// The timers of the castle NPCs (one `TimerKind::ResidenceNpc`): Java's quest timers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidenceNpcTimer {
    /// `startQuestTimer("TELEPORT_EXECUTE", time, npc, null)` of the mass gatekeeper.
    TeleporterExecute(ObjectId),
    /// `startQuestTimer("DESPAWN", time, npc, null)` of an envoy.
    EnvoyDespawn(ObjectId),
    /// The envoy that `onSpawn` deleted (one tick after its spawn).
    EnvoyRemove(ObjectId),
}

/// What the world does once a castle was read again for one of the NPCs (D-50), in
/// `ResidenceReadThen::Npc`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NpcThen {
    /// A chamberlain command (`Script CastleChamberlain <event>`).
    Chamberlain {
        player: ObjectId,
        npc: ObjectId,
        event: String,
    },
    /// "Receive" or the count page of the vault keeper; the castle and its owner's proofs of
    /// alliance were read.
    Warehouse {
        player: ObjectId,
        npc: ObjectId,
        event: String,
    },
    /// The owner's choice at an envoy (`signed`, `rejected`); the fortress was read.
    Ambassador {
        player: ObjectId,
        npc: ObjectId,
        event: String,
    },
}

/// A Java exception inside a script (a missing token, a number that does not parse): Quest
/// catches it, logs it and shows a GM the stack trace; nothing else happens.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ScriptFailed;

/// What an `onEvent` returns: the page, the whole html or the message (`None` for Java's `null`),
/// or the exception.
pub(crate) type Reply = Result<Option<String>, ScriptFailed>;

/// Java `new StringTokenizer(text, " ")`: the words, empty ones skipped.
pub(crate) fn words(text: &str) -> Vec<&str> {
    text.split(' ').filter(|w| !w.is_empty()).collect()
}

/// Java `Integer.parseInt(text)`.
pub(crate) fn parse_int(text: &str) -> Result<i32, ScriptFailed> {
    text.parse::<i32>().map_err(|_| {
        warn!("NumberFormatException: For input string: \"{text}\"");
        ScriptFailed
    })
}

/// Java `Long.parseLong(text)`.
pub(crate) fn parse_long(text: &str) -> Result<i64, ScriptFailed> {
    text.parse::<i64>().map_err(|_| {
        warn!("NumberFormatException: For input string: \"{text}\"");
        ScriptFailed
    })
}

/// Java `FormatUtil.formatAdena(amount)`: `NumberFormat.getIntegerInstance(Locale.ENGLISH)`,
/// comma separated groups of three digits.
pub(crate) fn format_adena(amount: i64) -> String {
    let digits = amount.unsigned_abs().to_string();
    let mut grouped = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(c);
    }
    if amount < 0 {
        format!("-{grouped}")
    } else {
        grouped
    }
}

/// Year, month (1 to 12) and day of the month of a day count since 1970-01-01 (the proleptic
/// Gregorian calendar, like `java.util.Calendar` for any date a function can end on).
pub(crate) fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era = (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

impl World {
    /// Installs the castle NPC configuration (`Feature.ini`).
    pub fn set_residence_npc_settings(&mut self, settings: ResidenceNpcSettings) {
        self.residence_npc.settings = settings;
    }

    /// The castle NPC configuration.
    pub fn residence_npc_settings(&self) -> &ResidenceNpcSettings {
        &self.residence_npc.settings
    }

    /// The script that listens for the NPC, if it is one of this module's.
    pub(crate) fn castle_script_of(&self, npc: ObjectId) -> Option<CastleScript> {
        self.npcs
            .get(&npc)
            .and_then(|n| CastleScript::of_npc_id(n.npc_id))
    }

    /// The NPC template id.
    pub(crate) fn castle_npc_id(&self, npc: ObjectId) -> i32 {
        self.npcs.get(&npc).map_or(0, |n| n.npc_id)
    }

    // ------------------------------------------------------------------ the click

    /// `NpcClick`'s branch for the NPCs of this module: the doormen show their window, a script
    /// with an `ON_NPC_FIRST_TALK` listener answers instead of `showChatWindow`. `true` when one
    /// of them took the click.
    pub(crate) fn residence_npc_on_click(&mut self, npc: ObjectId, player: ObjectId) -> bool {
        if self.castle_doorman_show_chat_window(npc, player) {
            return true;
        }
        self.residence_npc_first_talk(npc, player, true)
    }

    /// The first talk: `NpcClick` (`from_click`, which also makes a start NPC the quest NPC of
    /// the player: `ON_NPC_QUEST_START`) and `Chat 0` (`ChatLink`). `true` when a script of this
    /// module answered.
    pub(crate) fn residence_npc_first_talk(
        &mut self,
        npc: ObjectId,
        player: ObjectId,
        from_click: bool,
    ) -> bool {
        let Some(script) = self.castle_script_of(npc) else {
            return false;
        };
        if from_click
            && script.is_start_npc()
            && let Some(p) = self.players.get_mut(&player)
        {
            p.talk.last_quest_npc_object = npc.0;
        }
        // `Quest.notifyFirstTalk`.
        match self.castle_script_first_talk(script, npc, player) {
            Ok(result) => self.castle_script_result(script, player, result.as_deref(), npc),
            Err(ScriptFailed) => {
                warn!("{script:?}: onFirstTalk failed for {player} at {npc}.");
            }
        }
        true
    }

    /// The `onFirstTalk` of each script.
    fn castle_script_first_talk(&mut self, script: CastleScript, npc: ObjectId, player: ObjectId) -> Reply {
        match script {
            CastleScript::Chamberlain => Ok(Some(
                if self.castle_script_is_owner(player, npc) {
                    "chamberlain-01.html"
                } else {
                    "chamberlain-04.html"
                }
                .to_owned(),
            )),
            CastleScript::SiegeManager => Ok(self.siege_manager_first_talk(npc, player)),
            CastleScript::MercenaryManager => Ok(Some(self.mercenary_manager_first_talk(npc, player))),
            CastleScript::Warehouse => Ok(Some("warehouse-01.html".to_owned())),
            CastleScript::Teleporter => Ok(Some(self.castle_teleporter_first_talk(npc))),
            CastleScript::Blacksmith => Ok(Some(self.blacksmith_first_talk(npc, player))),
            CastleScript::CourtMagician => Ok(Some(
                if self.clan_of(player).is_some() && self.clan_id_of(player) == self.castle_owner_of_npc(npc)
                {
                    "courtmagician.html"
                } else {
                    "courtmagician-01.html"
                }
                .to_owned(),
            )),
            CastleScript::Ambassador => {
                self.ambassador_first_talk(npc, player);
                Ok(None)
            }
            CastleScript::MercenaryCaptain => Ok(Some(self.captain_first_talk(npc, player))),
            CastleScript::TerritoryManagers => Ok(Some(self.territory_manager_first_talk(npc, player))),
        }
    }

    // ------------------------------------------------------------------ Script <Name> <event>

    /// Java `Player.processScriptEvent(scriptName, event)` for the scripts of this module. `false`
    /// when the script is another one.
    pub(crate) fn residence_script_event(
        &mut self,
        player: ObjectId,
        script_name: &str,
        event: &str,
    ) -> bool {
        let Some(script) = CastleScript::by_name(script_name) else {
            return false;
        };
        if event.is_empty() {
            return true;
        }
        let Some(npc) = self.script_event_npc(player) else {
            return true;
        };
        self.castle_script_notify_event(script, event, npc, player);
        true
    }

    /// The NPC a `Script` event of the player goes to: the last folk the player talked to when it
    /// is in reach, else the last quest NPC in reach.
    pub(crate) fn script_event_npc(&self, player: ObjectId) -> Option<ObjectId> {
        let talk = &self.players.get(&player)?.talk;
        let (folk, quest_npc) = (talk.last_folk_npc, talk.last_quest_npc_object);
        let in_range = |npc: ObjectId| match (self.position_of(player), self.position_of(npc)) {
            (Some(me), Some(pos)) => distance_2d(me, pos) < f64::from(INTERACTION_DISTANCE),
            _ => false,
        };
        match folk.filter(|n| self.npcs.contains_key(n)) {
            Some(target) if in_range(target) => Some(target),
            _ if quest_npc > 0 => self
                .find_object(ObjectId(quest_npc))
                .filter(|o| self.npcs.contains_key(o))
                .filter(|o| in_range(*o)),
            _ => None,
        }
    }

    /// `Quest.notifyEvent(event, npc, player)`: `onEvent`, then `showResult`.
    fn castle_script_notify_event(
        &mut self,
        script: CastleScript,
        event: &str,
        npc: ObjectId,
        player: ObjectId,
    ) {
        // The chamberlain acts on the castle row: it is read again first (D-50).
        // The vault keeper hands out the proofs of alliance the Java game server adds to: the
        // castle (with its owner's count) is read again before it shows or gives them (D-50).
        let reread = match script {
            CastleScript::Chamberlain if chamberlain::needs_reread(event) => true,
            CastleScript::Warehouse => matches!(event, "Receive" | "warehouse-04.html"),
            _ => false,
        };
        if reread {
            let (player, npc, event) = (player, npc, event.to_owned());
            let then = if script == CastleScript::Chamberlain {
                NpcThen::Chamberlain { player, npc, event }
            } else {
                NpcThen::Warehouse { player, npc, event }
            };
            match self.npc_castle(npc) {
                Some(castle) => self.request_residence_reread(
                    crate::residence::ResidenceRef::Castle(castle),
                    crate::residence::ResidenceReadThen::Npc(then),
                ),
                None => warn!("{player}: the castle NPC {npc} stands at no castle."),
            }
            return;
        }
        self.castle_script_run_event(script, event, npc, player);
    }

    /// `onEvent` and `showResult`.
    fn castle_script_run_event(
        &mut self,
        script: CastleScript,
        event: &str,
        npc: ObjectId,
        player: ObjectId,
    ) {
        let reply = match script {
            CastleScript::Chamberlain => self.chamberlain_on_event(event, npc, player),
            CastleScript::SiegeManager => Ok(None),
            CastleScript::MercenaryManager => self.mercenary_manager_on_event(event, npc, player),
            CastleScript::Warehouse => self.warehouse_on_event(event, npc, player),
            CastleScript::Teleporter => self.castle_teleporter_on_event(event, npc),
            CastleScript::Blacksmith => Ok(self.blacksmith_on_event(event, npc, player)),
            CastleScript::CourtMagician => self.court_magician_on_event(event, npc, player),
            CastleScript::Ambassador => Ok(self.ambassador_on_event(event, npc, Some(player))),
            CastleScript::MercenaryCaptain => self.captain_on_event(event, npc, player),
            CastleScript::TerritoryManagers => self.territory_manager_on_event(event, npc, player),
        };
        match reply {
            Ok(result) => self.castle_script_result(script, player, result.as_deref(), npc),
            // `Quest.showError`.
            Err(ScriptFailed) => warn!("{}: onEvent \"{event}\" failed for {player}.", script.name()),
        }
    }

    /// The answer of a castle read again for a script (D-50).
    pub(crate) fn residence_npc_after_reread(&mut self, then: NpcThen) {
        match then {
            NpcThen::Chamberlain { player, npc, event } => {
                if !self.players.contains_key(&player) || !self.npcs.contains_key(&npc) {
                    return;
                }
                self.castle_script_run_event(CastleScript::Chamberlain, &event, npc, player);
            }
            NpcThen::Warehouse { player, npc, event } => {
                if !self.players.contains_key(&player) || !self.npcs.contains_key(&npc) {
                    return;
                }
                self.castle_script_run_event(CastleScript::Warehouse, &event, npc, player);
            }
            NpcThen::Ambassador { player, npc, event } => {
                if !self.players.contains_key(&player) || !self.npcs.contains_key(&npc) {
                    return;
                }
                self.ambassador_run_event(&event, npc, Some(player));
            }
        }
    }

    /// The timers of the castle NPCs.
    pub(crate) fn on_residence_npc_timer(&mut self, timer: ResidenceNpcTimer) {
        match timer {
            ResidenceNpcTimer::TeleporterExecute(npc) => self.castle_teleporter_execute(npc),
            ResidenceNpcTimer::EnvoyDespawn(npc) => self.ambassador_despawn_timer(npc),
            ResidenceNpcTimer::EnvoyRemove(npc) => {
                self.delete_npc(npc);
            }
        }
    }

    /// The `onSpawn` of the scripts (`addSpawnId`): only the envoys have one.
    pub(crate) fn residence_npc_on_spawn(&mut self, npc: ObjectId) {
        if self.castle_script_of(npc) == Some(CastleScript::Ambassador) {
            self.ambassador_on_spawn(npc);
        }
    }

    /// Whether the NPC is gone from the world before its timer fired.
    pub(crate) fn residence_npc_forget(&mut self, npc: ObjectId) {
        self.residence_npc.teleporting.remove(&npc);
        if let Some(timer) = self.residence_npc.envoy_timers.remove(&npc) {
            self.timers.cancel(timer);
        }
    }

    // ------------------------------------------------------------------ helpers

    /// `npc.getCastle().getOwnerId()`; 0 for an NPC that has no castle.
    pub(crate) fn castle_owner_of_npc(&self, npc: ObjectId) -> i32 {
        self.npc_castle(npc)
            .and_then(|c| self.castle(c))
            .map_or(0, |c| c.owner_id)
    }

    /// `CastleChamberlain.isOwner(player, npc)`.
    pub(crate) fn castle_script_is_owner(&self, player: ObjectId, npc: ObjectId) -> bool {
        self.is_gm(player)
            || (self.clan_of(player).is_some() && self.clan_id_of(player) == self.castle_owner_of_npc(npc))
    }

    /// `npc.getCastle().getSiege().isInProgress()`; `None` where Java's `getCastle()` is null.
    pub(crate) fn castle_siege_running_at(&self, npc: ObjectId) -> Option<bool> {
        let castle: CastleId = self.npc_castle(npc)?;
        Some(self.is_castle_siege_in_progress(castle))
    }

    /// `Quest.showResult(player, res, npc)`: a page of the script, a whole page, or a message.
    pub(crate) fn castle_script_result(
        &mut self,
        script: CastleScript,
        player: ObjectId,
        res: Option<&str>,
        npc: ObjectId,
    ) {
        let Some(res) = res.filter(|r| !r.is_empty()) else {
            return;
        };
        if res.ends_with(".htm") || res.ends_with(".html") {
            // `showHtmlFile`: `%objectId%` and `%playername%` replaced, then an `ActionFailed`;
            // nothing at all when the page is missing.
            match self.castle_script_htm(script, player, res) {
                Some(content) => {
                    let html = content.replace("%objectId%", &npc.0.to_string());
                    self.castle_script_send_page(player, npc, &html);
                    self.send(player, &ActionFailed);
                }
                None => debug!("{player}: page {res} of {} not found.", script.name()),
            }
        } else if res.starts_with("<html") {
            self.castle_script_send_page(player, npc, res);
            self.send(player, &ActionFailed);
        } else {
            self.send_message(player, res);
        }
    }

    /// `NpcHtmlMessage(npcObjId, html)` with `replace("%playername%", name)`.
    fn castle_script_send_page(&mut self, player: ObjectId, npc: ObjectId, html: &str) {
        let name = self
            .players
            .get(&player)
            .map(|p| p.data.name.clone())
            .unwrap_or_default();
        if let Some(message) = NpcHtmlMessage::new(npc.0, &html.replace("%playername%", &name)) {
            self.send_html(player, &message);
        }
    }

    /// Java `Quest.getHtm(player, fileName)`: the script's folder, then
    /// `data/scripts/quests/<name>`.
    pub(crate) fn castle_script_htm(
        &self,
        script: CastleScript,
        player: ObjectId,
        file: &str,
    ) -> Option<String> {
        [
            format!("data/scripts/{}/{file}", script.path()),
            format!("data/scripts/quests/{}/{file}", script.name()),
        ]
        .iter()
        .find_map(|path| self.get_htm(player, path))
    }

    /// `getHtmlPacket(player, npc, file)` and the `html.replace(..)` calls, in order. `None`
    /// where Java's `setHtml(null)` throws.
    pub(crate) fn castle_script_packet(
        &self,
        script: CastleScript,
        player: ObjectId,
        npc: ObjectId,
        file: &str,
        replace: &[(&str, String)],
    ) -> Option<NpcHtmlMessage> {
        let Some(mut html) = self.castle_script_htm(script, player, file) else {
            warn!("{}: page {file} not found.", script.name());
            return None;
        };
        for (pattern, value) in replace {
            html = html.replace(pattern, value);
        }
        NpcHtmlMessage::new(npc.0, &html)
    }

    /// `player.sendPacket(html)` of a page built by [`Self::castle_script_packet`].
    pub(crate) fn castle_script_send_packet(
        &mut self,
        script: CastleScript,
        player: ObjectId,
        npc: ObjectId,
        file: &str,
        replace: &[(&str, String)],
    ) -> bool {
        match self.castle_script_packet(script, player, npc, file, replace) {
            Some(message) => {
                self.send_html(player, &message);
                true
            }
            None => false,
        }
    }

    /// Java `Quest.giveItems(player, itemId, count)`: the items, "You have earned ...", and the
    /// load.
    // NOTE: `RatesConfig.QUEST_ITEM_DROP_AMOUNT_MULTIPLIER` applies to quest items only; none of
    // the items these scripts give is one.
    pub(crate) fn script_give_items(&mut self, player: ObjectId, item_id: i32, count: i64) {
        use l2_protocol_game::SystemMessageId;
        use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
        if count <= 0 {
            return;
        }
        if self.add_item(player, item_id, count, false).is_none() {
            return;
        }
        if item_id == l2_data::item::ADENA_ID {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouHaveEarnedS1Adena)
                    .with(SystemMessageParam::LongNumber(count)),
            );
        } else if count > 1 {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouHaveEarnedS2S1S)
                    .with(SystemMessageParam::ItemName(item_id))
                    .with(SystemMessageParam::LongNumber(count)),
            );
        } else {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouHaveEarnedS1)
                    .with(SystemMessageParam::ItemName(item_id)),
            );
        }
        self.send_cur_load(player);
    }

    /// Java `Quest.takeItems(player, itemId, amount)`.
    pub(crate) fn script_take_items(&mut self, player: ObjectId, item_id: i32, amount: i64) {
        self.destroy_item_by_item_id(player, item_id, amount, true);
    }

    /// Java `Quest.hasQuestItems(player, itemId)`.
    pub(crate) fn script_has_items(&self, player: ObjectId, item_id: i32) -> bool {
        self.inventory_item_by_item_id(player, item_id).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adena_is_grouped_by_thousands_like_number_format() {
        assert_eq!(format_adena(0), "0");
        assert_eq!(format_adena(999), "999");
        assert_eq!(format_adena(1000), "1,000");
        assert_eq!(format_adena(1_234_567), "1,234,567");
        assert_eq!(format_adena(-1_234_567), "-1,234,567");
        assert_eq!(format_adena(99_900_000_000), "99,900,000,000");
    }

    #[test]
    fn the_calendar_fields_of_a_function_end() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19_723), (2024, 1, 1));
        assert_eq!(civil_from_days(11_016), (2000, 2, 29));
    }

    #[test]
    fn a_string_tokenizer_skips_empty_words() {
        assert_eq!(words("  a  b c "), ["a", "b", "c"]);
        assert!(words("   ").is_empty());
        assert!(parse_int("12").is_ok() && parse_int("1x").is_err() && parse_int("").is_err());
    }
}
