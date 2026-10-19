//! One skill at one level: the typed fields Java's `Skill` constructor reads from the `StatSet`,
//! the leftover `StatSet` keys, the effect templates and the use conditions.
//!
//! Java: `mechanics/skill/Skill.java` (the constructor and the plain getters),
//! `util/StatSet.java` (the getters and their error rules), `mechanics/skill/SkillOperateType.java`,
//! `mechanics/skill/targets/TargetType.java`, `mechanics/skill/AbnormalVisualEffect.java`,
//! `mechanics/stats/{TraitType,BaseStat}.java`, `mechanics/skill/enums/FlyType.java`.
//!
//! Not stored here: `Skill.isHeroSkill/isGMSkill/isClanSkill` (they ask `SkillTreeData`, whose hero,
//! GM and clan trees are not loaded yet) and the item pre-condition list (`DocumentSkill` never
//! fills it).

use std::collections::BTreeMap;
use std::fmt;

use crate::item::types::java_enum;
use crate::stats::MessagedCondition;
use crate::stats::enums::AbnormalType;
use crate::stats::jnum::{java_split, parse_int};

use super::SkillLoadOptions;
use super::effect::EffectLists;

java_enum! {
    /// Java `SkillOperateType`.
    SkillOperateType {
        /// `A1`: active, instant effect (damage, heal).
        A1 = "A1",
        /// `A2`: active, continuous effect (buff, debuff, over time).
        A2 = "A2",
        /// `A3`: active, instant effect then continuous effect.
        A3 = "A3",
        /// `A4`: active, special event herb.
        A4 = "A4",
        /// `CA1`: continuous active, instant effect by ticks.
        CA1 = "CA1",
        /// `CA5`: continuous active, continuous effect by ticks.
        CA5 = "CA5",
        /// `DA1`: directional active, charge/rush instant effect.
        DA1 = "DA1",
        /// `DA2`: directional active, charge/rush continuous effect.
        DA2 = "DA2",
        /// `P`: passive.
        P = "P",
        /// `T`: toggle.
        T = "T",
    }
}

impl SkillOperateType {
    /// Java `isActive()`.
    pub fn is_active(self) -> bool {
        !matches!(self, SkillOperateType::P | SkillOperateType::T)
    }

    /// Java `isContinuous()`.
    pub fn is_continuous(self) -> bool {
        matches!(
            self,
            SkillOperateType::A2 | SkillOperateType::A4 | SkillOperateType::DA2
        )
    }

    /// Java `isSelfContinuous()`.
    pub fn is_self_continuous(self) -> bool {
        self == SkillOperateType::A3
    }

    /// Java `isPassive()`.
    pub fn is_passive(self) -> bool {
        self == SkillOperateType::P
    }

    /// Java `isToggle()`.
    pub fn is_toggle(self) -> bool {
        self == SkillOperateType::T
    }

    /// Java `isChanneling()`.
    pub fn is_channeling(self) -> bool {
        matches!(self, SkillOperateType::CA1 | SkillOperateType::CA5)
    }
}

java_enum! {
    /// Java `TargetType`.
    TargetType {
        /// `AREA`
        Area = "AREA",
        /// `AREA_CORPSE_MOB`
        AreaCorpseMob = "AREA_CORPSE_MOB",
        /// `AREA_FRIENDLY`
        AreaFriendly = "AREA_FRIENDLY",
        /// `AREA_SUMMON`
        AreaSummon = "AREA_SUMMON",
        /// `AREA_UNDEAD`
        AreaUndead = "AREA_UNDEAD",
        /// `AURA`
        Aura = "AURA",
        /// `AURA_CORPSE_MOB`
        AuraCorpseMob = "AURA_CORPSE_MOB",
        /// `AURA_FRIENDLY`
        AuraFriendly = "AURA_FRIENDLY",
        /// `BEHIND_AREA`
        BehindArea = "BEHIND_AREA",
        /// `BEHIND_AURA`
        BehindAura = "BEHIND_AURA",
        /// `CLAN`
        Clan = "CLAN",
        /// `CLAN_MEMBER`
        ClanMember = "CLAN_MEMBER",
        /// `COMMAND_CHANNEL`
        CommandChannel = "COMMAND_CHANNEL",
        /// `CORPSE`
        Corpse = "CORPSE",
        /// `CORPSE_CLAN`
        CorpseClan = "CORPSE_CLAN",
        /// `CORPSE_MOB`
        CorpseMob = "CORPSE_MOB",
        /// `ENEMY_SUMMON`
        EnemySummon = "ENEMY_SUMMON",
        /// `FLAGPOLE`
        Flagpole = "FLAGPOLE",
        /// `FRONT_AREA`
        FrontArea = "FRONT_AREA",
        /// `FRONT_AURA`
        FrontAura = "FRONT_AURA",
        /// `GROUND`
        Ground = "GROUND",
        /// `HOLY`
        Holy = "HOLY",
        /// `NONE`
        None = "NONE",
        /// `ONE`
        One = "ONE",
        /// `OWNER_PET`
        OwnerPet = "OWNER_PET",
        /// `PARTY`
        Party = "PARTY",
        /// `PARTY_CLAN`
        PartyClan = "PARTY_CLAN",
        /// `PARTY_MEMBER`
        PartyMember = "PARTY_MEMBER",
        /// `PARTY_NOTME`
        PartyNotMe = "PARTY_NOTME",
        /// `PARTY_OTHER`
        PartyOther = "PARTY_OTHER",
        /// `PC_BODY`
        PcBody = "PC_BODY",
        /// `PET`
        Pet = "PET",
        /// `SELF`
        SelfTarget = "SELF",
        /// `SERVITOR`
        Servitor = "SERVITOR",
        /// `SUMMON`
        Summon = "SUMMON",
        /// `TARGET_PARTY`
        TargetParty = "TARGET_PARTY",
        /// `UNDEAD`
        Undead = "UNDEAD",
        /// `UNLOCKABLE`
        Unlockable = "UNLOCKABLE",
    }
}

impl TargetType {
    /// Java `Skill.isAOE()`.
    pub fn is_aoe(self) -> bool {
        matches!(
            self,
            TargetType::Area
                | TargetType::Aura
                | TargetType::BehindArea
                | TargetType::BehindAura
                | TargetType::FrontArea
                | TargetType::FrontAura
        )
    }
}

