//! The small custom features of M9-C06: their switches, the state they keep and the hooks the
//! rest of the world calls (D-57). Each feature has its own file next to this one.
// Java: gameserver/config/custom/{ScreenWelcomeMessage,OnlineInfo,PasswordChange,
// MultilingualSupport,CustomMailManager,DualboxCheck,Captcha,ChatModeration,WalkerBotProtection,
// HellboundStatus,Banking}Config.java
//
// Every feature is off by default like in Java (the one exception is `ChatAdmin`, which Java
// switches on by default). `CustomMiscSettings::default()` is the Java default of each key, so a
// world built without the config behaves like a server with the shipped `.ini` files.

use std::collections::{BTreeMap, HashMap};

use l2_util::ObjectId;

use super::captcha::CaptchaState;
use super::dualbox::AntiFeed;
use crate::world::World;

/// `config/Custom/ScreenWelcomeMessage.ini`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenWelcomeSettings {
    /// `WELCOME_MESSAGE_ENABLED` (`ScreenWelcomeMessageEnable`).
    pub enabled: bool,
    /// `WELCOME_MESSAGE_TEXT` (`ScreenWelcomeMessageText`).
    pub text: String,
    /// `WELCOME_MESSAGE_TIME`: `ScreenWelcomeMessageTime` in milliseconds.
    pub time_ms: i32,
}

impl Default for ScreenWelcomeSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            text: "Welcome to our server!".to_owned(),
            time_ms: 10_000,
        }
    }
}

/// `config/Custom/MultilingualSupport.ini`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultilangSettings {
    /// `MULTILANG_ENABLE`.
    pub enable: bool,
    /// `MULTILANG_DEFAULT`, lower case.
    pub default_lang: String,
    /// `MULTILANG_ALLOWED`, lower case.
    pub allowed: Vec<String>,
    /// `MULTILANG_VOICED_ALLOW` (`MultiLangVoiceCommand`).
    pub voiced_allow: bool,
}

impl Default for MultilangSettings {
    fn default() -> Self {
        Self {
            enable: false,
            default_lang: "en".to_owned(),
            allowed: vec!["en".to_owned()],
            voiced_allow: true,
        }
    }
}

/// `config/Custom/DualboxCheck.ini`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DualboxSettings {
    /// `DUALBOX_CHECK_MAX_PLAYERS_PER_IP`.
    pub max_players_per_ip: i32,
    /// `DUALBOX_CHECK_MAX_OLYMPIAD_PARTICIPANTS_PER_IP`.
    pub max_olympiad_participants_per_ip: i32,
    /// `DUALBOX_CHECK_MAX_L2EVENT_PARTICIPANTS_PER_IP`.
    pub max_l2event_participants_per_ip: i32,
    /// `DUALBOX_CHECK_MAX_OFFLINEPLAY_PER_IP`.
    pub max_offlineplay_per_ip: i32,
    /// `DUALBOX_CHECK_MAX_OFFLINEPLAY_PREMIUM_PER_IP`.
    pub max_offlineplay_premium_per_ip: i32,
    /// `DUALBOX_COUNT_OFFLINE_TRADERS`.
    pub count_offline_traders: bool,
    /// `DUALBOX_CHECK_WHITELIST`: the IPv4 address as a big-endian `int` (Java's
    /// `InetAddress.hashCode()`) to the extra connections (-1: unlimited).
    pub whitelist: BTreeMap<i32, i32>,
}

/// `config/Custom/Captcha.ini`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptchaSettings {
    /// `ENABLE_CAPTCHA`.
    pub enable: bool,
    /// `KILL_COUNTER`.
    pub kill_counter: i32,
    /// `KILL_COUNTER_RANDOMIZATION`.
    pub kill_counter_randomization: i32,
    /// `KILL_COUNTER_RESET`.
    pub kill_counter_reset: bool,
    /// `KILL_COUNTER_RESET_TIME`, milliseconds.
    pub kill_counter_reset_time_ms: i32,
    /// `VALIDATION_TIME`, seconds.
    pub validation_time: i32,
    /// `CAPTCHA_ATTEMPTS`.
    pub captcha_attempts: i32,
    /// `PUNISHMENT`: 0 town, 1 kick, 2 jail, 3 ban.
    pub punishment: i32,
    /// `JAIL_TIME`, minutes.
    pub jail_time: i32,
    /// `DOUBLE_JAIL_TIME`.
    pub double_jail_time: bool,
    /// `ServerConfig.SERVER_ID`: the crest id of the picture is built from it.
    pub server_id: i32,
}

