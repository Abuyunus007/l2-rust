//! The settings of the misc custom features (M9-C06) as the world reads them.
// MultilingualSupport,CustomMailManager,DualboxCheck,Captcha,ChatModeration,WalkerBotProtection,
// HellboundStatus,Banking}Config.java

use l2_config::GameConfig;
use l2_world::{
    BankingSettings, CaptchaSettings, CustomMiscSettings, DualboxSettings, MultilangSettings,
    ScreenWelcomeSettings,
};

/// Builds [`CustomMiscSettings`] from the loaded config. `server_id` is the id the login server
/// gave this game server (the captcha picture's crest name is built from it).
pub fn custom_misc_settings(config: &GameConfig, server_id: i32) -> CustomMiscSettings {
    let manual = config.custom_misc();
    let w = &config.screen_welcome_message;
    let m = &config.multilingual_support;
    let d = &config.dualbox_check;
    let c = &config.captcha;
    let b = &config.banking;
    CustomMiscSettings {
        screen_welcome: ScreenWelcomeSettings {
            enabled: w.welcome_message_enabled,
            text: w.welcome_message_text.clone(),
            time_ms: manual.welcome_message_time,
        },
        online_command: config.online_info.enable_online_command,
        allow_change_password: config.password_change.allow_change_password,
        multilang: MultilangSettings {
            enable: m.multilang_enable,
            default_lang: manual.multilang_default.clone(),
            allowed: manual.multilang_allowed.clone(),
            voiced_allow: m.multilang_voiced_allow,
        },
        custom_mail_enabled: config.custom_mail_manager.custom_mail_manager_enabled,
        custom_mail_delay_ms: manual.custom_mail_manager_delay,
        dualbox: DualboxSettings {
            max_players_per_ip: d.dualbox_check_max_players_per_ip,
            max_olympiad_participants_per_ip: d.dualbox_check_max_olympiad_participants_per_ip,
            max_l2event_participants_per_ip: d.dualbox_check_max_l2event_participants_per_ip,
            max_offlineplay_per_ip: d.dualbox_check_max_offlineplay_per_ip,
            max_offlineplay_premium_per_ip: d.dualbox_check_max_offlineplay_premium_per_ip,
            count_offline_traders: d.dualbox_count_offline_traders,
            whitelist: manual
                .dualbox_check_whitelist
                .iter()
                .map(|(k, v)| (*k, *v))
                .collect(),
        },
        captcha: CaptchaSettings {
            enable: c.enable_captcha,
            kill_counter: c.kill_counter,
            kill_counter_randomization: c.kill_counter_randomization,
            kill_counter_reset: c.kill_counter_reset,
            kill_counter_reset_time_ms: manual.kill_counter_reset_time,
            validation_time: c.validation_time,
            captcha_attempts: c.captcha_attempts,
            punishment: c.punishment,
            jail_time: c.jail_time,
            double_jail_time: c.double_jail_time,
            server_id,
        },
        chat_admin: config.chat_moderation.chat_admin,
        l2walker_protection: config.walker_bot_protection.l2walker_protection,
        hellbound_status: config.hellbound_status.hellbound_status,
        banking: BankingSettings {
            enabled: b.banking_system_enabled,
            goldbar_count: b.banking_system_goldbar_count,
            adena_count: b.banking_system_adena_count,
            auto_convert_enabled: b.banking_system_auto_convert_enabled,
            auto_convert_adena_limit: b.banking_system_auto_convert_adena_limit,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shipped config directory gives the default settings: every feature off.
    #[test]
    fn a_config_without_files_gives_the_java_defaults() {
        let dir = std::env::temp_dir().join("l2-custom-misc-no-files");
        let config = GameConfig::load(&dir).unwrap();
        let settings = custom_misc_settings(&config, 1);
        let defaults = CustomMiscSettings::default();
        assert_eq!(settings.screen_welcome, defaults.screen_welcome);
        assert_eq!(settings.multilang, defaults.multilang);
        assert_eq!(settings.banking, defaults.banking);
        assert_eq!(
            settings.captcha,
            CaptchaSettings {
                server_id: 1,
                ..defaults.captcha
            }
        );
        assert_eq!(
            (
                settings.online_command,
                settings.allow_change_password,
                settings.custom_mail_enabled
            ),
            (false, false, false)
        );
        assert!(settings.chat_admin && !settings.l2walker_protection && !settings.hellbound_status);
        assert_eq!(settings.custom_mail_delay_ms, defaults.custom_mail_delay_ms);
        // 127.0.0.1,0 is the default whitelist.
        assert_eq!(settings.dualbox.whitelist.get(&0x7f00_0001), Some(&-1));
    }

    #[test]
    fn the_keys_reach_the_settings() {
        let dir = std::env::temp_dir().join(format!("l2-custom-misc-{}", std::process::id()));
        let custom = dir.join("Custom");
        std::fs::create_dir_all(&custom).unwrap();
        std::fs::write(
            custom.join("Banking.ini"),
            "BankingEnabled = True\nBankingGoldbarCount = 2\nBankingAutoConvert = True\n",
        )
        .unwrap();
        std::fs::write(
            custom.join("ScreenWelcomeMessage.ini"),
            "ScreenWelcomeMessageEnable = True\nScreenWelcomeMessageText = Hi\nScreenWelcomeMessageTime = 4\n",
        )
        .unwrap();
        std::fs::write(custom.join("ChatModeration.ini"), "ChatAdmin = False\n").unwrap();
        let config = GameConfig::load(&dir).unwrap();
        let settings = custom_misc_settings(&config, 7);
        assert!(settings.banking.enabled && settings.banking.auto_convert_enabled);
        assert_eq!(settings.banking.goldbar_count, 2);
        assert_eq!(settings.screen_welcome.text, "Hi");
        assert_eq!(settings.screen_welcome.time_ms, 4000);
        assert!(!settings.chat_admin);
        assert_eq!(settings.captcha.server_id, 7);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
