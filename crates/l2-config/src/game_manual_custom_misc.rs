//! Hand-ported fields of the misc custom configs (M9-C06): the `()` placeholders of
//! `ScreenWelcomeMessageConfig`, `MultilingualSupportConfig`, `CustomMailManagerConfig`,
//! `DualboxCheckConfig` and `CaptchaConfig`.
// Java: org/l2jmobius/gameserver/config/custom/{ScreenWelcomeMessage,MultilingualSupport,
// CustomMailManager,DualboxCheck,Captcha}Config.java
//
// They live in `GameManual::custom_misc`. Every key has the Java default; the `int` products wrap
// like Java's `int` arithmetic.

use std::collections::HashMap;

use tracing::warn;

use crate::game_manual::java_split;
use crate::{GameConfig, IniFile};

/// The hand-ported fields of the misc custom configs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomMiscManual {
    /// `WELCOME_MESSAGE_TIME`: `ScreenWelcomeMessageTime` (default 10) in milliseconds.
    pub welcome_message_time: i32,
    /// `MULTILANG_DEFAULT`: `MultiLangDefault` (default `en`), lower case.
    pub multilang_default: String,
    /// `MULTILANG_ALLOWED`: `MultiLangAllowed` (default: the default language) split on `;`,
    /// each lower case.
    pub multilang_allowed: Vec<String>,
    /// `CUSTOM_MAIL_MANAGER_DELAY`: `DatabaseQueryDelay` (default 30) in milliseconds.
    pub custom_mail_manager_delay: i32,
    /// `DUALBOX_CHECK_WHITELIST`: the IPv4 address as the 32-bit number (Java's
    /// `InetAddress.hashCode()` of an IPv4 address) to the extra connections; 0 is stored as -1
    /// (unlimited).
    pub dualbox_check_whitelist: HashMap<i32, i32>,
    /// `KILL_COUNTER_RESET_TIME`: `KillCounterResetTime` (default 20) in milliseconds.
    pub kill_counter_reset_time: i32,
}

impl CustomMiscManual {
    /// Reads the keys like the five `load()` methods.
    pub fn from_inis(
        welcome: &IniFile,
        multilingual: &IniFile,
        custom_mail: &IniFile,
        dualbox: &IniFile,
        captcha: &IniFile,
    ) -> Self {
        // Java: MultilingualSupportConfig.java:48-65
        let multilang_default = multilingual.get_string("MultiLangDefault", "en").to_lowercase();
        let allowed = multilingual.get_string("MultiLangAllowed", &multilang_default);
        let multilang_allowed: Vec<String> = java_split(&allowed, ';')
            .into_iter()
            .map(str::to_lowercase)
            .collect();
        if !multilang_allowed.contains(&multilang_default) {
            warn!(
                "MultiLang[MultilingualSupportConfig.load()]: default language: {multilang_default} is not in allowed list!"
            );
        }
        Self {
            welcome_message_time: welcome.get_int("ScreenWelcomeMessageTime", 10).wrapping_mul(1000),
            multilang_default,
            multilang_allowed,
            custom_mail_manager_delay: custom_mail.get_int("DatabaseQueryDelay", 30).wrapping_mul(1000),
            dualbox_check_whitelist: parse_whitelist(
                &dualbox.get_string("DualboxCheckWhitelist", "127.0.0.1,0"),
            ),
            kill_counter_reset_time: captcha.get_int("KillCounterResetTime", 20).wrapping_mul(60000),
        }
    }
}

impl Default for CustomMiscManual {
    /// Every key missing: the Java defaults.
    fn default() -> Self {
        let empty = IniFile::empty("Custom");
        Self::from_inis(&empty, &empty, &empty, &empty, &empty)
    }
}

/// Java `InetAddress.getByName(text).hashCode()` for an IPv4 literal: the address as a
/// big-endian `int`. `None` for anything else.
// NOTE: Java also resolves host names (and IPv6 literals); a name is reported as an invalid
// address here, which only differs for a whitelist written with a DNS name.
pub fn ipv4_hash(text: &str) -> Option<i32> {
    let address: std::net::Ipv4Addr = text.parse().ok()?;
    Some(i32::from_be_bytes(address.octets()))
}

/// Java `DualboxCheckConfig.load()`: `address,count;address,count`.
// Java: DualboxCheckConfig.java:65-92
fn parse_whitelist(text: &str) -> HashMap<i32, i32> {
    let mut map = HashMap::new();
    for entry in java_split(text, ';') {
        let parts = java_split(entry, ',');
        if parts.len() != 2 {
            warn!(
                "DualboxCheck[DualboxCheckConfig.load()]: invalid config property -> DualboxCheckWhitelist \"{entry}\""
            );
            continue;
        }
        // `Integer.parseInt`: a sign and digits, nothing else.
        let Ok(number) = parts[1].parse::<i32>() else {
            warn!(
                "DualboxCheck[DualboxCheckConfig.load()]: invalid number -> DualboxCheckWhitelist \"{}\"",
                parts[1]
            );
            continue;
        };
        let Some(hash) = ipv4_hash(parts[0]) else {
            warn!(
                "DualboxCheck[DualboxCheckConfig.load()]: invalid address -> DualboxCheckWhitelist \"{}\"",
                parts[0]
            );
            continue;
        };
        // Java resolves the address before it parses the number, so a bad address with a bad
        // number reports the address first; only the log line differs.
        map.insert(hash, if number == 0 { -1 } else { number });
    }
    map
}

impl GameConfig {
    /// The misc custom values the placeholders stand for.
    pub fn custom_misc(&self) -> &CustomMiscManual {
        &self.manual.custom_misc
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ini(text: &str) -> IniFile {
        IniFile::parse("test.ini", text).unwrap()
    }

    #[test]
    fn defaults_are_the_java_defaults() {
        let manual = CustomMiscManual::default();
        assert_eq!(manual.welcome_message_time, 10_000);
        assert_eq!(manual.multilang_default, "en");
        assert_eq!(manual.multilang_allowed, vec!["en".to_owned()]);
        assert_eq!(manual.custom_mail_manager_delay, 30_000);
        assert_eq!(manual.kill_counter_reset_time, 1_200_000);
        // 127.0.0.1,0: the local address with unlimited connections.
        assert_eq!(manual.dualbox_check_whitelist.len(), 1);
        assert_eq!(manual.dualbox_check_whitelist[&0x7f00_0001], -1);
    }

    #[test]
    fn keys_are_read_with_java_units() {
        let manual = CustomMiscManual::from_inis(
            &ini("ScreenWelcomeMessageTime = 3"),
            &ini("MultiLangDefault = EL\nMultiLangAllowed = en;EL"),
            &ini("DatabaseQueryDelay = 5"),
            &ini("DualboxCheckWhitelist = 10.0.0.1,2;bad;10.0.0.2,x;host,1;192.168.1.5,0"),
            &ini("KillCounterResetTime = 2"),
        );
        assert_eq!(manual.welcome_message_time, 3000);
        assert_eq!(manual.multilang_default, "el");
        assert_eq!(manual.multilang_allowed, vec!["en".to_owned(), "el".to_owned()]);
        assert_eq!(manual.custom_mail_manager_delay, 5000);
        assert_eq!(manual.kill_counter_reset_time, 120_000);
        assert_eq!(manual.dualbox_check_whitelist.len(), 2);
        assert_eq!(manual.dualbox_check_whitelist[&0x0a00_0001], 2);
        assert_eq!(manual.dualbox_check_whitelist[&(0xc0a8_0105u32 as i32)], -1);
    }
}
