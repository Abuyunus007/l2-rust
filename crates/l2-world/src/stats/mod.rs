//! The stat engine: per-creature calculators, the standard formula functions, the functions of
//! equipped items, condition evaluation and the stat getters every packet reads.
//! Design: `docs/design/m4-combat.md` ("Stat engine rules").
// Java: mechanics/stats/{Calculator,Stat,BaseStat,Formulas}.java, mechanics/stats/functions/**,
//       mechanics/conditions/*, entity/actor/stat/{CreatureStat,PlayableStat,PlayerStat,NpcStat}.java
//
// # How it fits together
// - Every creature has a [`Calculators`] value in its `CreatureCore`: for each [`Stat`] an
//   ordered list of [`Func`]s (Java's `Calculator[]`, indexed by `Stat.ordinal()`). Players get
//   their own set at enter world (`Formulas.addFuncsToNewCharacter`); NPCs share the standard
//   NPC set through an `Arc` and copy it on the first change, like Java's `NPC_STD_CALCULATOR`.
// - A function is data: its stat, order, value, owner ([`FuncOwner`], an id, never a reference)
//   and optional apply condition. The standard formula functions are [`StdFunc`] variants.
// - Reading a stat goes through a [`StatView`], a read-only view of one creature built from
//   `&World` (`World::stats`). `StatView::calc_stat` folds the value through the functions in
//   order; a standard function that needs another stat (`FuncPAtkMod` needs STR) asks the same
//   view again, exactly as Java recomputes `getSTR()` inside `calc`.
// - Conditions ([`conditions`]) are tested against the same view. A condition kind whose state
//   the world does not model yet evaluates to `false` and is logged once.
// - The immutable tables the engine reads come from [`StatData`], which the game server builds
//   over the datapack and hands over through `StaticData::stat_data`. Without it (tests that
//   load nothing) every template value is Java's `CreatureTemplate` default and every base stat
//   bonus is 1.
//
// - Skills ([`skills`]): `Creature.addSkill` replaces a skill of the same id; a passive skill
//   applies the stat functions of its passive effects (owned by the skill). Players get them at
//   enter world (item and armor set skills while the items are equipped, then the learned
//   skills, then the weight and grade penalties), NPCs from their template at spawn.
//
// Not here yet: effects other than passive stat functions (M5), transformations (hooks are
// `None`), summons and pets.

pub mod calculators;
pub mod conditions;
pub mod creature_stat;
pub mod npc_stat;
pub mod player_stat;
pub mod skills;
pub mod std_funcs;
#[cfg(test)]
mod world_tests;

use std::sync::Arc;

use l2_data::{ArmorSets, EnchantHpBonus, Hennas, ItemTemplates, PlayerTemplates, SkillTemplates};

pub use calculators::{CalcSet, Calculators, Func, FuncCalc, FuncOwner};
pub use creature_stat::{MoveType, StatView};
pub use l2_data::stats::NUM_STATS;
pub use l2_data::{FuncKind, Stat};
pub use player_stat::HennaStats;
pub use std_funcs::StdFunc;

/// Java `BaseStat`: the stats whose value picks a multiplier out of `statBonus.xml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BaseStat {
    Str,
    Int,
    Dex,
    Wit,
    Con,
    Men,
}

impl BaseStat {
    const ALL: [BaseStat; 6] = [
        BaseStat::Str,
        BaseStat::Int,
        BaseStat::Dex,
        BaseStat::Wit,
        BaseStat::Con,
        BaseStat::Men,
    ];

    fn index(self) -> usize {
        match self {
            BaseStat::Str => 0,
            BaseStat::Int => 1,
            BaseStat::Dex => 2,
            BaseStat::Wit => 3,
            BaseStat::Con => 4,
            BaseStat::Men => 5,
        }
    }

    /// The section name in `statBonus.xml` (Java compares it ignoring case).
    pub fn from_section(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|s| s.section().eq_ignore_ascii_case(name))
    }

    fn section(self) -> &'static str {
        match self {
            BaseStat::Str => "STR",
            BaseStat::Int => "INT",
            BaseStat::Dex => "DEX",
            BaseStat::Wit => "WIT",
            BaseStat::Con => "CON",
            BaseStat::Men => "MEN",
        }
    }
}

/// Java `BaseStat.MAX_STAT_VALUE`: the length of each bonus table.
pub const MAX_STAT_VALUE: usize = 100;

/// The six tables of `data/stats/statBonus.xml` (Java `BaseStat.STRbonus` ...). The game server
/// parses the file; the world only reads it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BaseStatBonus {
    /// `None` = nothing loaded: every bonus is 1 (what Java's `calcBonus` returns for a null
    /// actor), so tests without a datapack get plain template values.
    tables: Option<Box<[[f64; MAX_STAT_VALUE]; 6]>>,
}

impl BaseStatBonus {
    /// Loaded tables with every entry 0.0, the state of Java's arrays before the file fills them.
    pub fn loaded() -> Self {
        Self {
            tables: Some(Box::new([[0.0; MAX_STAT_VALUE]; 6])),
        }
    }

