//! Flood protectors: the minimum time between two requests of the same kind, with an optional
//! punishment for a player who keeps sending them (M6-FU-06).
// Java: util/FloodProtectors.java, util/FloodProtectorAction.java, util/FloodProtectorSettings.java
// Java: config/FloodProtectorConfig.java, dist/game/config/FloodProtector.ini
//
// # How it fits
// Java keeps one `FloodProtectors` (fifteen `FloodProtectorAction`s) per `GameClient`. Here every
// player in the world has one [`PlayerFlood`] (World::flood), a handler asks
// `World::flood_allows(player, FloodKind::...)` where Java calls `getFloodProtectors().canXxx()`
// and answers with Java's message (or nothing) when it is refused. A GM is never refused.
//
// The unit is the game tick of 100 ms (`GameTimeTaskManager`, [`World::game_ticks`]): the
// intervals of `FloodProtector.ini` are in those ticks, not in the 50 ms ticks of the world.
//
// The state machine ([`FloodAction`]) does not know the world, so the character screen of the
// game server (`CharacterSelect`, a client without a player) uses the same code.
//
// # Left out
// - `_punishmentInProgress`: Java sets it inside the `synchronized` method and clears it in a
//   `finally` before the method returns, so no other call can ever see it set.
// - The state of a client that logs out and reconnects is new in Java (a new `GameClient`). Here
//   the state of a player goes when the player leaves for good, and stays (until it has run out)
//   when the player only goes back to the character screen, like the client does in Java.

use std::collections::BTreeMap;

use l2_util::ObjectId;
use tracing::warn;

use crate::command::LeaveReason;
use crate::punishments::{NewPunishment, PunishmentAffect, PunishmentType};
use crate::world::{World, unix_ms_now};

/// `GameTimeTaskManager.MILLIS_IN_TICK`.
pub const MILLIS_IN_TICK: i32 = 100;
/// `FloodProtectorAction.MINUTES_TO_MILLISECONDS`.
const MINUTES_TO_MILLISECONDS: i64 = 60_000;
/// `getClass().getSimpleName()`, the "punished by" of the punishment rows.
const PUNISHED_BY: &str = "FloodProtectorAction";

/// The fifteen protectors of `FloodProtectors`, in the order of the Java fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FloodKind {
    /// `canUseItem` (`UseItem`, `RequestPetUseItem`).
    UseItem,
    /// `canRollDice`.
    RollDice,
    /// `canUsePetSummonItem`.
    ItemPetSummon,
    /// `canUseHeroVoice`.
    HeroVoice,
    /// `canUseGlobalChat`.
    GlobalChat,
    /// `canChangeSubclass`.
    Subclass,
    /// `canDropItem`.
    DropItem,
    /// `canUseServerBypass`.
    ServerBypass,
    /// `canUseMultiSell`.
    MultiSell,
    /// `canPerformTransaction`.
    Transaction,
    /// `canManufacture`.
    Manufacture,
    /// `canSendMail`.
    SendMail,
    /// `canSelectCharacter`.
    CharacterSelect,
    /// `canUseItemAuction`.
    ItemAuction,
    /// `canPerformPlayerAction`.
    PlayerAction,
}

impl FloodKind {
    pub const ALL: [FloodKind; 15] = [
        Self::UseItem,
        Self::RollDice,
        Self::ItemPetSummon,
        Self::HeroVoice,
        Self::GlobalChat,
        Self::Subclass,
        Self::DropItem,
        Self::ServerBypass,
        Self::MultiSell,
        Self::Transaction,
        Self::Manufacture,
        Self::SendMail,
        Self::CharacterSelect,
        Self::ItemAuction,
        Self::PlayerAction,
    ];

    fn index(self) -> usize {
        self as usize
    }

