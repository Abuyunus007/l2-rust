//! Punishments: bans, chat bans, party bans and jail (`punishments` table), and the penalty of an
//! illegal player action.
// Java: managers/PunishmentManager.java
// Java: mechanics/punishment/{PunishmentTask,PunishmentAffect,PunishmentType}.java
// Java: data/holders/PunishmentHolder.java
// Java: entity/actor/tasks/player/IllegalPlayerActionTask.java, TeleportTask.java
// Java: entity/actor/enums/player/IllegalActionPunishmentType.java
// Java: dist/game/data/scripts/handlers/punishments/{BanHandler,ChatBanHandler,JailHandler}.java
// Java: entity/zone/type/JailZone.java (the two locations), commons/time/TimeUtil.formatDuration
//
// # How it fits (D-39)
// The world owns the live punishments (Java's `PunishmentHolder`s). The rows of the `punishments`
// table are loaded before the world starts (`World::load_punishments`), a new punishment goes
// out as `PersistCommand::StorePunishment` (Java's `INSERT`), an ended one as
// `PersistCommand::EndPunishment` (the `UPDATE` of `expiration` to the time it ended). The Java
// game server reads and writes the same rows. A ban of an account tells the login server
// (`LsEvent::AccessLevel`, Java `sendAccessLevel`), a ban of a character writes
// `characters.accessLevel` (`PersistCommand::CharAccessLevel`, Java `changeCharAccessLevel`).
//
// The expiry of a punishment is a timer of the wheel (Java `ThreadPool.schedule`), like the five
// second delay of an illegal action.
//
// # Left out, with the system that brings it
// - `HWID` punishments never match: the Rust game server has no hardware info (the key is kept
//   and stored, nothing is affected).
// - A client that has an account but no character in the world (the character screen) is not
//   kicked by an account ban (Java: `Disconnection.of(client)`); it is refused when it enters.
// - `OlympiadManager.removeDisconnectedCompetitor` of the jail handler (M7), the `PARTY_BAN`
//   check of `RequestJoinParty` (parties: M6).

use std::collections::{BTreeMap, HashMap};

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{ChatType, NpcHtmlMessage, SystemMessage};
use l2_util::ObjectId;
use tracing::{info, warn};

use crate::command::LeaveReason;
use crate::outbox::{LsEvent, PersistCommand};
use crate::teleport::TeleportTarget;
use crate::timer::{TimerId, TimerKind, TimerWheel};
use crate::world::{World, unix_ms_now};
use crate::zones::ZoneId;

/// Java `JailZone.JAIL_IN_LOC`.
pub const JAIL_IN: (i32, i32, i32) = (-114_356, -249_645, -2984);
/// Java `JailZone.JAIL_OUT_LOC`.
pub const JAIL_OUT: (i32, i32, i32) = (17_836, 170_178, -3507);

/// `ThreadPool.schedule(new IllegalPlayerActionTask(...), 5000)`.
const ILLEGAL_ACTION_DELAY_MS: u64 = 5000;
/// `ThreadPool.schedule(new TeleportTask(...), 2000)`.
const TELEPORT_DELAY_MS: u64 = 2000;

/// Java `PunishmentAffect`: what a punishment's key names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PunishmentAffect {
    Account,
    Character,
    Ip,
    Hwid,
}

impl PunishmentAffect {
    /// Java declaration order.
    pub const ALL: [PunishmentAffect; 4] = [Self::Account, Self::Character, Self::Ip, Self::Hwid];

    /// Java `name()`, the value of the `affect` column.
    pub fn name(self) -> &'static str {
        match self {
            Self::Account => "ACCOUNT",
            Self::Character => "CHARACTER",
            Self::Ip => "IP",
            Self::Hwid => "HWID",
        }
    }

    /// Java `getByName`: ignoring case, `None` for an unknown name.
    pub fn by_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|a| a.name().eq_ignore_ascii_case(name))
    }
}

/// Java `PunishmentType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PunishmentType {
    Ban,
    ChatBan,
    PartyBan,
    Jail,
}

impl PunishmentType {
    /// Java declaration order.
    pub const ALL: [PunishmentType; 4] = [Self::Ban, Self::ChatBan, Self::PartyBan, Self::Jail];

    /// Java `name()`, the value of the `type` column.
    pub fn name(self) -> &'static str {
        match self {
            Self::Ban => "BAN",
            Self::ChatBan => "CHAT_BAN",
            Self::PartyBan => "PARTY_BAN",
            Self::Jail => "JAIL",
        }
    }

    /// Java `getByName`: ignoring case, `None` for an unknown name.
    pub fn by_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|t| t.name().eq_ignore_ascii_case(name))
    }
}

