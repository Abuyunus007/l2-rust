//! What the config editor knows about the values of a key besides its type: a slider range,
//! the allowed words of an enum-like key, list keys and colours (GUI card G-07).
// `org/l2jmobius/gameserver/config` loaders; they only steer the *controls*.
//! None of this validates anything. A slider covers the usual values of a key; the number field
//! next to it still takes any valid number, and a value outside the slider just parks the slider
//! at its end. The tables are committed data on purpose (no guessing from the numbers at run
//! time), and the tests in `tests/config_hints.rs` check them against `dist/`: every entry names
//! an existing key, the Java default lies inside the range, and `min < max`.
//!
//! # Adding an entry
//! Add a [`RangeSpec`] line to [`RANGES`]. The **first** matching entry wins, so a specific key
//! goes above a general pattern (`RateKarmaLost` above `Rate*`). `*` in a pattern matches any
//! text (`Drop*LevelGapChance`). Leave `file` empty for a pattern that is right in every file.

/// A slider range for a numeric key.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RangeSpec {
    /// Path below the config folder (`Rates.ini`, `Custom/Captcha.ini`); empty = every file.
    pub file: &'static str,
    /// A key, or a pattern with `*` wildcards.
    pub pattern: &'static str,
    pub min: f64,
    pub max: f64,
    pub step: f64,
    /// Unit shown after the number (`s`, `ms`, `min`, `%`); empty = none.
    pub unit: &'static str,
}

impl RangeSpec {
    pub fn unit(&self) -> Option<&'static str> {
        (!self.unit.is_empty()).then_some(self.unit)
    }

    pub fn contains(&self, v: f64) -> bool {
        (self.min..=self.max).contains(&v)
    }

    /// Whole-number step and bounds: the key is edited as an integer.
    pub fn is_integral(&self) -> bool {
        self.step.fract() == 0.0 && self.min.fract() == 0.0 && self.max.fract() == 0.0
    }

    /// Decimals worth showing for `step` (0.05 gives 2).
    pub fn decimals(&self) -> usize {
        let mut d = 0;
        let mut s = self.step;
        while d < 6 && (s - s.round()).abs() > 1e-9 {
            s *= 10.0;
            d += 1;
        }
        d
    }
}

const fn r(
    file: &'static str,
    pattern: &'static str,
    min: f64,
    max: f64,
    step: f64,
    unit: &'static str,
) -> RangeSpec {
    RangeSpec {
        file,
        pattern,
        min,
        max,
        step,
        unit,
    }
}

/// `*` matches any run of characters (also none); everything else must match exactly. Keys are
/// case-sensitive, like in `java.util.Properties`.
pub fn key_matches(pattern: &str, key: &str) -> bool {
    if !pattern.contains('*') {
        return pattern == key;
    }
    let mut parts = pattern.split('*');
    let first = parts.next().unwrap_or("");
    let Some(mut rest) = key.strip_prefix(first) else {
        return false;
    };
    let mut parts: Vec<&str> = parts.collect();
    let last = parts.pop().unwrap_or("");
    for mid in parts {
        match rest.find(mid) {
            Some(i) => rest = &rest[i + mid.len()..],
            None => return false,
        }
    }
    rest.ends_with(last)
}

/// The slider range of `key` in `rel_file`, if the table has one. The caller only uses it for a
/// numeric value.
pub fn range_for(rel_file: &str, key: &str) -> Option<&'static RangeSpec> {
    RANGES
        .iter()
        .find(|r| (r.file.is_empty() || r.file == rel_file) && key_matches(r.pattern, key))
}

/// Keys that hold a list although the value in a file may be a single number (`LogItemsIdsList =
/// 4356`). They get a text field; a number control would refuse `4356,57`.
pub const LIST_KEYS: &[&str] = &[
    "*Ids",
    "*IdsList",
    "*List",
    "ListOf*",
    "ListPetRentNpc",
    "ServerRestartDays",
    "OlympiadCompetitionDays",
    "OlympiadRestrictedItems",
];

pub fn is_list_key(key: &str) -> bool {
    LIST_KEYS.iter().any(|p| key_matches(p, key))
}