java_enum! {
    /// Java `TraitType`.
    TraitType {
        /// `NONE`
        None = "NONE",
        /// `SWORD`
        Sword = "SWORD",
        /// `BLUNT`
        Blunt = "BLUNT",
        /// `DAGGER`
        Dagger = "DAGGER",
        /// `POLE`
        Pole = "POLE",
        /// `FIST`
        Fist = "FIST",
        /// `BOW`
        Bow = "BOW",
        /// `ETC`
        Etc = "ETC",
        /// `UNK_8`
        Unk8 = "UNK_8",
        /// `POISON`
        Poison = "POISON",
        /// `HOLD`
        Hold = "HOLD",
        /// `BLEED`
        Bleed = "BLEED",
        /// `SLEEP`
        Sleep = "SLEEP",
        /// `SHOCK`
        Shock = "SHOCK",
        /// `DERANGEMENT`
        Derangement = "DERANGEMENT",
        /// `BUG_WEAKNESS`
        BugWeakness = "BUG_WEAKNESS",
        /// `ANIMAL_WEAKNESS`
        AnimalWeakness = "ANIMAL_WEAKNESS",
        /// `PLANT_WEAKNESS`
        PlantWeakness = "PLANT_WEAKNESS",
        /// `BEAST_WEAKNESS`
        BeastWeakness = "BEAST_WEAKNESS",
        /// `DRAGON_WEAKNESS`
        DragonWeakness = "DRAGON_WEAKNESS",
        /// `PARALYZE`
        Paralyze = "PARALYZE",
        /// `DUAL`
        Dual = "DUAL",
        /// `DUALFIST`
        DualFist = "DUALFIST",
        /// `BOSS`
        Boss = "BOSS",
        /// `GIANT_WEAKNESS`
        GiantWeakness = "GIANT_WEAKNESS",
        /// `CONSTRUCT_WEAKNESS`
        ConstructWeakness = "CONSTRUCT_WEAKNESS",
        /// `DEATH`
        Death = "DEATH",
        /// `VALAKAS`
        Valakas = "VALAKAS",
        /// `ANESTHESIA`
        Anesthesia = "ANESTHESIA",
        /// `CRITICAL_POISON`
        CriticalPoison = "CRITICAL_POISON",
        /// `ROOT_PHYSICALLY`
        RootPhysically = "ROOT_PHYSICALLY",
        /// `ROOT_MAGICALLY`
        RootMagically = "ROOT_MAGICALLY",
        /// `RAPIER`
        Rapier = "RAPIER",
        /// `CROSSBOW`
        Crossbow = "CROSSBOW",
        /// `ANCIENTSWORD`
        AncientSword = "ANCIENTSWORD",
        /// `TURN_STONE`
        TurnStone = "TURN_STONE",
        /// `GUST`
        Gust = "GUST",
        /// `PHYSICAL_BLOCKADE`
        PhysicalBlockade = "PHYSICAL_BLOCKADE",
        /// `TARGET`
        Target = "TARGET",
        /// `PHYSICAL_WEAKNESS`
        PhysicalWeakness = "PHYSICAL_WEAKNESS",
        /// `MAGICAL_WEAKNESS`
        MagicalWeakness = "MAGICAL_WEAKNESS",
        /// `DUALDAGGER`
        DualDagger = "DUALDAGGER",
    }
}

impl TraitType {
    /// Java `getType()`: 0 none, 1 weapon, 2 weakness, 3 resistance.
    pub fn kind(self) -> i32 {
        use TraitType::*;
        match self {
            None | Unk8 => 0,
            Sword | Blunt | Dagger | Pole | Fist | Bow | Etc | Dual | DualFist | Rapier | Crossbow
            | AncientSword | DualDagger => 1,
            BugWeakness | AnimalWeakness | PlantWeakness | BeastWeakness | DragonWeakness | GiantWeakness
            | ConstructWeakness | Valakas | Anesthesia => 2,
            Poison | Hold | Bleed | Sleep | Shock | Derangement | Paralyze | Boss | Death
            | CriticalPoison | RootPhysically | RootMagically | TurnStone | Gust | PhysicalBlockade
            | Target | PhysicalWeakness | MagicalWeakness => 3,
        }
    }
}

java_enum! {
    /// Java `BaseStat` (the constant only; the bonus tables belong to the stat engine).
    BaseStat {
        /// `STR`
        Str = "STR",
        /// `INT`
        Int = "INT",
        /// `DEX`
        Dex = "DEX",
        /// `WIT`
        Wit = "WIT",
        /// `CON`
        Con = "CON",
        /// `MEN`
        Men = "MEN",
        /// `NONE`
        None = "NONE",
    }
}

java_enum! {
    /// Java `FlyType`.
    FlyType {
        /// `THROW_UP`
        ThrowUp = "THROW_UP",
        /// `THROW_HORIZONTAL`
        ThrowHorizontal = "THROW_HORIZONTAL",
        /// `DUMMY`
        Dummy = "DUMMY",
        /// `CHARGE`
        Charge = "CHARGE",
    }
}

java_enum! {
    /// Java `EffectScope`: which list of a skill an effect belongs to.
    EffectScope {
        /// `GENERAL`: the plain `<effects>` of an active skill.
        General = "GENERAL",
        /// `START`: `<startEffects>`.
        Start = "START",
        /// `SELF`: `<selfEffects>`.
        SelfScope = "SELF",
        /// `PASSIVE`: the plain `<effects>` of a passive skill.
        Passive = "PASSIVE",
        /// `CHANNELING`: `<channelingEffects>`.
        Channeling = "CHANNELING",
        /// `PVP`: `<pvpEffects>`.
        Pvp = "PVP",
        /// `PVE`: `<pveEffects>`.
        Pve = "PVE",
        /// `END`: `<endEffects>`.
        End = "END",
    }
}