/// Java `IllegalActionPunishmentType` (`DefaultPunish`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IllegalActionPunishment {
    None,
    Broadcast,
    Kick,
    KickBan,
    Jail,
}

impl IllegalActionPunishment {
    /// Java `name()`.
    pub fn name(self) -> &'static str {
        match self {
            Self::None => "NONE",
            Self::Broadcast => "BROADCAST",
            Self::Kick => "KICK",
            Self::KickBan => "KICKBAN",
            Self::Jail => "JAIL",
        }
    }

    /// Java `findByName`: ignoring case, an unknown name is `NONE`.
    pub fn find_by_name(name: &str) -> Self {
        [Self::None, Self::Broadcast, Self::Kick, Self::KickBan, Self::Jail]
            .into_iter()
            .find(|p| p.name().eq_ignore_ascii_case(name))
            .unwrap_or(Self::None)
    }
}

/// The values the punishments read (Java `GeneralConfig`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PunishmentSettings {
    /// `DefaultPunish`.
    pub default_punish: IllegalActionPunishment,
    /// `DEFAULT_PUNISH_PARAM`: seconds a `KICKBAN` or `JAIL` lasts (a zero `DefaultPunishParam`
    /// is already the one hundred years Java makes of it).
    pub default_punish_param: i64,
    /// `JailDisableChat`.
    pub jail_disable_chat: bool,
    /// `BanChatChannels`: the channels a chat ban closes.
    pub ban_chat_channels: Vec<ChatType>,
}

impl Default for PunishmentSettings {
    fn default() -> Self {
        Self {
            default_punish: IllegalActionPunishment::Kick,
            default_punish_param: 3_155_695_200,
            jail_disable_chat: true,
            // The shipped `General.ini` value. NOTE(java-quirk): Java's own default names `WORLD`,
            // which is not a chat type of High Five: `Enum.valueOf` would throw while loading.
            ban_chat_channels: vec![
                ChatType::General,
                ChatType::Shout,
                ChatType::Trade,
                ChatType::HeroVoice,
                ChatType::Whisper,
            ],
        }
    }
}

/// A row of the `punishments` table as the game server hands it to the world at startup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PunishmentRecord {
    pub id: i32,
    pub key: String,
    pub affect: String,
    pub kind: String,
    pub expiration: i64,
    pub reason: String,
    pub punished_by: String,
}

/// What `PunishmentTask.onStart` stores: the `INSERT` of a new punishment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredPunishment {
    pub key: String,
    pub affect: PunishmentAffect,
    pub kind: PunishmentType,
    pub expiration: i64,
    pub reason: String,
    pub punished_by: String,
}

/// The arguments of Java's `new PunishmentTask(...)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewPunishment {
    pub key: String,
    pub affect: PunishmentAffect,
    pub kind: PunishmentType,
    /// Epoch milliseconds; 0 or less is forever.
    pub expiration: i64,
    pub reason: String,
    pub punished_by: String,
    /// Java `isStored`: `true` for a task that must not be inserted (the effects of the bot
    /// report keep their punishment in memory only).
    pub is_stored: bool,
}

impl NewPunishment {
    /// `new PunishmentTask(key, affect, type, expirationTime, reason, punishedBy)`.
    pub fn new(
        key: impl ToString,
        affect: PunishmentAffect,
        kind: PunishmentType,
        expiration: i64,
        reason: &str,
        punished_by: &str,
    ) -> Self {
        Self {
            key: key.to_string(),
            affect,
            kind,
            expiration,
            reason: reason.to_owned(),
            punished_by: punished_by.to_owned(),
            is_stored: false,
        }
    }

    /// `new PunishmentTask(0, key, affect, type, 0, reason, "system", true)`: the punishment of a
    /// bot report effect.
    pub fn in_memory(key: impl ToString, kind: PunishmentType, reason: &str) -> Self {
        Self {
            is_stored: true,
            ..Self::new(key, PunishmentAffect::Character, kind, 0, reason, "system")
        }
    }
}

/// Java `PunishmentTask`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PunishmentTask {
    /// The database id (0 until the persistence task knows it; it finds the row by the key then).
    id: i32,
    key: String,
    affect: PunishmentAffect,
    kind: PunishmentType,
    expiration: i64,
    reason: String,
    punished_by: String,
    /// Java `_isStored`.
    stored: bool,
    /// A row of the table belongs to this task (a task made `stored` by the caller has none).
    in_db: bool,
    timer: Option<TimerId>,
}

impl PunishmentTask {
    /// Java `isExpired()`.
    fn is_expired(&self, now: i64) -> bool {
        self.expiration > 0 && now > self.expiration
    }
}