/// The slider ranges. Units follow the comment of the key in `dist/`; "(ms)" keys that are
/// millions long are left out on purpose (a slider over 0..86400000 helps nobody).
#[rustfmt::skip]
pub static RANGES: &[RangeSpec] = &[
    // ---- Login server, Server.ini ----
    r("Server.ini", "LoginserverPort", 1.0, 65535.0, 1.0, ""),
    r("Server.ini", "LoginPort", 1.0, 65535.0, 1.0, ""),
    r("Server.ini", "GameserverPort", 1.0, 65535.0, 1.0, ""),
    r("Server.ini", "LoginTryBeforeBan", 1.0, 20.0, 1.0, ""),
    r("Server.ini", "LoginBlockAfterBan", 0.0, 86400.0, 60.0, "s"),
    r("Server.ini", "FastConnectionLimit", 1.0, 100.0, 1.0, ""),
    r("Server.ini", "NormalConnectionTime", 0.0, 5000.0, 50.0, "ms"),
    r("Server.ini", "FastConnectionTime", 0.0, 5000.0, 50.0, "ms"),
    r("Server.ini", "MaxConnectionPerIP", 1.0, 500.0, 1.0, ""),
    r("Server.ini", "LoginRestartTime", 1.0, 168.0, 1.0, "h"),
    // ---- Game server, Server.ini ----
    r("Server.ini", "RequestServerID", 0.0, 127.0, 1.0, ""),
    r("Server.ini", "MaximumOnlineUsers", 0.0, 10000.0, 50.0, ""),
    r("Server.ini", "DeadlockCheckInterval", 1.0, 600.0, 1.0, "s"),
    r("Server.ini", "MaxPlayersPerHWID", 0.0, 20.0, 1.0, ""),
    r("Server.ini", "HwidCheckDelay", 0.0, 60000.0, 100.0, "ms"),
    r("Server.ini", "CharMaxNumber", 0.0, 7.0, 1.0, ""),
    r("Server.ini", "PrecautionaryRestartPercentage", 1.0, 100.0, 1.0, "%"),
    r("Server.ini", "PrecautionaryRestartDelay", 1.0, 3600.0, 10.0, "s"),
    r("Server.ini", "ServerRestartScheduleCountdown", 0.0, 3600.0, 10.0, "s"),
    // ---- Network.ini, Threads.ini, Database.ini (both servers) ----
    r("Network.ini", "ShutdownWaitTime", 0.0, 300.0, 1.0, "s"),
    r("Network.ini", "DropPacketThreshold", 0.0, 10000.0, 50.0, ""),
    r("Network.ini", "ThreadPoolSize", 0.0, 256.0, 1.0, ""),
    r("Network.ini", "ThreadPriority", 1.0, 10.0, 1.0, ""),
    r("Threads.ini", "ScheduledThreadPoolSize", -1.0, 256.0, 1.0, ""),
    r("Threads.ini", "InstantThreadPoolSize", -1.0, 256.0, 1.0, ""),
    r("Database.ini", "MaximumDatabaseConnections", 1.0, 1000.0, 1.0, ""),
    r("Database.ini", "BackupDays", 0.0, 365.0, 1.0, "days"),
    // ---- Rates.ini ----
    r("Rates.ini", "RateKarmaLost", -1.0, 100.0, 0.1, ""),
    r("Rates.ini", "Rate*", 0.0, 100.0, 0.1, ""),
    r("Rates.ini", "*Multiplier", 0.0, 10.0, 0.1, ""),
    r("Rates.ini", "PlayerDropLimit", 0.0, 100.0, 1.0, ""),
    r("Rates.ini", "KarmaDropLimit", 0.0, 100.0, 1.0, ""),
    r("Rates.ini", "*RateDrop*", 0.0, 100.0, 1.0, "%"),
    r("Rates.ini", "*Rate", 0.0, 100.0, 0.1, ""),
    r("Rates.ini", "*LevelDifference", 0.0, 100.0, 1.0, "levels"),
    r("Rates.ini", "Drop*LevelGapChance", 0.0, 100.0, 1.0, "%"),
    r("Rates.ini", "DropMaxOccurrences*", 0.0, 50.0, 1.0, ""),
    r("Rates.ini", "BossDropMinLevel", 1.0, 999.0, 1.0, ""),
    r("Rates.ini", "BossDropMaxLevel", 1.0, 999.0, 1.0, ""),
    // ---- Player.ini ----
    r("Player.ini", "DeathPenaltyChance", 0.0, 100.0, 1.0, "%"),
    r("Player.ini", "RespawnRestore*", 0.0, 100.0, 1.0, "%"),
    r("Player.ini", "*RegenMultiplier", 0.0, 1000.0, 5.0, "%"),
    r("Player.ini", "RunSpeedBoost", 0.0, 250.0, 1.0, ""),
    r("Player.ini", "MaxBuffAmount", 0.0, 64.0, 1.0, ""),
    r("Player.ini", "MaxTriggeredBuffAmount", 0.0, 64.0, 1.0, ""),
    r("Player.ini", "MaxDanceAmount", 0.0, 64.0, 1.0, ""),
    r("Player.ini", "AltPerfectShieldBlockRate", 0.0, 100.0, 1.0, "%"),
    r("Player.ini", "StartingVitalityPoints", 0.0, 20000.0, 100.0, ""),
    r("Player.ini", "MaxExpBonus", 0.0, 20.0, 0.1, ""),
    r("Player.ini", "MaxSpBonus", 0.0, 20.0, 0.1, ""),
    r("Player.ini", "MaxRunSpeed", 0.0, 1000.0, 1.0, ""),
    r("Player.ini", "MaxPCritRate", 0.0, 1000.0, 1.0, ""),
    r("Player.ini", "MaxMCritRate", 0.0, 1000.0, 1.0, ""),
    r("Player.ini", "MaxPAtkSpeed", 0.0, 5000.0, 10.0, ""),
    r("Player.ini", "MaxMAtkSpeed", 0.0, 5000.0, 10.0, ""),
    r("Player.ini", "MaxEvasion", 0.0, 1000.0, 1.0, ""),
    r("Player.ini", "MinAbnormalStateSuccessRate", 0.0, 100.0, 1.0, "%"),
    r("Player.ini", "MaxAbnormalStateSuccessRate", 0.0, 100.0, 1.0, "%"),
    r("Player.ini", "MaximumPlayerLevel", 1.0, 85.0, 1.0, ""),
    r("Player.ini", "MaxSubclass", 0.0, 10.0, 1.0, ""),
    r("Player.ini", "BaseSubclassLevel", 1.0, 85.0, 1.0, ""),
    r("Player.ini", "MaxSubclassLevel", 1.0, 85.0, 1.0, ""),
    r("Player.ini", "MaxPvtStore*Slots*", 0.0, 50.0, 1.0, ""),
    r("Player.ini", "MaximumSlots*", 0.0, 500.0, 1.0, ""),
    r("Player.ini", "MaximumWarehouseSlots*", 0.0, 500.0, 1.0, ""),
    r("Player.ini", "MaximumFreightSlots", 0.0, 500.0, 1.0, ""),
    r("Player.ini", "EnchantChanceElement*", 0.0, 100.0, 1.0, "%"),
    r("Player.ini", "Augmentation*Chance", 0.0, 100.0, 1.0, "%"),
    r("Player.ini", "SoulCrystalChanceMultiplier", 0.0, 10.0, 0.1, ""),
    r("Player.ini", "*ZoneFameTaskFrequency", 0.0, 3600.0, 10.0, "s"),
    r("Player.ini", "DwarfRecipeLimit", 0.0, 500.0, 1.0, ""),
    r("Player.ini", "CommonRecipeLimit", 0.0, 500.0, 1.0, ""),
    r("Player.ini", "Days*", 0.0, 90.0, 1.0, "days"),
    r("Player.ini", "AltMaxNumOfClansInAlly", 1.0, 20.0, 1.0, ""),
    r("Player.ini", "AltClanMembersForWar", 1.0, 100.0, 1.0, ""),
    r("Player.ini", "AltPartyRange", 0.0, 5000.0, 50.0, ""),
    r("Player.ini", "StartingLevel", 1.0, 85.0, 1.0, ""),
    r("Player.ini", "RaidLootRightsInterval", 0.0, 3600.0, 10.0, "s"),
    r("Player.ini", "RaidLootRightsCCSize", 1.0, 200.0, 1.0, ""),
    r("Player.ini", "UnstuckInterval", 0.0, 3600.0, 10.0, "s"),
    r("Player.ini", "PlayerSpawnProtection", 0.0, 3600.0, 10.0, "s"),
    r("Player.ini", "PlayerTeleportProtection", 0.0, 3600.0, 10.0, "s"),
    r("Player.ini", "TeleportWatchdogTimeout", 0.0, 600.0, 5.0, "s"),
    r("Player.ini", "MaxOffsetOnTeleport", 0.0, 500.0, 5.0, ""),
    r("Player.ini", "MaxPetitionsPerPlayer", 0.0, 50.0, 1.0, ""),
    r("Player.ini", "MaxPetitionsPending", 0.0, 200.0, 1.0, ""),
    r("Player.ini", "MaxFreeTeleportLevel", 1.0, 85.0, 1.0, ""),
    r("Player.ini", "DeleteCharAfterDays", 0.0, 365.0, 1.0, "days"),
    r("Player.ini", "PartyXpCutoffPercent", 0.0, 100.0, 0.5, "%"),
    r("Player.ini", "PartyXpCutoffLevel", 1.0, 85.0, 1.0, ""),
    r("Player.ini", "NevitBonusEffectTime", 0.0, 3600.0, 10.0, "s"),
    // ---- General.ini ----
    r("General.ini", "AutoDestroyDroppedItemAfter", 0.0, 3600.0, 10.0, "s"),
    r("General.ini", "AutoDestroyHerbTime", 0.0, 3600.0, 10.0, "s"),
    r("General.ini", "CharacterDataStoreInterval", 1.0, 120.0, 1.0, "min"),
    r("General.ini", "SaveDroppedItemInterval", 0.0, 1440.0, 5.0, "min"),
    r("General.ini", "Min*Animation", 0.0, 300.0, 1.0, "s"),
    r("General.ini", "Max*Animation", 0.0, 300.0, 1.0, "s"),
    r("General.ini", "GridNeighborTurn*Time", 0.0, 600.0, 1.0, "s"),
    r("General.ini", "WearDelay", 0.0, 60.0, 1.0, "s"),
    r("General.ini", "MinimumChatLevel", 0.0, 85.0, 1.0, ""),
    r("General.ini", "AltManor*Min", 0.0, 59.0, 1.0, "min"),
    r("General.ini", "AltManor*Time", 0.0, 23.0, 1.0, "h"),
    r("General.ini", "AltLottery*NumberRate", 0.0, 1.0, 0.05, ""),
    r("General.ini", "AltItemAuctionExpiredAfter", 1.0, 60.0, 1.0, "days"),
    r("General.ini", "AltItemAuctionTimeExtendsOnBid", 0.0, 3600.0, 10.0, "s"),
    r("General.ini", "MaxRiftJumps", 0.0, 20.0, 1.0, ""),
    r("General.ini", "RiftMinPartySize", 1.0, 9.0, 1.0, ""),
    r("General.ini", "AutoJumpsDelay*", 0.0, 3600.0, 10.0, "s"),
    r("General.ini", "BossRoomTimeMultiply", 0.0, 10.0, 0.1, ""),
    r("General.ini", "BlockCheckerMinTeamMembers", 1.0, 9.0, 1.0, ""),
    r("General.ini", "BotReportDelay", 0.0, 1440.0, 5.0, "min"),
    // ---- NPC.ini ----
    r("NPC.ini", "MinNPCLevelFor*", 1.0, 85.0, 1.0, ""),
    r("NPC.ini", "DefaultCorpseTime", 0.0, 3600.0, 1.0, "s"),
    r("NPC.ini", "SpoiledCorpseExtendTime", 0.0, 3600.0, 1.0, "s"),
    r("NPC.ini", "MaxAggroRange", 0.0, 5000.0, 10.0, ""),
    r("NPC.ini", "MaxDriftRange", 0.0, 5000.0, 10.0, ""),
    r("NPC.ini", "AggroDistanceCheck*Range", 0.0, 10000.0, 50.0, ""),
    r("NPC.ini", "MaximumSlotsForPet", 0.0, 100.0, 1.0, ""),
    r("NPC.ini", "*RegenMultiplier", 0.0, 1000.0, 5.0, "%"),
    r("NPC.ini", "Raid*RespawnMultiplier", 0.0, 10.0, 0.1, ""),
    r("NPC.ini", "Raid*Multiplier", 0.0, 1000.0, 5.0, "%"),
    r("NPC.ini", "*ChaosTime", 0.0, 600.0, 1.0, "s"),
    // ---- Grand bosses, Gracia seeds ----
    r("GrandBoss.ini", "IntervalOf*Spawn", 0.0, 720.0, 1.0, "h"),
    r("GrandBoss.ini", "RandomOf*Spawn", 0.0, 168.0, 1.0, "h"),
    r("GrandBoss.ini", "*WaitTime", 0.0, 120.0, 1.0, "min"),
    r("GrandBoss.ini", "BelethMinPlayers", 1.0, 100.0, 1.0, ""),
    r("GraciaSeeds.ini", "MinLevel", 1.0, 85.0, 1.0, ""),
    r("GraciaSeeds.ini", "Min*Players", 0.0, 200.0, 1.0, ""),
    r("GraciaSeeds.ini", "Max*Players", 0.0, 200.0, 1.0, ""),
    r("GraciaSeeds.ini", "*KillCount*", 0.0, 100.0, 1.0, ""),
    // ---- Olympiad.ini ----
    r("Olympiad.ini", "OlympiadStartTime", 0.0, 23.0, 1.0, "h"),
    r("Olympiad.ini", "OlympiadMin", 0.0, 59.0, 1.0, "min"),
    r("Olympiad.ini", "OlympiadMaxBuffs", 0.0, 40.0, 1.0, ""),
    r("Olympiad.ini", "OlympiadStartPoints", 0.0, 100.0, 1.0, ""),
    r("Olympiad.ini", "OlympiadWeeklyPoints", 0.0, 100.0, 1.0, ""),
    r("Olympiad.ini", "OlympiadMaxPoints", 0.0, 100.0, 1.0, ""),
    r("Olympiad.ini", "Olympiad*Participants", 0.0, 100.0, 1.0, ""),
    r("Olympiad.ini", "OlympiadRegistrationDisplayNumber", 0.0, 1000.0, 10.0, ""),
    r("Olympiad.ini", "OlympiadMinMatchesForPoints", 0.0, 100.0, 1.0, ""),
    r("Olympiad.ini", "OlympiadHeroPoints", 0.0, 1000.0, 10.0, ""),
    r("Olympiad.ini", "OlympiadRank*Points", 0.0, 1000.0, 5.0, ""),
    r("Olympiad.ini", "OlympiadEnchantLimit", -1.0, 127.0, 1.0, ""),
    r("Olympiad.ini", "OlympiadWaitTime", 0.0, 600.0, 5.0, "s"),
    r("Olympiad.ini", "OlympiadDivider*", 1.0, 100.0, 1.0, ""),
    r("Olympiad.ini", "OlympiadMaxWeeklyMatches*", 0.0, 500.0, 5.0, ""),
    r("Olympiad.ini", "OlympiadPeriodMultiplier", 1.0, 12.0, 1.0, ""),
    // ---- PVP.ini, Flood protector ----
    r("PVP.ini", "MinimumPKRequiredToDrop", 0.0, 100.0, 1.0, ""),
    r("PVP.ini", "AntiFeedInterval", 0.0, 3600.0, 10.0, "s"),
    r("FloodProtector.ini", "FloodProtector*Interval", 0.0, 1000.0, 1.0, "ticks"),
    r("FloodProtector.ini", "FloodProtector*PunishmentLimit", 0.0, 1000.0, 1.0, ""),
    r("FloodProtector.ini", "FloodProtector*PunishmentTime", 0.0, 1440.0, 5.0, "min"),
    // ---- Sieges and residences ----
    r("Siege.ini", "SiegeCycle", 1.0, 8.0, 1.0, "weeks"),
    r("Siege.ini", "SiegeLength", 0.0, 240.0, 5.0, "min"),
    r("Siege.ini", "MaxFlags", 0.0, 20.0, 1.0, ""),
    r("Siege.ini", "SiegeClanMinLevel", 0.0, 11.0, 1.0, ""),
    r("Siege.ini", "AttackerMaxClans", 0.0, 1000.0, 10.0, ""),
    r("Siege.ini", "DefenderMaxClans", 0.0, 1000.0, 10.0, ""),
    r("FortSiege.ini", "SiegeLength", 0.0, 240.0, 5.0, "min"),
    r("FortSiege.ini", "CountDownLength", 0.0, 60.0, 1.0, "min"),
    r("FortSiege.ini", "MaxFlags", 0.0, 20.0, 1.0, ""),
    r("FortSiege.ini", "SiegeClanMinLevel", 0.0, 11.0, 1.0, ""),
    r("FortSiege.ini", "AttackerMaxClans", 0.0, 1000.0, 10.0, ""),
    r("TerritoryWar.ini", "WarLength", 0.0, 240.0, 5.0, "min"),
    r("TerritoryWar.ini", "ClanMinLevel", 0.0, 11.0, 1.0, ""),
    r("TerritoryWar.ini", "PlayerMinLevel", 1.0, 85.0, 1.0, ""),
    r("TerritoryWar.ini", "Defender*", 0.0, 1000.0, 10.0, ""),
    r("ConquerableHallSiege.ini", "MinClanLevel", 0.0, 11.0, 1.0, ""),
    r("ConquerableHallSiege.ini", "MaxAttackers", 0.0, 1000.0, 10.0, ""),
    r("ConquerableHallSiege.ini", "MaxFlagsPerClan", 0.0, 20.0, 1.0, ""),
    r("UndergroundColiseum.ini", "BattlePeriod", 0.0, 24.0, 1.0, "h"),
    r("UndergroundColiseum.ini", "PartySize", 1.0, 9.0, 1.0, ""),
    r("UndergroundColiseum.ini", "ResurrectionTime", 0.0, 120.0, 1.0, "s"),
    // ---- Geodata, id manager ----
    r("GeoEngine.ini", "*Weight", 0.0, 10.0, 0.05, ""),
    r("GeoEngine.ini", "MaxPostfilterPasses", 0.0, 20.0, 1.0, ""),
    r("IdManager.ini", "ResizeThreshold", 0.0, 1.0, 0.05, ""),
    r("IdManager.ini", "ResizeMultiplier", 1.0, 10.0, 0.1, ""),
    // ---- Custom/*.ini ----
    r("Custom/AutoPlay.ini", "ShortRange", 0.0, 5000.0, 50.0, ""),
    r("Custom/AutoPlay.ini", "LongRange", 0.0, 10000.0, 50.0, ""),
    r("Custom/AutoPotions.ini", "AutoPotionMinimumLevel", 1.0, 85.0, 1.0, ""),
    r("Custom/AutoPotions.ini", "Auto*Percentage", 0.0, 100.0, 1.0, "%"),
    r("Custom/CancelReturn.ini", "TimeToReturn", 0.0, 600.0, 1.0, "s"),
    r("Custom/Captcha.ini", "KillCounter", 0.0, 10000.0, 10.0, ""),
    r("Custom/Captcha.ini", "KillCounterRandomization", 0.0, 1000.0, 10.0, ""),
    r("Custom/Captcha.ini", "KillCounterResetTime", 0.0, 168.0, 1.0, "h"),
    r("Custom/Captcha.ini", "ValidationTime", 0.0, 600.0, 5.0, "s"),
    r("Custom/Captcha.ini", "CaptchaAttempts", 1.0, 10.0, 1.0, ""),
    r("Custom/Captcha.ini", "JailTime", 0.0, 1440.0, 5.0, "min"),
    r("Custom/ChampionMonsters.ini", "ChampionFrequency", 0.0, 100.0, 1.0, "%"),
    r("Custom/ChampionMonsters.ini", "Champion*Level", 1.0, 85.0, 1.0, ""),
    r("Custom/ChampionMonsters.ini", "ChampionHp", 1.0, 100.0, 1.0, ""),
    r("Custom/ChampionMonsters.ini", "ChampionHpRegen", 0.0, 100.0, 0.1, ""),
    r("Custom/ChampionMonsters.ini", "ChampionRewards*", 0.0, 100.0, 0.1, ""),
    r("Custom/ChampionMonsters.ini", "ChampionAdenasRewards*", 0.0, 100.0, 0.1, ""),
    r("Custom/ChampionMonsters.ini", "ChampionAtk", 0.0, 10.0, 0.1, ""),
    r("Custom/ChampionMonsters.ini", "ChampionSpdAtk", 0.0, 10.0, 0.1, ""),
    r("Custom/ChampionMonsters.ini", "ChampionReward*ItemChance", 0.0, 100.0, 1.0, "%"),
    r("Custom/DelevelManager.ini", "MimimumDelevel", 1.0, 85.0, 1.0, ""),
    r("Custom/DualboxCheck.ini", "DualboxCheckMax*", 0.0, 20.0, 1.0, ""),
    r("Custom/FactionSystem.ini", "BalancePlayerExceedLimit", 0.0, 100.0, 1.0, ""),
    r("Custom/NoblessMaster.ini", "LevelRequirement", 1.0, 85.0, 1.0, ""),
    r("Custom/NpcStatMultipliers.ini", "Monster*", 0.0, 10.0, 0.1, ""),
    r("Custom/NpcStatMultipliers.ini", "Raidboss*", 0.0, 10.0, 0.1, ""),
    r("Custom/NpcStatMultipliers.ini", "Guard*", 0.0, 10.0, 0.1, ""),
    r("Custom/NpcStatMultipliers.ini", "Defender*", 0.0, 10.0, 0.1, ""),
    r("Custom/OfflineTrade.ini", "OfflineMaxDays", 0.0, 365.0, 1.0, "days"),
    r("Custom/PremiumSystem.ini", "DoublingAcquisitionPointsChance", 0.0, 100.0, 1.0, "%"),
    r("Custom/PremiumSystem.ini", "RewardLowExpKillsChance", 0.0, 100.0, 1.0, "%"),
    r("Custom/PremiumSystem.ini", "AcquisitionPointsRate", 0.0, 10.0, 0.1, ""),
    r("Custom/PremiumSystem.ini", "PremiumRate*", 0.0, 100.0, 0.1, ""),
    r("Custom/PrivateStoreRange.ini", "ShopMinRange*", 0.0, 1000.0, 10.0, ""),
    r("Custom/RandomSpawns.ini", "MaxSpawnMobRange", 0.0, 2000.0, 10.0, ""),
    r("Custom/Rebirth.ini", "RebirthMin", 1.0, 85.0, 1.0, ""),
    r("Custom/Rebirth.ini", "RebirthMaxCount", 0.0, 100.0, 1.0, ""),
    r("Custom/Rebirth.ini", "RebirthMaxSelectedSkills", 0.0, 20.0, 1.0, ""),
    r("Custom/Rebirth.ini", "RebirthScreenMessageTime", 0.0, 60000.0, 500.0, "ms"),
    r("Custom/SchemeBuffer.ini", "BufferMaxSchemesPerChar", 1.0, 20.0, 1.0, ""),
    r("Custom/ScreenWelcomeMessage.ini", "ScreenWelcomeMessageTime", 0.0, 300.0, 1.0, "s"),
    r("Custom/SellBuffs.ini", "MpCostMultipler", 0.0, 10.0, 0.1, ""),
    r("Custom/SellBuffs.ini", "MaxBuffs", 1.0, 40.0, 1.0, ""),
    r("Custom/Wedding.ini", "WeddingTeleportDuration", 0.0, 600.0, 5.0, "s"),
    r("Custom/Wedding.ini", "WeddingDivorceCosts", 0.0, 100.0, 1.0, "%"),
];

