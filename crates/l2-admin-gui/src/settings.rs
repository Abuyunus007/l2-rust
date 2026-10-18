//! GUI settings, persisted in eframe's storage (a RON file in the user's app-data folder).
//!
//! The value is stored as one JSON string under [`STORAGE_KEY`], so a field added later falls
//! back to its default (`#[serde(default)]`) instead of losing the other settings.
//!
//! The token is stored **as-is, in plain text** (card M1-GUI-01); the connection view says so.

use std::fmt;

use serde::{Deserialize, Serialize};

pub const STORAGE_KEY: &str = "l2_admin_gui.settings";
pub const DEFAULT_URL: &str = "ws://127.0.0.1:7900";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

impl Theme {
    pub const ALL: [Theme; 3] = [Theme::System, Theme::Light, Theme::Dark];

    pub fn label(self) -> &'static str {
        match self {
            Theme::System => "System",
            Theme::Light => "Light",
            Theme::Dark => "Dark",
        }
    }

    pub fn preference(self) -> egui::ThemePreference {
        match self {
            Theme::System => egui::ThemePreference::System,
            Theme::Light => egui::ThemePreference::Light,
            Theme::Dark => egui::ThemePreference::Dark,
        }
    }
}

/// Where the in-process packet proxy listens by default. Not 2106, so it never clashes with a
/// running login server on the same PC (point the game client at this port).
pub const DEFAULT_PROXY_LISTEN: &str = "127.0.0.1:2108";
/// The real login server the proxy talks to.
pub const DEFAULT_PROXY_UPSTREAM: &str = "127.0.0.1:2106";
/// Packets that are sent constantly and drown out everything else (Java constant names, comma
/// or space separated). Entries may also be hex opcodes like `0x59`.
pub const DEFAULT_NOISY: &str = "MOVE_TO_LOCATION, VALIDATE_POSITION, VALIDATE_LOCATION, NET_PING";
/// Under `captures/`, which git ignores (captures may hold session data).
pub const DEFAULT_CAPTURE_PATH: &str = "captures/capture.jsonl";

/// Settings of the packet inspector (`views/packets`).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct PacketSettings {
    /// Where the game client connects, as typed (`ip:port`).
    pub listen: String,
    /// The real login server, as typed (`ip:port`).
    pub upstream: String,
    /// IPv4 address the client uses to reach the proxy's game ports. Empty = the listen IP.
    pub advertise_ip: String,
    /// Hide the packets named in [`PacketSettings::noisy`].
    pub hide_noisy: bool,
    /// The user-editable list of noisy packets.
    pub noisy: String,
    /// File used by the Save and Open buttons.
    pub capture_path: String,
}

impl Default for PacketSettings {
    fn default() -> Self {
        Self {
            listen: DEFAULT_PROXY_LISTEN.to_string(),
            upstream: DEFAULT_PROXY_UPSTREAM.to_string(),
            advertise_ip: String::new(),
            hide_noisy: false,
            noisy: DEFAULT_NOISY.to_string(),
            capture_path: DEFAULT_CAPTURE_PATH.to_string(),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Supervisor address as typed by the user; see [`Settings::ws_url`].
    pub url: String,
    /// Shared secret for the admin API (plain text, see the module docs).
    pub token: String,
    pub theme: Theme,
    /// Connect when the app starts (only if a token is set).
    pub auto_connect: bool,
    /// Packet inspector and in-process proxy (D-35).
    pub packets: PacketSettings,
    /// The datapack folder (the one that contains `data/`) the data browser loads. Empty = not set.
    pub datapack_path: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            url: DEFAULT_URL.to_string(),
            token: String::new(),
            theme: Theme::default(),
            auto_connect: true,
            packets: PacketSettings::default(),
            datapack_path: String::new(),
        }
    }
}

// Hand-written so the token never ends up in a log line.
impl fmt::Debug for Settings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Settings")
            .field("url", &self.url)
            .field("token", &"<redacted>")
            .field("theme", &self.theme)
            .field("auto_connect", &self.auto_connect)
            .field("packets", &self.packets)
            .field("datapack_path", &self.datapack_path)
            .finish()
    }
}

impl Settings {
    /// Loads the settings; missing or unreadable data gives the defaults.
    pub fn load(storage: Option<&dyn eframe::Storage>) -> Self {
        let Some(raw) = storage.and_then(|s| s.get_string(STORAGE_KEY)) else {
            return Self::default();
        };
        match serde_json::from_str(&raw) {
            Ok(settings) => settings,
            Err(e) => {
                tracing::warn!("ignoring unreadable stored settings: {e}");
                Self::default()
            }
        }
    }