java_enum! {
    /// Java `AbnormalVisualEffect`.
    AbnormalVisualEffect {
        /// `NONE`
        None = "NONE",
        /// `DOT_BLEEDING`
        DotBleeding = "DOT_BLEEDING",
        /// `DOT_POISON`
        DotPoison = "DOT_POISON",
        /// `DOT_FIRE`
        DotFire = "DOT_FIRE",
        /// `DOT_WATER`
        DotWater = "DOT_WATER",
        /// `DOT_WIND`
        DotWind = "DOT_WIND",
        /// `DOT_SOIL`
        DotSoil = "DOT_SOIL",
        /// `STUN`
        Stun = "STUN",
        /// `SLEEP`
        Sleep = "SLEEP",
        /// `SILENCE`
        Silence = "SILENCE",
        /// `ROOT`
        Root = "ROOT",
        /// `PARALYZE`
        Paralyze = "PARALYZE",
        /// `FLESH_STONE`
        FleshStone = "FLESH_STONE",
        /// `DOT_MP`
        DotMp = "DOT_MP",
        /// `BIG_HEAD`
        BigHead = "BIG_HEAD",
        /// `DOT_FIRE_AREA`
        DotFireArea = "DOT_FIRE_AREA",
        /// `CHANGE_TEXTURE`
        ChangeTexture = "CHANGE_TEXTURE",
        /// `BIG_BODY`
        BigBody = "BIG_BODY",
        /// `FLOATING_ROOT`
        FloatingRoot = "FLOATING_ROOT",
        /// `DANCE_ROOT`
        DanceRoot = "DANCE_ROOT",
        /// `GHOST_STUN`
        GhostStun = "GHOST_STUN",
        /// `STEALTH`
        Stealth = "STEALTH",
        /// `SEIZURE1`
        Seizure1 = "SEIZURE1",
        /// `SEIZURE2`
        Seizure2 = "SEIZURE2",
        /// `MAGIC_SQUARE`
        MagicSquare = "MAGIC_SQUARE",
        /// `FREEZING`
        Freezing = "FREEZING",
        /// `SHAKE`
        Shake = "SHAKE",
        /// `BLIND`
        Blind = "BLIND",
        /// `ULTIMATE_DEFENCE`
        UltimateDefence = "ULTIMATE_DEFENCE",
        /// `VP_UP`
        VpUp = "VP_UP",
        /// `REAL_TARGET`
        RealTarget = "REAL_TARGET",
        /// `DEATH_MARK`
        DeathMark = "DEATH_MARK",
        /// `TURN_FLEE`
        TurnFlee = "TURN_FLEE",
        /// `VP_KEEP`
        VpKeep = "VP_KEEP",
        /// `INVINCIBILITY` (special)
        Invincibility = "INVINCIBILITY",
        /// `AIR_BATTLE_SLOW` (special)
        AirBattleSlow = "AIR_BATTLE_SLOW",
        /// `AIR_BATTLE_ROOT` (special)
        AirBattleRoot = "AIR_BATTLE_ROOT",
        /// `CHANGE_WP` (special)
        ChangeWp = "CHANGE_WP",
        /// `CHANGE_HAIR_G` (special)
        ChangeHairG = "CHANGE_HAIR_G",
        /// `CHANGE_HAIR_P` (special)
        ChangeHairP = "CHANGE_HAIR_P",
        /// `CHANGE_HAIR_B` (special)
        ChangeHairB = "CHANGE_HAIR_B",
        /// `STIGMA_OF_SILEN` (special)
        StigmaOfSilen = "STIGMA_OF_SILEN",
        /// `SPEED_DOWN` (special)
        SpeedDown = "SPEED_DOWN",
        /// `FROZEN_PILLAR` (special)
        FrozenPillar = "FROZEN_PILLAR",
        /// `CHANGE_VES_S` (special)
        ChangeVesS = "CHANGE_VES_S",
        /// `CHANGE_VES_C` (special)
        ChangeVesC = "CHANGE_VES_C",
        /// `CHANGE_VES_D` (special)
        ChangeVesD = "CHANGE_VES_D",
        /// `TIME_BOMB` (special)
        TimeBomb = "TIME_BOMB",
        /// `MP_SHIELD` (special)
        MpShield = "MP_SHIELD",
        /// `NAVIT_ADVENT` (special)
        NavitAdvent = "NAVIT_ADVENT",
        /// `BR_NONE` (event)
        BrNone = "BR_NONE",
        /// `BR_AFRO_NORMAL` (event)
        BrAfroNormal = "BR_AFRO_NORMAL",
        /// `BR_AFRO_PINK` (event)
        BrAfroPink = "BR_AFRO_PINK",
        /// `BR_AFRO_GOLD` (event)
        BrAfroGold = "BR_AFRO_GOLD",
        /// `BR_POWER_OF_EVA` (event)
        BrPowerOfEva = "BR_POWER_OF_EVA",
        /// `BR_HEADPHONE` (event)
        BrHeadphone = "BR_HEADPHONE",
        /// `BR_VESPER1` (event)
        BrVesper1 = "BR_VESPER1",
        /// `BR_VESPER2` (event)
        BrVesper2 = "BR_VESPER2",
        /// `BR_VESPER3` (event)
        BrVesper3 = "BR_VESPER3",
        /// `BR_SOUL_AVATAR` (event)
        BrSoulAvatar = "BR_SOUL_AVATAR",
    }
}

/// `(mask, type)` per [`AbnormalVisualEffect`] in declaration order; type 0 normal, 1 special,
/// 2 event. `VP_KEEP` shares the mask of `VP_UP` in Java (marked "TODO: Find").
const AVE_TABLE: [(u32, u8); 60] = [
    (0x0000_0000, 0),
    (0x0000_0001, 0),
    (0x0000_0002, 0),
    (0x0000_0004, 0),
    (0x0000_0008, 0),
    (0x0000_0010, 0),
    (0x0000_0020, 0),
    (0x0000_0040, 0),
    (0x0000_0080, 0),
    (0x0000_0100, 0),
    (0x0000_0200, 0),
    (0x0000_0400, 0),
    (0x0000_0800, 0),
    (0x0000_1000, 0),
    (0x0000_2000, 0),
    (0x0000_4000, 0),
    (0x0000_8000, 0),
    (0x0001_0000, 0),
    (0x0002_0000, 0),
    (0x0004_0000, 0),
    (0x0008_0000, 0),
    (0x0010_0000, 0),
    (0x0020_0000, 0),
    (0x0040_0000, 0),
    (0x0080_0000, 0),
    (0x0100_0000, 0),
    (0x0200_0000, 0),
    (0x0400_0000, 0),
    (0x0800_0000, 0),
    (0x1000_0000, 0),
    (0x2000_0000, 0),
    (0x4000_0000, 0),
    (0x8000_0000, 0),
    (0x1000_0000, 0),
    (0x0000_0001, 1),
    (0x0000_0002, 1),
    (0x0000_0004, 1),
    (0x0000_0008, 1),
    (0x0000_0010, 1),
    (0x0000_0020, 1),
    (0x0000_0040, 1),
    (0x0000_0100, 1),
    (0x0000_0200, 1),
    (0x0000_0400, 1),
    (0x0000_0800, 1),
    (0x0000_1000, 1),
    (0x0000_2000, 1),
    (0x0000_4000, 1),
    (0x0000_8000, 1),
    (0x0008_0000, 1),
    (0x0000_0000, 2),
    (0x0000_0001, 2),
    (0x0000_0002, 2),
    (0x0000_0004, 2),
    (0x0000_0008, 2),
    (0x0000_0010, 2),
    (0x0000_0020, 2),
    (0x0000_0040, 2),
    (0x0000_0080, 2),
    (0x0000_0100, 2),
];

impl AbnormalVisualEffect {
    /// Java `getMask()`. (The Java constant is an `int`; the top bit of `TURN_FLEE` is the sign.)
    pub fn mask(self) -> i32 {
        Self::table_entry(self).0 as i32
    }