/// The identity of a punishment in the holders: Java's `PunishmentHolder` keys it by
/// `String.valueOf(key)` and type, one holder per affect.
type TaskKey = (PunishmentAffect, String, PunishmentType);

/// The punishments of the world (Java `PunishmentManager`).
#[derive(Debug, Default)]
pub(crate) struct PunishmentWorld {
    tasks: BTreeMap<TaskKey, PunishmentTask>,
    settings: PunishmentSettings,
    /// Java `CharInfoTable`: the character ids by lower case name, for the characters that are
    /// not online (loaded at startup; every character that enters is added).
    names: HashMap<String, i32>,
}

/// The timers of this module (one `TimerKind::Punishment`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PunishmentTimer {
    /// `PunishmentTask.run()`: the end of a punishment.
    Expire {
        key: String,
        affect: PunishmentAffect,
        kind: PunishmentType,
    },
    /// `IllegalPlayerActionTask.run()`.
    IllegalAction(IllegalPlayerActionTask),
    /// `TeleportTask.run()`.
    Teleport {
        player: ObjectId,
        x: i32,
        y: i32,
        z: i32,
    },
}

/// Java `IllegalPlayerActionTask`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IllegalPlayerActionTask {
    actor: ObjectId,
    /// `Player.toString()` of the actor, for the audit line.
    actor_text: String,
    actor_was_gm: bool,
    message: String,
    punishment: IllegalActionPunishment,
}

/// Java `TimeUtil.formatDuration(millis)`.
pub fn format_duration(millis: i64) -> String {
    if millis < 1 {
        return "0 milliseconds".to_owned();
    }
    let mut rest = millis;
    let days = rest / 86_400_000;
    rest %= 86_400_000;
    let hours = rest / 3_600_000;
    rest %= 3_600_000;
    let minutes = rest / 60_000;
    rest %= 60_000;
    let seconds = rest / 1000;
    let millis = rest % 1000;

    let mut text = String::new();
    for (value, unit) in [
        (days, "day"),
        (hours, "hour"),
        (minutes, "minute"),
        (seconds, "second"),
    ] {
        if value > 0 {
            text.push_str(&format!("{value} {unit}{}, ", if value > 1 { "s" } else { "" }));
        }
    }
    if millis > 0 {
        text.push_str(&format!(
            "{millis} millisecond{}",
            if millis > 1 { "s" } else { "" }
        ));
    }
    // Remove the trailing comma and space, if present.
    if text.len() > 2 && text.ends_with(", ") {
        text.truncate(text.len() - 2);
    }
    text
}

impl World {
    // ------------------------------------------------------------------ setup

    /// Installs the values the punishments read. Call before players enter.
    pub fn set_punishment_settings(&mut self, settings: PunishmentSettings) {
        self.punishments.settings = settings;
    }

    /// `DefaultPunish`.
    pub fn default_punish(&self) -> IllegalActionPunishment {
        self.punishments.settings.default_punish
    }

    pub(crate) fn punishment_settings(&self) -> &PunishmentSettings {
        &self.punishments.settings
    }

    /// Java `CharInfoTable`'s constructor: the names of the characters in the database. Call
    /// before players enter.
    pub fn load_char_names(&mut self, names: impl IntoIterator<Item = (i32, String)>) {
        for (id, name) in names {
            self.punishments.names.insert(name.to_lowercase(), id);
        }
    }

    /// Java `CharInfoTable.getIdByName`: online characters first, then the startup table.
    pub fn char_id_by_name(&self, name: &str) -> Option<i32> {
        self.player_by_name(name)
            .map(|id| id.0)
            .or_else(|| self.punishments.names.get(&name.to_lowercase()).copied())
    }

    /// Java `PunishmentManager.load()`: the stored punishments. A row with an unknown affect or
    /// type is skipped; an expired one is only counted. Nobody is online yet, so no handler has
    /// anything to do (Java's `onStart` finds no player either).
    pub fn load_punishments(&mut self, rows: Vec<PunishmentRecord>) {
        let now = unix_ms_now();
        let (mut initiated, mut expired) = (0, 0);
        for row in rows {
            let (Some(affect), Some(kind)) = (
                PunishmentAffect::by_name(&row.affect),
                PunishmentType::by_name(&row.kind),
            ) else {
                continue;
            };
            let mut task = PunishmentTask {
                id: row.id,
                key: row.key,
                affect,
                kind,
                expiration: row.expiration,
                reason: row.reason,
                punished_by: row.punished_by,
                stored: true,
                in_db: true,
                timer: None,
            };
            if task.is_expired(now) {
                expired += 1;
                continue;
            }
            initiated += 1;
            task.timer = self.schedule_punishment_end(&task, now);
            self.punishments
                .tasks
                .insert((affect, task.key.clone(), kind), task);
        }
        info!("PunishmentManager: Loaded {initiated} active and {expired} expired punishments.");
    }