    pub fn save(&self, storage: &mut dyn eframe::Storage) {
        match serde_json::to_string(self) {
            Ok(json) => storage.set_string(STORAGE_KEY, json),
            Err(e) => tracing::warn!("could not serialize settings: {e}"),
        }
    }

    /// The URL to connect to: the typed address, trimmed, with `ws://` added when no scheme was
    /// given (so `127.0.0.1:7900` works).
    pub fn ws_url(&self) -> String {
        let url = self.url.trim();
        if url.contains("://") {
            url.to_string()
        } else {
            format!("ws://{url}")
        }
    }

    /// True when connecting could work at all (the listener rejects an empty token).
    pub fn can_connect(&self) -> bool {
        !self.url.trim().is_empty() && !self.token.is_empty()
    }

    /// Whether to start connecting right after launch.
    pub fn should_auto_connect(&self) -> bool {
        self.auto_connect && self.can_connect()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use eframe::Storage;

    use super::*;

    #[derive(Default)]
    struct MemStorage(HashMap<String, String>);

    impl eframe::Storage for MemStorage {
        fn get_string(&self, key: &str) -> Option<String> {
            self.0.get(key).cloned()
        }
        fn set_string(&mut self, key: &str, value: String) {
            self.0.insert(key.to_string(), value);
        }
        fn remove_string(&mut self, key: &str) {
            self.0.remove(key);
        }
        fn flush(&mut self) {}
    }

    #[test]
    fn defaults_when_nothing_is_stored() {
        let settings = Settings::load(Some(&MemStorage::default()));
        assert_eq!(settings, Settings::default());
        assert_eq!(Settings::load(None), Settings::default());
        assert_eq!(settings.url, DEFAULT_URL);
        assert!(!settings.can_connect(), "no token yet");
    }

    #[test]
    fn save_then_load_round_trips() {
        let settings = Settings {
            url: "ws://10.0.0.5:7900".into(),
            token: "s3cret".into(),
            theme: Theme::Dark,
            auto_connect: false,
            packets: PacketSettings {
                listen: "127.0.0.1:2999".into(),
                hide_noisy: true,
                noisy: "MOVE_TO_LOCATION".into(),
                ..PacketSettings::default()
            },
            datapack_path: "some/datapack/game".into(),
        };
        let mut storage = MemStorage::default();
        settings.save(&mut storage);
        assert_eq!(Settings::load(Some(&storage)), settings);
    }

    #[test]
    fn garbage_in_storage_falls_back_to_defaults() {
        let mut storage = MemStorage::default();
        storage.set_string(STORAGE_KEY, "not json".into());
        assert_eq!(Settings::load(Some(&storage)), Settings::default());
    }

    #[test]
    fn missing_fields_take_their_defaults() {
        let mut storage = MemStorage::default();
        storage.set_string(STORAGE_KEY, r#"{"token":"abc"}"#.into());
        let settings = Settings::load(Some(&storage));
        assert_eq!(settings.token, "abc");
        assert_eq!(settings.url, DEFAULT_URL);
        assert_eq!(settings.theme, Theme::System);
        assert_eq!(settings.packets, PacketSettings::default());
        assert!(
            settings.datapack_path.is_empty(),
            "no datapack until the user sets one"
        );
    }

    #[test]
    fn proxy_defaults_do_not_clash_with_the_login_server() {
        let packets = PacketSettings::default();
        assert_eq!(packets.listen, "127.0.0.1:2108");
        assert_eq!(packets.upstream, "127.0.0.1:2106");
        assert!(!packets.hide_noisy, "nothing is hidden until the user asks");
    }

    #[test]
    fn ws_url_adds_scheme_and_trims() {
        let ws_url = |url: &str| {
            Settings {
                url: url.into(),
                ..Settings::default()
            }
            .ws_url()
        };
        assert_eq!(ws_url("  127.0.0.1:7900 "), "ws://127.0.0.1:7900");
        assert_eq!(ws_url("ws://host:1"), "ws://host:1");
        assert_eq!(ws_url("wss://host"), "wss://host");
    }

    #[test]
    fn auto_connect_needs_a_token_and_the_flag() {
        let mut settings = Settings::default();
        assert!(!settings.should_auto_connect());
        settings.token = "t".into();
        assert!(settings.should_auto_connect());
        settings.auto_connect = false;
        assert!(!settings.should_auto_connect());
    }

    #[test]
    fn debug_output_hides_the_token() {
        let settings = Settings {
            token: "hunter2".into(),
            ..Settings::default()
        };
        assert!(!format!("{settings:?}").contains("hunter2"));
    }
}