    /// Sets one entry (`<stat value=".." bonus=".."/>`). `false` for a value outside the table
    /// (Java: `ArrayIndexOutOfBoundsException` while loading).
    pub fn set(&mut self, stat: BaseStat, value: usize, bonus: f64) -> bool {
        let tables = self
            .tables
            .get_or_insert_with(|| Box::new([[0.0; MAX_STAT_VALUE]; 6]));
        match tables[stat.index()].get_mut(value) {
            Some(slot) => {
                *slot = bonus;
                true
            }
            None => false,
        }
    }

    /// Java `BaseStat.X.calcBonus(actor)`: the multiplier of `stat` at `value`.
    // NOTE(java-quirk): Java indexes the array with the raw stat value, so a stat of 100 or more
    // (or below 0) throws `ArrayIndexOutOfBoundsException` in the middle of the calculation. Here
    // such a value reads as 0.0, the value of an unlisted entry.
    pub fn bonus(&self, stat: BaseStat, value: i32) -> f64 {
        let Some(tables) = &self.tables else {
            return 1.0;
        };
        usize::try_from(value)
            .ok()
            .and_then(|v| tables[stat.index()].get(v))
            .copied()
            .unwrap_or(0.0)
    }
}

/// The immutable datapack tables the stat engine reads.
#[derive(Debug, Clone, Default)]
pub struct StatData {
    /// Item templates: their stat functions, kinds, grades and body parts.
    pub items: Arc<ItemTemplates>,
    /// Player class templates: base stats, per-level HP/MP/CP, per-slot defence, speeds.
    pub player_templates: Arc<PlayerTemplates>,
    /// Armor sets (`FuncArmorSet`).
    pub armor_sets: Arc<ArmorSets>,
    /// The enchant HP bonus (`FuncEnchantHp`).
    pub enchant_hp_bonus: Arc<EnchantHpBonus>,
    /// Hennas (`FuncHenna`).
    pub hennas: Arc<Hennas>,
    /// `statBonus.xml`.
    pub base_stat_bonus: Arc<BaseStatBonus>,
    /// The skill templates (passive skill effects, item and armor set skills).
    pub skills: Arc<SkillTemplates>,
}

/// The skill a stat is calculated for (Java's `Skill skill` argument of `calcStat`): the parts
/// the stat getters and conditions read. The skill tables (M4-DATA-11) will widen it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalcSkill {
    pub id: i32,
    pub level: i32,
    /// Java `Skill.isMagic()`.
    pub magic: bool,
}

/// `ClassBalanceConfig` arrays the stat getters read (`Custom/ClassBalance.ini`), indexed by
/// class id. Every entry is 1.0 unless the file says otherwise (Java `Arrays.fill(.., 1f)`).
#[derive(Debug, Clone, PartialEq)]
pub struct ClassBalance {
    /// `PvpPhysicalAttackDefenceMultipliers`.
    pub pvp_physical_attack_defence: Vec<f32>,
    /// `PvePhysicalAttackDefenceMultipliers`.
    pub pve_physical_attack_defence: Vec<f32>,
    /// `PveMagicalSkillDefenceMultipliers`.
    pub pve_magical_skill_defence: Vec<f32>,
    /// `PvePhysicalSkillDefenceMultipliers`.
    pub pve_physical_skill_defence: Vec<f32>,
    /// `PvpMagicalSkillCriticalChanceMultipliers`.
    pub pvp_magical_skill_critical_chance: Vec<f32>,
    /// `PveMagicalSkillCriticalChanceMultipliers`.
    pub pve_magical_skill_critical_chance: Vec<f32>,
}

impl ClassBalance {
    /// Java's array length (`new float[137]`).
    pub const CLASSES: usize = 137;

    /// The entry of `class_id`; 1.0 outside the array.
    // NOTE(java-quirk): Java throws `ArrayIndexOutOfBoundsException` for a class id outside the
    // array; no H5 class id is.
    pub(crate) fn of(table: &[f32], class_id: i32) -> f32 {
        usize::try_from(class_id)
            .ok()
            .and_then(|c| table.get(c))
            .copied()
            .unwrap_or(1.0)
    }
}

impl Default for ClassBalance {
    fn default() -> Self {
        let ones = vec![1.0; Self::CLASSES];
        Self {
            pvp_physical_attack_defence: ones.clone(),
            pve_physical_attack_defence: ones.clone(),
            pve_magical_skill_defence: ones.clone(),
            pve_physical_skill_defence: ones.clone(),
            pvp_magical_skill_critical_chance: ones.clone(),
            pve_magical_skill_critical_chance: ones,
        }
    }
}