    // ------------------------------------------------------------------ queries

    /// Java `hasPunishment(key, affect, type)`.
    pub fn has_punishment(&self, key: &str, affect: PunishmentAffect, kind: PunishmentType) -> bool {
        self.punishments
            .tasks
            .contains_key(&(affect, key.to_owned(), kind))
    }

    /// Java `getPunishmentExpiration(key, affect, type)`: 0 when there is none.
    pub fn punishment_expiration(&self, key: &str, affect: PunishmentAffect, kind: PunishmentType) -> i64 {
        self.punishments
            .tasks
            .get(&(affect, key.to_owned(), kind))
            .map_or(0, |t| t.expiration)
    }

    /// The `account`, `IP` keys of a player (the `HWID` key does not exist here).
    fn punishment_keys(&self, id: ObjectId) -> Option<(String, Option<String>)> {
        let player = self.players.get(&id)?;
        Some((
            player.data.account.clone(),
            player.client_ip.map(|ip| ip.to_string()),
        ))
    }

    /// The pattern of `Player.isJailed()`, `isChatBanned()`: the character, its account and its
    /// IP address.
    fn is_punished(&self, id: ObjectId, kind: PunishmentType) -> bool {
        let Some((account, ip)) = self.punishment_keys(id) else {
            return false;
        };
        self.has_punishment(&id.0.to_string(), PunishmentAffect::Character, kind)
            || self.has_punishment(&account, PunishmentAffect::Account, kind)
            || ip.is_some_and(|ip| self.has_punishment(&ip, PunishmentAffect::Ip, kind))
    }

    /// Java `Player.isJailed()`.
    pub fn is_jailed(&self, id: ObjectId) -> bool {
        self.is_punished(id, PunishmentType::Jail)
    }

    /// Java `Player.isChatBanned()`.
    pub fn is_chat_banned(&self, id: ObjectId) -> bool {
        self.is_punished(id, PunishmentType::ChatBan)
    }

    /// Java `Player.isPartyBanned()`: only the character's own ban counts.
    pub fn is_party_banned(&self, id: ObjectId) -> bool {
        self.has_punishment(
            &id.0.to_string(),
            PunishmentAffect::Character,
            PunishmentType::PartyBan,
        )
    }

    /// Java `CharacterSelect`: the character, its account and its IP address are not banned.
    pub(crate) fn is_banned_at_entry(&self, id: ObjectId, account: &str, ip: Option<&str>) -> bool {
        self.has_punishment(
            &id.0.to_string(),
            PunishmentAffect::Character,
            PunishmentType::Ban,
        ) || self.has_punishment(account, PunishmentAffect::Account, PunishmentType::Ban)
            || ip.is_some_and(|ip| self.has_punishment(ip, PunishmentAffect::Ip, PunishmentType::Ban))
    }

    // ------------------------------------------------------------------ start and stop

    /// Java `new PunishmentTask(...)` followed by `PunishmentManager.startPunishment(task)`.
    pub fn start_punishment(&mut self, new: NewPunishment) {
        let now = unix_ms_now();
        let mut task = PunishmentTask {
            id: 0,
            key: new.key,
            affect: new.affect,
            kind: new.kind,
            expiration: new.expiration,
            reason: new.reason,
            punished_by: new.punished_by,
            stored: new.is_stored,
            in_db: false,
            timer: None,
        };
        // The constructor: `startPunishment()` does nothing for an expired task.
        if !task.is_expired(now) {
            // `onStart`: store the task, then run the handler of the type.
            if !task.stored {
                self.outbox
                    .persist(PersistCommand::StorePunishment(StoredPunishment {
                        key: task.key.clone(),
                        affect: task.affect,
                        kind: task.kind,
                        expiration: task.expiration,
                        reason: task.reason.clone(),
                        punished_by: task.punished_by.clone(),
                    }));
                task.stored = true;
                task.in_db = true;
            }
            self.punishment_handler_start(&task);
            task.timer = self.schedule_punishment_end(&task, now);
        }

        // `PunishmentManager.startPunishment`: a ban changes the access level.
        if task.kind == PunishmentType::Ban {
            match task.affect {
                PunishmentAffect::Account => self.outbox.ls_event(LsEvent::AccessLevel {
                    account: task.key.clone(),
                    level: -1,
                }),
                PunishmentAffect::Character => self.change_char_access_level(&task.key, -1),
                PunishmentAffect::Ip | PunishmentAffect::Hwid => {}
            }
        }
        // `PunishmentHolder.addPunishment`: an expired task is not kept.
        // NOTE(java-quirk): a second punishment of the same key and type replaces the first, whose
        // end timer stays and ends the new one (the admin commands check `hasPunishment` first).
        if !task.is_expired(now) {
            self.punishments
                .tasks
                .insert((task.affect, task.key.clone(), task.kind), task);
        }
    }

