//! The stats a creature has and items/skills modify.
//!
//! Java: `gameserver/mechanics/stats/Stat.java`.
//!
//! Variants are in Java's declaration order (`ordinal()` is the slot index of Java's
//! `Calculator` array, `NUM_STATS` its length). Each variant is listed with its Java constant and its
//! XML name.

macro_rules! stat_enum {
    (
        $( $variant:ident = $java:literal, $xml:literal ),+ $(,)?
    ) => {
        /// Java `Stat`.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum Stat {
            $( #[doc = concat!("`", $java, "`, XML `", $xml, "`.")] $variant ),+
        }

        impl Stat {
            /// Every stat, in Java's declaration (ordinal) order.
            pub const ALL: &'static [Stat] = &[ $( Stat::$variant ),+ ];

            /// The Java constant name, e.g. `MAX_HP` (it appears in Java's log lines).
            pub fn java_name(self) -> &'static str {
                match self { $( Stat::$variant => $java ),+ }
            }

            /// Java `getValue()`: the name the XML files use, e.g. `maxHp`.
            pub fn xml_name(self) -> &'static str {
                match self { $( Stat::$variant => $xml ),+ }
            }

            /// Java `valueOfXml(name)`: exact and case sensitive (`pAtk`, not `patk`).
            /// `None` where Java throws `NoSuchElementException`.
            pub fn from_xml(name: &str) -> Option<Self> {
                match name { $( $xml => Some(Stat::$variant), )+ _ => None }
            }

            /// Java `ordinal()`.
            pub fn ordinal(self) -> usize {
                self as usize
            }
        }
    };
}

/// Java `Stat.NUM_STATS`: the number of stats.
pub const NUM_STATS: usize = Stat::ALL.len();