/// The configuration values the stat getters read. `Default` is the Java default of each key.
#[derive(Debug, Clone, PartialEq)]
pub struct StatSettings {
    /// `PlayerConfig.MAX_PATK` (`MaxPAtk`).
    pub max_patk: i32,
    /// `MaxMAtk`.
    pub max_matk: i32,
    /// `MaxPCritRate`.
    pub max_pcrit_rate: i32,
    /// `MaxMCritRate`.
    pub max_mcrit_rate: i32,
    /// `MaxEvasion`.
    pub max_evasion: i32,
    /// `MaxPAtkSpeed`.
    pub max_patk_speed: i32,
    /// `MaxMAtkSpeed`.
    pub max_matk_speed: i32,
    /// `MaxRunSpeed`.
    pub max_run_speed: i32,
    /// `RunSpeedBoost`.
    pub run_spd_boost: i32,
    /// `AltWeightLimit`.
    pub alt_weight_limit: f64,
    /// `ChampionMonstersConfig.CHAMPION_ENABLE` (`ChampionEnable`).
    pub champion_enable: bool,
    /// `ChampionAtk` (a Java `float`).
    pub champion_atk: f32,
    /// `ChampionSpdAtk` (a Java `float`).
    pub champion_spd_atk: f32,
    /// `NpcConfig.RAID_PDEFENCE_MULTIPLIER`: `RaidPDefenceMultiplier / 100`.
    pub raid_pdefence_multiplier: f64,
    /// `RaidMDefenceMultiplier / 100`.
    pub raid_mdefence_multiplier: f64,
    /// `RaidPAttackMultiplier / 100`.
    pub raid_pattack_multiplier: f64,
    /// `RaidMAttackMultiplier / 100`.
    pub raid_mattack_multiplier: f64,
    /// `OlympiadConfig.OLYMPIAD_ENCHANT_LIMIT` (`OlympiadEnchantLimit`).
    pub olympiad_enchant_limit: i32,
    /// `FeatureConfig.ALT_SIEGE_DAWN_GATES_PDEF_MULT` (`AltDawnGatesPdefMult`) and the other three.
    pub dawn_gates_pdef_mult: f64,
    pub dusk_gates_pdef_mult: f64,
    pub dawn_gates_mdef_mult: f64,
    pub dusk_gates_mdef_mult: f64,
    /// `Custom/ClassBalance.ini`.
    pub class_balance: ClassBalance,
    /// `GeneralConfig.GM_SKILL_RESTRICTION` (`GMSkillRestriction`): when off, a GM's passive
    /// skills ignore their conditions.
    pub gm_skill_restriction: bool,
    /// `PlayerConfig.EXPERTISE_PENALTY` (`ExpertisePenalty`).
    pub expertise_penalty: bool,
}

impl Default for StatSettings {
    fn default() -> Self {
        Self {
            max_patk: 999_999,
            max_matk: 999_999,
            max_pcrit_rate: 500,
            max_mcrit_rate: 200,
            max_evasion: 250,
            max_patk_speed: 1500,
            max_matk_speed: 1999,
            max_run_speed: 250,
            run_spd_boost: 0,
            alt_weight_limit: 1.0,
            champion_enable: false,
            champion_atk: 1.0,
            champion_spd_atk: 1.0,
            raid_pdefence_multiplier: 1.0,
            raid_mdefence_multiplier: 1.0,
            raid_pattack_multiplier: 1.0,
            raid_mattack_multiplier: 1.0,
            olympiad_enchant_limit: -1,
            dawn_gates_pdef_mult: 1.1,
            dusk_gates_pdef_mult: 0.8,
            dawn_gates_mdef_mult: 1.1,
            dusk_gates_mdef_mult: 0.8,
            class_balance: ClassBalance::default(),
            gm_skill_restriction: true,
            expertise_penalty: true,
        }
    }
}

/// Java `Math.round(double)`: `floor(x + 0.5)` as a `long` (NaN is 0, infinities saturate).
pub(crate) fn java_round(value: f64) -> i64 {
    if value.is_nan() {
        0
    } else {
        (value + 0.5).floor() as i64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stat_bonus_tables_are_one_until_loaded_and_zero_where_unlisted() {
        let empty = BaseStatBonus::default();
        assert_eq!(empty.bonus(BaseStat::Str, 40), 1.0);
        assert_eq!(empty.bonus(BaseStat::Men, 500), 1.0);
        let mut t = BaseStatBonus::loaded();
        assert!(t.set(BaseStat::Dex, 24, 1.04));
        assert!(!t.set(BaseStat::Dex, 100, 9.0));
        assert_eq!(t.bonus(BaseStat::Dex, 24), 1.04);
        assert_eq!(t.bonus(BaseStat::Dex, 25), 0.0);
        assert_eq!(t.bonus(BaseStat::Str, 24), 0.0);
        assert_eq!(t.bonus(BaseStat::Dex, 100), 0.0);
        assert_eq!(t.bonus(BaseStat::Dex, -1), 0.0);
        assert_eq!(BaseStat::from_section("wit"), Some(BaseStat::Wit));
        assert_eq!(BaseStat::from_section("LUC"), None);
    }

    #[test]
    fn java_round_is_floor_of_half_up() {
        assert_eq!(java_round(2.5), 3);
        assert_eq!(java_round(-2.5), -2);
        assert_eq!(java_round(43.86), 44);
        assert_eq!(java_round(f64::NAN), 0);
    }
}