    /// Java `PunishmentManager.stopPunishment(key, affect, type)`.
    pub fn stop_punishment(&mut self, key: &str, affect: PunishmentAffect, kind: PunishmentType) {
        let tasks_key = (affect, key.to_owned(), kind);
        let Some(task) = self.punishments.tasks.get(&tasks_key).cloned() else {
            return;
        };
        if kind == PunishmentType::Ban {
            match affect {
                PunishmentAffect::Account => self.outbox.ls_event(LsEvent::AccessLevel {
                    account: task.key.clone(),
                    level: 0,
                }),
                PunishmentAffect::Character => self.change_char_access_level(&task.key, 0),
                PunishmentAffect::Ip | PunishmentAffect::Hwid => {}
            }
        }
        // `PunishmentTask.stopPunishment()`: abort the timer, `onEnd`.
        if let Some(timer) = task.timer {
            self.timers.cancel(timer);
        }
        // An effect's task has no row (`UPDATE ... WHERE id = 0` changes nothing in Java).
        if task.stored && task.in_db {
            self.outbox.persist(PersistCommand::EndPunishment {
                id: task.id,
                key: task.key.clone(),
                affect,
                kind,
                ended_at: unix_ms_now(),
            });
        }
        self.punishment_handler_end(&task);
        // NOTE(java-quirk): the holder forgets the task only after its handler ran, so the
        // `EtcStatusUpdate` the chat ban handler sends still shows the chat as banned.
        self.punishments.tasks.remove(&tasks_key);
    }

    /// Java `PunishmentManager.changeCharAccessLevel(charId, level)`.
    fn change_char_access_level(&mut self, key: &str, level: i32) {
        match key.parse::<i32>() {
            Ok(char_id) => self
                .outbox
                .persist(PersistCommand::CharAccessLevel { char_id, level }),
            Err(_) => warn!("PunishmentManager: Error updating access level for character: {key}."),
        }
    }

    /// `ThreadPool.schedule(this, _expirationTime - System.currentTimeMillis())` for a task that
    /// expires.
    fn schedule_punishment_end(&mut self, task: &PunishmentTask, now: i64) -> Option<TimerId> {
        if task.expiration <= 0 {
            return None;
        }
        let delay = u64::try_from(task.expiration.saturating_sub(now)).unwrap_or(0);
        let due = self.tick + TimerWheel::ms_to_ticks(delay);
        Some(self.timers.schedule_at(
            due,
            TimerKind::Punishment(PunishmentTimer::Expire {
                key: task.key.clone(),
                affect: task.affect,
                kind: task.kind,
            }),
        ))
    }

    /// Dispatch of `TimerKind::Punishment`.
    pub(crate) fn on_punishment_timer(&mut self, timer: PunishmentTimer) {
        match timer {
            PunishmentTimer::Expire { key, affect, kind } => self.stop_punishment(&key, affect, kind),
            PunishmentTimer::IllegalAction(task) => self.run_illegal_player_action(task),
            PunishmentTimer::Teleport { player, x, y, z } => {
                // `_player != null && _player.isOnline()`; `teleToLocation(loc, true)`.
                if self.players.contains_key(&player) {
                    let offset = self.settings.teleport.max_offset_on_teleport;
                    self.tele_to_location(player, TeleportTarget::at(x, y, z).with_random_offset(offset));
                }
            }
        }
    }

    // ------------------------------------------------------------------ the handlers

    /// The players a punishment affects (the loop of every handler's `onStart` / `onEnd`):
    /// the character, the player of the account, the players of the IP address.
    fn punished_players(&self, task: &PunishmentTask) -> Vec<ObjectId> {
        let mut ids: Vec<ObjectId> = match task.affect {
            PunishmentAffect::Character => task
                .key
                .parse::<i32>()
                .ok()
                .map(ObjectId)
                .filter(|id| self.players.contains_key(id))
                .into_iter()
                .collect(),
            PunishmentAffect::Account => self
                .players
                .values()
                .filter(|p| p.data.account == task.key)
                .map(|p| p.core.object_id)
                .collect(),
            PunishmentAffect::Ip => self
                .players
                .values()
                .filter(|p| p.client_ip.is_some_and(|ip| ip.to_string() == task.key))
                .map(|p| p.core.object_id)
                .collect(),
            // No hardware info.
            PunishmentAffect::Hwid => Vec::new(),
        };
        ids.sort_unstable_by_key(|id| id.0);
        ids
    }