    /// Java `isSpecial()`.
    pub fn is_special(self) -> bool {
        Self::table_entry(self).1 == 1
    }

    /// Java `isEvent()`.
    pub fn is_event(self) -> bool {
        Self::table_entry(self).1 == 2
    }

    fn table_entry(self) -> (u32, u8) {
        // A test pins `AVE_TABLE` to the variant count; the fallback is never used.
        AVE_TABLE.get(self as usize).copied().unwrap_or((0, 0))
    }
}

/// What Java's `StatSet` stores for one key (the `Object`).
#[derive(Debug, Clone, PartialEq)]
pub enum SetValue {
    /// A `String` (the XML text, or a resolved table entry).
    Str(String),
    /// A `Boolean` (`StringUtil.parseValue` of `true`/`false`).
    Bool(bool),
    /// A `Long` (`StringUtil.parseValue` of an integer).
    Long(i64),
    /// An `Integer` (`skill_id`, `level`).
    Int(i32),
    /// A `Float` (`StringUtil.parseValue` of a decimal number).
    Float(f32),
}

impl SetValue {
    /// Java `StringUtil.parseValue`: `Boolean`, else `Long`, else `Float`, else the trimmed text.
    /// (Java's `Integer` and `Double` steps are unreachable: a `Long` parse covers every integer
    /// and a `Float` parse never fails where a `Double` parse would succeed.)
    pub fn parse(text: &str) -> SetValue {
        let value = text.trim_matches(|c: char| c <= ' ');
        if value.eq_ignore_ascii_case("true") {
            SetValue::Bool(true)
        } else if value.eq_ignore_ascii_case("false") {
            SetValue::Bool(false)
        } else if let Some(long) = parse_long(value) {
            SetValue::Long(long)
        } else if let Some(float) = parse_float(value) {
            SetValue::Float(float)
        } else {
            SetValue::Str(value.to_owned())
        }
    }

    /// Java `String.valueOf(value)`.
    pub fn as_string(&self) -> String {
        match self {
            SetValue::Str(text) => text.clone(),
            SetValue::Bool(value) => value.to_string(),
            SetValue::Long(value) => value.to_string(),
            SetValue::Int(value) => value.to_string(),
            SetValue::Float(value) => {
                if value.fract() == 0.0 && value.abs() < 1e7 {
                    format!("{value:.1}")
                } else {
                    value.to_string()
                }
            }
        }
    }
}

/// Java `Long.valueOf(String)`: optional sign and decimal digits.
fn parse_long(text: &str) -> Option<i64> {
    if text.is_empty()
        || !text
            .bytes()
            .enumerate()
            .all(|(i, b)| b.is_ascii_digit() || (i == 0 && (b == b'-' || b == b'+')))
    {
        return None;
    }
    text.parse().ok()
}

/// Java `Float.parseFloat` for the decimal forms the datapack can contain (see
/// `stats::jnum::parse_double`).
pub(super) fn parse_float(text: &str) -> Option<f32> {
    let trimmed = text.trim_matches(|c: char| c <= ' ');
    if trimmed.is_empty() {
        return None;
    }
    match trimmed {
        "NaN" | "+NaN" | "-NaN" => return Some(f32::NAN),
        "Infinity" | "+Infinity" => return Some(f32::INFINITY),
        "-Infinity" => return Some(f32::NEG_INFINITY),
        _ => {}
    }
    let number = trimmed.strip_suffix(['d', 'D', 'f', 'F']).unwrap_or(trimmed);
    if !number
        .chars()
        .all(|c| c.is_ascii_digit() || matches!(c, '.' | 'e' | 'E' | '+' | '-'))
    {
        return None;
    }
    number.parse().ok()
}

/// A `StatSet` getter that Java answers with an `IllegalArgumentException`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct SetError(pub String);

/// Java `StatSet`, the part the skill documents use: string keys, typed values, Java's getter
/// rules (a number converts like `Number.xxxValue()`, text is parsed when it is read).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StatSet {
    values: BTreeMap<String, SetValue>,
}

impl StatSet {
    /// Java `set(name, value)`.
    pub fn set(&mut self, name: &str, value: SetValue) {
        self.values.insert(name.to_owned(), value);
    }

    /// Java `set(String, String)`.
    pub fn set_str(&mut self, name: &str, value: &str) {
        self.set(name, SetValue::Str(value.to_owned()));
    }

    /// Java `set(String, int)`.
    pub fn set_int(&mut self, name: &str, value: i32) {
        self.set(name, SetValue::Int(value));
    }

    /// The stored value, if any.
    pub fn get(&self, name: &str) -> Option<&SetValue> {
        self.values.get(name)
    }

    /// Every key in name order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &SetValue)> {
        self.values.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Java `getInt(key)` (required).
    pub fn int(&self, key: &str) -> Result<i32, SetError> {
        match self.values.get(key) {
            None => Err(SetError(format!(
                "Integer value required, but not specified: {key}!"
            ))),
            Some(_) => self.int_or(key, 0),
        }
    }

    /// Java `getInt(key, default)`.
    pub fn int_or(&self, key: &str, default: i32) -> Result<i32, SetError> {
        match self.values.get(key) {
            None => Ok(default),
            Some(SetValue::Int(v)) => Ok(*v),
            // `Number.intValue()`: truncation, saturating for floats.
            Some(SetValue::Long(v)) => Ok(*v as i32),
            Some(SetValue::Float(v)) => Ok(*v as i32),
            Some(SetValue::Str(text)) => {
                parse_int(text).ok_or_else(|| SetError(format!("Integer value required, but found: {text}")))
            }
            Some(SetValue::Bool(v)) => Err(SetError(format!("Integer value required, but found: {v}"))),
        }
    }

    /// Java `getByte(key, default)`.
    pub fn byte_or(&self, key: &str, default: i8) -> Result<i8, SetError> {
        match self.values.get(key) {
            None => Ok(default),
            Some(SetValue::Int(v)) => Ok(*v as i8),
            Some(SetValue::Long(v)) => Ok(*v as i8),
            Some(SetValue::Float(v)) => Ok(*v as i32 as i8),
            Some(SetValue::Str(text)) => text
                .parse()
                .map_err(|_| SetError(format!("Byte value required, but found: {text}"))),
            Some(SetValue::Bool(v)) => Err(SetError(format!("Byte value required, but found: {v}"))),
        }
    }

