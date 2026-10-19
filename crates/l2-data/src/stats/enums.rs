//! Name-only enums the condition parser needs: the values are validated like Java's
//! `Enum.valueOf` (exact, case sensitive) and kept as typed values for the stat engine.
//!
//! Java: `mechanics/skill/AbnormalType.java`, `data/enums/CategoryType.java`,
//! `entity/actor/enums/creature/InstanceType.java`, `entity/actor/enums/player/PlayerState.java`.
//! Variant names are Rust style; `name()` is the Java constant. Declaration order is Java's.
//! The Java `Race` is [`crate::npc::NpcRace`].

use crate::item::types::java_enum;

java_enum! {
    /// Java `PlayerState`.
    PlayerState {
        /// `RESTING`
        Resting = "RESTING",
        /// `MOVING`
        Moving = "MOVING",
        /// `RUNNING`
        Running = "RUNNING",
        /// `STANDING`
        Standing = "STANDING",
        /// `FLYING`
        Flying = "FLYING",
        /// `BEHIND`
        Behind = "BEHIND",
        /// `FRONT`
        Front = "FRONT",
        /// `CHAOTIC`
        Chaotic = "CHAOTIC",
        /// `OLYMPIAD`
        Olympiad = "OLYMPIAD",
    }
}

java_enum! {
    /// Java `AbnormalType`.
    AbnormalType {
        /// `ABILITY_CHANGE`
        AbilityChange = "ABILITY_CHANGE",
        /// `ABNORMAL_INVINCIBILITY`
        AbnormalInvincibility = "ABNORMAL_INVINCIBILITY",
        /// `ABNORMAL_ITEM`
        AbnormalItem = "ABNORMAL_ITEM",
        /// `AB_HAWK_EYE`
        AbHawkEye = "AB_HAWK_EYE",
        /// `ALL_ATTACK_DOWN`
        AllAttackDown = "ALL_ATTACK_DOWN",
        /// `ALL_REGEN_UP`
        AllRegenUp = "ALL_REGEN_UP",
        /// `ALL_SPEED_DOWN`
        AllSpeedDown = "ALL_SPEED_DOWN",
        /// `ANESTHESIA`
        Anesthesia = "ANESTHESIA",
        /// `ANTARAS_DEBUFF`
        AntarasDebuff = "ANTARAS_DEBUFF",
        /// `APELLA`
        Apella = "APELLA",
        /// `ARCHER_SPECIAL`
        ArcherSpecial = "ARCHER_SPECIAL",
        /// `ARCHER_SPECIAL_I`
        ArcherSpecialI = "ARCHER_SPECIAL_I",
        /// `ARMOR_EARTH`
        ArmorEarth = "ARMOR_EARTH",
        /// `ARMOR_FIRE`
        ArmorFire = "ARMOR_FIRE",
        /// `ARMOR_HOLY`
        ArmorHoly = "ARMOR_HOLY",
        /// `ARMOR_UNHOLY`
        ArmorUnholy = "ARMOR_UNHOLY",
        /// `ARMOR_WATER`
        ArmorWater = "ARMOR_WATER",
        /// `ARMOR_WIND`
        ArmorWind = "ARMOR_WIND",
        /// `ARROW_RAIN`
        ArrowRain = "ARROW_RAIN",
        /// `ATTACK_SPEED_UP_BOW`
        AttackSpeedUpBow = "ATTACK_SPEED_UP_BOW",
        /// `ATTACK_TIME_DOWN`
        AttackTimeDown = "ATTACK_TIME_DOWN",
        /// `ATTACK_TIME_DOWN_SPECIAL`
        AttackTimeDownSpecial = "ATTACK_TIME_DOWN_SPECIAL",
        /// `ATTACK_TIME_UP`
        AttackTimeUp = "ATTACK_TIME_UP",
        /// `ATTRIBUTE_POTION`
        AttributePotion = "ATTRIBUTE_POTION",
        /// `AVOID_DOWN`
        AvoidDown = "AVOID_DOWN",
        /// `AVOID_SKILL`
        AvoidSkill = "AVOID_SKILL",
        /// `AVOID_UP`
        AvoidUp = "AVOID_UP",
        /// `AVOID_UP_SPECIAL`
        AvoidUpSpecial = "AVOID_UP_SPECIAL",
        /// `BERSERKER`
        Berserker = "BERSERKER",
        /// `BETRAYAL_MARK`
        BetrayalMark = "BETRAYAL_MARK",
        /// `BIG_BODY`
        BigBody = "BIG_BODY",
        /// `BIG_HEAD`
        BigHead = "BIG_HEAD",
        /// `BLEEDING`
        Bleeding = "BLEEDING",
        /// `BLESS_THE_BLOOD`
        BlessTheBlood = "BLESS_THE_BLOOD",
        /// `BLOCK_RESURRECTION`
        BlockResurrection = "BLOCK_RESURRECTION",
        /// `BLOCK_SHIELD_UP`
        BlockShieldUp = "BLOCK_SHIELD_UP",
        /// `BLOCK_SPEED_UP`
        BlockSpeedUp = "BLOCK_SPEED_UP",
        /// `BLOCK_TRANSFORM`
        BlockTransform = "BLOCK_TRANSFORM",
        /// `BLOOD_CONSTRACT`
        BloodConstract = "BLOOD_CONSTRACT",
        /// `BOT_PENALTY`
        BotPenalty = "BOT_PENALTY",
        /// `BR_EVENT_BUF1`
        BrEventBuf1 = "BR_EVENT_BUF1",
        /// `BR_EVENT_BUF10`
        BrEventBuf10 = "BR_EVENT_BUF10",
        /// `BR_EVENT_BUF2`
        BrEventBuf2 = "BR_EVENT_BUF2",
        /// `BR_EVENT_BUF3`
        BrEventBuf3 = "BR_EVENT_BUF3",
        /// `BR_EVENT_BUF5`
        BrEventBuf5 = "BR_EVENT_BUF5",
        /// `BR_EVENT_BUF6`
        BrEventBuf6 = "BR_EVENT_BUF6",
        /// `BR_EVENT_BUF7`
        BrEventBuf7 = "BR_EVENT_BUF7",
        /// `BUFF_QUEEN_OF_CAT`
        BuffQueenOfCat = "BUFF_QUEEN_OF_CAT",
        /// `BUFF_UNICORN_SERAPHIM`
        BuffUnicornSeraphim = "BUFF_UNICORN_SERAPHIM",
        /// `CANCEL_PROB_DOWN`
        CancelProbDown = "CANCEL_PROB_DOWN",
        /// `CASTING_TIME_DOWN`
        CastingTimeDown = "CASTING_TIME_DOWN",
        /// `CASTING_TIME_UP`
        CastingTimeUp = "CASTING_TIME_UP",
        /// `CHEAP_MAGIC`
        CheapMagic = "CHEAP_MAGIC",
        /// `COMBINATION`
        Combination = "COMBINATION",
        /// `COUNTER_CRITICAL`
        CounterCritical = "COUNTER_CRITICAL",
        /// `COUNTER_CRITICAL_TRIGGER`
        CounterCriticalTrigger = "COUNTER_CRITICAL_TRIGGER",
        /// `COUNTER_SKILL`
        CounterSkill = "COUNTER_SKILL",
        /// `CP_UP`
        CpUp = "CP_UP",
        /// `CRITICAL_DMG_DOWN`
        CriticalDmgDown = "CRITICAL_DMG_DOWN",
        /// `CRITICAL_DMG_UP`
        CriticalDmgUp = "CRITICAL_DMG_UP",
        /// `CRITICAL_POISON`
        CriticalPoison = "CRITICAL_POISON",
        /// `CRITICAL_PROB_DOWN`
        CriticalProbDown = "CRITICAL_PROB_DOWN",
        /// `CRITICAL_PROB_UP`
        CriticalProbUp = "CRITICAL_PROB_UP",
        /// `CURSE_LIFE_FLOW`
        CurseLifeFlow = "CURSE_LIFE_FLOW",
        /// `DAMAGE_AMPLIFY`
        DamageAmplify = "DAMAGE_AMPLIFY",
        /// `DANCE_DEFENCE_MOTION1`
        DanceDefenceMotion1 = "DANCE_DEFENCE_MOTION1",
        /// `DANCE_OF_ALIGNMENT`
        DanceOfAlignment = "DANCE_OF_ALIGNMENT",
        /// `DANCE_OF_AQUA_GUARD`
        DanceOfAquaGuard = "DANCE_OF_AQUA_GUARD",
        /// `DANCE_OF_BERSERKER`
        DanceOfBerserker = "DANCE_OF_BERSERKER",
        /// `DANCE_OF_BLADESTORM`
        DanceOfBladestorm = "DANCE_OF_BLADESTORM",
        /// `DANCE_OF_CONCENTRATION`
        DanceOfConcentration = "DANCE_OF_CONCENTRATION",
        /// `DANCE_OF_EARTH_GUARD`
        DanceOfEarthGuard = "DANCE_OF_EARTH_GUARD",
        /// `DANCE_OF_FIRE`
        DanceOfFire = "DANCE_OF_FIRE",
        /// `DANCE_OF_FURY`
        DanceOfFury = "DANCE_OF_FURY",
        /// `DANCE_OF_INSPIRATION`
        DanceOfInspiration = "DANCE_OF_INSPIRATION",
        /// `DANCE_OF_LIGHT`
        DanceOfLight = "DANCE_OF_LIGHT",
        /// `DANCE_OF_MYSTIC`
        DanceOfMystic = "DANCE_OF_MYSTIC",
        /// `DANCE_OF_PROTECTION`
        DanceOfProtection = "DANCE_OF_PROTECTION",
        /// `DANCE_OF_SHADOW`
        DanceOfShadow = "DANCE_OF_SHADOW",
        /// `DANCE_OF_SIREN`
        DanceOfSiren = "DANCE_OF_SIREN",
        /// `DANCE_OF_VAMPIRE`
        DanceOfVampire = "DANCE_OF_VAMPIRE",
        /// `DANCE_OF_WARRIOR`
        DanceOfWarrior = "DANCE_OF_WARRIOR",
        /// `DARK_SEED`
        DarkSeed = "DARK_SEED",
        /// `DD_RESIST`
        DdResist = "DD_RESIST",
        /// `DEATHWORM`
        Deathworm = "DEATHWORM",
        /// `DEATH_CLACK`
        DeathClack = "DEATH_CLACK",
        /// `DEATH_MARK`
        DeathMark = "DEATH_MARK",
        /// `DEATH_PENALTY`
        DeathPenalty = "DEATH_PENALTY",
        /// `DEBUFF_NIGHTSHADE`
        DebuffNightshade = "DEBUFF_NIGHTSHADE",
        /// `DEBUFF_SHIELD`
        DebuffShield = "DEBUFF_SHIELD",
        /// `DECREASE_WEIGHT_PENALTY`
        DecreaseWeightPenalty = "DECREASE_WEIGHT_PENALTY",
        /// `DERANGEMENT`
        Derangement = "DERANGEMENT",
        /// `DETECT_WEAKNESS`
        DetectWeakness = "DETECT_WEAKNESS",
        /// `DISARM`
        Disarm = "DISARM",
        /// `DISTRUST`
        Distrust = "DISTRUST",
        /// `DMG_SHIELD`
        DmgShield = "DMG_SHIELD",
        /// `DOT_ATTR`
        DotAttr = "DOT_ATTR",
        /// `DOT_MP`
        DotMp = "DOT_MP",
        /// `DRAGON_BUFF`
        DragonBuff = "DRAGON_BUFF",
        /// `DRAGON_BREATH`
        DragonBreath = "DRAGON_BREATH",
        /// `DUELIST_SPIRIT`
        DuelistSpirit = "DUELIST_SPIRIT",
        /// `DWARF_ATTACK_BUFF`
        DwarfAttackBuff = "DWARF_ATTACK_BUFF",
        /// `DWARF_DEFENCE_BUFF`
        DwarfDefenceBuff = "DWARF_DEFENCE_BUFF",
        /// `ELEMENTAL_ARMOR`
        ElementalArmor = "ELEMENTAL_ARMOR",
        /// `ENERVATION`
        Enervation = "ENERVATION",
        /// `ENTRY_FOR_GAME`
        EntryForGame = "ENTRY_FOR_GAME",
        /// `EVASION_BUFF`
        EvasionBuff = "EVASION_BUFF",
        /// `EVENT_GAWI`
        EventGawi = "EVENT_GAWI",
        /// `EVENT_SANTA_REWARD`
        EventSantaReward = "EVENT_SANTA_REWARD",
        /// `EVENT_TERRITORY`
        EventTerritory = "EVENT_TERRITORY",
        /// `EVENT_WIN`
        EventWin = "EVENT_WIN",
        /// `EVIL_BLOOD`
        EvilBlood = "EVIL_BLOOD",
        /// `EXPOSE_WEAK_POINT`
        ExposeWeakPoint = "EXPOSE_WEAK_POINT",
        /// `FATAL_POISON`
        FatalPoison = "FATAL_POISON",
        /// `FINAL_SECRET`
        FinalSecret = "FINAL_SECRET",
        /// `FIRE_DOT`
        FireDot = "FIRE_DOT",
        /// `FISHING_MASTERY_DOWN`
        FishingMasteryDown = "FISHING_MASTERY_DOWN",
        /// `FLAME_HAWK`
        FlameHawk = "FLAME_HAWK",
        /// `FLY_AWAY`
        FlyAway = "FLY_AWAY",
        /// `FOCUS_DAGGER`
        FocusDagger = "FOCUS_DAGGER",
        /// `FORCE_MEDITATION`
        ForceMeditation = "FORCE_MEDITATION",
        /// `FORCE_OF_DESTRUCTION`
        ForceOfDestruction = "FORCE_OF_DESTRUCTION",
        /// `FREEZING`
        Freezing = "FREEZING",
        /// `GHOST_PIERCING`
        GhostPiercing = "GHOST_PIERCING",
        /// `HEAL_EFFECT_DOWN`
        HealEffectDown = "HEAL_EFFECT_DOWN",
        /// `HEAL_EFFECT_UP`
        HealEffectUp = "HEAL_EFFECT_UP",
        /// `HEAL_POWER_UP`
        HealPowerUp = "HEAL_POWER_UP",
        /// `HERO_BUFF`
        HeroBuff = "HERO_BUFF",
        /// `HERO_DEBUFF`
        HeroDebuff = "HERO_DEBUFF",
        /// `HIDE`
        Hide = "HIDE",
        /// `HIT_DOWN`
        HitDown = "HIT_DOWN",
        /// `HIT_UP`
        HitUp = "HIT_UP",
        /// `HOLY_ATTACK`
        HolyAttack = "HOLY_ATTACK",
        /// `HOT_GROUND`
        HotGround = "HOT_GROUND",
        /// `HP_RECOVER`
        HpRecover = "HP_RECOVER",
        /// `HP_REGEN_DOWN`
        HpRegenDown = "HP_REGEN_DOWN",
        /// `HP_REGEN_UP`
        HpRegenUp = "HP_REGEN_UP",
        /// `IMPROVE_CRT_RATE_DMG_UP`
        ImproveCrtRateDmgUp = "IMPROVE_CRT_RATE_DMG_UP",
        /// `IMPROVE_HIT_DEFENCE_CRT_RATE_UP`
        ImproveHitDefenceCrtRateUp = "IMPROVE_HIT_DEFENCE_CRT_RATE_UP",
        /// `IMPROVE_HP_MP_UP`
        ImproveHpMpUp = "IMPROVE_HP_MP_UP",
        /// `IMPROVE_MA_MD_UP`
        ImproveMaMdUp = "IMPROVE_MA_MD_UP",
        /// `IMPROVE_PA_PD_UP`
        ImprovePaPdUp = "IMPROVE_PA_PD_UP",
        /// `IMPROVE_SHIELD_RATE_DEFENCE_UP`
        ImproveShieldRateDefenceUp = "IMPROVE_SHIELD_RATE_DEFENCE_UP",
        /// `IMPROVE_SPEED_AVOID_UP`
        ImproveSpeedAvoidUp = "IMPROVE_SPEED_AVOID_UP",
        /// `IMPROVE_VAMPIRIC_HASTE`
        ImproveVampiricHaste = "IMPROVE_VAMPIRIC_HASTE",
        /// `INSTINCT`
        Instinct = "INSTINCT",
        /// `INVINCIBILITY`
        Invincibility = "INVINCIBILITY",
        /// `IRON_SHIELD`
        IronShield = "IRON_SHIELD",
        /// `IRON_SHIELD_I`
        IronShieldI = "IRON_SHIELD_I",
        /// `KAMAEL_SPECIAL`
        KamaelSpecial = "KAMAEL_SPECIAL",
        /// `KNIGHT_AURA`
        KnightAura = "KNIGHT_AURA",
        /// `LIFE_FORCE_KAMAEL`
        LifeForceKamael = "LIFE_FORCE_KAMAEL",
        /// `LIFE_FORCE_ORC`
        LifeForceOrc = "LIFE_FORCE_ORC",
        /// `LIFE_FORCE_OTHERS`
        LifeForceOthers = "LIFE_FORCE_OTHERS",
        /// `LIMIT`
        Limit = "LIMIT",
        /// `MAGICAL_STANCE`
        MagicalStance = "MAGICAL_STANCE",
        /// `MAGIC_CRITICAL_UP`
        MagicCriticalUp = "MAGIC_CRITICAL_UP",
        /// `MAJESTY`
        Majesty = "MAJESTY",
        /// `MAXIMUM_ABILITY`
        MaximumAbility = "MAXIMUM_ABILITY",
        /// `MAX_BREATH_UP`
        MaxBreathUp = "MAX_BREATH_UP",
        /// `MAX_HP_DOWN`
        MaxHpDown = "MAX_HP_DOWN",
        /// `MAX_HP_UP`
        MaxHpUp = "MAX_HP_UP",
        /// `MAX_HP_CP_UP`
        MaxHpCpUp = "MAX_HP_CP_UP",
        /// `MAX_MP_UP`
        MaxMpUp = "MAX_MP_UP",
        /// `MA_DOWN`
        MaDown = "MA_DOWN",
        /// `MA_MD_UP`
        MaMdUp = "MA_MD_UP",
        /// `MA_UP`
        MaUp = "MA_UP",
        /// `MA_UP_HERB`
        MaUpHerb = "MA_UP_HERB",
        /// `MA_UP_SPECIAL`
        MaUpSpecial = "MA_UP_SPECIAL",
        /// `MD_DOWN`
        MdDown = "MD_DOWN",
        /// `MD_UP`
        MdUp = "MD_UP",
        /// `MD_UP_ATTR`
        MdUpAttr = "MD_UP_ATTR",
        /// `MENTAL_IMPOVERISH`
        MentalImpoverish = "MENTAL_IMPOVERISH",
        /// `METEOR`
        Meteor = "METEOR",
        /// `MIGHT_MORTAL`
        MightMortal = "MIGHT_MORTAL",
        /// `MIRAGE`
        Mirage = "MIRAGE",
        /// `MIRAGE_TRAP`
        MirageTrap = "MIRAGE_TRAP",
        /// `MP_COST_DOWN`
        MpCostDown = "MP_COST_DOWN",
        /// `MP_COST_UP`
        MpCostUp = "MP_COST_UP",
        /// `MP_REGEN_UP`
        MpRegenUp = "MP_REGEN_UP",
        /// `MULTI_BUFF`
        MultiBuff = "MULTI_BUFF",
        /// `MULTI_BUFF_A`
        MultiBuffA = "MULTI_BUFF_A",
        /// `MULTI_DEBUFF`
        MultiDebuff = "MULTI_DEBUFF",
        /// `MULTI_DEBUFF_A`
        MultiDebuffA = "MULTI_DEBUFF_A",
        /// `MULTI_DEBUFF_B`
        MultiDebuffB = "MULTI_DEBUFF_B",
        /// `MULTI_DEBUFF_C`
        MultiDebuffC = "MULTI_DEBUFF_C",
        /// `MULTI_DEBUFF_D`
        MultiDebuffD = "MULTI_DEBUFF_D",
        /// `MULTI_DEBUFF_E`
        MultiDebuffE = "MULTI_DEBUFF_E",
        /// `MULTI_DEBUFF_F`
        MultiDebuffF = "MULTI_DEBUFF_F",
        /// `MULTI_DEBUFF_FIRE`
        MultiDebuffFire = "MULTI_DEBUFF_FIRE",
        /// `MULTI_DEBUFF_G`
        MultiDebuffG = "MULTI_DEBUFF_G",
        /// `MULTI_DEBUFF_HOLY`
        MultiDebuffHoly = "MULTI_DEBUFF_HOLY",
        /// `MULTI_DEBUFF_SOUL`
        MultiDebuffSoul = "MULTI_DEBUFF_SOUL",
        /// `MULTI_DEBUFF_UNHOLY`
        MultiDebuffUnholy = "MULTI_DEBUFF_UNHOLY",
        /// `MULTI_DEBUFF_WATER`
        MultiDebuffWater = "MULTI_DEBUFF_WATER",
        /// `MULTI_DEBUFF_WIND`
        MultiDebuffWind = "MULTI_DEBUFF_WIND",
        /// `NONE`
        None = "NONE",
        /// `NORMAL_ATTACK_BLOCK`
        NormalAttackBlock = "NORMAL_ATTACK_BLOCK",
        /// `OBLIVION`
        Oblivion = "OBLIVION",
        /// `PARALYZE`
        Paralyze = "PARALYZE",
        /// `PATIENCE`
        Patience = "PATIENCE",
        /// `PA_DOWN`
        PaDown = "PA_DOWN",
        /// `PA_PD_UP`
        PaPdUp = "PA_PD_UP",
        /// `PA_UP`
        PaUp = "PA_UP",
        /// `PA_UP_HERB`
        PaUpHerb = "PA_UP_HERB",
        /// `PA_UP_SPECIAL`
        PaUpSpecial = "PA_UP_SPECIAL",
        /// `PD_DOWN`
        PdDown = "PD_DOWN",
        /// `PD_UP`
        PdUp = "PD_UP",
        /// `PD_UP_BOW`
        PdUpBow = "PD_UP_BOW",
        /// `PD_UP_SPECIAL`
        PdUpSpecial = "PD_UP_SPECIAL",
        /// `PHYSICAL_STANCE`
        PhysicalStance = "PHYSICAL_STANCE",
        /// `PINCH`
        Pinch = "PINCH",
        /// `PK_PROTECT`
        PkProtect = "PK_PROTECT",
        /// `POISON`
        Poison = "POISON",
        /// `POLEARM_ATTACK`
        PolearmAttack = "POLEARM_ATTACK",
        /// `POSSESSION`
        Possession = "POSSESSION",
        /// `POSSESSION_SPECIAL`
        PossessionSpecial = "POSSESSION_SPECIAL",
        /// `POTION_OF_GENESIS`
        PotionOfGenesis = "POTION_OF_GENESIS",
        /// `PRESERVE_ABNORMAL`
        PreserveAbnormal = "PRESERVE_ABNORMAL",
        /// `PROTECTION`
        Protection = "PROTECTION",
        /// `PUBLIC_SLOT`
        PublicSlot = "PUBLIC_SLOT",
        /// `PVP_DMG_DOWN`
        PvpDmgDown = "PVP_DMG_DOWN",
        /// `PVP_WEAPON_BUFF`
        PvpWeaponBuff = "PVP_WEAPON_BUFF",
        /// `PVP_WEAPON_DEBUFF`
        PvpWeaponDebuff = "PVP_WEAPON_DEBUFF",
        /// `RAGE_MIGHT`
        RageMight = "RAGE_MIGHT",
        /// `REAL_TARGET`
        RealTarget = "REAL_TARGET",
        /// `RECHARGE_UP`
        RechargeUp = "RECHARGE_UP",
        /// `REDUCE_DROP_PENALTY`
        ReduceDropPenalty = "REDUCE_DROP_PENALTY",
        /// `REFLECT_ABNORMAL`
        ReflectAbnormal = "REFLECT_ABNORMAL",
        /// `REFLECT_MAGIC_DD`
        ReflectMagicDd = "REFLECT_MAGIC_DD",
        /// `RESIST_BLEEDING`
        ResistBleeding = "RESIST_BLEEDING",
        /// `RESIST_DEBUFF_DISPEL`
        ResistDebuffDispel = "RESIST_DEBUFF_DISPEL",
        /// `RESIST_DERANGEMENT`
        ResistDerangement = "RESIST_DERANGEMENT",
        /// `RESIST_HOLY_UNHOLY`
        ResistHolyUnholy = "RESIST_HOLY_UNHOLY",
        /// `RESIST_POISON`
        ResistPoison = "RESIST_POISON",
        /// `RESIST_SHOCK`
        ResistShock = "RESIST_SHOCK",
        /// `RESIST_SPIRITLESS`
        ResistSpiritless = "RESIST_SPIRITLESS",
        /// `RESURRECTION_SPECIAL`
        ResurrectionSpecial = "RESURRECTION_SPECIAL",
        /// `REUSE_DELAY_DOWN`
        ReuseDelayDown = "REUSE_DELAY_DOWN",
        /// `REUSE_DELAY_UP`
        ReuseDelayUp = "REUSE_DELAY_UP",
        /// `ROOT_MAGICALLY`
        RootMagically = "ROOT_MAGICALLY",
        /// `ROOT_PHYSICALLY`
        RootPhysically = "ROOT_PHYSICALLY",
        /// `SEED_OF_CRITICAL`
        SeedOfCritical = "SEED_OF_CRITICAL",
        /// `SEED_OF_KNIGHT`
        SeedOfKnight = "SEED_OF_KNIGHT",
        /// `SEIZURE_A`
        SeizureA = "SEIZURE_A",
        /// `SEIZURE_B`
        SeizureB = "SEIZURE_B",
        /// `SEIZURE_C`
        SeizureC = "SEIZURE_C",
        /// `SEIZURE_PENALTY`
        SeizurePenalty = "SEIZURE_PENALTY",
        /// `SHIELD_DEFENCE_UP`
        ShieldDefenceUp = "SHIELD_DEFENCE_UP",
        /// `SHIELD_PROB_UP`
        ShieldProbUp = "SHIELD_PROB_UP",
        /// `SIGNAL_A`
        SignalA = "SIGNAL_A",
        /// `SIGNAL_B`
        SignalB = "SIGNAL_B",
        /// `SIGNAL_C`
        SignalC = "SIGNAL_C",
        /// `SIGNAL_D`
        SignalD = "SIGNAL_D",
        /// `SIGNAL_E`
        SignalE = "SIGNAL_E",
        /// `SILENCE`
        Silence = "SILENCE",
        /// `SILENCE_ALL`
        SilenceAll = "SILENCE_ALL",
        /// `SILENCE_PHYSICAL`
        SilencePhysical = "SILENCE_PHYSICAL",
        /// `SKILL_IGNORE`
        SkillIgnore = "SKILL_IGNORE",
        /// `SLEEP`
        Sleep = "SLEEP",
        /// `SNIPE`
        Snipe = "SNIPE",
        /// `SOA_BUFF1`
        SoaBuff1 = "SOA_BUFF1",
        /// `SOA_BUFF2`
        SoaBuff2 = "SOA_BUFF2",
        /// `SOA_BUFF3`
        SoaBuff3 = "SOA_BUFF3",
        /// `SONG_BATTLE_WHISPER`
        SongBattleWhisper = "SONG_BATTLE_WHISPER",
        /// `SONG_OF_CHAMPION`
        SongOfChampion = "SONG_OF_CHAMPION",
        /// `SONG_OF_EARTH`
        SongOfEarth = "SONG_OF_EARTH",
        /// `SONG_OF_ELEMENTAL`
        SongOfElemental = "SONG_OF_ELEMENTAL",
        /// `SONG_OF_FLAME_GUARD`
        SongOfFlameGuard = "SONG_OF_FLAME_GUARD",
        /// `SONG_OF_HUNTER`
        SongOfHunter = "SONG_OF_HUNTER",
        /// `SONG_OF_INVOCATION`
        SongOfInvocation = "SONG_OF_INVOCATION",
        /// `SONG_OF_LIFE`
        SongOfLife = "SONG_OF_LIFE",
        /// `SONG_OF_MEDITATION`
        SongOfMeditation = "SONG_OF_MEDITATION",
        /// `SONG_OF_PURIFICATION`
        SongOfPurification = "SONG_OF_PURIFICATION",
        /// `SONG_OF_RENEWAL`
        SongOfRenewal = "SONG_OF_RENEWAL",
        /// `SONG_OF_STORM_GUARD`
        SongOfStormGuard = "SONG_OF_STORM_GUARD",
        /// `SONG_OF_VENGEANCE`
        SongOfVengeance = "SONG_OF_VENGEANCE",
        /// `SONG_OF_VITALITY`
        SongOfVitality = "SONG_OF_VITALITY",
        /// `SONG_OF_WARDING`
        SongOfWarding = "SONG_OF_WARDING",
        /// `SONG_OF_WATER`
        SongOfWater = "SONG_OF_WATER",
        /// `SONG_OF_WIND`
        SongOfWind = "SONG_OF_WIND",
        /// `SONG_OF_WINDSTORM`
        SongOfWindstorm = "SONG_OF_WINDSTORM",
        /// `SPA_DISEASE_A`
        SpaDiseaseA = "SPA_DISEASE_A",
        /// `SPA_DISEASE_B`
        SpaDiseaseB = "SPA_DISEASE_B",
        /// `SPA_DISEASE_C`
        SpaDiseaseC = "SPA_DISEASE_C",
        /// `SPA_DISEASE_D`
        SpaDiseaseD = "SPA_DISEASE_D",
        /// `SPEED_DOWN`
        SpeedDown = "SPEED_DOWN",
        /// `SPEED_UP`
        SpeedUp = "SPEED_UP",
        /// `SPEED_UP_SPECIAL`
        SpeedUpSpecial = "SPEED_UP_SPECIAL",
        /// `SPITE`
        Spite = "SPITE",
        /// `SPOIL_BOMB`
        SpoilBomb = "SPOIL_BOMB",
        /// `SSQ_TOWN_BLESSING`
        SsqTownBlessing = "SSQ_TOWN_BLESSING",
        /// `SSQ_TOWN_CURSE`
        SsqTownCurse = "SSQ_TOWN_CURSE",
        /// `STAR_FALL`
        StarFall = "STAR_FALL",
        /// `STEALTH`
        Stealth = "STEALTH",
        /// `STIGMA_A`
        StigmaA = "STIGMA_A",
        /// `STIGMA_OF_SILEN`
        StigmaOfSilen = "STIGMA_OF_SILEN",
        /// `STUN`
        Stun = "STUN",
        /// `SUB_TRIGGER_CRT_RATE_UP`
        SubTriggerCrtRateUp = "SUB_TRIGGER_CRT_RATE_UP",
        /// `SUB_TRIGGER_DEFENCE`
        SubTriggerDefence = "SUB_TRIGGER_DEFENCE",
        /// `SUB_TRIGGER_HASTE`
        SubTriggerHaste = "SUB_TRIGGER_HASTE",
        /// `SUB_TRIGGER_SPIRIT`
        SubTriggerSpirit = "SUB_TRIGGER_SPIRIT",
        /// `SUMMON_CONDITION`
        SummonCondition = "SUMMON_CONDITION",
        /// `TALISMAN`
        Talisman = "TALISMAN",
        /// `TARGET_LOCK`
        TargetLock = "TARGET_LOCK",
        /// `THIN_SKIN`
        ThinSkin = "THIN_SKIN",
        /// `THRILL_FIGHT`
        ThrillFight = "THRILL_FIGHT",
        /// `TIME_CHECK`
        TimeCheck = "TIME_CHECK",
        /// `TOUCH_OF_DEATH`
        TouchOfDeath = "TOUCH_OF_DEATH",
        /// `TOUCH_OF_LIFE`
        TouchOfLife = "TOUCH_OF_LIFE",
        /// `TRANSFER_DAMAGE`
        TransferDamage = "TRANSFER_DAMAGE",
        /// `TRANSFORM`
        Transform = "TRANSFORM",
        /// `TRANSFORM_HANGOVER`
        TransformHangover = "TRANSFORM_HANGOVER",
        /// `TRANSFORM_SCRIFICE`
        TransformScrifice = "TRANSFORM_SCRIFICE",
        /// `TRANSFORM_SCRIFICE_P`
        TransformScrificeP = "TRANSFORM_SCRIFICE_P",
        /// `TURN_FLEE`
        TurnFlee = "TURN_FLEE",
        /// `TURN_PASSIVE`
        TurnPassive = "TURN_PASSIVE",
        /// `TURN_STONE`
        TurnStone = "TURN_STONE",
        /// `T_CRT_DMG_DOWN`
        TCrtDmgDown = "T_CRT_DMG_DOWN",
        /// `T_CRT_DMG_UP`
        TCrtDmgUp = "T_CRT_DMG_UP",
        /// `T_CRT_RATE_UP`
        TCrtRateUp = "T_CRT_RATE_UP",
        /// `ULTIMATE_BUFF`
        UltimateBuff = "ULTIMATE_BUFF",
        /// `ULTIMATE_DEBUFF`
        UltimateDebuff = "ULTIMATE_DEBUFF",
        /// `VALAKAS_ITEM`
        ValakasItem = "VALAKAS_ITEM",
        /// `VAMPIRIC_ATTACK`
        VampiricAttack = "VAMPIRIC_ATTACK",
        /// `VAMPIRIC_ATTACK_SPECIAL`
        VampiricAttackSpecial = "VAMPIRIC_ATTACK_SPECIAL",
        /// `VIBRATION`
        Vibration = "VIBRATION",
        /// `VOTE`
        Vote = "VOTE",
        /// `VP_KEEP`
        VpKeep = "VP_KEEP",
        /// `VP_UP`
        VpUp = "VP_UP",
        /// `WATCHER_GAZE`
        WatcherGaze = "WATCHER_GAZE",
        /// `WATER_DOT`
        WaterDot = "WATER_DOT",
        /// `WEAK_CONSTITUTION`
        WeakConstitution = "WEAK_CONSTITUTION",
        /// `WEAPON_MASTERY`
        WeaponMastery = "WEAPON_MASTERY",
        /// `WILL`
        Will = "WILL",
        /// `WIND_DOT`
        WindDot = "WIND_DOT",
        /// `WP_CHANGE_EVENT`
        WpChangeEvent = "WP_CHANGE_EVENT",
    }
}