    /// `IPunishmentHandler.onStart(task)`.
    fn punishment_handler_start(&mut self, task: &PunishmentTask) {
        for player in self.punished_players(task) {
            match task.kind {
                // `Disconnection.of(player).storeAndDeleteWith(LeaveWorld.STATIC_PACKET)`.
                PunishmentType::Ban => self.leave(player, LeaveReason::Logout),
                PunishmentType::ChatBan => self.chat_ban_apply(task, player),
                PunishmentType::Jail => self.jail_apply(Some(task), player),
                PunishmentType::PartyBan => {}
            }
        }
    }

    /// `IPunishmentHandler.onEnd(task)`.
    fn punishment_handler_end(&mut self, task: &PunishmentTask) {
        for player in self.punished_players(task) {
            match task.kind {
                PunishmentType::ChatBan => {
                    self.send(player, &SystemMessage::new(SystemMessageId::ChatEnabled));
                    self.send_etc_status_update(player);
                }
                PunishmentType::Jail => self.jail_remove(player),
                // "Should not do anything."
                PunishmentType::Ban | PunishmentType::PartyBan => {}
            }
        }
    }

    /// `ChatBanHandler.applyToPlayer`.
    fn chat_ban_apply(&mut self, task: &PunishmentTask, player: ObjectId) {
        let delay = ((task.expiration - unix_ms_now()) / 1000) + 1;
        if delay > 0 {
            let hours = delay / 3600;
            let minutes = (delay % 3600) / 60;
            let mut message = String::from("You've been chat banned for ");
            if hours > 0 {
                // NOTE(java-quirk): the plural is "'s" ("2 hour's.").
                message.push_str(&format!("{hours} hour{}", if hours > 1 { "'s." } else { "." }));
                if minutes > 0 {
                    message.push_str(" and ");
                }
            }
            if minutes > 0 || hours == 0 {
                message.push_str(&format!(
                    "{minutes} minute{}",
                    if minutes > 1 { "'s." } else { "." }
                ));
            }
            self.send(player, &SystemMessage::new(SystemMessageId::ChatDisabled));
            self.send_message(player, &message);
        } else {
            self.send_message(player, "You've been chat banned forever.");
        }
        // NOTE(java-quirk): sent before the holder has the task, so the icon is not lit yet.
        self.send_etc_status_update(player);
    }

    /// `JailHandler.applyToPlayer(task, player)`; `task` is `None` for the login hook.
    fn jail_apply(&mut self, task: Option<&PunishmentTask>, player: ObjectId) {
        // `player.setInstanceId(0)`.
        if let Some(entry) = self.objects.get_mut(&player) {
            entry.instance_id = 0;
        }
        self.schedule_teleport(player, JAIL_IN);

        // Show the jail reason html message.
        let content = self
            .get_htm(player, "data/html/jail_in.htm")
            .map(|page| {
                page.replace("%reason%", task.map_or("", |t| t.reason.as_str()))
                    .replace("%punishedBy%", task.map_or("", |t| t.punished_by.as_str()))
            })
            .unwrap_or_else(|| "<html><body>You have been put in jail by an admin.</body></html>".to_owned());
        if let Some(message) = NpcHtmlMessage::new(0, &content) {
            self.send_html(player, &message);
        }

        // Inform the player about the jail duration.
        if let Some(task) = task {
            let duration = task.expiration - unix_ms_now();
            if duration > 0 {
                // Round up to nearest second to avoid showing 1 second less.
                let duration = ((duration + 999) / 1000) * 1000;
                self.send_message(
                    player,
                    &format!("You've been jailed for {}!", format_duration(duration)),
                );
            } else {
                self.send_message(player, "You've been jailed forever.");
            }
        }
    }

    /// `JailHandler.removeFromPlayer`.
    fn jail_remove(&mut self, player: ObjectId) {
        self.schedule_teleport(player, JAIL_OUT);
        let content = self.get_htm(player, "data/html/jail_out.htm").unwrap_or_else(|| {
            "<html><body>You are free for now, respect server rules!</body></html>".to_owned()
        });
        if let Some(message) = NpcHtmlMessage::new(0, &content) {
            self.send_html(player, &message);
        }
    }

