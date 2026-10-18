//! `Interface.ini`.
// Java: org/l2jmobius/commons/config/InterfaceConfig.java

use crate::IniFile;

/// File name inside the config directory.
pub const INTERFACE_CONFIG_FILE: &str = "Interface.ini";

/// The Java Swing GUI settings. The Rust servers run headless (D-30), so nothing reads these;
/// they are parsed so the file stays valid and complete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterfaceConfig {
    /// Key: `EnableGUI`, default true.
    ///
    /// NOTE(java-quirk): Java also ANDs this with `!GraphicsEnvironment.isHeadless()`. That is a
    /// property of the JVM host, not of the file, so the raw file value is kept.
    pub enable_gui: bool,
    /// Key: `DarkTheme`, default true. Java only reads it when `EnableGUI` is on, otherwise the
    /// field stays false (and the key is not looked up, so no warning either).
    pub dark_theme: bool,
}

impl InterfaceConfig {
    /// Reads the values like `InterfaceConfig.load()`.
    pub fn from_ini(ini: &IniFile) -> Self {
        let enable_gui = ini.get_bool("EnableGUI", true);
        let dark_theme = enable_gui && ini.get_bool("DarkTheme", true);
        Self {
            enable_gui,
            dark_theme,
        }
    }
}