    /// Java `getFloat(key, default)`.
    pub fn float_or(&self, key: &str, default: f32) -> Result<f32, SetError> {
        match self.values.get(key) {
            None => Ok(default),
            Some(SetValue::Int(v)) => Ok(*v as f32),
            Some(SetValue::Long(v)) => Ok(*v as f32),
            Some(SetValue::Float(v)) => Ok(*v),
            Some(SetValue::Str(text)) => {
                parse_float(text).ok_or_else(|| SetError(format!("Float value required, but found: {text}")))
            }
            Some(SetValue::Bool(v)) => Err(SetError(format!("Float value required, but found: {v}"))),
        }
    }

    /// Java `getBoolean(key, default)`: anything but a `Boolean` or a `String` is the default.
    pub fn bool_or(&self, key: &str, default: bool) -> bool {
        match self.values.get(key) {
            Some(SetValue::Bool(v)) => *v,
            Some(SetValue::Str(text)) => text.eq_ignore_ascii_case("true"),
            _ => default,
        }
    }

    /// Java `getString(key)` (required).
    pub fn string(&self, key: &str) -> Result<String, SetError> {
        self.values
            .get(key)
            .map(SetValue::as_string)
            .ok_or_else(|| SetError("String value required, but not specified".to_owned()))
    }

    /// Java `getString(key, default)`.
    pub fn string_or(&self, key: &str, default: &str) -> String {
        self.values
            .get(key)
            .map_or_else(|| default.to_owned(), SetValue::as_string)
    }

    /// Java `getEnum(key, class)` (required): `Enum.valueOf` of the stored text.
    pub fn enum_value<T>(
        &self,
        key: &str,
        what: &str,
        from_name: fn(&str) -> Option<T>,
    ) -> Result<T, SetError> {
        match self.values.get(key) {
            None => Err(SetError(format!(
                "Enum value of type {what} required, but not specified"
            ))),
            Some(value) => {
                let text = value.as_string();
                from_name(&text)
                    .ok_or_else(|| SetError(format!("Enum value of type {what} required, but found: {text}")))
            }
        }
    }

    /// Java `getEnum(key, class, default)`.
    pub fn enum_or<T>(
        &self,
        key: &str,
        what: &str,
        default: T,
        from_name: fn(&str) -> Option<T>,
    ) -> Result<T, SetError> {
        if self.values.contains_key(key) {
            self.enum_value(key, what, from_name)
        } else {
            Ok(default)
        }
    }
}

/// The `StatSet` keys the constructor reads; everything else stays in [`SkillTemplate::set`].
const CONSUMED_KEYS: &[&str] = &[
    "skill_id",
    "level",
    "name",
    "displayId",
    "displayLevel",
    "operateType",
    "isMagic",
    "trait",
    "staticReuse",
    "mpConsume",
    "mpInitialConsume",
    "mpPerChanneling",
    "hpConsume",
    "itemConsumeCount",
    "itemConsumeId",
    "castRange",
    "effectRange",
    "abnormalLevel",
    "abnormalType",
    "abnormalTime",
    "abnormalInstant",
    "abnormalVisualEffect",
    "attribute",
    "stayAfterDeath",
    "stayOnSubclassChange",
    "hitTime",
    "coolTime",
    "isDebuff",
    "isRecoveryHerb",
    "feed",
    "reuseDelay",
    "affectRange",
    "affectLimit",
    "targetType",
    "power",
    "pvpPower",
    "pvePower",
    "magicLevel",
    "lvlBonusRate",
    "activateRate",
    "minChance",
    "maxChance",
    "ignoreShld",
    "nextActionAttack",
    "removedOnAnyActionExceptMove",
    "removedOnDamage",
    "blockedInOlympiad",
    "element",
    "elementPower",
    "basicProperty",
    "overHit",
    "isSuicideAttack",
    "minPledgeClass",
    "chargeConsume",
    "soulMaxConsumeCount",
    "blowChance",
    "baseCritRate",
    "dmgDirectlyToHp",
    "isTriggeredSkill",
    "effectPoint",
    "flyType",
    "flyRadius",
    "flyCourse",
    "canBeDispeled",
    "excludedFromCheck",
    "simultaneousCast",
    "icon",
    "channelingSkillId",
    "channelingTickInterval",
    "channelingTickInitialDelay",
    "isPvPOnly",
];