/// One choice of a drop-down.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnumOption {
    /// What is written to the file.
    pub value: String,
    /// What the drop-down shows (`2 - Enabled using geodata cells`).
    pub label: String,
}

impl EnumOption {
    fn word(w: &str) -> Self {
        Self {
            value: w.to_owned(),
            label: w.to_owned(),
        }
    }
}

/// Enum-like keys whose comment does not list the words in a form [`enum_from_help`] reads.
const EXPLICIT_ENUMS: &[(&str, &[&str])] = &[
    ("FloodProtector*PunishmentType", &["none", "kick", "ban", "jail"]),
    (
        "RebirthScreenMessagePosition",
        &[
            "TOP_LEFT",
            "TOP_CENTER",
            "TOP_RIGHT",
            "MIDDLE_LEFT",
            "MIDDLE_CENTER",
            "MIDDLE_RIGHT",
            "BOTTOM_CENTER",
            "BOTTOM_RIGHT",
        ],
    ),
];

/// The allowed values of `key`, from the explicit table or from its comment. `None` for a key
/// that is not enum-like.
pub fn enum_options(key: &str, help: &[String]) -> Option<Vec<EnumOption>> {
    for (pattern, words) in EXPLICIT_ENUMS {
        if key_matches(pattern, key) {
            return Some(words.iter().map(|w| EnumOption::word(w)).collect());
        }
    }
    enum_from_help(help)
}

