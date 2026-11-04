// Java: org/l2jmobius/gameserver/config/custom/CustomMailManagerConfig.java

use crate::IniFile;
/// Settings of `config/Custom/CustomMailManager.ini` (Java: `CustomMailManagerConfig`).
#[derive(Debug, Clone, PartialEq)]
pub struct CustomMailManagerConfig {
    /// Key: `CustomMailManagerEnabled`, default false.
    pub custom_mail_manager_enabled: bool,
    /// Hand-ported, the value is `GameConfig::manual.custom_misc.custom_mail_manager_delay` (Java type `int`); the unit type keeps this field from being used by mistake.
    /// Java (line 42): `CUSTOM_MAIL_MANAGER_DELAY = custom.getInt("DatabaseQueryDelay", 30) * 1000;`
    /// Key: `DatabaseQueryDelay`, Java default `30` (before the Java post-processing).
    pub custom_mail_manager_delay: (),
}
impl CustomMailManagerConfig {
    /// File name inside the config directory.
    pub const FILE: &'static str = "Custom/CustomMailManager.ini";
    /// Reads the values like `CustomMailManagerConfig.load()`.
    pub fn from_ini(ini: &IniFile) -> Self {
        Self {
            custom_mail_manager_enabled: ini.get_bool("CustomMailManagerEnabled", false),
            custom_mail_manager_delay: (),
        }
    }
}
impl Default for CustomMailManagerConfig {
    /// Every key missing: the Java defaults.
    fn default() -> Self {
        Self::from_ini(&IniFile::empty(Self::FILE))
    }
}