impl Default for CaptchaSettings {
    fn default() -> Self {
        Self {
            enable: false,
            kill_counter: 100,
            kill_counter_randomization: 50,
            kill_counter_reset: false,
            kill_counter_reset_time_ms: 20 * 60_000,
            validation_time: 60,
            captcha_attempts: 2,
            punishment: 0,
            jail_time: 2,
            double_jail_time: false,
            server_id: 1,
        }
    }
}

/// `config/Custom/Banking.ini`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BankingSettings {
    /// `BANKING_SYSTEM_ENABLED` (`BankingEnabled`).
    pub enabled: bool,
    /// `BANKING_SYSTEM_GOLDBAR_COUNT` (`BankingGoldbarCount`).
    pub goldbar_count: i32,
    /// `BANKING_SYSTEM_ADENA_COUNT` (`BankingAdenaCount`).
    pub adena_count: i32,
    /// `BANKING_SYSTEM_AUTO_CONVERT_ENABLED` (`BankingAutoConvert`).
    pub auto_convert_enabled: bool,
    /// `BANKING_SYSTEM_AUTO_CONVERT_ADENA_LIMIT` (`BankingAutoConvertAdenaLimit`).
    pub auto_convert_adena_limit: i64,
}

impl Default for BankingSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            goldbar_count: 1,
            adena_count: 1_000_000_000,
            auto_convert_enabled: false,
            auto_convert_adena_limit: 51_000_000_000,
        }
    }
}

/// The switches of the features of M9-C06.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomMiscSettings {
    pub screen_welcome: ScreenWelcomeSettings,
    /// `ENABLE_ONLINE_COMMAND` (`OnlineInfo.ini`).
    pub online_command: bool,
    /// `ALLOW_CHANGE_PASSWORD` (`PasswordChange.ini`).
    pub allow_change_password: bool,
    pub multilang: MultilangSettings,
    /// `CUSTOM_MAIL_MANAGER_ENABLED` (`CustomMailManager.ini`).
    pub custom_mail_enabled: bool,
    /// `CUSTOM_MAIL_MANAGER_DELAY`, milliseconds.
    pub custom_mail_delay_ms: i32,
    pub dualbox: DualboxSettings,
    pub captcha: CaptchaSettings,
    /// `CHAT_ADMIN` (`ChatModeration.ini`): Java's default is `True`.
    pub chat_admin: bool,
    /// `L2WALKER_PROTECTION` (`WalkerBotProtection.ini`).
    pub l2walker_protection: bool,
    /// `HELLBOUND_STATUS` (`HellboundStatus.ini`).
    pub hellbound_status: bool,
    pub banking: BankingSettings,
}

impl Default for CustomMiscSettings {
    fn default() -> Self {
        Self {
            screen_welcome: ScreenWelcomeSettings::default(),
            online_command: false,
            allow_change_password: false,
            multilang: MultilangSettings::default(),
            custom_mail_enabled: false,
            custom_mail_delay_ms: 30_000,
            dualbox: DualboxSettings::default(),
            captcha: CaptchaSettings::default(),
            chat_admin: true,
            l2walker_protection: false,
            hellbound_status: false,
            banking: BankingSettings::default(),
        }
    }
}

/// What the features keep between two calls.
#[derive(Debug, Default)]
pub struct CustomMiscState {
    /// `Player._lang` of the players who chose one with `.lang`.
    pub(crate) lang: HashMap<ObjectId, String>,
    /// `AntiFeedManager`'s `_eventIPs`.
    pub(crate) anti_feed: AntiFeed,
    /// `CaptchaManager`'s maps.
    pub(crate) captcha: CaptchaState,
    /// A `custom_mail` read is out and its answer has not come back yet.
    pub(crate) custom_mail_pending: bool,
}

/// The timers of the features of M9-C06.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomMiscTimer {
    /// Java `CustomMailManager`'s `scheduleAtFixedRate` run.
    CustomMail,
    /// Java `CaptchaManager.ReportCheckTask`: the time to answer is over.
    CaptchaReport(ObjectId),
    /// Java `CaptchaManager.countdown`: one second of the countdown.
    CaptchaCountdown(ObjectId, i32),
    /// Java `applyJailPunishment`'s `schedule(..., 2000)` warning.
    CaptchaJailWarning(ObjectId),
}

/// `new StringTokenizer(text)`: whitespace set `" \t\n\r\f"`.
pub(crate) fn tokens(text: &str) -> Vec<&str> {
    text.split([' ', '\t', '\n', '\r', '\u{c}'])
        .filter(|t| !t.is_empty())
        .collect()
}