    /// The `configString` of `FloodProtectorConfig.loadFloodProtectorConfigs`: the middle of the
    /// keys `FloodProtector<name>Interval`, `...LogFlooding`, `...PunishmentLimit`,
    /// `...PunishmentType` and `...PunishmentTime`.
    pub fn config_name(self) -> &'static str {
        match self {
            Self::UseItem => "UseItem",
            Self::RollDice => "RollDice",
            Self::ItemPetSummon => "ItemPetSummon",
            Self::HeroVoice => "HeroVoice",
            Self::GlobalChat => "GlobalChat",
            Self::Subclass => "Subclass",
            Self::DropItem => "DropItem",
            Self::ServerBypass => "ServerBypass",
            Self::MultiSell => "MultiSell",
            Self::Transaction => "Transaction",
            Self::Manufacture => "Manufacture",
            Self::SendMail => "SendMail",
            Self::CharacterSelect => "CharacterSelect",
            Self::ItemAuction => "ItemAuction",
            Self::PlayerAction => "PlayerAction",
        }
    }

    /// `FloodProtectorSettings.getFloodProtectorType()`, the name in the log lines.
    pub fn type_name(self) -> &'static str {
        match self {
            Self::UseItem => "UseItemFloodProtector",
            Self::RollDice => "RollDiceFloodProtector",
            Self::ItemPetSummon => "ItemPetSummonFloodProtector",
            Self::HeroVoice => "HeroVoiceFloodProtector",
            Self::GlobalChat => "GlobalChatFloodProtector",
            Self::Subclass => "SubclassFloodProtector",
            Self::DropItem => "DropItemFloodProtector",
            Self::ServerBypass => "ServerBypassFloodProtector",
            Self::MultiSell => "MultiSellFloodProtector",
            Self::Transaction => "TransactionFloodProtector",
            Self::Manufacture => "ManufactureFloodProtector",
            Self::SendMail => "SendMailFloodProtector",
            Self::CharacterSelect => "CharacterSelectFloodProtector",
            Self::ItemAuction => "ItemAuctionFloodProtector",
            Self::PlayerAction => "PlayerActionFloodProtector",
        }
    }

    /// The default interval of `loadFloodProtectorConfig(...)` (the value a missing key gets;
    /// the shipped `FloodProtector.ini` sets other numbers for some of them).
    pub fn java_default_interval(self) -> i32 {
        match self {
            Self::UseItem => 4,
            Self::RollDice => 42,
            Self::ItemPetSummon => 16,
            Self::HeroVoice => 100,
            Self::GlobalChat => 5,
            Self::Subclass => 20,
            Self::DropItem => 10,
            Self::ServerBypass => 5,
            Self::MultiSell => 1,
            Self::Transaction => 10,
            Self::Manufacture => 3,
            Self::SendMail => 100,
            Self::CharacterSelect => 30,
            Self::ItemAuction => 9,
            Self::PlayerAction => 3,
        }
    }
}

/// `...PunishmentType`: the string `FloodProtectorAction` switches on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FloodPunishment {
    /// `"kick"`.
    Kick,
    /// `"ban"`: the account.
    Ban,
    /// `"jail"`: the character.
    Jail,
    /// Anything else, including the default `"none"`: Java logs "Unknown punishment type
    /// configured" and does nothing else. The switch is case sensitive.
    Unknown(String),
}

impl FloodPunishment {
    /// The `switch (punishmentType)` of `canPerformAction`.
    pub fn parse(text: &str) -> Self {
        match text {
            "kick" => Self::Kick,
            "ban" => Self::Ban,
            "jail" => Self::Jail,
            other => Self::Unknown(other.to_owned()),
        }
    }
}

/// Java `FloodProtectorSettings` (one protector).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FloodConfig {
    /// `...Interval`, in game ticks of 100 ms.
    pub protection_interval: i32,
    /// `...LogFlooding`.
    pub log_flooding: bool,
    /// `...PunishmentLimit`: the blocked requests after which the punishment starts (0 = never).
    pub punishment_limit: i32,
    /// `...PunishmentType`.
    pub punishment_type: FloodPunishment,
    /// `...PunishmentTime` in milliseconds (the key is in minutes, times 60000).
    pub punishment_time: i64,
}

impl FloodConfig {
    /// A protector with the Java defaults of every key but the interval.
    pub fn with_interval(protection_interval: i32) -> Self {
        Self {
            protection_interval,
            log_flooding: false,
            punishment_limit: 0,
            punishment_type: FloodPunishment::Unknown("none".to_owned()),
            punishment_time: 0,
        }
    }
}

/// The fifteen `FloodProtectorSettings` of `FloodProtectorConfig`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FloodSettings {
    configs: [FloodConfig; 15],
}

impl FloodSettings {
    /// Every key missing from `FloodProtector.ini`: the defaults of `FloodProtectorConfig`.
    pub fn java_defaults() -> Self {
        Self {
            configs: FloodKind::ALL.map(|k| FloodConfig::with_interval(k.java_default_interval())),
        }
    }

    /// No protector refuses anything (interval 0, no punishment): a world that was not given the
    /// configuration, and the tests of the handlers that are not about flooding.
    pub fn disabled() -> Self {
        Self {
            configs: FloodKind::ALL.map(|_| FloodConfig::with_interval(0)),
        }
    }

    pub fn get(&self, kind: FloodKind) -> &FloodConfig {
        &self.configs[kind.index()]
    }

