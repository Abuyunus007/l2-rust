// Java: org/l2jmobius/gameserver/config/custom/BossAnnouncementsConfig.java

use crate::IniFile;
/// Settings of `config/Custom/BossAnnouncements.ini` (Java: `BossAnnouncementsConfig`).
#[derive(Debug, Clone, PartialEq)]
pub struct BossAnnouncementsConfig {
    /// Key: `RaidBossSpawnAnnouncements`, default false.
    pub raidboss_spawn_announcements: bool,
    /// Key: `RaidBossDefeatAnnouncements`, default false.
    pub raidboss_defeat_announcements: bool,
    /// Key: `RaidBossInstanceAnnouncements`, default false.
    pub raidboss_instance_announcements: bool,
    /// Key: `GrandBossSpawnAnnouncements`, default false.
    pub grandboss_spawn_announcements: bool,
    /// Key: `GrandBossDefeatAnnouncements`, default false.
    pub grandboss_defeat_announcements: bool,
    /// Key: `GrandBossInstanceAnnouncements`, default false.
    pub grandboss_instance_announcements: bool,
    /// Key: `RaidbossExcludedFromSpawnAnnouncements`, default empty (Java type `Set<Integer>`).
    /// Java (line 57): `RAIDBOSSES_EXCLUDED_FROM_SPAWN_ANNOUNCEMENTS.clear();`
    pub raidbosses_excluded_from_spawn_announcements: std::collections::BTreeSet<i32>,
    /// Key: `RaidbossExcludedFromDefeatAnnouncements`, default empty (Java type `Set<Integer>`).
    /// Java (line 66): `RAIDBOSSES_EXCLUDED_FROM_DEFEAT_ANNOUNCEMENTS.clear();`
    pub raidbosses_excluded_from_defeat_announcements: std::collections::BTreeSet<i32>,
}
impl BossAnnouncementsConfig {
    /// File name inside the config directory.
    pub const FILE: &'static str = "Custom/BossAnnouncements.ini";
    /// Reads the values like `BossAnnouncementsConfig.load()`.
    pub fn from_ini(ini: &IniFile) -> Self {
        Self {
            raidboss_spawn_announcements: ini
                .get_bool("RaidBossSpawnAnnouncements", false),
            raidboss_defeat_announcements: ini
                .get_bool("RaidBossDefeatAnnouncements", false),
            raidboss_instance_announcements: ini
                .get_bool("RaidBossInstanceAnnouncements", false),
            grandboss_spawn_announcements: ini
                .get_bool("GrandBossSpawnAnnouncements", false),
            grandboss_defeat_announcements: ini
                .get_bool("GrandBossDefeatAnnouncements", false),
            grandboss_instance_announcements: ini
                .get_bool("GrandBossInstanceAnnouncements", false),
            raidbosses_excluded_from_spawn_announcements: ini
                .get_int_set("RaidbossExcludedFromSpawnAnnouncements", ""),
            raidbosses_excluded_from_defeat_announcements: ini
                .get_int_set("RaidbossExcludedFromDefeatAnnouncements", ""),
        }
    }
}
impl Default for BossAnnouncementsConfig {
    /// Every key missing: the Java defaults.
    fn default() -> Self {
        Self::from_ini(&IniFile::empty(Self::FILE))
    }
}