    /// `ThreadPool.schedule(new TeleportTask(player, loc), 2000)`.
    pub(crate) fn schedule_teleport(&mut self, player: ObjectId, (x, y, z): (i32, i32, i32)) {
        let due = self.tick + TimerWheel::ms_to_ticks(TELEPORT_DELAY_MS);
        self.timers.schedule_at(
            due,
            TimerKind::Punishment(PunishmentTimer::Teleport { player, x, y, z }),
        );
    }

    /// `JailHandler.onPlayerLogin` (`ON_PLAYER_LOGIN`), called at the end of the enter world
    /// sequence.
    pub(crate) fn punishment_on_player_login(&mut self, id: ObjectId) {
        // Remember the name for the admin commands (Java adds it when the character is created).
        if let Some(player) = self.players.get(&id) {
            self.punishments
                .names
                .insert(player.data.name.to_lowercase(), id.0);
        }
        let jailed = self.is_jailed(id);
        let in_jail = self.is_inside_zone(id, ZoneId::Jail);
        if jailed && !in_jail {
            self.jail_apply(None, id);
        } else if jailed {
            // Notify about the remaining jail time.
            let task = self
                .punishments
                .tasks
                .get(&(
                    PunishmentAffect::Character,
                    id.0.to_string(),
                    PunishmentType::Jail,
                ))
                .cloned();
            if let Some(task) = task {
                let remaining = task.expiration - unix_ms_now();
                if remaining > 0 {
                    // Round up to nearest second to avoid showing 1 second less.
                    let remaining = ((remaining + 999) / 1000) * 1000;
                    self.send_message(
                        id,
                        &format!("Jail time remaining: {}.", format_duration(remaining)),
                    );
                } else {
                    self.send_message(id, "You are jailed forever.");
                }
            }
        } else if in_jail && !self.is_gm(id) {
            self.jail_remove(id);
        }
    }

    /// `JailZone.onExit` for a jailed player: "teleport him back to jail".
    pub(crate) fn jail_zone_exit(&mut self, id: ObjectId) {
        if self.is_jailed(id) {
            self.schedule_teleport(id, JAIL_IN);
            self.send_message(
                id,
                "You cannot cheat your way out of here. You must wait until your jail time is over.",
            );
        }
    }

    // ------------------------------------------------------------------ illegal actions

    /// Java `PunishmentManager.handleIllegalPlayerAction(actor, message, punishment)`: the
    /// constructor of `IllegalPlayerActionTask` tells the player, the task runs five seconds later.
    pub fn handle_illegal_player_action(
        &mut self,
        id: ObjectId,
        message: &str,
        punishment: IllegalActionPunishment,
    ) {
        let Some(player) = self.players.get(&id) else {
            return;
        };
        let actor_text = format!("Player:{}[{}]", player.data.name, id.0);
        let account = player.data.account.clone();
        let is_gm = self.is_gm(id);

        match punishment {
            IllegalActionPunishment::Kick => {
                self.send_message(id, "You will be kicked for illegal action, GM informed.");
            }
            IllegalActionPunishment::KickBan => {
                if !is_gm {
                    // `setAccessLevel(-1)` (and its `broadcastUserInfo`), `setAccountAccesslevel(-1)`.
                    if let Some(player) = self.players.get_mut(&id) {
                        player.data.access_level = -1;
                    }
                    self.broadcast_user_info(id);
                    self.outbox.ls_event(LsEvent::AccessLevel { account, level: -1 });
                }
                self.send_message(id, "You are banned for illegal action, GM informed.");
            }
            IllegalActionPunishment::Jail => {
                self.send_message(id, "Illegal action performed!");
                self.send_message(
                    id,
                    "You will be teleported to GM Consultation Service area and jailed.",
                );
            }
            IllegalActionPunishment::None | IllegalActionPunishment::Broadcast => {}
        }

        let due = self.tick + TimerWheel::ms_to_ticks(ILLEGAL_ACTION_DELAY_MS);
        self.timers.schedule_at(
            due,
            TimerKind::Punishment(PunishmentTimer::IllegalAction(IllegalPlayerActionTask {
                actor: id,
                actor_text,
                actor_was_gm: is_gm,
                message: message.to_owned(),
                punishment,
            })),
        );
    }