/// Java `Skill`: one skill at one level. Times are milliseconds, as in the XML.
///
/// Levels 1..=`levels` are the plain levels, 101.. are enchant route 1, 201.. route 2 and so on
/// (`DocumentSkill` numbers them `route * 100 + n`).
#[derive(Debug, Clone, PartialEq)]
pub struct SkillTemplate {
    /// `skill_id`.
    pub id: i32,
    /// `level`.
    pub level: i32,
    /// `displayId` (default: the id), the id the client shows.
    pub display_id: i32,
    /// `displayLevel` (default: the level).
    pub display_level: i32,
    /// `name`.
    pub name: String,
    /// `operateType` (required).
    pub operate_type: SkillOperateType,
    /// `isMagic` raw: 0 physical, 1 magic, 2 static, 3 dance.
    pub magic: i32,
    /// `trait` (default `NONE`).
    pub trait_type: TraitType,
    /// `staticReuse`.
    pub static_reuse: bool,
    /// `mpConsume`.
    pub mp_consume: i32,
    /// `mpInitialConsume`.
    pub mp_initial_consume: i32,
    /// `mpPerChanneling` (default: `mpConsume`).
    pub mp_per_channeling: i32,
    /// `hpConsume`.
    pub hp_consume: i32,
    /// `itemConsumeCount`.
    pub item_consume_count: i32,
    /// `itemConsumeId`.
    pub item_consume_id: i32,
    /// `castRange` (default -1).
    pub cast_range: i32,
    /// `effectRange` (default -1).
    pub effect_range: i32,
    /// `abnormalInstant` (herbs).
    pub abnormal_instant: bool,
    /// `abnormalLevel`.
    pub abnormal_level: i32,
    /// `abnormalType` (default `NONE`).
    pub abnormal_type: AbnormalType,
    /// `abnormalTime` in seconds, after the `SkillDurationList` option.
    pub abnormal_time: i32,
    /// The normal entries of `abnormalVisualEffect` (`a;b;c`).
    pub abnormal_visual_effects: Vec<AbnormalVisualEffect>,
    /// The special entries of `abnormalVisualEffect`.
    pub abnormal_visual_effects_special: Vec<AbnormalVisualEffect>,
    /// The event entries of `abnormalVisualEffect`.
    pub abnormal_visual_effects_event: Vec<AbnormalVisualEffect>,
    /// `stayAfterDeath`.
    pub stay_after_death: bool,
    /// `stayOnSubclassChange` (default true).
    pub stay_on_subclass_change: bool,
    /// `isRecoveryHerb`.
    pub is_recovery_herb: bool,
    /// `hitTime`.
    pub hit_time: i32,
    /// `coolTime`.
    pub cool_time: i32,
    /// Java `_reuseHashCode`: `id * 1021 + level`.
    pub reuse_hash_code: i32,
    /// `reuseDelay`, or the `SkillReuseList` option.
    pub reuse_delay: i32,
    /// `targetType` (default `SELF`).
    pub target_type: TargetType,
    /// `feed`.
    pub feed: i32,
    /// `power` (a Java `float` widened to `double`).
    pub power: f32,
    /// `pvpPower` (default: `power`).
    pub pvp_power: f32,
    /// `pvePower` (default: `power`).
    pub pve_power: f32,
    /// `magicLevel`.
    pub magic_level: i32,
    /// `lvlBonusRate`.
    pub lvl_bonus_rate: i32,
    /// `activateRate` (default -1).
    pub activate_rate: i32,
    /// `minChance` (default: `MinAbnormalStateSuccessRate`).
    pub min_chance: i32,
    /// `maxChance` (default: `MaxAbnormalStateSuccessRate`).
    pub max_chance: i32,
    /// `blowChance`.
    pub blow_chance: i32,
    /// `affectRange`.
    pub affect_range: i32,
    /// `affectLimit` (`min-max`, both 0 when absent).
    pub affect_limit: [i32; 2],
    /// `nextActionAttack`.
    pub next_action_is_attack: bool,
    /// `removedOnAnyActionExceptMove`.
    pub removed_on_any_action_except_move: bool,
    /// `removedOnDamage`.
    pub removed_on_damage: bool,
    /// `blockedInOlympiad`.
    pub blocked_in_olympiad: bool,
    /// `element` (default -1).
    pub element: i8,
    /// `elementPower`.
    pub element_power: i32,
    /// `basicProperty` (default `NONE`).
    pub basic_property: BaseStat,
    /// `overHit`.
    pub overhit: bool,
    /// `minPledgeClass`.
    pub min_pledge_class: i32,
    /// `chargeConsume`.
    pub charge_consume: i32,
    /// `soulMaxConsumeCount`.
    pub soul_max_consume: i32,
    /// Java `_isSevenSigns`: ids 4361..=4366.
    pub is_seven_signs: bool,
    /// `baseCritRate`.
    pub base_crit_rate: i32,
    /// `dmgDirectlyToHp`.
    pub direct_hp_dmg: bool,
    /// `isTriggeredSkill`.
    pub is_triggered_skill: bool,
    /// `effectPoint`.
    pub effect_point: i32,
    /// `flyType`, absent for most skills.
    pub fly_type: Option<FlyType>,
    /// `flyRadius`.
    pub fly_radius: i32,
    /// `flyCourse`.
    pub fly_course: f32,
    /// `isDebuff`.
    pub is_debuff: bool,
    /// `attribute` (default empty).
    pub attribute: String,
    /// `ignoreShld`.
    pub ignore_shield: bool,
    /// `isSuicideAttack`.
    pub is_suicide_attack: bool,
    /// `canBeDispeled` (default true).
    pub can_be_dispeled: bool,
    /// `excludedFromCheck`.
    pub excluded_from_check: bool,
    /// `simultaneousCast`.
    pub simultaneous_cast: bool,
    /// `icon` (default `icon.skill0000`).
    pub icon: String,
    /// `channelingSkillId`.
    pub channeling_skill_id: i32,
    /// `channelingTickInterval` in milliseconds (the XML value is seconds, default 2).
    pub channeling_tick_interval: i32,
    /// `channelingTickInitialDelay` in milliseconds (default: the interval in seconds).
    pub channeling_tick_initial_delay: i32,
    /// `isPvPOnly`.
    pub is_pvp_only: bool,
    /// Every `StatSet` key the fields above do not read (`afterEffectId`, `fanRange`, ...), as
    /// Java kept them for the effect handlers.
    pub set: BTreeMap<String, SetValue>,
    /// The effect templates per [`EffectScope`].
    pub effects: EffectLists,
    /// Java `_preCondition`: the `<conditions>` blocks, in file order. A block that holds no
    /// recognised condition is dropped (Java would keep a `null` that fails when the skill is
    /// used).
    pub pre_conditions: Vec<MessagedCondition>,
}

impl SkillTemplate {
    /// Java `Skill.getId()`/`getLevel()` as the key `SkillData.getSkillHashCode` uses.
    pub fn hash_code(&self) -> i32 {
        self.reuse_hash_code
    }

    /// Java `isPassive()`.
    pub fn is_passive(&self) -> bool {
        self.operate_type.is_passive()
    }

    /// Java `isActive()`.
    pub fn is_active(&self) -> bool {
        self.operate_type.is_active()
    }

    /// Java `isToggle()`.
    pub fn is_toggle(&self) -> bool {
        self.operate_type.is_toggle()
    }

    /// Java `isPhysical()`.
    pub fn is_physical(&self) -> bool {
        self.magic == 0
    }

    /// Java `isMagic()`.
    pub fn is_magic(&self) -> bool {
        self.magic == 1
    }

    /// Java `isStatic()`.
    pub fn is_static(&self) -> bool {
        self.magic == 2
    }

    /// Java `isDance()`.
    pub fn is_dance(&self) -> bool {
        self.magic == 3
    }

    /// Java `isAOE()`.
    pub fn is_aoe(&self) -> bool {
        self.target_type.is_aoe()
    }