java_enum! {
    /// Java `CategoryType`.
    CategoryType {
        /// `FIGHTER_GROUP`
        FighterGroup = "FIGHTER_GROUP",
        /// `MAGE_GROUP`
        MageGroup = "MAGE_GROUP",
        /// `WIZARD_GROUP`
        WizardGroup = "WIZARD_GROUP",
        /// `CLERIC_GROUP`
        ClericGroup = "CLERIC_GROUP",
        /// `ATTACKER_GROUP`
        AttackerGroup = "ATTACKER_GROUP",
        /// `TANKER_GROUP`
        TankerGroup = "TANKER_GROUP",
        /// `FIRST_CLASS_GROUP`
        FirstClassGroup = "FIRST_CLASS_GROUP",
        /// `SECOND_CLASS_GROUP`
        SecondClassGroup = "SECOND_CLASS_GROUP",
        /// `THIRD_CLASS_GROUP`
        ThirdClassGroup = "THIRD_CLASS_GROUP",
        /// `FOURTH_CLASS_GROUP`
        FourthClassGroup = "FOURTH_CLASS_GROUP",
        /// `BOUNTY_HUNTER_GROUP`
        BountyHunterGroup = "BOUNTY_HUNTER_GROUP",
        /// `WARSMITH_GROUP`
        WarsmithGroup = "WARSMITH_GROUP",
        /// `SUMMON_NPC_GROUP`
        SummonNpcGroup = "SUMMON_NPC_GROUP",
        /// `KNIGHT_GROUP`
        KnightGroup = "KNIGHT_GROUP",
        /// `WHITE_MAGIC_GROUP`
        WhiteMagicGroup = "WHITE_MAGIC_GROUP",
        /// `HEAL_GROUP`
        HealGroup = "HEAL_GROUP",
        /// `ASSIST_MAGIC_GROUP`
        AssistMagicGroup = "ASSIST_MAGIC_GROUP",
        /// `WARRIOR_GROUP`
        WarriorGroup = "WARRIOR_GROUP",
        /// `HUMAN_2ND_GROUP`
        Human2ndGroup = "HUMAN_2ND_GROUP",
        /// `ELF_2ND_GROUP`
        Elf2ndGroup = "ELF_2ND_GROUP",
        /// `DELF_2ND_GROUP`
        Delf2ndGroup = "DELF_2ND_GROUP",
        /// `ORC_2ND_GROUP`
        Orc2ndGroup = "ORC_2ND_GROUP",
        /// `DWARF_2ND_GROUP`
        Dwarf2ndGroup = "DWARF_2ND_GROUP",
        /// `STRIDER`
        Strider = "STRIDER",
        /// `STRIDER_GROUP`
        StriderGroup = "STRIDER_GROUP",
        /// `RED_STRIDER_GROUP`
        RedStriderGroup = "RED_STRIDER_GROUP",
        /// `WOLF_GROUP`
        WolfGroup = "WOLF_GROUP",
        /// `GROWN_UP_WOLF_GROUP`
        GrownUpWolfGroup = "GROWN_UP_WOLF_GROUP",
        /// `HATCHLING_GROUP`
        HatchlingGroup = "HATCHLING_GROUP",
        /// `BABY_PET_GROUP`
        BabyPetGroup = "BABY_PET_GROUP",
        /// `UPGRADE_BABY_PET_GROUP`
        UpgradeBabyPetGroup = "UPGRADE_BABY_PET_GROUP",
        /// `WYVERN_GROUP`
        WyvernGroup = "WYVERN_GROUP",
        /// `ALL_WOLF_GROUP`
        AllWolfGroup = "ALL_WOLF_GROUP",
        /// `WOLF`
        Wolf = "WOLF",
        /// `SIN_EATER_GROUP`
        SinEaterGroup = "SIN_EATER_GROUP",
        /// `PET_GROUP`
        PetGroup = "PET_GROUP",
        /// `ITEM_EQUIP_PET_GROUP`
        ItemEquipPetGroup = "ITEM_EQUIP_PET_GROUP",
        /// `SUBJOB_GROUP_DAGGER`
        SubjobGroupDagger = "SUBJOB_GROUP_DAGGER",
        /// `SUBJOB_GROUP_BOW`
        SubjobGroupBow = "SUBJOB_GROUP_BOW",
        /// `SUBJOB_GROUP_KNIGHT`
        SubjobGroupKnight = "SUBJOB_GROUP_KNIGHT",
        /// `SUBJOB_GROUP_SUMMONER`
        SubjobGroupSummoner = "SUBJOB_GROUP_SUMMONER",
        /// `SUBJOB_GROUP_HALF_HEALER`
        SubjobGroupHalfHealer = "SUBJOB_GROUP_HALF_HEALER",
        /// `SUBJOB_GROUP_DANCE`
        SubjobGroupDance = "SUBJOB_GROUP_DANCE",
        /// `SUBJOB_GROUP_WIZARD`
        SubjobGroupWizard = "SUBJOB_GROUP_WIZARD",
        /// `HUMAN_FALL_CLASS`
        HumanFallClass = "HUMAN_FALL_CLASS",
        /// `HUMAN_WALL_CLASS`
        HumanWallClass = "HUMAN_WALL_CLASS",
        /// `HUMAN_MALL_CLASS`
        HumanMallClass = "HUMAN_MALL_CLASS",
        /// `HUMAN_CALL_CLASS`
        HumanCallClass = "HUMAN_CALL_CLASS",
        /// `ELF_FALL_CLASS`
        ElfFallClass = "ELF_FALL_CLASS",
        /// `ELF_MALL_CLASS`
        ElfMallClass = "ELF_MALL_CLASS",
        /// `ELF_WALL_CLASS`
        ElfWallClass = "ELF_WALL_CLASS",
        /// `ELF_CALL_CLASS`
        ElfCallClass = "ELF_CALL_CLASS",
        /// `DELF_FALL_CLASS`
        DelfFallClass = "DELF_FALL_CLASS",
        /// `DELF_MALL_CLASS`
        DelfMallClass = "DELF_MALL_CLASS",
        /// `DELF_WALL_CLASS`
        DelfWallClass = "DELF_WALL_CLASS",
        /// `DELF_CALL_CLASS`
        DelfCallClass = "DELF_CALL_CLASS",
        /// `ORC_FALL_CLASS`
        OrcFallClass = "ORC_FALL_CLASS",
        /// `ORC_MALL_CLASS`
        OrcMallClass = "ORC_MALL_CLASS",
        /// `DWARF_ALL_CLASS`
        DwarfAllClass = "DWARF_ALL_CLASS",
        /// `DWARF_BOUNTY_CLASS`
        DwarfBountyClass = "DWARF_BOUNTY_CLASS",
        /// `DWARF_SMITH_CLASS`
        DwarfSmithClass = "DWARF_SMITH_CLASS",
        /// `KAMAEL_ALL_CLASS`
        KamaelAllClass = "KAMAEL_ALL_CLASS",
        /// `KAMAEL_FIRST_CLASS_GROUP`
        KamaelFirstClassGroup = "KAMAEL_FIRST_CLASS_GROUP",
        /// `KAMAEL_SECOND_CLASS_GROUP`
        KamaelSecondClassGroup = "KAMAEL_SECOND_CLASS_GROUP",
        /// `KAMAEL_THIRD_CLASS_GROUP`
        KamaelThirdClassGroup = "KAMAEL_THIRD_CLASS_GROUP",
        /// `KAMAEL_FOURTH_CLASS_GROUP`
        KamaelFourthClassGroup = "KAMAEL_FOURTH_CLASS_GROUP",
        /// `BEGINNER_FIGHTER`
        BeginnerFighter = "BEGINNER_FIGHTER",
        /// `BEGINNER_MAGE`
        BeginnerMage = "BEGINNER_MAGE",
        /// `KAMAEL_MALE_MAIN_OCCUPATION`
        KamaelMaleMainOccupation = "KAMAEL_MALE_MAIN_OCCUPATION",
        /// `KAMAEL_FEMALE_MAIN_OCCUPATION`
        KamaelFemaleMainOccupation = "KAMAEL_FEMALE_MAIN_OCCUPATION",
        /// `ARCHER_GROUP`
        ArcherGroup = "ARCHER_GROUP",
        /// `SHIELD_MASTER`
        ShieldMaster = "SHIELD_MASTER",
        /// `BARD`
        Bard = "BARD",
        /// `FORCE_MASTER`
        ForceMaster = "FORCE_MASTER",
        /// `WEAPON_MASTER`
        WeaponMaster = "WEAPON_MASTER",
        /// `BOW_MASTER`
        BowMaster = "BOW_MASTER",
        /// `DAGGER_MASTER`
        DaggerMaster = "DAGGER_MASTER",
        /// `HEAL_MASTER`
        HealMaster = "HEAL_MASTER",
        /// `WIZARD_MASTER`
        WizardMaster = "WIZARD_MASTER",
        /// `BUFF_MASTER`
        BuffMaster = "BUFF_MASTER",
        /// `SUMMON_MASTER`
        SummonMaster = "SUMMON_MASTER",
        /// `WARRIOR_CLOACK`
        WarriorCloack = "WARRIOR_CLOACK",
        /// `ROGUE_CLOACK`
        RogueCloack = "ROGUE_CLOACK",
        /// `MAGE_CLOACK`
        MageCloack = "MAGE_CLOACK",
        /// `SHIELD_MASTER2_3`
        ShieldMaster23 = "SHIELD_MASTER2_3",
        /// `BARD2_3`
        Bard23 = "BARD2_3",
        /// `FORCE_MASTER2_3`
        ForceMaster23 = "FORCE_MASTER2_3",
        /// `WEAPON_MASTER2_3`
        WeaponMaster23 = "WEAPON_MASTER2_3",
        /// `BOW_MASTER2_3`
        BowMaster23 = "BOW_MASTER2_3",
        /// `DAGGER_MASTER2_3`
        DaggerMaster23 = "DAGGER_MASTER2_3",
        /// `HEAL_MASTER2_3`
        HealMaster23 = "HEAL_MASTER2_3",
        /// `WIZARD_MASTER2_3`
        WizardMaster23 = "WIZARD_MASTER2_3",
        /// `BUFF_MASTER2_3`
        BuffMaster23 = "BUFF_MASTER2_3",
        /// `SUMMON_MASTER2_3`
        SummonMaster23 = "SUMMON_MASTER2_3",
        /// `ATTRIBUTE_GROUP_SUMMONER`
        AttributeGroupSummoner = "ATTRIBUTE_GROUP_SUMMONER",
        /// `SUB_GROUP_WARRIOR`
        SubGroupWarrior = "SUB_GROUP_WARRIOR",
        /// `SUB_GROUP_ROGUE`
        SubGroupRogue = "SUB_GROUP_ROGUE",
        /// `SUB_GROUP_KNIGHT`
        SubGroupKnight = "SUB_GROUP_KNIGHT",
        /// `SUB_GROUP_SUMMONER`
        SubGroupSummoner = "SUB_GROUP_SUMMONER",
        /// `SUB_GROUP_WIZARD`
        SubGroupWizard = "SUB_GROUP_WIZARD",
        /// `SUB_GROUP_HEALER`
        SubGroupHealer = "SUB_GROUP_HEALER",
        /// `SUB_GROUP_ENCHANTER`
        SubGroupEnchanter = "SUB_GROUP_ENCHANTER",
        /// `SUB_GROUP_HEC`
        SubGroupHec = "SUB_GROUP_HEC",
        /// `SUB_GROUP_HEW`
        SubGroupHew = "SUB_GROUP_HEW",
        /// `SUB_GROUP_HEF`
        SubGroupHef = "SUB_GROUP_HEF",
        /// `SUB_GROUP_ORC`
        SubGroupOrc = "SUB_GROUP_ORC",
        /// `SUB_GROUP_WARE`
        SubGroupWare = "SUB_GROUP_WARE",
        /// `SUB_GROUP_BLACK`
        SubGroupBlack = "SUB_GROUP_BLACK",
        /// `SUB_GROUP_DE`
        SubGroupDe = "SUB_GROUP_DE",
        /// `SUB_GROUP_KAMAEL`
        SubGroupKamael = "SUB_GROUP_KAMAEL",
        /// `LIGHT_TANKER_GROUP`
        LightTankerGroup = "LIGHT_TANKER_GROUP",
        /// `DARK_TANKER_GROUP`
        DarkTankerGroup = "DARK_TANKER_GROUP",
        /// `MELEE_ATTACKER`
        MeleeAttacker = "MELEE_ATTACKER",
        /// `RECOM_KNIGHT_GROUP`
        RecomKnightGroup = "RECOM_KNIGHT_GROUP",
        /// `RECOM_MAGIC_GROUP`
        RecomMagicGroup = "RECOM_MAGIC_GROUP",
        /// `RECOM_WARRIOR_GROUP`
        RecomWarriorGroup = "RECOM_WARRIOR_GROUP",
        /// `RECOM_ROGUE_GROUP`
        RecomRogueGroup = "RECOM_ROGUE_GROUP",
        /// `RECOM_KAMAEL_GROUP`
        RecomKamaelGroup = "RECOM_KAMAEL_GROUP",
        /// `RECOM_ORCF_GROUP`
        RecomOrcfGroup = "RECOM_ORCF_GROUP",
        /// `RECOM_ORCM_GROUP`
        RecomOrcmGroup = "RECOM_ORCM_GROUP",
        /// `DEINONYCHUS_PET_GROUP`
        DeinonychusPetGroup = "DEINONYCHUS_PET_GROUP",
        /// `BEASTFARM_BEAST`
        BeastfarmBeast = "BEASTFARM_BEAST",
        /// `BEASTFARM_INVADER`
        BeastfarmInvader = "BEASTFARM_INVADER",
        /// `ICEQUEEN_NPC`
        IcequeenNpc = "ICEQUEEN_NPC",
        /// `TANKER_CATEGORY`
        TankerCategory = "TANKER_CATEGORY",
        /// `WARRIOR_CATEGORY`
        WarriorCategory = "WARRIOR_CATEGORY",
        /// `ROGUE_CATEGORY`
        RogueCategory = "ROGUE_CATEGORY",
        /// `ARCHER_CATEGORY`
        ArcherCategory = "ARCHER_CATEGORY",
        /// `WIZARD_CATEGORY`
        WizardCategory = "WIZARD_CATEGORY",
        /// `SUMMONER_CATEGORY`
        SummonerCategory = "SUMMONER_CATEGORY",
        /// `HEALER_CATEGORY`
        HealerCategory = "HEALER_CATEGORY",
        /// `APM_ZERO_WARRIOR_GROUP`
        ApmZeroWarriorGroup = "APM_ZERO_WARRIOR_GROUP",
        /// `APM_ZERO_MAGE_GROUP`
        ApmZeroMageGroup = "APM_ZERO_MAGE_GROUP",
        /// `APM_1ST_KNIGHT_GROUP`
        Apm1stKnightGroup = "APM_1ST_KNIGHT_GROUP",
        /// `APM_1ST_WARRIOR_GROUP`
        Apm1stWarriorGroup = "APM_1ST_WARRIOR_GROUP",
        /// `APM_1ST_WIZARD_GROUP`
        Apm1stWizardGroup = "APM_1ST_WIZARD_GROUP",
        /// `APM_1ST_HEALER_GROUP`
        Apm1stHealerGroup = "APM_1ST_HEALER_GROUP",
        /// `APM_2ND_KNIGHT_GROUP`
        Apm2ndKnightGroup = "APM_2ND_KNIGHT_GROUP",
        /// `APM_2ND_WARRIOR_GROUP`
        Apm2ndWarriorGroup = "APM_2ND_WARRIOR_GROUP",
        /// `APM_2ND_ROGUE_GROUP`
        Apm2ndRogueGroup = "APM_2ND_ROGUE_GROUP",
        /// `APM_2ND_WIZARD_GROUP`
        Apm2ndWizardGroup = "APM_2ND_WIZARD_GROUP",
        /// `APM_2ND_ARCHER_GROUP`
        Apm2ndArcherGroup = "APM_2ND_ARCHER_GROUP",
        /// `APM_2ND_BUFFER_GROUP`
        Apm2ndBufferGroup = "APM_2ND_BUFFER_GROUP",
        /// `APM_2ND_SUMMONER_GROUP`
        Apm2ndSummonerGroup = "APM_2ND_SUMMONER_GROUP",
        /// `APM_2ND_ONLY_GROUP`
        Apm2ndOnlyGroup = "APM_2ND_ONLY_GROUP",
        /// `APM_3RD_KNIGHT_GROUP`
        Apm3rdKnightGroup = "APM_3RD_KNIGHT_GROUP",
        /// `APM_3RD_WARRIOR_GROUP`
        Apm3rdWarriorGroup = "APM_3RD_WARRIOR_GROUP",
        /// `APM_3RD_ROGUE_GROUP`
        Apm3rdRogueGroup = "APM_3RD_ROGUE_GROUP",
        /// `APM_3RD_WIZARD_GROUP`
        Apm3rdWizardGroup = "APM_3RD_WIZARD_GROUP",
        /// `APM_3RD_ARCHER_GROUP`
        Apm3rdArcherGroup = "APM_3RD_ARCHER_GROUP",
        /// `APM_3RD_BUFFER_GROUP`
        Apm3rdBufferGroup = "APM_3RD_BUFFER_GROUP",
        /// `APM_3RD_SUMMONER_GROUP`
        Apm3rdSummonerGroup = "APM_3RD_SUMMONER_GROUP",
        /// `APM_3RD_ONLY_GROUP`
        Apm3rdOnlyGroup = "APM_3RD_ONLY_GROUP",
        /// `SUBCLASS_TANKER`
        SubclassTanker = "SUBCLASS_TANKER",
        /// `SUBCLASS_WARRIOR`
        SubclassWarrior = "SUBCLASS_WARRIOR",
        /// `SUBCLASS_ROUGE`
        SubclassRouge = "SUBCLASS_ROUGE",
        /// `SUBCLASS_ARCHER`
        SubclassArcher = "SUBCLASS_ARCHER",
        /// `SUBCLASS_WIZARD`
        SubclassWizard = "SUBCLASS_WIZARD",
        /// `SUBCLASS_SUMMON`
        SubclassSummon = "SUBCLASS_SUMMON",
        /// `SUBCLASS_HEAL`
        SubclassHeal = "SUBCLASS_HEAL",
        /// `SUBCLASS_ENCHANTER`
        SubclassEnchanter = "SUBCLASS_ENCHANTER",
        /// `SUBCLASS_SPEACIAL`
        SubclassSpeacial = "SUBCLASS_SPEACIAL",
        /// `SUBCLASS_KAMAEL`
        SubclassKamael = "SUBCLASS_KAMAEL",
        /// `EXCEPT_KAMAEL_GROUP`
        ExceptKamaelGroup = "EXCEPT_KAMAEL_GROUP",
        /// `DIVISION_TANKER`
        DivisionTanker = "DIVISION_TANKER",
        /// `DIVISION_WARRIOR`
        DivisionWarrior = "DIVISION_WARRIOR",
        /// `DIVISION_ROUGE`
        DivisionRouge = "DIVISION_ROUGE",
        /// `DIVISION_ARCHER`
        DivisionArcher = "DIVISION_ARCHER",
        /// `DIVISION_WIZARD`
        DivisionWizard = "DIVISION_WIZARD",
        /// `DIVISION_SUMMON`
        DivisionSummon = "DIVISION_SUMMON",
        /// `DIVISION_ENCHANTER`
        DivisionEnchanter = "DIVISION_ENCHANTER",
        /// `DIVISION_HEAL`
        DivisionHeal = "DIVISION_HEAL",
        /// `HUMAN_FOURTH_CLASS_GROUP`
        HumanFourthClassGroup = "HUMAN_FOURTH_CLASS_GROUP",
        /// `ELF_FOURTH_CLASS_GROUP`
        ElfFourthClassGroup = "ELF_FOURTH_CLASS_GROUP",
        /// `DARKELF_FOURTH_CLASS_GROUP`
        DarkelfFourthClassGroup = "DARKELF_FOURTH_CLASS_GROUP",
        /// `ORC_FOURTH_CLASS_GROUP`
        OrcFourthClassGroup = "ORC_FOURTH_CLASS_GROUP",
        /// `DWARF_FOURTH_CLASS_GROUP`
        DwarfFourthClassGroup = "DWARF_FOURTH_CLASS_GROUP",
        /// `HEAVY_ARMOR_CLASS`
        HeavyArmorClass = "HEAVY_ARMOR_CLASS",
        /// `LIGHT_ARMOR_CLASS`
        LightArmorClass = "LIGHT_ARMOR_CLASS",
        /// `ROBE_CLASS`
        RobeClass = "ROBE_CLASS",
    }
}