    /// `IllegalPlayerActionTask.run()`.
    fn run_illegal_player_action(&mut self, task: IllegalPlayerActionTask) {
        info!(
            target: "audit",
            "AUDIT, {}, {}, {}",
            task.message,
            task.actor_text,
            task.punishment.name()
        );
        let is_gm = if self.players.contains_key(&task.actor) {
            self.is_gm(task.actor)
        } else {
            task.actor_was_gm
        };
        if is_gm {
            return;
        }
        let expiration = unix_ms_now().saturating_add(
            self.punishments
                .settings
                .default_punish_param
                .saturating_mul(1000),
        );
        match task.punishment {
            IllegalActionPunishment::Broadcast => self.broadcast_message_to_gms(&task.message),
            IllegalActionPunishment::Kick => {
                // `Disconnection.of(_actor).storeAndDeleteWith(LeaveWorld.STATIC_PACKET)`.
                self.leave(task.actor, LeaveReason::Logout);
            }
            IllegalActionPunishment::KickBan => self.start_punishment(NewPunishment::new(
                task.actor.0,
                PunishmentAffect::Character,
                PunishmentType::Ban,
                expiration,
                &task.message,
                "IllegalPlayerActionTask",
            )),
            IllegalActionPunishment::Jail => self.start_punishment(NewPunishment::new(
                task.actor.0,
                PunishmentAffect::Character,
                PunishmentType::Jail,
                expiration,
                &task.message,
                "IllegalPlayerActionTask",
            )),
            IllegalActionPunishment::None => {}
        }
    }

    /// Java `AdminData.broadcastMessageToGMs(message)`: `sendMessage` to every GM online.
    pub(crate) fn broadcast_message_to_gms(&self, text: &str) {
        let mut gms: Vec<ObjectId> = self.players.keys().copied().filter(|p| self.is_gm(*p)).collect();
        gms.sort_unstable_by_key(|p| p.0);
        for gm in gms {
            self.send_message(gm, text);
        }
    }

    /// Java `Player.toString()` (`WorldObject.toString`): `Player:name[objectId]`.
    pub(crate) fn player_text(&self, id: ObjectId) -> String {
        self.players.get(&id).map_or_else(
            || format!("Player:?[{}]", id.0),
            |p| format!("Player:{}[{}]", p.data.name, id.0),
        )
    }

    /// The account name of a player, for the illegal action messages.
    pub(crate) fn account_of(&self, id: ObjectId) -> String {
        self.players
            .get(&id)
            .map(|p| p.data.account.clone())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_java_names_and_ignore_case() {
        assert_eq!(
            PunishmentAffect::by_name("character"),
            Some(PunishmentAffect::Character)
        );
        assert_eq!(PunishmentAffect::by_name("Hwid"), Some(PunishmentAffect::Hwid));
        assert_eq!(PunishmentAffect::by_name("clan"), None);
        assert_eq!(PunishmentType::by_name("chat_ban"), Some(PunishmentType::ChatBan));
        assert_eq!(
            PunishmentType::by_name("PARTY_BAN").map(PunishmentType::name),
            Some("PARTY_BAN")
        );
        assert_eq!(PunishmentType::by_name("mute"), None);
        // `IllegalActionPunishmentType.findByName` falls back to NONE.
        assert_eq!(
            IllegalActionPunishment::find_by_name("kickban"),
            IllegalActionPunishment::KickBan
        );
        assert_eq!(
            IllegalActionPunishment::find_by_name("JAIL"),
            IllegalActionPunishment::Jail
        );
        assert_eq!(
            IllegalActionPunishment::find_by_name("whatever"),
            IllegalActionPunishment::None
        );
    }

    #[test]
    fn format_duration_is_time_util() {
        assert_eq!(format_duration(0), "0 milliseconds");
        assert_eq!(format_duration(-5), "0 milliseconds");
        assert_eq!(format_duration(1000), "1 second");
        assert_eq!(format_duration(61_000), "1 minute, 1 second");
        assert_eq!(
            format_duration(2 * 3_600_000 + 30 * 60_000),
            "2 hours, 30 minutes"
        );
        assert_eq!(format_duration(86_400_000 * 3 + 5), "3 days, 5 milliseconds");
        assert_eq!(format_duration(1), "1 millisecond");
        assert_eq!(
            format_duration(86_400_000 + 3_600_000 + 60_000 + 1000 + 2),
            "1 day, 1 hour, 1 minute, 1 second, 2 milliseconds"
        );
    }

    #[test]
    fn an_expiration_in_the_past_has_expired_and_forever_never_does() {
        let task = |expiration| PunishmentTask {
            id: 0,
            key: "1".into(),
            affect: PunishmentAffect::Character,
            kind: PunishmentType::Jail,
            expiration,
            reason: String::new(),
            punished_by: String::new(),
            stored: true,
            in_db: false,
            timer: None,
        };
        assert!(task(1000).is_expired(1001));
        assert!(!task(1000).is_expired(1000));
        assert!(!task(0).is_expired(i64::MAX));
        assert!(!task(-1).is_expired(i64::MAX));
    }
}
