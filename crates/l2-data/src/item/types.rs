//! The enums an item template is made of: weapon/armor/etc types, material, crystal grade,
//! default action and body part.
//!
//! Java: `entity/item/type/{WeaponType,ArmorType,EtcItemType,MaterialType,CrystalType,ActionType}.java`,
//! `entity/item/enums/{BodyPart,ItemGrade}.java`.
//!
//! Variant names are Rust style; [`name`](WeaponType::name) gives the Java constant (the spelling
//! the XML uses). Java reads these with `Enum.valueOf`, which is **exact and case sensitive**, so
//! `from_name("sword")` is `None`. The declaration order is Java's ordinal order: masks depend on it.

/// Declares a Java-style enum: variants listed in Java's order, each with its Java constant name.
macro_rules! java_enum {
    (
        $(#[$meta:meta])*
        $name:ident {
            $( $(#[$vmeta:meta])* $variant:ident = $java:literal ),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum $name {
            $( $(#[$vmeta])* $variant ),+
        }

        impl $name {
            /// Every variant, in Java's declaration (ordinal) order.
            pub const ALL: &'static [$name] = &[ $( $name::$variant ),+ ];

            /// The Java constant name, e.g. `FINE_STEEL`.
            pub fn name(self) -> &'static str {
                match self { $( $name::$variant => $java ),+ }
            }

            /// Java `valueOf(name)`: exact, case-sensitive; `None` where Java throws.
            pub fn from_name(name: &str) -> Option<Self> {
                match name { $( $java => Some($name::$variant), )+ _ => None }
            }

            /// Java `ordinal()`.
            pub fn ordinal(self) -> usize {
                self as usize
            }
        }
    };
}

// Also used by the name-only enums of the condition parser (`stats/enums.rs`).
pub(crate) use java_enum;

java_enum! {
    /// Java `WeaponType`.
    WeaponType {
        /// `SWORD`
        Sword = "SWORD",
        /// `BLUNT`
        Blunt = "BLUNT",
        /// `DAGGER`
        Dagger = "DAGGER",
        /// `BOW`
        Bow = "BOW",
        /// `POLE`
        Pole = "POLE",
        /// `NONE`
        None = "NONE",
        /// `DUAL`
        Dual = "DUAL",
        /// `ETC`
        Etc = "ETC",
        /// `FIST`
        Fist = "FIST",
        /// `DUALFIST`
        DualFist = "DUALFIST",
        /// `FISHINGROD`
        FishingRod = "FISHINGROD",
        /// `RAPIER`
        Rapier = "RAPIER",
        /// `ANCIENTSWORD`
        AncientSword = "ANCIENTSWORD",
        /// `CROSSBOW`
        CrossBow = "CROSSBOW",
        /// `FLAG`
        Flag = "FLAG",
        /// `OWNTHING`
        OwnThing = "OWNTHING",
        /// `DUALDAGGER`
        DualDagger = "DUALDAGGER",
    }
}

impl WeaponType {
    /// Java `mask()`: `1 << ordinal`.
    pub fn mask(self) -> i32 {
        1 << self.ordinal()
    }

    /// Java `isRanged()`.
    pub fn is_ranged(self) -> bool {
        matches!(self, WeaponType::Bow | WeaponType::CrossBow)
    }
}

java_enum! {
    /// Java `ArmorType`. `Shield` is an L2J addition: the XML has no shield type, Java derives it
    /// from `NONE` plus the left-hand body part.
    ArmorType {
        /// `NONE`
        None = "NONE",
        /// `LIGHT`
        Light = "LIGHT",
        /// `HEAVY`
        Heavy = "HEAVY",
        /// `MAGIC`
        Magic = "MAGIC",
        /// `SIGIL`
        Sigil = "SIGIL",
        /// `SHIELD`
        Shield = "SHIELD",
    }
}

impl ArmorType {
    /// Java `mask()`: `1 << (ordinal + WeaponType.values().length)`.
    pub fn mask(self) -> i32 {
        1 << (self.ordinal() + WeaponType::ALL.len())
    }
}

java_enum! {
    /// Java `EtcItemType`. `Shot` is an L2J addition set for soulshot/spiritshot default actions.
    EtcItemType {
        /// `NONE`
        None = "NONE",
        /// `ARROW`
        Arrow = "ARROW",
        /// `POTION`
        Potion = "POTION",
        /// `SCRL_ENCHANT_WP`
        ScrlEnchantWp = "SCRL_ENCHANT_WP",
        /// `SCRL_ENCHANT_AM`
        ScrlEnchantAm = "SCRL_ENCHANT_AM",
        /// `SCROLL`
        Scroll = "SCROLL",
        /// `RECIPE`
        Recipe = "RECIPE",
        /// `MATERIAL`
        Material = "MATERIAL",
        /// `PET_COLLAR`
        PetCollar = "PET_COLLAR",
        /// `CASTLE_GUARD`
        CastleGuard = "CASTLE_GUARD",
        /// `LOTTO`
        Lotto = "LOTTO",
        /// `RACE_TICKET`
        RaceTicket = "RACE_TICKET",
        /// `DYE`
        Dye = "DYE",
        /// `SEED`
        Seed = "SEED",
        /// `CROP`
        Crop = "CROP",
        /// `MATURECROP`
        MatureCrop = "MATURECROP",
        /// `HARVEST`
        Harvest = "HARVEST",
        /// `SEED2`
        Seed2 = "SEED2",
        /// `TICKET_OF_LORD`
        TicketOfLord = "TICKET_OF_LORD",
        /// `LURE`
        Lure = "LURE",
        /// `BLESS_SCRL_ENCHANT_WP`
        BlessScrlEnchantWp = "BLESS_SCRL_ENCHANT_WP",
        /// `BLESS_SCRL_ENCHANT_AM`
        BlessScrlEnchantAm = "BLESS_SCRL_ENCHANT_AM",
        /// `COUPON`
        Coupon = "COUPON",
        /// `ELIXIR`
        Elixir = "ELIXIR",
        /// `SCRL_ENCHANT_ATTR`
        ScrlEnchantAttr = "SCRL_ENCHANT_ATTR",
        /// `BOLT`
        Bolt = "BOLT",
        /// `SCRL_INC_ENCHANT_PROP_WP`
        ScrlIncEnchantPropWp = "SCRL_INC_ENCHANT_PROP_WP",
        /// `SCRL_INC_ENCHANT_PROP_AM`
        ScrlIncEnchantPropAm = "SCRL_INC_ENCHANT_PROP_AM",
        /// `ANCIENT_CRYSTAL_ENCHANT_WP`
        AncientCrystalEnchantWp = "ANCIENT_CRYSTAL_ENCHANT_WP",
        /// `ANCIENT_CRYSTAL_ENCHANT_AM`
        AncientCrystalEnchantAm = "ANCIENT_CRYSTAL_ENCHANT_AM",
        /// `RUNE_SELECT`
        RuneSelect = "RUNE_SELECT",
        /// `RUNE`
        Rune = "RUNE",
        /// `SHOT`
        Shot = "SHOT",
    }
}

impl EtcItemType {
    /// Java `mask()`: always 0 for etc items.
    pub fn mask(self) -> i32 {
        0
    }
}

java_enum! {
    /// Java `MaterialType`.
    MaterialType {
        /// `STEEL`
        Steel = "STEEL",
        /// `FINE_STEEL`
        FineSteel = "FINE_STEEL",
        /// `COTTON`
        Cotton = "COTTON",
        /// `BLOOD_STEEL`
        BloodSteel = "BLOOD_STEEL",
        /// `BRONZE`
        Bronze = "BRONZE",
        /// `SILVER`
        Silver = "SILVER",
        /// `GOLD`
        Gold = "GOLD",
        /// `MITHRIL`
        Mithril = "MITHRIL",
        /// `ORIHARUKON`
        Oriharukon = "ORIHARUKON",
        /// `PAPER`
        Paper = "PAPER",
        /// `WOOD`
        Wood = "WOOD",
        /// `CLOTH`
        Cloth = "CLOTH",
        /// `LEATHER`
        Leather = "LEATHER",
        /// `BONE`
        Bone = "BONE",
        /// `HORN`
        Horn = "HORN",
        /// `DAMASCUS`
        Damascus = "DAMASCUS",
        /// `ADAMANTAITE`
        Adamantaite = "ADAMANTAITE",
        /// `CHRYSOLITE`
        Chrysolite = "CHRYSOLITE",
        /// `CRYSTAL`
        Crystal = "CRYSTAL",
        /// `LIQUID`
        Liquid = "LIQUID",
        /// `SCALE_OF_DRAGON`
        ScaleOfDragon = "SCALE_OF_DRAGON",
        /// `DYESTUFF`
        Dyestuff = "DYESTUFF",
        /// `COBWEB`
        Cobweb = "COBWEB",
        /// `SEED`
        Seed = "SEED",
        /// `FISH`
        Fish = "FISH",
        /// `RUNE_XP`
        RuneXp = "RUNE_XP",
        /// `RUNE_SP`
        RuneSp = "RUNE_SP",
        /// `RUNE_REMOVE_PENALTY`
        RuneRemovePenalty = "RUNE_REMOVE_PENALTY",
    }
}

java_enum! {
    /// Java `ActionType`: what double-clicking the item does.
    ActionType {
        /// `CALC`
        Calc = "CALC",
        /// `CALL_SKILL`
        CallSkill = "CALL_SKILL",
        /// `CAPSULE`
        Capsule = "CAPSULE",
        /// `CREATE_MPCC`
        CreateMpcc = "CREATE_MPCC",
        /// `DICE`
        Dice = "DICE",
        /// `EQUIP`
        Equip = "EQUIP",
        /// `FISHINGSHOT`
        FishingShot = "FISHINGSHOT",
        /// `HARVEST`
        Harvest = "HARVEST",
        /// `HIDE_NAME`
        HideName = "HIDE_NAME",
        /// `KEEP_EXP`
        KeepExp = "KEEP_EXP",
        /// `NICK_COLOR`
        NickColor = "NICK_COLOR",
        /// `NONE`
        None = "NONE",
        /// `PEEL`
        Peel = "PEEL",
        /// `RECIPE`
        Recipe = "RECIPE",
        /// `SEED`
        Seed = "SEED",
        /// `SHOW_ADVENTURER_GUIDE_BOOK`
        ShowAdventurerGuideBook = "SHOW_ADVENTURER_GUIDE_BOOK",
        /// `SHOW_HTML`
        ShowHtml = "SHOW_HTML",
        /// `SHOW_SSQ_STATUS`
        ShowSsqStatus = "SHOW_SSQ_STATUS",
        /// `SKILL_MAINTAIN`
        SkillMaintain = "SKILL_MAINTAIN",
        /// `SKILL_REDUCE`
        SkillReduce = "SKILL_REDUCE",
        /// `SOULSHOT`
        SoulShot = "SOULSHOT",
        /// `SPIRITSHOT`
        SpiritShot = "SPIRITSHOT",
        /// `START_QUEST`
        StartQuest = "START_QUEST",
        /// `SUMMON_SOULSHOT`
        SummonSoulShot = "SUMMON_SOULSHOT",
        /// `SUMMON_SPIRITSHOT`
        SummonSpiritShot = "SUMMON_SPIRITSHOT",
        /// `XMAS_OPEN`
        XmasOpen = "XMAS_OPEN",
    }
}

java_enum! {
    /// Java `CrystalType`: the item grade including the S80/S84 sub-grades.
    CrystalType {
        /// `NONE`
        None = "NONE",
        /// `D`
        D = "D",
        /// `C`
        C = "C",
        /// `B`
        B = "B",
        /// `A`
        A = "A",
        /// `S`
        S = "S",
        /// `S80`
        S80 = "S80",
        /// `S84`
        S84 = "S84",
    }
}

impl CrystalType {
    /// Java `getLevel()`.
    pub fn level(self) -> i32 {
        self.ordinal() as i32
    }

    /// Java `getCrystalId()`: the crystal item this grade crystallizes into (0 for `NONE`).
    pub fn crystal_id(self) -> i32 {
        match self {
            CrystalType::None => 0,
            CrystalType::D => 1458,
            CrystalType::C => 1459,
            CrystalType::B => 1460,
            CrystalType::A => 1461,
            CrystalType::S | CrystalType::S80 | CrystalType::S84 => 1462,
        }
    }

    /// Java `getCrystalEnchantBonusArmor()`.
    pub fn crystal_enchant_bonus_armor(self) -> i32 {
        match self {
            CrystalType::None => 0,
            CrystalType::D => 11,
            CrystalType::C => 6,
            CrystalType::B => 11,
            CrystalType::A => 20,
            CrystalType::S | CrystalType::S80 | CrystalType::S84 => 25,
        }
    }

    /// Java `getCrystalEnchantBonusWeapon()`.
    pub fn crystal_enchant_bonus_weapon(self) -> i32 {
        match self {
            CrystalType::None => 0,
            CrystalType::D => 90,
            CrystalType::C => 45,
            CrystalType::B => 67,
            CrystalType::A => 145,
            CrystalType::S | CrystalType::S80 | CrystalType::S84 => 250,
        }
    }

    /// Java `ItemTemplate.getCrystalTypePlus()`: S80 and S84 count as S.
    pub fn plus(self) -> CrystalType {
        match self {
            CrystalType::S80 | CrystalType::S84 => CrystalType::S,
            other => other,
        }
    }

    /// Java `ItemGrade.valueOf(CrystalType)`.
    pub fn item_grade(self) -> ItemGrade {
        match self {
            CrystalType::None => ItemGrade::None,
            CrystalType::D => ItemGrade::D,
            CrystalType::C => ItemGrade::C,
            CrystalType::B => ItemGrade::B,
            CrystalType::A => ItemGrade::A,
            CrystalType::S | CrystalType::S80 | CrystalType::S84 => ItemGrade::S,
        }
    }
}

java_enum! {
    /// Java `ItemGrade`: the general grade, without S80/S84.
    ItemGrade {
        /// `NONE`
        None = "NONE",
        /// `D`
        D = "D",
        /// `C`
        C = "C",
        /// `B`
        B = "B",
        /// `A`
        A = "A",
        /// `S`
        S = "S",
    }
}

/// Java `BodyPart`: which paperdoll slot(s) an item occupies, and the bit mask the client expects
/// in `ItemList`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BodyPart {
    /// `NONE`: not equippable.
    None,
    /// `UNDERWEAR` (`underwear`, `shirt`).
    Underwear,
    /// `R_EAR` (`rear`).
    RightEar,
    /// `L_EAR` (`lear`).
    LeftEar,
    /// `LR_EAR` (`rear;lear`): earrings fit either ear.
    BothEars,
    /// `NECK`.
    Neck,
    /// `R_FINGER`.
    RightFinger,
    /// `L_FINGER`.
    LeftFinger,
    /// `LR_FINGER` (`rfinger;lfinger`): rings fit either finger.
    BothFingers,
    /// `HEAD`.
    Head,
    /// `R_HAND`.
    RightHand,
    /// `L_HAND`.
    LeftHand,
    /// `GLOVES`.
    Gloves,
    /// `CHEST`.
    Chest,
    /// `LEGS`.
    Legs,
    /// `FEET`.
    Feet,
    /// `BACK` (cloak).
    Back,
    /// `LR_HAND` (`lrhand`): two-handed weapon.
    BothHands,
    /// `FULL_ARMOR` (`fullarmor`, `onepiece`).
    FullArmor,
    /// `HAIR`.
    Hair,
    /// `ALLDRESS`.
    AllDress,
    /// `HAIR2`.
    Hair2,
    /// `HAIRALL` (`hairall`, `dhair`).
    HairAll,
    /// `R_BRACELET`.
    RightBracelet,
    /// `L_BRACELET`.
    LeftBracelet,
    /// `DECO` (`deco1`, `talisman`).
    Deco,
    /// `BELT` (`belt`, `waist`).
    Belt,
    /// `WOLF`: pet armor.
    Wolf,
    /// `HATCHLING`: pet armor.
    Hatchling,
    /// `STRIDER`: pet armor.
    Strider,
    /// `BABYPET`: pet armor.
    BabyPet,
    /// `GREATWOLF`: pet armor.
    GreatWolf,
}

impl BodyPart {
    /// Java `getMask()`: the slot bit mask. Pet armor uses negative pseudo-masks.
    pub fn mask(self) -> i32 {
        match self {
            BodyPart::None => 0x0000,
            BodyPart::Underwear => 0x0001,
            BodyPart::RightEar => 0x0002,
            BodyPart::LeftEar => 0x0004,
            BodyPart::BothEars => 0x0006,
            BodyPart::Neck => 0x0008,
            BodyPart::RightFinger => 0x0010,
            BodyPart::LeftFinger => 0x0020,
            BodyPart::BothFingers => 0x0030,
            BodyPart::Head => 0x0040,
            BodyPart::RightHand => 0x0080,
            BodyPart::LeftHand => 0x0100,
            BodyPart::Gloves => 0x0200,
            BodyPart::Chest => 0x0400,
            BodyPart::Legs => 0x0800,
            BodyPart::Feet => 0x1000,
            BodyPart::Back => 0x2000,
            BodyPart::BothHands => 0x4000,
            BodyPart::FullArmor => 0x8000,
            BodyPart::Hair => 0x01_0000,
            BodyPart::AllDress => 0x02_0000,
            BodyPart::Hair2 => 0x04_0000,
            BodyPart::HairAll => 0x08_0000,
            BodyPart::RightBracelet => 0x10_0000,
            BodyPart::LeftBracelet => 0x20_0000,
            BodyPart::Deco => 0x40_0000,
            BodyPart::Belt => 0x1000_0000,
            BodyPart::Wolf => -100,
            BodyPart::Hatchling => -101,
            BodyPart::Strider => -102,
            BodyPart::BabyPet => -103,
            BodyPart::GreatWolf => -104,
        }
    }

    /// Java `BodyPart.fromName`: the lower-case name used in the XML (`bodypart`). Exact match;
    /// `None` where Java returns `null`.
    ///
    /// NOTE(java-quirk): `rbracelet` is listed under both `R_EAR` and `R_BRACELET` in Java; the map is
    /// filled in declaration order, so the later one (`R_BRACELET`) wins.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "none" => BodyPart::None,
            "underwear" | "shirt" => BodyPart::Underwear,
            "rear" => BodyPart::RightEar,
            "lear" => BodyPart::LeftEar,
            "rear;lear" => BodyPart::BothEars,
            "neck" => BodyPart::Neck,
            "rfinger" => BodyPart::RightFinger,
            "lfinger" => BodyPart::LeftFinger,
            "rfinger;lfinger" => BodyPart::BothFingers,
            "head" => BodyPart::Head,
            "rhand" => BodyPart::RightHand,
            "lhand" => BodyPart::LeftHand,
            "gloves" => BodyPart::Gloves,
            "chest" => BodyPart::Chest,
            "legs" => BodyPart::Legs,
            "feet" => BodyPart::Feet,
            "back" => BodyPart::Back,
            "lrhand" => BodyPart::BothHands,
            "fullarmor" | "onepiece" => BodyPart::FullArmor,
            "hair" => BodyPart::Hair,
            "alldress" => BodyPart::AllDress,
            "hair2" => BodyPart::Hair2,
            "hairall" | "dhair" => BodyPart::HairAll,
            "rbracelet" => BodyPart::RightBracelet,
            "lbracelet" => BodyPart::LeftBracelet,
            "deco1" | "talisman" => BodyPart::Deco,
            "belt" | "waist" => BodyPart::Belt,
            "wolf" => BodyPart::Wolf,
            "hatchling" => BodyPart::Hatchling,
            "strider" => BodyPart::Strider,
            "babypet" => BodyPart::BabyPet,
            "greatwolf" => BodyPart::GreatWolf,
            _ => return Option::None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enum_names_round_trip_and_are_case_sensitive() {
        for &t in WeaponType::ALL {
            assert_eq!(WeaponType::from_name(t.name()), Some(t));
        }
        for &t in EtcItemType::ALL {
            assert_eq!(EtcItemType::from_name(t.name()), Some(t));
        }
        assert_eq!(
            MaterialType::from_name("FINE_STEEL"),
            Some(MaterialType::FineSteel)
        );
        assert_eq!(MaterialType::from_name("fine_steel"), None);
        assert_eq!(ActionType::from_name("EQUIP"), Some(ActionType::Equip));
        assert_eq!(ActionType::from_name("equip"), None);
    }

    #[test]
    fn type_masks_follow_java_ordinals() {
        assert_eq!(WeaponType::Sword.mask(), 1);
        assert_eq!(WeaponType::Bow.mask(), 1 << 3);
        assert_eq!(WeaponType::DualDagger.mask(), 1 << 16);
        assert_eq!(WeaponType::ALL.len(), 17);
        assert_eq!(ArmorType::None.mask(), 1 << 17);
        assert_eq!(ArmorType::Shield.mask(), 1 << 22);
        assert_eq!(EtcItemType::Potion.mask(), 0);
    }

    #[test]
    fn crystal_grades() {
        assert_eq!(CrystalType::D.level(), 1);
        assert_eq!(CrystalType::S84.level(), 7);
        assert_eq!(CrystalType::S80.plus(), CrystalType::S);
        assert_eq!(CrystalType::S84.item_grade(), ItemGrade::S);
        assert_eq!(CrystalType::B.crystal_id(), 1460);
        assert_eq!(CrystalType::A.crystal_enchant_bonus_weapon(), 145);
    }

    #[test]
    fn body_part_masks_and_names() {
        assert_eq!(BodyPart::from_name("rhand").unwrap().mask(), 0x80);
        assert_eq!(BodyPart::from_name("lrhand").unwrap().mask(), 0x4000);
        assert_eq!(BodyPart::from_name("rear;lear").unwrap().mask(), 0x6);
        assert_eq!(BodyPart::from_name("onepiece"), Some(BodyPart::FullArmor));
        assert_eq!(BodyPart::from_name("waist").unwrap().mask(), 0x1000_0000);
        assert_eq!(BodyPart::from_name("greatwolf").unwrap().mask(), -104);
        assert_eq!(BodyPart::from_name("RHAND"), None);
        // NOTE(java-quirk): the later enum constant wins the shared name.
        assert_eq!(BodyPart::from_name("rbracelet"), Some(BodyPart::RightBracelet));
    }
}