    pub fn set(&mut self, kind: FloodKind, config: FloodConfig) {
        self.configs[kind.index()] = config;
    }

    /// Builder form of [`set`](Self::set) for the tests.
    pub fn with(mut self, kind: FloodKind, config: FloodConfig) -> Self {
        self.set(kind, config);
        self
    }
}

impl Default for FloodSettings {
    /// A world without configuration does not flood protect (see [`FloodSettings::disabled`]);
    /// the server installs the configuration read from `FloodProtector.ini`.
    fn default() -> Self {
        Self::disabled()
    }
}

/// What one call of [`FloodAction::check`] decided and what Java logs about it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FloodCheck {
    /// `canPerformAction()`.
    pub allowed: bool,
    /// The first refused request of an interval, with `LogFlooding` on: "called command X ~N ms
    /// after previous command."
    pub log_flooding_ms: Option<i32>,
    /// The first allowed request after refused ones, with `LogFlooding` on: "issued N extra
    /// requests within ~N ms."
    pub log_extra: Option<i32>,
    /// The punishment limit was reached by this refused request: apply the configured punishment
    /// (Java does it on every refused request from then on, the count is not reset).
    pub punish: bool,
}

/// Java `FloodProtectorAction`'s state: one protector of one client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FloodAction {
    /// `_requestCount`.
    request_count: i32,
    /// `_nextGameTick`. Java starts it at the tick the client was created; every tick since is
    /// not before it, so `i32::MIN` is the same.
    next_game_tick: i32,
    /// `_logged`.
    logged: bool,
}

impl Default for FloodAction {
    fn default() -> Self {
        Self {
            request_count: 0,
            next_game_tick: i32::MIN,
            logged: false,
        }
    }
}

impl FloodAction {
    /// Java `canPerformAction()` without the GM exception and the side effects (the caller logs
    /// and punishes). `current_tick` is `GameTimeTaskManager.getGameTicks()`.
    pub fn check(&mut self, config: &FloodConfig, current_tick: i32) -> FloodCheck {
        if current_tick < self.next_game_tick {
            let mut log_flooding_ms = None;
            if config.log_flooding && !self.logged {
                // NOTE(java-quirk): `interval - (next - current)` is the time since the previous
                // command, in ticks; Java multiplies in `int`.
                let since_previous = config
                    .protection_interval
                    .wrapping_sub(self.next_game_tick.wrapping_sub(current_tick));
                log_flooding_ms = Some(since_previous.wrapping_mul(MILLIS_IN_TICK));
                self.logged = true;
            }
            self.request_count = self.request_count.wrapping_add(1);
            // `punishmentType != null` is always true: the config gives "none" by default.
            let punish = config.punishment_limit > 0 && self.request_count >= config.punishment_limit;
            return FloodCheck {
                allowed: false,
                log_flooding_ms,
                log_extra: None,
                punish,
            };
        }
        let log_extra = (config.log_flooding && self.request_count > 0).then_some(self.request_count);
        self.next_game_tick = current_tick.wrapping_add(config.protection_interval);
        self.logged = false;
        self.request_count = 0;
        FloodCheck {
            allowed: true,
            log_flooding_ms: None,
            log_extra,
            punish: false,
        }
    }

    /// The state has nothing left to remember at `tick`: no interval running and no counted
    /// requests.
    fn is_idle(&self, tick: i32) -> bool {
        tick >= self.next_game_tick && self.request_count == 0
    }
}

/// Java `FloodProtectors`: the fifteen protectors of one client.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlayerFlood {
    actions: [FloodAction; 15],
}

impl PlayerFlood {
    pub fn action_mut(&mut self, kind: FloodKind) -> &mut FloodAction {
        &mut self.actions[kind.index()]
    }

    fn is_idle(&self, tick: i32) -> bool {
        self.actions.iter().all(|a| a.is_idle(tick))
    }
}

/// The flood protectors of the world.
#[derive(Debug, Default)]
pub(crate) struct FloodWorld {
    settings: FloodSettings,
    /// The state of every player who made a request of a kind that is limited.
    states: BTreeMap<ObjectId, PlayerFlood>,
}

/// The "duration" part of the punishment log lines: "forever" or "for N mins.".
fn duration_text(punishment_time: i64) -> String {
    if punishment_time <= 0 {
        "forever".to_owned()
    } else {
        format!("for {} mins.", punishment_time / MINUTES_TO_MILLISECONDS)
    }
}

impl World {
    /// Installs `FloodProtector.ini`. Call before players enter.
    pub fn set_flood_settings(&mut self, settings: FloodSettings) {
        self.flood.settings = settings;
    }