    /// Java `new Skill(StatSet)`: reads every field with Java's defaults. An `Err` is what the
    /// constructor throws (missing `operateType`, a bad number, an unknown enum constant, a bad
    /// `affectLimit`); `DocumentSkill.makeSkills` logs it.
    pub fn from_set(set: &StatSet, options: &SkillLoadOptions) -> Result<SkillTemplate, SetError> {
        let id = set.int("skill_id")?;
        let level = set.int("level")?;
        let operate_type = set.enum_value("operateType", "SkillOperateType", SkillOperateType::from_name)?;
        let mp_consume = set.int_or("mpConsume", 0)?;

        let mut abnormal_time = set.int_or("abnormalTime", 0)?;
        if options.enable_modify_skill_duration
            && operate_type != SkillOperateType::T
            && let Some(&duration) = options.skill_duration_list.get(&id)
        {
            if !(100..=140).contains(&level) {
                abnormal_time = duration;
            } else if level < 140 {
                abnormal_time += duration;
            }
        }

        let reuse_delay = match options.skill_reuse_list.get(&id) {
            Some(&delay) if options.enable_modify_skill_reuse => delay,
            _ => set.int_or("reuseDelay", 0)?,
        };

        let (mut normal, mut special, mut event) = (Vec::new(), Vec::new(), Vec::new());
        if let Some(value) = set.get("abnormalVisualEffect") {
            for part in java_split(&value.as_string(), ';') {
                let effect = AbnormalVisualEffect::from_name(part)
                    .ok_or_else(|| SetError(format!("No enum constant AbnormalVisualEffect.{part}")))?;
                if effect.is_event() {
                    event.push(effect);
                } else if effect.is_special() {
                    special.push(effect);
                } else {
                    normal.push(effect);
                }
            }
        }

        let mut affect_limit = [0, 0];
        if let Some(value) = set.get("affectLimit") {
            let text = value.as_string();
            let parts = java_split(&text, '-');
            let parsed = (|| Some([parse_int(parts.first()?)?, parse_int(parts.get(1)?)?]))();
            affect_limit = parsed.ok_or_else(|| {
                SetError(format!(
                    "SkillId: {id} invalid affectLimit value: {text}, \"percent-percent\" required"
                ))
            })?;
        }

        let power = set.float_or("power", 0.0)?;
        let channeling_tick_interval = set.int_or("channelingTickInterval", 2)?.wrapping_mul(1000);
        let consumed: std::collections::HashSet<&str> = CONSUMED_KEYS.iter().copied().collect();
        let rest = set
            .iter()
            .filter(|(key, _)| !consumed.contains(key))
            .map(|(key, value)| (key.to_owned(), value.clone()))
            .collect();

        Ok(SkillTemplate {
            id,
            level,
            display_id: set.int_or("displayId", id)?,
            display_level: set.int_or("displayLevel", level)?,
            name: set.string_or("name", ""),
            operate_type,
            magic: set.int_or("isMagic", 0)?,
            trait_type: set.enum_or("trait", "TraitType", TraitType::None, TraitType::from_name)?,
            static_reuse: set.bool_or("staticReuse", false),
            mp_consume,
            mp_initial_consume: set.int_or("mpInitialConsume", 0)?,
            mp_per_channeling: set.int_or("mpPerChanneling", mp_consume)?,
            hp_consume: set.int_or("hpConsume", 0)?,
            item_consume_count: set.int_or("itemConsumeCount", 0)?,
            item_consume_id: set.int_or("itemConsumeId", 0)?,
            cast_range: set.int_or("castRange", -1)?,
            effect_range: set.int_or("effectRange", -1)?,
            abnormal_instant: set.bool_or("abnormalInstant", false),
            abnormal_level: set.int_or("abnormalLevel", 0)?,
            abnormal_type: set.enum_or(
                "abnormalType",
                "AbnormalType",
                AbnormalType::None,
                AbnormalType::from_name,
            )?,
            abnormal_time,
            abnormal_visual_effects: normal,
            abnormal_visual_effects_special: special,
            abnormal_visual_effects_event: event,
            stay_after_death: set.bool_or("stayAfterDeath", false),
            stay_on_subclass_change: set.bool_or("stayOnSubclassChange", true),
            is_recovery_herb: set.bool_or("isRecoveryHerb", false),
            hit_time: set.int_or("hitTime", 0)?,
            cool_time: set.int_or("coolTime", 0)?,
            reuse_hash_code: super::SkillTemplates::hash_code(id, level),
            reuse_delay,
            target_type: set.enum_or(
                "targetType",
                "TargetType",
                TargetType::SelfTarget,
                TargetType::from_name,
            )?,
            feed: set.int_or("feed", 0)?,
            power,
            pvp_power: set.float_or("pvpPower", power)?,
            pve_power: set.float_or("pvePower", power)?,
            magic_level: set.int_or("magicLevel", 0)?,
            lvl_bonus_rate: set.int_or("lvlBonusRate", 0)?,
            activate_rate: set.int_or("activateRate", -1)?,
            min_chance: set.int_or("minChance", options.min_abnormal_state_success_rate)?,
            max_chance: set.int_or("maxChance", options.max_abnormal_state_success_rate)?,
            blow_chance: set.int_or("blowChance", 0)?,
            affect_range: set.int_or("affectRange", 0)?,
            affect_limit,
            next_action_is_attack: set.bool_or("nextActionAttack", false),
            removed_on_any_action_except_move: set.bool_or("removedOnAnyActionExceptMove", false),
            removed_on_damage: set.bool_or("removedOnDamage", false),
            blocked_in_olympiad: set.bool_or("blockedInOlympiad", false),
            element: set.byte_or("element", -1)?,
            element_power: set.int_or("elementPower", 0)?,
            basic_property: set.enum_or("basicProperty", "BaseStat", BaseStat::None, BaseStat::from_name)?,
            overhit: set.bool_or("overHit", false),
            min_pledge_class: set.int_or("minPledgeClass", 0)?,
            charge_consume: set.int_or("chargeConsume", 0)?,
            soul_max_consume: set.int_or("soulMaxConsumeCount", 0)?,
            is_seven_signs: id > 4360 && id < 4367,
            base_crit_rate: set.int_or("baseCritRate", 0)?,
            direct_hp_dmg: set.bool_or("dmgDirectlyToHp", false),
            is_triggered_skill: set.bool_or("isTriggeredSkill", false),
            effect_point: set.int_or("effectPoint", 0)?,
            fly_type: if set.get("flyType").is_some() {
                Some(set.enum_value("flyType", "FlyType", FlyType::from_name)?)
            } else {
                None
            },
            fly_radius: set.int_or("flyRadius", 0)?,
            fly_course: set.float_or("flyCourse", 0.0)?,
            is_debuff: set.bool_or("isDebuff", false),
            attribute: set.string_or("attribute", ""),
            ignore_shield: set.bool_or("ignoreShld", false),
            is_suicide_attack: set.bool_or("isSuicideAttack", false),
            can_be_dispeled: set.bool_or("canBeDispeled", true),
            excluded_from_check: set.bool_or("excludedFromCheck", false),
            simultaneous_cast: set.bool_or("simultaneousCast", false),
            icon: set.string_or("icon", "icon.skill0000"),
            channeling_skill_id: set.int_or("channelingSkillId", 0)?,
            channeling_tick_interval,
            channeling_tick_initial_delay: set
                .int_or("channelingTickInitialDelay", channeling_tick_interval / 1000)?
                .wrapping_mul(1000),
            is_pvp_only: set.bool_or("isPvPOnly", false),
            set: rest,
            effects: EffectLists::default(),
            pre_conditions: Vec::new(),
        })
    }
}