/// Reads the allowed values out of the comment above a key. Three shapes are known:
/// - `Available Options: a, b, c` (also `Available values:`, `Accepted Values:`);
/// - at least two lines `N = text` in a row (`0 = Disabled`);
/// - at least two lines `WORD - text` in a row (`KICK - kick player`).
pub fn enum_from_help(help: &[String]) -> Option<Vec<EnumOption>> {
    const LISTS: [&str; 5] = [
        "available options:",
        "available values:",
        "accepted values:",
        "valid values:",
        "possible values:",
    ];
    for line in help {
        let lower = line.trim().to_lowercase();
        for prefix in LISTS {
            if let Some(rest) = lower.strip_prefix(prefix) {
                // Cut the original text at the same place (lower-casing keeps ASCII lengths).
                let original = line.trim().get(prefix.len()..).unwrap_or(rest);
                let words: Vec<&str> = original
                    .trim()
                    .trim_end_matches('.')
                    .split(',')
                    .map(str::trim)
                    .filter(|w| !w.is_empty())
                    .collect();
                if words.len() >= 2 && words.iter().all(|w| !w.contains(char::is_whitespace)) {
                    return Some(words.into_iter().map(EnumOption::word).collect());
                }
            }
        }
    }
    best_run(help, number_line).or_else(|| best_run(help, word_line))
}