    pub fn flood_settings(&self) -> &FloodSettings {
        &self.flood.settings
    }

    /// Java `getClient().getFloodProtectors().canXxx()` of the player's client: false when the
    /// request came too soon (the handler answers with Java's message, or nothing). A GM is never
    /// refused. A refused request counts towards the punishment of the protector.
    pub(crate) fn flood_allows(&mut self, id: ObjectId, kind: FloodKind) -> bool {
        if self.is_gm(id) {
            return true;
        }
        let tick = self.game_ticks();
        let config = self.flood.settings.get(kind).clone();
        let check = self
            .flood
            .states
            .entry(id)
            .or_default()
            .action_mut(kind)
            .check(&config, tick);

        if let Some(ms) = check.log_flooding_ms {
            warn!(
                "{}: {} called command {} ~{ms} ms after previous command.",
                kind.type_name(),
                self.flood_log_who(id),
                kind.type_name()
            );
        }
        if let Some(extra) = check.log_extra {
            warn!(
                "{}: {} issued {extra} extra requests within ~{} ms.",
                kind.type_name(),
                self.flood_log_who(id),
                config.protection_interval.wrapping_mul(MILLIS_IN_TICK)
            );
        }
        if check.punish {
            self.flood_punish(id, kind, &config);
        }
        check.allowed
    }

    /// `Name(objectId) ` of `buildLogPrefix()` (a player is always in game here).
    fn flood_log_who(&self, id: ObjectId) -> String {
        match self.players.get(&id) {
            Some(p) => format!("{}({}) ", p.data.name, id.0),
            None => String::new(),
        }
    }

    /// The `switch (punishmentType)` of `canPerformAction`.
    fn flood_punish(&mut self, id: ObjectId, kind: FloodKind, config: &FloodConfig) {
        let who = self.flood_log_who(id);
        let prefix = format!("{}: {who}", kind.type_name());
        match &config.punishment_type {
            FloodPunishment::Kick => {
                // `Disconnection.of(_client).storeAndDeleteWith(LeaveWorld.STATIC_PACKET)`.
                self.leave(id, LeaveReason::Logout);
                warn!("{prefix} kicked for flooding");
            }
            FloodPunishment::Ban => {
                let account = self.account_of(id);
                // NOTE(java-quirk): a time of 0 is "forever" in the log line, but the task gets
                // `now + 0`, which is over at once.
                self.start_punishment(NewPunishment::new(
                    account,
                    PunishmentAffect::Account,
                    PunishmentType::Ban,
                    unix_ms_now().wrapping_add(config.punishment_time),
                    "",
                    PUNISHED_BY,
                ));
                warn!(
                    "{prefix} banned for flooding {}",
                    duration_text(config.punishment_time)
                );
            }
            FloodPunishment::Jail => {
                if id.0 > 0 && self.players.contains_key(&id) {
                    self.start_punishment(NewPunishment::new(
                        id.0,
                        PunishmentAffect::Character,
                        PunishmentType::Jail,
                        unix_ms_now().wrapping_add(config.punishment_time),
                        "",
                        PUNISHED_BY,
                    ));
                }
                warn!(
                    "{prefix} jailed for flooding {}",
                    duration_text(config.punishment_time)
                );
            }
            FloodPunishment::Unknown(text) => {
                warn!("FloodProtector: Unknown punishment type configured: {text}");
            }
        }
    }