java_enum! {
    /// Java `InstanceType` (the constant names are camel case in Java too).
    InstanceType {
        /// `WorldObject`
        WorldObject = "WorldObject",
        /// `Item`
        Item = "Item",
        /// `Creature`
        Creature = "Creature",
        /// `Npc`
        Npc = "Npc",
        /// `Playable`
        Playable = "Playable",
        /// `Summon`
        Summon = "Summon",
        /// `Player`
        Player = "Player",
        /// `Folk`
        Folk = "Folk",
        /// `Merchant`
        Merchant = "Merchant",
        /// `Warehouse`
        Warehouse = "Warehouse",
        /// `StaticObject`
        StaticObject = "StaticObject",
        /// `Door`
        Door = "Door",
        /// `TerrainObject`
        TerrainObject = "TerrainObject",
        /// `EffectPoint`
        EffectPoint = "EffectPoint",
        /// `Servitor`
        Servitor = "Servitor",
        /// `Pet`
        Pet = "Pet",
        /// `BabyPet`
        BabyPet = "BabyPet",
        /// `Decoy`
        Decoy = "Decoy",
        /// `Trap`
        Trap = "Trap",
        /// `Attackable`
        Attackable = "Attackable",
        /// `Guard`
        Guard = "Guard",
        /// `QuestGuard`
        QuestGuard = "QuestGuard",
        /// `Monster`
        Monster = "Monster",
        /// `Chest`
        Chest = "Chest",
        /// `FeedableBeast`
        FeedableBeast = "FeedableBeast",
        /// `TamedBeast`
        TamedBeast = "TamedBeast",
        /// `FriendlyMob`
        FriendlyMob = "FriendlyMob",
        /// `RiftInvader`
        RiftInvader = "RiftInvader",
        /// `RaidBoss`
        RaidBoss = "RaidBoss",
        /// `GrandBoss`
        GrandBoss = "GrandBoss",
        /// `FlyNpc`
        FlyNpc = "FlyNpc",
        /// `FlyMonster`
        FlyMonster = "FlyMonster",
        /// `FlyRaidBoss`
        FlyRaidBoss = "FlyRaidBoss",
        /// `FlyTerrainObject`
        FlyTerrainObject = "FlyTerrainObject",
        /// `FestivalGuide`
        FestivalGuide = "FestivalGuide",
        /// `FestivalMonster`
        FestivalMonster = "FestivalMonster",
        /// `Vehicle`
        Vehicle = "Vehicle",
        /// `Boat`
        Boat = "Boat",
        /// `AirShip`
        AirShip = "AirShip",
        /// `ControllableAirShip`
        ControllableAirShip = "ControllableAirShip",
        /// `Defender`
        Defender = "Defender",
        /// `Artefact`
        Artefact = "Artefact",
        /// `ControlTower`
        ControlTower = "ControlTower",
        /// `FlameTower`
        FlameTower = "FlameTower",
        /// `SiegeFlag`
        SiegeFlag = "SiegeFlag",
        /// `FortCommander`
        FortCommander = "FortCommander",
        /// `FortLogistics`
        FortLogistics = "FortLogistics",
        /// `FortManager`
        FortManager = "FortManager",
        /// `SignsPriest`
        SignsPriest = "SignsPriest",
        /// `DawnPriest`
        DawnPriest = "DawnPriest",
        /// `DuskPriest`
        DuskPriest = "DuskPriest",
        /// `DungeonGatekeeper`
        DungeonGatekeeper = "DungeonGatekeeper",
        /// `Adventurer`
        Adventurer = "Adventurer",
        /// `Auctioneer`
        Auctioneer = "Auctioneer",
        /// `BroadcastingTower`
        BroadcastingTower = "BroadcastingTower",
        /// `ClanHallManager`
        ClanHallManager = "ClanHallManager",
        /// `Fisherman`
        Fisherman = "Fisherman",
        /// `OlympiadManager`
        OlympiadManager = "OlympiadManager",
        /// `PetManager`
        PetManager = "PetManager",
        /// `RaceManager`
        RaceManager = "RaceManager",
        /// `Teleporter`
        Teleporter = "Teleporter",
        /// `Trainer`
        Trainer = "Trainer",
        /// `VillageMaster`
        VillageMaster = "VillageMaster",
        /// `Doorman`
        Doorman = "Doorman",
        /// `CastleDoorman`
        CastleDoorman = "CastleDoorman",
        /// `FortDoorman`
        FortDoorman = "FortDoorman",
        /// `ClanHallDoorman`
        ClanHallDoorman = "ClanHallDoorman",
        /// `ClassMaster`
        ClassMaster = "ClassMaster",
        /// `SchemeBuffer`
        SchemeBuffer = "SchemeBuffer",
        /// `EventMob`
        EventMob = "EventMob",
        /// `UCManagerInstance`
        UCManagerInstance = "UCManagerInstance",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip_and_are_case_sensitive() {
        for &t in AbnormalType::ALL {
            assert_eq!(AbnormalType::from_name(t.name()), Some(t));
        }
        for &t in CategoryType::ALL {
            assert_eq!(CategoryType::from_name(t.name()), Some(t));
        }
        for &t in InstanceType::ALL {
            assert_eq!(InstanceType::from_name(t.name()), Some(t));
        }
        assert_eq!(AbnormalType::ALL.len(), 337);
        assert_eq!(CategoryType::ALL.len(), 180);
        assert_eq!(InstanceType::ALL.len(), 71);
        assert_eq!(
            AbnormalType::from_name("ABILITY_CHANGE"),
            Some(AbnormalType::AbilityChange)
        );
        assert_eq!(AbnormalType::from_name("ability_change"), None);
        assert_eq!(InstanceType::from_name("Folk"), Some(InstanceType::Folk));
        assert_eq!(InstanceType::from_name("folk"), None);
        assert_eq!(PlayerState::from_name("OLYMPIAD"), Some(PlayerState::Olympiad));
    }
}
