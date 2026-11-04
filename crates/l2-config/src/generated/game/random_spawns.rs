
use crate::IniFile;
/// Settings of `config/Custom/RandomSpawns.ini` (Java: `RandomSpawnsConfig`).
#[derive(Debug, Clone, PartialEq)]
pub struct RandomSpawnsConfig {
    /// Key: `EnableRandomMonsterSpawns`, default false.
    /// TODO(M2-CFG-01): Java also uses this field after reading it (line 49): `if (ENABLE_RANDOM_MONSTER_SPAWNS) { final String[] mobsIds = config.getString("MobsSpawnNotRandom", "18812,18813,18814,22138").split(","); MOBS_LIST_NOT_RANDOM  ...`
    pub enable_random_monster_spawns: bool,
    /// Key: `MaxSpawnMobRange`, default 150.
    /// TODO(M2-CFG-01): Java also uses this field after reading it (line 48): `MOB_MIN_SPAWN_RANGE = MOB_MAX_SPAWN_RANGE * -1;`
    pub mob_max_spawn_range: i32,
    /// TODO(M2-CFG-01): not generated, hand-port it. Java type `int`; the unit type makes any use fail to compile until then.
    /// Java (line 48): `MOB_MIN_SPAWN_RANGE = MOB_MAX_SPAWN_RANGE * -1;`
    pub mob_min_spawn_range: (),
    /// Key: `MobsSpawnNotRandom`, default `18812,18813,18814,22138` (Java type `Set<Integer>`).
    /// Java (line 49): `if (ENABLE_RANDOM_MONSTER_SPAWNS) { final String[] mobsIds = config.getString("MobsSpawnNotRandom", "18812,18813,18814,22138").split(","); MOBS_LIST_NOT_RANDOM = new HashSet<>(mobsIds.length); for (St ...`
    pub mobs_list_not_random: std::collections::BTreeSet<i32>,
}
impl RandomSpawnsConfig {
    /// File name inside the config directory.
    pub const FILE: &'static str = "Custom/RandomSpawns.ini";
    /// Reads the values like `RandomSpawnsConfig.load()`.
    pub fn from_ini(ini: &IniFile) -> Self {
        Self {
            enable_random_monster_spawns: ini
                .get_bool("EnableRandomMonsterSpawns", false),
            mob_max_spawn_range: ini.get_int("MaxSpawnMobRange", 150),
            mob_min_spawn_range: (),
            mobs_list_not_random: ini
                .get_int_set("MobsSpawnNotRandom", "18812,18813,18814,22138"),
        }
    }
}
impl Default for RandomSpawnsConfig {
    /// Every key missing: the Java defaults.
    fn default() -> Self {
        Self::from_ini(&IniFile::empty(Self::FILE))
    }
}