/// `ChatGeneral.onChat`: the command of a text that starts with `.` and its parameters (`None`
/// for the `paramsValue` of general chat, which is null).
fn command_and_params(text: &str) -> (String, Option<String>) {
    let parts = tokens(text);
    if parts.len() > 1 {
        let command = parts[0].get(1..).unwrap_or("").to_owned();
        // `text.substring(command.length() + 2)`
        let skip = command.chars().count() + 2;
        let params: String = text.chars().skip(skip).collect();
        (command, Some(params))
    } else {
        (text.get(1..).unwrap_or("").to_owned(), None)
    }
}

impl World {
    /// The voiced commands of the features of this file, as `ChatGeneral.onChat` runs them:
    /// `VoicedCommandHandler.getHandler(command)` finds a handler only for a switched-on
    /// feature. `true` when a handler took the text (`vcdUsed`, whatever it answered).
    pub(crate) fn custom_misc_voiced_command(&mut self, player: ObjectId, text: &str) -> bool {
        if !text.starts_with('.') {
            return false;
        }
        let (command, params) = command_and_params(text);
        self.custom_misc_voiced_handler(player, &command, params.as_deref())
            .is_some()
    }

    /// `VoicedCommandHandler.getHandler(command)` and `onCommand(command, player, params)` of
    /// the features of this file. `None`: no handler (the feature is off or it is not its word);
    /// else what the handler answered (the `voice` bypass hands that on).
    pub(crate) fn custom_misc_voiced_handler(
        &mut self,
        player: ObjectId,
        command: &str,
        params: Option<&str>,
    ) -> Option<bool> {
        // `VoicedCommandHandler.getHandler`: the text up to the first space.
        let word = command.split(' ').next().unwrap_or(command);
        let s = &self.settings.custom_misc;
        match word {
            "bank" | "withdraw" | "deposit" if s.banking.enabled => {
                Some(self.banking_voiced(player, command))
            }
            "banchat" | "chatban" | "unbanchat" | "chatunban" if s.chat_admin => {
                Some(self.chat_admin_voiced(player, command, params))
            }
            "lang" if s.multilang.enable && s.multilang.voiced_allow => {
                Some(self.lang_voiced(player, params))
            }
            "changepassword" if s.allow_change_password => Some(self.change_password_voiced(player, params)),
            "online" if s.online_command => Some(self.online_voiced(player, command)),
            _ => None,
        }
    }

    /// `EnterWorld`: the screen welcome message (after the mail notice).
    pub(crate) fn custom_misc_on_enter(&self, id: ObjectId) {
        self.screen_welcome_on_enter(id);
    }

    /// `deleteMe`/`Disconnection`: what the features forget of a player. Called while the player
    /// is still in the tables. (Java's captcha maps keep the player's entries.)
    pub(crate) fn custom_misc_on_leave(&mut self, id: ObjectId) {
        self.dualbox_on_leave(id);
        self.custom_misc.lang.remove(&id);
    }

    /// The timers of [`CustomMiscTimer`].
    pub(crate) fn on_custom_misc_timer(&mut self, timer: CustomMiscTimer) {
        match timer {
            CustomMiscTimer::CustomMail => self.on_custom_mail_timer(),
            CustomMiscTimer::CaptchaReport(id) => self.on_captcha_report(id),
            CustomMiscTimer::CaptchaCountdown(id, time) => self.on_captcha_countdown(id, time),
            CustomMiscTimer::CaptchaJailWarning(id) => {
                self.send_message(id, "Warning: Your next jail time will be increased.");
            }
        }
    }

    /// The first start of the recurring timers; the game server calls it once after the settings
    /// are installed (Java: `GameServer` creates the `CustomMailManager`).
    pub fn start_custom_misc(&mut self) {
        self.start_custom_mail();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_javas_and_every_feature_is_off_but_chat_admin() {
        let s = CustomMiscSettings::default();
        assert!(!s.screen_welcome.enabled && !s.online_command && !s.allow_change_password);
        assert!(!s.multilang.enable && !s.custom_mail_enabled && !s.captcha.enable);
        assert!(!s.l2walker_protection && !s.hellbound_status && !s.banking.enabled);
        assert!(!s.banking.auto_convert_enabled);
        assert!(s.chat_admin);
        assert_eq!(s.dualbox.max_players_per_ip, 0);
    }

    #[test]
    fn the_command_and_its_params() {
        assert_eq!(command_and_params(".online"), ("online".to_owned(), None));
        assert_eq!(
            command_and_params(".banchat Bob 10"),
            ("banchat".to_owned(), Some("Bob 10".to_owned()))
        );
        // One token with a trailing space is still one token: the command keeps the space.
        assert_eq!(command_and_params(".online "), ("online ".to_owned(), None));
        // Two spaces: the second one is part of the params.
        assert_eq!(
            command_and_params(".lang  el"),
            ("lang".to_owned(), Some(" el".to_owned()))
        );
    }
}