stat_enum! {
        MaxHp = "MAX_HP", "maxHp",
        MaxMp = "MAX_MP", "maxMp",
        MaxCp = "MAX_CP", "maxCp",
        MaxRecoverableHp = "MAX_RECOVERABLE_HP", "maxRecoverableHp",
        MaxRecoverableMp = "MAX_RECOVERABLE_MP", "maxRecoverableMp",
        MaxRecoverableCp = "MAX_RECOVERABLE_CP", "maxRecoverableCp",
        RegenerateHpRate = "REGENERATE_HP_RATE", "regHp",
        RegenerateCpRate = "REGENERATE_CP_RATE", "regCp",
        RegenerateMpRate = "REGENERATE_MP_RATE", "regMp",
        ManaCharge = "MANA_CHARGE", "manaCharge",
        HealEffect = "HEAL_EFFECT", "healEffect",
        PowerDefence = "POWER_DEFENCE", "pDef",
        MagicDefence = "MAGIC_DEFENCE", "mDef",
        PowerAttack = "POWER_ATTACK", "pAtk",
        MagicAttack = "MAGIC_ATTACK", "mAtk",
        PhysicalSkillPower = "PHYSICAL_SKILL_POWER", "physicalSkillPower",
        PowerAttackSpeed = "POWER_ATTACK_SPEED", "pAtkSpd",
        MagicAttackSpeed = "MAGIC_ATTACK_SPEED", "mAtkSpd",
        AtkReuse = "ATK_REUSE", "atkReuse",
        PReuse = "P_REUSE", "pReuse",
        MagicReuseRate = "MAGIC_REUSE_RATE", "mReuse",
        DanceReuse = "DANCE_REUSE", "dReuse",
        ShieldDefence = "SHIELD_DEFENCE", "sDef",
        CriticalDamage = "CRITICAL_DAMAGE", "critDmg",
        CriticalDamagePos = "CRITICAL_DAMAGE_POS", "critDmgPos",
        CriticalDamageAdd = "CRITICAL_DAMAGE_ADD", "critDmgAdd",
        MagicCritDmg = "MAGIC_CRIT_DMG", "mCritPower",
        PvpPhysicalDmg = "PVP_PHYSICAL_DMG", "pvpPhysDmg",
        PvpMagicalDmg = "PVP_MAGICAL_DMG", "pvpMagicalDmg",
        PvpPhysSkillDmg = "PVP_PHYS_SKILL_DMG", "pvpPhysSkillsDmg",
        PvpPhysicalDef = "PVP_PHYSICAL_DEF", "pvpPhysDef",
        PvpMagicalDef = "PVP_MAGICAL_DEF", "pvpMagicalDef",
        PvpPhysSkillDef = "PVP_PHYS_SKILL_DEF", "pvpPhysSkillsDef",
        PvePhysicalDmg = "PVE_PHYSICAL_DMG", "pvePhysDmg",
        PvePhysSkillDmg = "PVE_PHYS_SKILL_DMG", "pvePhysSkillsDmg",
        PveBowDmg = "PVE_BOW_DMG", "pveBowDmg",
        PveBowSkillDmg = "PVE_BOW_SKILL_DMG", "pveBowSkillsDmg",
        PveMagicalDmg = "PVE_MAGICAL_DMG", "pveMagicalDmg",
        EvasionRate = "EVASION_RATE", "rEvas",
        PSkillEvasion = "P_SKILL_EVASION", "pSkillEvas",
        DefenceCriticalRate = "DEFENCE_CRITICAL_RATE", "defCritRate",
        DefenceCriticalRateAdd = "DEFENCE_CRITICAL_RATE_ADD", "defCritRateAdd",
        DefenceCriticalDamage = "DEFENCE_CRITICAL_DAMAGE", "defCritDamage",
        DefenceCriticalDamageAdd = "DEFENCE_CRITICAL_DAMAGE_ADD", "defCritDamageAdd",
        ShieldRate = "SHIELD_RATE", "rShld",
        CriticalRate = "CRITICAL_RATE", "critRate",
        CriticalRatePos = "CRITICAL_RATE_POS", "critRatePos",
        BlowRate = "BLOW_RATE", "blowRate",
        McriticalRate = "MCRITICAL_RATE", "mCritRate",
        ExpspRate = "EXPSP_RATE", "rExp",
        BonusExp = "BONUS_EXP", "bonusExp",
        BonusSp = "BONUS_SP", "bonusSp",
        BonusDropAdena = "BONUS_DROP_ADENA", "bonusDropAdena",
        BonusDropAmount = "BONUS_DROP_AMOUNT", "bonusDropAmount",
        BonusDropRate = "BONUS_DROP_RATE", "bonusDropRate",
        BonusSpoilRate = "BONUS_SPOIL_RATE", "bonusSpoilRate",
        AttackCancel = "ATTACK_CANCEL", "cancel",
        AccuracyCombat = "ACCURACY_COMBAT", "accCombat",
        PowerAttackRange = "POWER_ATTACK_RANGE", "pAtkRange",
        MagicAttackRange = "MAGIC_ATTACK_RANGE", "mAtkRange",
        AttackCountMax = "ATTACK_COUNT_MAX", "atkCountMax",
        MoveSpeed = "MOVE_SPEED", "runSpd",
        StatStr = "STAT_STR", "STR",
        StatCon = "STAT_CON", "CON",
        StatDex = "STAT_DEX", "DEX",
        StatInt = "STAT_INT", "INT",
        StatWit = "STAT_WIT", "WIT",
        StatMen = "STAT_MEN", "MEN",
        Breath = "BREATH", "breath",
        Fall = "FALL", "fall",
        DamageZoneVuln = "DAMAGE_ZONE_VULN", "damageZoneVuln",
        MovementVuln = "MOVEMENT_VULN", "movementVuln",
        CancelVuln = "CANCEL_VULN", "cancelVuln",
        DebuffVuln = "DEBUFF_VULN", "debuffVuln",
        BuffVuln = "BUFF_VULN", "buffVuln",
        FireRes = "FIRE_RES", "fireRes",
        WindRes = "WIND_RES", "windRes",
        WaterRes = "WATER_RES", "waterRes",
        EarthRes = "EARTH_RES", "earthRes",
        HolyRes = "HOLY_RES", "holyRes",
        DarkRes = "DARK_RES", "darkRes",
        MagicSuccessRes = "MAGIC_SUCCESS_RES", "magicSuccRes",
        DebuffImmunity = "DEBUFF_IMMUNITY", "debuffImmunity",
        FirePower = "FIRE_POWER", "firePower",
        WaterPower = "WATER_POWER", "waterPower",
        WindPower = "WIND_POWER", "windPower",
        EarthPower = "EARTH_POWER", "earthPower",
        HolyPower = "HOLY_POWER", "holyPower",
        DarkPower = "DARK_POWER", "darkPower",
        CancelProf = "CANCEL_PROF", "cancelProf",
        ReflectDamagePercent = "REFLECT_DAMAGE_PERCENT", "reflectDam",
        ReflectSkillMagic = "REFLECT_SKILL_MAGIC", "reflectSkillMagic",
        ReflectSkillPhysic = "REFLECT_SKILL_PHYSIC", "reflectSkillPhysic",
        VengeanceSkillMagicDamage = "VENGEANCE_SKILL_MAGIC_DAMAGE", "vengeanceMdam",
        VengeanceSkillPhysicalDamage = "VENGEANCE_SKILL_PHYSICAL_DAMAGE", "vengeancePdam",
        AbsorbDamagePercent = "ABSORB_DAMAGE_PERCENT", "absorbDam",
        TransferDamagePercent = "TRANSFER_DAMAGE_PERCENT", "transDam",
        ManaShieldPercent = "MANA_SHIELD_PERCENT", "manaShield",
        TransferDamageToPlayer = "TRANSFER_DAMAGE_TO_PLAYER", "transDamToPlayer",
        AbsorbManaDamagePercent = "ABSORB_MANA_DAMAGE_PERCENT", "absorbDamMana",
        WeightLimit = "WEIGHT_LIMIT", "weightLimit",
        WeightPenalty = "WEIGHT_PENALTY", "weightPenalty",
        InvLim = "INV_LIM", "inventoryLimit",
        WhLim = "WH_LIM", "whLimit",
        FreightLim = "FREIGHT_LIM", "freightLimit",
        PSellLim = "P_SELL_LIM", "privateSellLimit",
        PBuyLim = "P_BUY_LIM", "privateBuyLimit",
        RecDLim = "REC_D_LIM", "dwarfRecipeLimit",
        RecCLim = "REC_C_LIM", "commonRecipeLimit",
        PhysicalMpConsumeRate = "PHYSICAL_MP_CONSUME_RATE", "physicalMpConsumeRate",
        MagicalMpConsumeRate = "MAGICAL_MP_CONSUME_RATE", "magicalMpConsumeRate",
        DanceMpConsumeRate = "DANCE_MP_CONSUME_RATE", "danceMpConsumeRate",
        BowMpConsumeRate = "BOW_MP_CONSUME_RATE", "bowMpConsumeRate",
        MpConsume = "MP_CONSUME", "mpConsume",
        ShieldDefenceAngle = "SHIELD_DEFENCE_ANGLE", "shieldDefAngle",
        SkillMastery = "SKILL_MASTERY", "skillMastery",
        SkillMasteryRate = "SKILL_MASTERY_RATE", "skillMasteryRate",
        VitalityConsumeRate = "VITALITY_CONSUME_RATE", "vitalityConsumeRate",
        MaxSouls = "MAX_SOULS", "maxSouls",
        ReduceExpLostByPvp = "REDUCE_EXP_LOST_BY_PVP", "reduceExpLostByPvp",
        ReduceExpLostByMob = "REDUCE_EXP_LOST_BY_MOB", "reduceExpLostByMob",
        ReduceExpLostByRaid = "REDUCE_EXP_LOST_BY_RAID", "reduceExpLostByRaid",
        ReduceDeathPenaltyByPvp = "REDUCE_DEATH_PENALTY_BY_PVP", "reduceDeathPenaltyByPvp",
        ReduceDeathPenaltyByMob = "REDUCE_DEATH_PENALTY_BY_MOB", "reduceDeathPenaltyByMob",
        ReduceDeathPenaltyByRaid = "REDUCE_DEATH_PENALTY_BY_RAID", "reduceDeathPenaltyByRaid",
        FishingExpertise = "FISHING_EXPERTISE", "fishingExpertise",
        RandomDamage = "RANDOM_DAMAGE", "randomDamage",
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn every_stat_round_trips_through_its_xml_name() {
        let mut seen = HashSet::new();
        for (index, &stat) in Stat::ALL.iter().enumerate() {
            assert_eq!(Stat::from_xml(stat.xml_name()), Some(stat));
            assert!(
                stat.java_name()
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c == '_')
            );
            assert_eq!(stat.ordinal(), index);
            assert!(
                seen.insert(stat.xml_name()),
                "duplicate xml name {}",
                stat.xml_name()
            );
        }
        assert_eq!(NUM_STATS, 127);
    }

    #[test]
    fn matching_is_exact_and_case_sensitive() {
        assert_eq!(Stat::from_xml("pAtk"), Some(Stat::PowerAttack));
        assert_eq!(Stat::from_xml("patk"), None);
        assert_eq!(Stat::from_xml("PATK"), None);
        assert_eq!(Stat::from_xml(" pAtk"), None);
        assert_eq!(Stat::from_xml(""), None);
        // The six base stats are upper case, like in Java.
        assert_eq!(Stat::from_xml("STR"), Some(Stat::StatStr));
        assert_eq!(Stat::from_xml("str"), None);
    }

    #[test]
    fn ends_and_samples_of_javas_order() {
        assert_eq!(Stat::ALL[0], Stat::MaxHp);
        assert_eq!(Stat::ALL[NUM_STATS - 1], Stat::RandomDamage);
        assert_eq!(Stat::ALL[Stat::PowerAttack.ordinal() - 1], Stat::MagicDefence);
        assert_eq!(Stat::MoveSpeed.xml_name(), "runSpd");
        assert_eq!(Stat::MaxHp.java_name(), "MAX_HP");
        assert_eq!(Stat::PowerAttackSpeed.java_name(), "POWER_ATTACK_SPEED");
        assert_eq!(Stat::MagicSuccessRes.xml_name(), "magicSuccRes");
    }
}