    /// A player leaves: the state goes with the client, unless the player only returns to the
    /// character screen (the client stays). What has run out is dropped either way.
    pub(crate) fn forget_flood(&mut self, id: ObjectId, reason: LeaveReason) {
        if reason != LeaveReason::Restart {
            self.flood.states.remove(&id);
        }
        let tick = self.game_ticks();
        let players = &self.players;
        self.flood
            .states
            .retain(|owner, state| players.contains_key(owner) || !state.is_idle(tick));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(interval: i32) -> FloodConfig {
        FloodConfig::with_interval(interval)
    }

    #[test]
    fn interval_blocks_until_it_has_passed() {
        let mut action = FloodAction::default();
        let cfg = config(10);
        assert!(action.check(&cfg, 100).allowed);
        // next_game_tick = 110: the ticks 101..=109 are refused, 110 is allowed again.
        assert!(!action.check(&cfg, 101).allowed);
        assert!(!action.check(&cfg, 109).allowed);
        assert!(action.check(&cfg, 110).allowed);
        assert!(!action.check(&cfg, 119).allowed);
        assert!(action.check(&cfg, 120).allowed);
    }

    #[test]
    fn interval_zero_never_blocks() {
        let mut action = FloodAction::default();
        let cfg = config(0);
        for _ in 0..5 {
            assert!(action.check(&cfg, 7).allowed);
        }
    }

    #[test]
    fn a_blocked_request_does_not_start_a_new_interval() {
        let mut action = FloodAction::default();
        let cfg = config(10);
        assert!(action.check(&cfg, 100).allowed);
        assert!(!action.check(&cfg, 105).allowed);
        // Still 110, not 115.
        assert!(action.check(&cfg, 110).allowed);
    }

    #[test]
    fn punishment_limit_counts_blocked_requests_and_is_not_reset() {
        let mut action = FloodAction::default();
        let cfg = FloodConfig {
            punishment_limit: 3,
            ..config(10)
        };
        assert!(action.check(&cfg, 100).allowed);
        assert!(!action.check(&cfg, 101).punish);
        assert!(!action.check(&cfg, 102).punish);
        let third = action.check(&cfg, 103);
        assert!(!third.allowed && third.punish);
        // NOTE(java-quirk): every further blocked request punishes again.
        assert!(action.check(&cfg, 104).punish);
        // An allowed request resets the count.
        assert!(action.check(&cfg, 110).allowed);
        assert!(!action.check(&cfg, 111).punish);
    }

    #[test]
    fn limit_zero_never_punishes() {
        let mut action = FloodAction::default();
        let cfg = config(10);
        action.check(&cfg, 100);
        for tick in 101..109 {
            assert!(!action.check(&cfg, tick).punish);
        }
    }

    #[test]
    fn flooding_is_logged_once_per_interval_with_the_time_since_the_previous_command() {
        let mut action = FloodAction::default();
        let cfg = FloodConfig {
            log_flooding: true,
            ..config(10)
        };
        assert_eq!(action.check(&cfg, 100).log_flooding_ms, None);
        // 4 ticks after the previous command: 400 ms.
        assert_eq!(action.check(&cfg, 104).log_flooding_ms, Some(400));
        // Only the first refused request of the interval is logged.
        assert_eq!(action.check(&cfg, 105).log_flooding_ms, None);
        let next = action.check(&cfg, 110);
        assert!(next.allowed);
        // Two refused requests: "issued 2 extra requests".
        assert_eq!(next.log_extra, Some(2));
        // The log flag was reset.
        assert_eq!(action.check(&cfg, 111).log_flooding_ms, Some(100));
    }

    #[test]
    fn nothing_is_logged_with_the_option_off() {
        let mut action = FloodAction::default();
        let cfg = config(10);
        action.check(&cfg, 100);
        assert_eq!(action.check(&cfg, 101).log_flooding_ms, None);
        assert_eq!(action.check(&cfg, 110).log_extra, None);
    }

    #[test]
    fn punishment_types_are_case_sensitive_like_the_java_switch() {
        assert_eq!(FloodPunishment::parse("kick"), FloodPunishment::Kick);
        assert_eq!(FloodPunishment::parse("ban"), FloodPunishment::Ban);
        assert_eq!(FloodPunishment::parse("jail"), FloodPunishment::Jail);
        assert_eq!(
            FloodPunishment::parse("Kick"),
            FloodPunishment::Unknown("Kick".to_owned())
        );
        assert_eq!(
            FloodPunishment::parse("none"),
            FloodPunishment::Unknown("none".to_owned())
        );
    }

    #[test]
    fn java_defaults_are_the_numbers_of_the_config_loader() {
        let defaults = FloodSettings::java_defaults();
        let intervals: Vec<i32> = FloodKind::ALL
            .iter()
            .map(|k| defaults.get(*k).protection_interval)
            .collect();
        assert_eq!(intervals, [4, 42, 16, 100, 5, 20, 10, 5, 1, 10, 3, 100, 30, 9, 3]);
        assert!(!defaults.get(FloodKind::UseItem).log_flooding);
        assert_eq!(defaults.get(FloodKind::UseItem).punishment_limit, 0);
        assert_eq!(defaults.get(FloodKind::UseItem).punishment_time, 0);
    }

    #[test]
    fn type_names_are_the_java_ones() {
        assert_eq!(FloodKind::MultiSell.type_name(), "MultiSellFloodProtector");
        assert_eq!(FloodKind::MultiSell.config_name(), "MultiSell");
        assert_eq!(FloodKind::SendMail.type_name(), "SendMailFloodProtector");
        assert_eq!(FloodKind::ALL.len(), 15);
    }

    #[test]
    fn duration_text_matches_the_log_lines() {
        assert_eq!(duration_text(0), "forever");
        assert_eq!(duration_text(300_000), "for 5 mins.");
    }
}