impl fmt::Display for SkillTemplate {
    /// Java `Skill.toString()`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Skill {}({},{})", self.name, self.id, self.level)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options() -> SkillLoadOptions {
        SkillLoadOptions::default()
    }

    fn base() -> StatSet {
        let mut set = StatSet::default();
        set.set_int("skill_id", 7);
        set.set_int("level", 2);
        set.set_str("name", "Test");
        set.set_str("operateType", "A1");
        set
    }

    #[test]
    fn ave_table_matches_the_enum() {
        assert_eq!(AbnormalVisualEffect::ALL.len(), AVE_TABLE.len());
        assert_eq!(AbnormalVisualEffect::TurnFlee.mask(), i32::MIN);
        assert_eq!(
            AbnormalVisualEffect::VpKeep.mask(),
            AbnormalVisualEffect::VpUp.mask()
        );
        assert!(AbnormalVisualEffect::NavitAdvent.is_special());
        assert_eq!(AbnormalVisualEffect::NavitAdvent.mask(), 0x80000);
        assert!(AbnormalVisualEffect::BrSoulAvatar.is_event());
        assert!(!AbnormalVisualEffect::Stun.is_event() && !AbnormalVisualEffect::Stun.is_special());
    }

    #[test]
    fn defaults_follow_the_java_constructor() {
        let skill = SkillTemplate::from_set(&base(), &options()).unwrap();
        assert_eq!(
            (skill.id, skill.level, skill.display_id, skill.display_level),
            (7, 2, 7, 2)
        );
        assert_eq!(
            (skill.cast_range, skill.effect_range, skill.activate_rate),
            (-1, -1, -1)
        );
        assert_eq!((skill.min_chance, skill.max_chance), (10, 90));
        assert!(skill.stay_on_subclass_change && skill.can_be_dispeled);
        assert_eq!(skill.target_type, TargetType::SelfTarget);
        assert_eq!(skill.abnormal_type, AbnormalType::None);
        assert_eq!(skill.icon, "icon.skill0000");
        assert_eq!(skill.element, -1);
        assert_eq!(
            (
                skill.channeling_tick_interval,
                skill.channeling_tick_initial_delay
            ),
            (2000, 2000)
        );
        assert_eq!(skill.hash_code(), 7 * 1021 + 2);
        assert!(skill.fly_type.is_none());
    }

    #[test]
    fn typed_values_and_leftovers() {
        let mut set = base();
        set.set_str("mpConsume", "19");
        set.set_str("power", "36");
        set.set("hitTime", SetValue::Long(1080));
        set.set_str("isMagic", "1");
        set.set_str("affectLimit", "1-3");
        set.set_str("abnormalVisualEffect", "STUN;INVINCIBILITY;BR_NONE");
        set.set_str("afterEffectId", "1234");
        let skill = SkillTemplate::from_set(&set, &options()).unwrap();
        assert_eq!(
            (skill.mp_consume, skill.mp_per_channeling, skill.hit_time),
            (19, 19, 1080)
        );
        assert_eq!(
            (skill.power, skill.pvp_power, skill.pve_power),
            (36.0, 36.0, 36.0)
        );
        assert!(skill.is_magic() && !skill.is_physical());
        assert_eq!(skill.affect_limit, [1, 3]);
        assert_eq!(skill.abnormal_visual_effects, [AbnormalVisualEffect::Stun]);
        assert_eq!(
            skill.abnormal_visual_effects_special,
            [AbnormalVisualEffect::Invincibility]
        );
        assert_eq!(
            skill.abnormal_visual_effects_event,
            [AbnormalVisualEffect::BrNone]
        );
        assert_eq!(skill.set.keys().collect::<Vec<_>>(), ["afterEffectId"]);
    }

    #[test]
    fn constructor_errors_are_errors() {
        let mut no_type = StatSet::default();
        no_type.set_int("skill_id", 1);
        no_type.set_int("level", 1);
        assert!(SkillTemplate::from_set(&no_type, &options()).is_err());
        for (key, value) in [
            ("operateType", "X"),
            ("mpConsume", "1.5"),
            ("targetType", "nowhere"),
            ("affectLimit", "5"),
            ("abnormalVisualEffect", "NOPE"),
            ("trait", "sword"),
        ] {
            let mut set = base();
            set.set_str(key, value);
            assert!(
                SkillTemplate::from_set(&set, &options()).is_err(),
                "{key}={value}"
            );
        }
    }

    #[test]
    fn config_options_change_duration_and_reuse() {
        let mut opts = options();
        opts.enable_modify_skill_duration = true;
        opts.skill_duration_list.insert(7, 500);
        opts.enable_modify_skill_reuse = true;
        opts.skill_reuse_list.insert(7, 42);
        let mut set = base();
        set.set_str("abnormalTime", "20");
        set.set_str("reuseDelay", "9000");
        let plain = SkillTemplate::from_set(&set, &opts).unwrap();
        assert_eq!((plain.abnormal_time, plain.reuse_delay), (500, 42));
        // Enchant levels 100..139 add the configured time, 140+ and toggles keep theirs.
        set.set_int("level", 105);
        assert_eq!(SkillTemplate::from_set(&set, &opts).unwrap().abnormal_time, 520);
        set.set_int("level", 140);
        assert_eq!(SkillTemplate::from_set(&set, &opts).unwrap().abnormal_time, 20);
        set.set_int("level", 2);
        set.set_str("operateType", "T");
        assert_eq!(SkillTemplate::from_set(&set, &opts).unwrap().abnormal_time, 20);
    }

    #[test]
    fn parse_value_picks_the_java_types() {
        assert_eq!(SetValue::parse(" TRUE "), SetValue::Bool(true));
        assert_eq!(SetValue::parse("-7"), SetValue::Long(-7));
        assert_eq!(SetValue::parse("1.5"), SetValue::Float(1.5));
        assert_eq!(SetValue::parse("abc"), SetValue::Str("abc".into()));
        let mut set = StatSet::default();
        set.set("a", SetValue::Float(2.9));
        set.set("b", SetValue::Bool(true));
        assert_eq!(set.int_or("a", 0), Ok(2));
        assert!(set.int_or("b", 0).is_err());
        assert!(set.bool_or("b", false));
        assert_eq!(SetValue::Float(3.0).as_string(), "3.0");
    }

    #[test]
    fn operate_type_helpers() {
        assert!(SkillOperateType::P.is_passive() && !SkillOperateType::P.is_active());
        assert!(SkillOperateType::T.is_toggle() && !SkillOperateType::T.is_active());
        assert!(SkillOperateType::A2.is_continuous() && SkillOperateType::A3.is_self_continuous());
        assert!(SkillOperateType::CA5.is_channeling() && SkillOperateType::DA1.is_active());
        assert_eq!(TraitType::Shock.kind(), 3);
        assert_eq!(TraitType::Sword.kind(), 1);
        assert_eq!(TraitType::Valakas.kind(), 2);
        assert_eq!(TraitType::None.kind(), 0);
    }
}