/// The longest run (at least two) of consecutive lines `line` accepts.
fn best_run(help: &[String], line: fn(&str) -> Option<EnumOption>) -> Option<Vec<EnumOption>> {
    let mut best: Vec<EnumOption> = Vec::new();
    let mut run: Vec<EnumOption> = Vec::new();
    for h in help {
        match line(h.trim()) {
            Some(o) => run.push(o),
            None => {
                if run.len() > best.len() {
                    best = std::mem::take(&mut run);
                }
                run.clear();
            }
        }
    }
    if run.len() > best.len() {
        best = run;
    }
    (best.len() >= 2).then_some(best)
}

/// `0 = Disabled` / `2 = Enabled using geodata cells (recommended).`
fn number_line(line: &str) -> Option<EnumOption> {
    let (num, text) = line.split_once('=')?;
    let num = num.trim();
    num.parse::<i64>().ok()?;
    let text = text.trim().trim_end_matches('.');
    (!text.is_empty()).then(|| EnumOption {
        value: num.to_owned(),
        label: format!("{num} - {text}"),
    })
}

/// `KICK - kick player (default)`: an upper-case word, a dash, text.
fn word_line(line: &str) -> Option<EnumOption> {
    let (word, text) = line.split_once(" - ")?;
    let word = word.trim();
    let valid = word.len() >= 3 && word.chars().all(|c| c.is_ascii_uppercase() || c == '_');
    (valid && !text.trim().is_empty()).then(|| EnumOption {
        value: word.to_owned(),
        label: format!("{word} - {}", text.trim()),
    })
}

/// A key that holds an `RRGGBB`-style hex colour (`GoodNameColor = 00FF00`).
pub fn is_color_key(key: &str) -> bool {
    key.contains("Color")
}

/// The six hex digits of a colour value as (r, g, b), or `None` if the text is not six hex digits.
///
/// Java reads these with `Integer.decode("0x" + value)` and hands the int to the client, which
/// takes it as `0xBBGGRR`. So `0000FF` (the default `EvilNameColor`) shows as red in game, and
/// the preview reads the bytes the same way.
pub fn parse_color_bgr(value: &str) -> Option<(u8, u8, u8)> {
    let v = value.trim();
    if v.len() != 6 || !v.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let n = u32::from_str_radix(v, 16).ok()?;
    let [_, b, g, r] = n.to_be_bytes();
    Some((r, g, b))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn help(lines: &[&str]) -> Vec<String> {
        lines.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn patterns_use_star_wildcards() {
        assert!(key_matches("RateXp", "RateXp"));
        assert!(!key_matches("RateXp", "RateXpX"));
        assert!(key_matches("Rate*", "RateXp"));
        assert!(key_matches("Rate*", "Rate"));
        assert!(key_matches("*Chance", "DeathChance"));
        assert!(!key_matches("*Chance", "ChanceOf"));
        assert!(key_matches("Drop*LevelGapChance", "DropAdenaMinLevelGapChance"));
        assert!(!key_matches("Drop*LevelGapChance", "DropAdenaMinLevelDifference"));
        assert!(key_matches("A*B*C", "AxxBxxC"));
        assert!(!key_matches("A*B*C", "AxxCxxB"));
        // The text between two stars may not overlap what the ends already used.
        assert!(!key_matches("AB*BC", "ABC"));
        assert!(key_matches("*", "anything"));
    }

    #[test]
    fn the_first_entry_wins() {
        let karma = range_for("Rates.ini", "RateKarmaLost").unwrap();
        assert_eq!(karma.min, -1.0);
        let xp = range_for("Rates.ini", "RateXp").unwrap();
        assert_eq!((xp.min, xp.max), (0.0, 100.0));
        // The file scope matters: the same name elsewhere is not the rate.
        assert!(range_for("General.ini", "RateXp").is_none());
        // A pattern with a file scope reaches only that file's keys.
        assert_eq!(
            range_for("Player.ini", "EnchantChanceElementJewel")
                .unwrap()
                .unit(),
            Some("%")
        );
        assert!(range_for("Rates.ini", "EnchantChanceElementJewel").is_none());
    }

    #[test]
    fn range_helpers() {
        let p = r("", "x", 1.0, 65535.0, 1.0, "");
        assert!(p.is_integral() && p.contains(2106.0) && !p.contains(0.0));
        assert_eq!(p.unit(), None);
        let f = r("", "x", 0.0, 1.0, 0.05, "s");
        assert!(!f.is_integral());
        assert_eq!(f.decimals(), 2);
        assert_eq!(f.unit(), Some("s"));
        assert_eq!(r("", "x", 0.0, 10.0, 0.1, "").decimals(), 1);
        assert_eq!(p.decimals(), 0);
    }

    #[test]
    fn list_keys_are_recognised() {
        for k in [
            "LogItemsIdsList",
            "AutoLootItemIds",
            "ListOfProtectedItems",
            "ServerRestartDays",
            "SkillReuseList",
        ] {
            assert!(is_list_key(k), "{k}");
        }
        for k in ["RateXp", "RewardPvpItemId", "NpcId", "PathFinding"] {
            assert!(!is_list_key(k), "{k}");
        }
    }

    #[test]
    fn enum_from_an_options_line() {
        let h = help(&[
            "Global Chat.",
            "Available Options: ON, OFF, GM, GLOBAL",
            "Default: ON",
        ]);
        let o = enum_options("GlobalChat", &h).unwrap();
        let values: Vec<&str> = o.iter().map(|o| o.value.as_str()).collect();
        assert_eq!(values, ["ON", "OFF", "GM", "GLOBAL"]);
        let h = help(&[
            "Notes:",
            "Accepted Values: Normal, Relax, Test, Broad, Restricted, Event, Free, World, New, Classic",
            "Default: Free",
        ]);
        assert_eq!(enum_options("ServerListType", &h).unwrap().len(), 10);
        let h = help(&["Accepted values: 0, 15, 18"]);
        assert_eq!(enum_options("ServerListAge", &h).unwrap()[2].value, "18");
        // Placeholders are not an enum.
        assert!(enum_options("X", &help(&["Available placeholders: %player%, %max%"])).is_none());
    }

    #[test]
    fn enum_from_numbered_lines() {
        let h = help(&[
            "Pathfinding options:",
            "0 = Disabled",
            "1 = Enabled using path node files.",
            "2 = Enabled using geodata cells at runtime (recommended).",
            "Default: 2",
        ]);
        let o = enum_options("PathFinding", &h).unwrap();
        assert_eq!(o.len(), 3);
        assert_eq!(o[2].value, "2");
        assert_eq!(o[0].label, "0 - Disabled");
        // A single "1 = disable." line is a hint about one value, not a list.
        assert!(enum_options("CaptchaAttempts", &help(&["Number of retries.", "1 = disable."])).is_none());
        // Tabs in front of the numbers (as in Captcha.ini) are fine.
        let h = help(&[
            "Punishments:",
            "\t0 = move character",
            " \t1 = kick",
            "\t2 = jail",
        ]);
        assert_eq!(enum_options("Punishment", &h).unwrap().len(), 3);
    }

    #[test]
    fn enum_from_word_lines() {
        let h = help(&[
            "Player punishment for illegal actions:",
            "BROADCAST - broadcast warning to GMs only",
            "KICK - kick player (default)",
            "KICKBAN - kick and ban player",
            "JAIL - jail player",
        ]);
        let o = enum_options("DefaultPunish", &h).unwrap();
        let values: Vec<&str> = o.iter().map(|o| o.value.as_str()).collect();
        assert_eq!(values, ["BROADCAST", "KICK", "KICKBAN", "JAIL"]);
        assert!(enum_options("X", &help(&["KICK - only one line"])).is_none());
    }

    #[test]
    fn explicit_enums_win_and_plain_keys_have_none() {
        let o = enum_options("FloodProtectorUseItemPunishmentType", &[]).unwrap();
        assert_eq!(o.len(), 4);
        assert_eq!(
            enum_options("RebirthScreenMessagePosition", &[]).unwrap().len(),
            8
        );
        assert!(enum_options("RateXp", &help(&["Experience multiplier"])).is_none());
    }

    #[test]
    fn colours_are_read_as_the_client_does() {
        assert!(is_color_key("GoodNameColor") && is_color_key("ColorForAmount1"));
        assert!(!is_color_key("RateXp"));
        // 0xBBGGRR: the default EvilNameColor (0000FF) is red.
        assert_eq!(parse_color_bgr("0000FF"), Some((255, 0, 0)));
        assert_eq!(parse_color_bgr("00ff00"), Some((0, 255, 0)));
        assert_eq!(parse_color_bgr("808080"), Some((128, 128, 128)));
        assert_eq!(parse_color_bgr("12345"), None);
        assert_eq!(parse_color_bgr("GGGGGG"), None);
        assert_eq!(parse_color_bgr(""), None);
    }
}
