//! Quest 335: The Song of the Hunter (Grey sets two tests for a hunter's license, then Tor hands
//! out the requests of the hunter's circles and Cybellin turns a blood crystal into adena).
// Java: dist/game/data/scripts/quests/Q00335_TheSongOfTheHunter/Q00335_TheSongOfTheHunter.java
//
// Pattern: a repeatable quest driven by tables (`REWARDS`, `DROPLIST`), a memo state that packs
// five list entries of 5 bits, and an html page with `<?replyN?>` markers
// (`docs/design/scripts.md`, "Quest patterns": kill count with a drop chance).

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::script::{QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const GREY: i32 = 30744;
const TOR: i32 = 30745;
const CYBELLIN: i32 = 30746;

// Monsters
const BREKA_ORC_SHAMAN: i32 = 20269;
const BREKA_ORC_WARRIOR: i32 = 20271;
const GUARDIAN_BASILISK: i32 = 20550;
const FETTERED_SOUL: i32 = 20552;
const WINDSUS: i32 = 20553;
const GRANDIS: i32 = 20554;
const GIANT_FUNGUS: i32 = 20555;
const GIANT_MONSTEREYE: i32 = 20556;
const DIRE_WYRM: i32 = 20557;
const ROTTING_TREE: i32 = 20558;
const TRISALIM_SPIDER: i32 = 20560;
const TRISALIM_TARANTULA: i32 = 20561;
const SPORE_ZOMBIE: i32 = 20562;
const MANASHEN_GARGOYLE: i32 = 20563;
const ENCHANTED_STONE_GOLEM: i32 = 20565;
const ENCHANTED_GARGOYLE: i32 = 20567;
const TARLK_BUGBEAR_WARRIOR: i32 = 20571;
const LETO_LIZARDMAN_ARCHER: i32 = 20578;
const LETO_LIZARDMAN_SOLDIER: i32 = 20579;
const LETO_LIZARDMAN_SHAMAN: i32 = 20581;
const LETO_LIZARDMAN_OVERLORD: i32 = 20582;
const TIMAK_ORC_WARRIOR: i32 = 20586;
const TIMAK_ORC_OVERLORD: i32 = 20588;
const FLINE: i32 = 20589;
const LIELE: i32 = 20590;
const VALLEY_TREANT: i32 = 20591;
const SATYR: i32 = 20592;
const UNICORN: i32 = 20593;
const FOREST_RUNNER: i32 = 20594;
const VALLEY_TREANT_ELDER: i32 = 20597;
const SATYR_ELDER: i32 = 20598;
const UNICORN_ELDER: i32 = 20599;
const KARUL_BUGBEAR: i32 = 20600;
const TAMLIN_ORC: i32 = 20601;
const TAMLIN_ORC_ARCHER: i32 = 20602;
const KRONBE_SPIDER: i32 = 20603;
const TAIK_ORC_ARCHER: i32 = 20631;
const TAIK_ORC_WARRIOR: i32 = 20632;
const TAIK_ORC_SHAMAN: i32 = 20633;
const TAIK_ORC_CAPTAIN: i32 = 20634;
const MIRROR: i32 = 20639;
const HARIT_LIZARDMAN_GRUNT: i32 = 20641;
const HARIT_LIZARDMAN_ARCHER: i32 = 20642;
const HARIT_LIZARDMAN_WARRIOR: i32 = 20643;
const GRAVE_WANDERER: i32 = 20659;
const ARCHER_OF_GREED: i32 = 20660;
const HATAR_RATMAN_THIEF: i32 = 20661;
const HATAR_RATMAN_BOSS: i32 = 20662;
const DEPRIVE: i32 = 20664;
const FARCRAN: i32 = 20667;
const TAIRIM: i32 = 20675;
const JUDGE_OF_MARSH: i32 = 20676;
const VANOR_SILENOS_GRUNT: i32 = 20682;
const VANOR_SILENOS_SCOUT: i32 = 20683;
const VANOR_SILENOS_WARRIOR: i32 = 20684;
const VANOR_SILENOS_CHIEFTAIN: i32 = 20686;
const BREKA_OVERLORD_HAKA: i32 = 27140;
const BREKA_OVERLORD_JAKA: i32 = 27141;
const BREKA_OVERLORD_MARKA: i32 = 27142;
const WINDSUS_ALEPH: i32 = 27143;
const TARLK_RAIDER_ATHU: i32 = 27144;
const TARLK_RAIDER_LANKA: i32 = 27145;
const TARLK_RAIDER_TRISKA: i32 = 27146;
const TARLK_RAIDER_MOTURA: i32 = 27147;
const TARLK_RAIDER_KALATH: i32 = 27148;
const GREMLIN_FILCHER: i32 = 27149;
const BLACK_LEGION_STORMTROOPER: i32 = 27150;
const LETO_SHAMAN_KETZ: i32 = 27156;
const LETO_CHIEF_NARAK: i32 = 27157;
const TIMAK_RAIDER_KAIKEE: i32 = 27158;
const TIMAK_OVERLORD_OKUN: i32 = 27159;
const GOK_MAGOK: i32 = 27160;
const TAIK_OVERLORD_KAKRAN: i32 = 27161;
const HATAR_CHIEFTAIN_KUBEL: i32 = 27162;
const VANOR_ELDER_KERUNOS: i32 = 27163;
const KARUL_CHIEF_OROOTO: i32 = 27164;

// Misc
const MIN_LEVEL: i32 = 35;
const MAX_LEVEL: i32 = 45;
const CYBELLINS_DAGGER: i32 = 3471;
const FIRST_CIRCLE_HUNTER_LICENSE: i32 = 3692;
const SECOND_CIRCLE_HUNTER_LICENSE: i32 = 3693;
const LAUREL_LEAF_PIN: i32 = 3694;
const TEST_INSTRUCTIONS_1: i32 = 3695;
const TEST_INSTRUCTIONS_2: i32 = 3696;
const CYBELLINS_REQUEST: i32 = 3697;
const BLOOD_CRYSTAL_PURITY_1: i32 = 3698;
const BLOOD_CRYSTAL_PURITY_2: i32 = 3699;
const BLOOD_CRYSTAL_PURITY_3: i32 = 3700;
const BLOOD_CRYSTAL_PURITY_4: i32 = 3701;
const BLOOD_CRYSTAL_PURITY_5: i32 = 3702;
const BLOOD_CRYSTAL_PURITY_6: i32 = 3703;
const BLOOD_CRYSTAL_PURITY_7: i32 = 3704;
const BLOOD_CRYSTAL_PURITY_8: i32 = 3705;
const BLOOD_CRYSTAL_PURITY_9: i32 = 3706;
const BLOOD_CRYSTAL_PURITY_10: i32 = 3707;
const BROKEN_BLOOD_CRYSTAL: i32 = 3708;
const GUARDIAN_BASILISK_SCALE: i32 = 3709;
const KARUT_WEED: i32 = 3710;
const HAKAS_HEAD: i32 = 3711;
const JAKAS_HEAD: i32 = 3712;
const MARKAS_HEAD: i32 = 3713;
const WINDSUS_ALEPH_SKIN: i32 = 3714;
const INDIGO_SPIRIT_ORE: i32 = 3715;
const SPORESEA_SEED: i32 = 3716;
const TIMAK_ORC_TOTEM: i32 = 3717;
const TRISALIM_SILK: i32 = 3718;
const AMBROSIUS_FRUIT: i32 = 3719;
const BALEFIRE_CRYSTAL: i32 = 3720;
const IMPERIAL_ARROWHEAD: i32 = 3721;
const ATHUS_HEAD: i32 = 3722;
const LANKAS_HEAD: i32 = 3723;
const TRISKAS_HEAD: i32 = 3724;
const MOTURAS_HEAD: i32 = 3725;
const KALATHS_HEAD: i32 = 3726;
const FIRST_CIRCLE_REQUEST_1C: i32 = 3727;
const FIRST_CIRCLE_REQUEST_2C: i32 = 3728;
const FIRST_CIRCLE_REQUEST_3C: i32 = 3729;
const FIRST_CIRCLE_REQUEST_4C: i32 = 3730;
const FIRST_CIRCLE_REQUEST_5C: i32 = 3731;
const FIRST_CIRCLE_REQUEST_6C: i32 = 3732;
const FIRST_CIRCLE_REQUEST_7C: i32 = 3733;
const FIRST_CIRCLE_REQUEST_8C: i32 = 3734;
const FIRST_CIRCLE_REQUEST_9C: i32 = 3735;
const FIRST_CIRCLE_REQUEST_10C: i32 = 3736;
const FIRST_CIRCLE_REQUEST_11C: i32 = 3737;
const FIRST_CIRCLE_REQUEST_12C: i32 = 3738;
const FIRST_CIRCLE_REQUEST_1B: i32 = 3739;
const FIRST_CIRCLE_REQUEST_2B: i32 = 3740;
const FIRST_CIRCLE_REQUEST_3B: i32 = 3741;
const FIRST_CIRCLE_REQUEST_4B: i32 = 3742;
const FIRST_CIRCLE_REQUEST_5B: i32 = 3743;
const FIRST_CIRCLE_REQUEST_6B: i32 = 3744;
const FIRST_CIRCLE_REQUEST_1A: i32 = 3745;
const FIRST_CIRCLE_REQUEST_2A: i32 = 3746;
const FIRST_CIRCLE_REQUEST_3A: i32 = 3747;
const SECOND_CIRCLE_REQUEST_1C: i32 = 3748;
const SECOND_CIRCLE_REQUEST_2C: i32 = 3749;
const SECOND_CIRCLE_REQUEST_3C: i32 = 3750;
const SECOND_CIRCLE_REQUEST_4C: i32 = 3751;
const SECOND_CIRCLE_REQUEST_5C: i32 = 3752;
const SECOND_CIRCLE_REQUEST_6C: i32 = 3753;
const SECOND_CIRCLE_REQUEST_7C: i32 = 3754;
const SECOND_CIRCLE_REQUEST_8C: i32 = 3755;
const SECOND_CIRCLE_REQUEST_9C: i32 = 3756;
const SECOND_CIRCLE_REQUEST_10C: i32 = 3757;
const SECOND_CIRCLE_REQUEST_11C: i32 = 3758;
const SECOND_CIRCLE_REQUEST_12C: i32 = 3759;
const SECOND_CIRCLE_REQUEST_1B: i32 = 3760;
const SECOND_CIRCLE_REQUEST_2B: i32 = 3761;
const SECOND_CIRCLE_REQUEST_3B: i32 = 3762;
const SECOND_CIRCLE_REQUEST_4B: i32 = 3763;
const SECOND_CIRCLE_REQUEST_5B: i32 = 3764;
const SECOND_CIRCLE_REQUEST_6B: i32 = 3765;
const SECOND_CIRCLE_REQUEST_1A: i32 = 3766;
const SECOND_CIRCLE_REQUEST_2A: i32 = 3767;
const SECOND_CIRCLE_REQUEST_3A: i32 = 3768;
const CHARM_OF_KADESH: i32 = 3769;
const TIMAK_JADE_NECKLACE: i32 = 3770;
const ENCHANTED_GOLEM_SHARD: i32 = 3771;
const GIANT_MONSTER_EYE_MEAT: i32 = 3772;
const DIRE_WYRM_EGG: i32 = 3773;
const GUARDIAN_BASILISK_TALON: i32 = 3774;
const REVENANTS_CHAINS: i32 = 3775;
const WINDSUS_TUSK: i32 = 3776;
const GRANDISS_SKULL: i32 = 3777;
const TAIK_OBSIDIAN_AMULET: i32 = 3778;
const KARUL_BUGBEAR_HEAD: i32 = 3779;
const TAMLIN_IVORY_CHARM: i32 = 3780;
const FANG_OF_NARAK: i32 = 3781;
const ENCHANTED_GARGOYLES_HORN: i32 = 3782;
const COILED_SERPENT_TOTEM: i32 = 3783;
const TOTEM_OF_KADESH: i32 = 3784;
const KAIKIS_HEAD: i32 = 3785;
const KRONBE_VENOM_SAC: i32 = 3786;
const EVAS_CHARM: i32 = 3787;
const TITANS_TABLET: i32 = 3788;
const BOOK_OF_SHUNAIMAN: i32 = 3789;
const ROTTING_TREE_SPORES: i32 = 3790;
const TRISALIM_VENOM_SAC: i32 = 3791;
const TAIK_ORC_TOTEM: i32 = 3792;
const HARIT_BARBED_NECKLACE: i32 = 3793;
const COIN_OF_OLD_EMPIRE: i32 = 3794;
const SKIN_OF_FARCRAN: i32 = 3795;
const TEMPEST_SHARD: i32 = 3796;
const TSUNAMI_SHARD: i32 = 3797;
const SATYR_MANE: i32 = 3798;
const HAMADRYAD_SHARD: i32 = 3799;
const VANOR_SILENOS_MANE: i32 = 3800;
const TALK_BUGBEAR_TOTEM: i32 = 3801;
const OKUNS_HEAD: i32 = 3802;
const KAKRANS_HEAD: i32 = 3803;
const NARCISSUSS_SOULSTONE: i32 = 3804;
const DEPRIVE_EYE: i32 = 3805;
const UNICORNS_HORN: i32 = 3806;
const KERUNOSS_GOLD_MANE: i32 = 3807;
const SKULL_OF_EXECUTED: i32 = 3808;
const BUST_OF_TRAVIS: i32 = 3809;
const SWORD_OF_CADMUS: i32 = 3810;

// Rewards

// Monsters drop

/// `REWARDS`: (request, item, count, adena).
const REWARDS: [(i32, i32, i64, i64); 41] = [
    (FIRST_CIRCLE_REQUEST_1C, CHARM_OF_KADESH, 40, 2090),
    (FIRST_CIRCLE_REQUEST_3C, ENCHANTED_GOLEM_SHARD, 50, 9480),
    (FIRST_CIRCLE_REQUEST_4C, GIANT_MONSTER_EYE_MEAT, 30, 9110),
    (FIRST_CIRCLE_REQUEST_5C, DIRE_WYRM_EGG, 40, 8690),
    (FIRST_CIRCLE_REQUEST_6C, GUARDIAN_BASILISK_TALON, 100, 9480),
    (FIRST_CIRCLE_REQUEST_7C, REVENANTS_CHAINS, 50, 11280),
    (FIRST_CIRCLE_REQUEST_8C, WINDSUS_TUSK, 30, 9640),
    (FIRST_CIRCLE_REQUEST_9C, GRANDISS_SKULL, 100, 9180),
    (FIRST_CIRCLE_REQUEST_10C, TAIK_OBSIDIAN_AMULET, 50, 5160),
    (FIRST_CIRCLE_REQUEST_11C, KARUL_BUGBEAR_HEAD, 30, 3140),
    (FIRST_CIRCLE_REQUEST_12C, TAMLIN_IVORY_CHARM, 40, 3160),
    (FIRST_CIRCLE_REQUEST_1B, FANG_OF_NARAK, 1, 6370),
    (FIRST_CIRCLE_REQUEST_2B, ENCHANTED_GARGOYLES_HORN, 50, 19080),
    (FIRST_CIRCLE_REQUEST_3B, COILED_SERPENT_TOTEM, 50, 19080),
    (FIRST_CIRCLE_REQUEST_4B, TOTEM_OF_KADESH, 1, 5790),
    (FIRST_CIRCLE_REQUEST_5B, KAIKIS_HEAD, 1, 8560),
    (FIRST_CIRCLE_REQUEST_6B, KRONBE_VENOM_SAC, 30, 8320),
    (FIRST_CIRCLE_REQUEST_1A, EVAS_CHARM, 30, 30310),
    (FIRST_CIRCLE_REQUEST_2A, TITANS_TABLET, 1, 27540),
    (FIRST_CIRCLE_REQUEST_3A, BOOK_OF_SHUNAIMAN, 1, 20560),
    (SECOND_CIRCLE_REQUEST_1C, ROTTING_TREE_SPORES, 40, 6850),
    (SECOND_CIRCLE_REQUEST_2C, TRISALIM_VENOM_SAC, 40, 7250),
    (SECOND_CIRCLE_REQUEST_3C, TAIK_ORC_TOTEM, 50, 7160),
    (SECOND_CIRCLE_REQUEST_4C, HARIT_BARBED_NECKLACE, 40, 6580),
    (SECOND_CIRCLE_REQUEST_5C, COIN_OF_OLD_EMPIRE, 20, 10100),
    (SECOND_CIRCLE_REQUEST_6C, SKIN_OF_FARCRAN, 30, 13000),
    (SECOND_CIRCLE_REQUEST_7C, TEMPEST_SHARD, 40, 7660),
    (SECOND_CIRCLE_REQUEST_8C, TSUNAMI_SHARD, 40, 7660),
    (SECOND_CIRCLE_REQUEST_9C, SATYR_MANE, 40, 11260),
    (SECOND_CIRCLE_REQUEST_10C, HAMADRYAD_SHARD, 40, 7660),
    (SECOND_CIRCLE_REQUEST_11C, VANOR_SILENOS_MANE, 30, 8810),
    (SECOND_CIRCLE_REQUEST_12C, TALK_BUGBEAR_TOTEM, 30, 7350),
    (SECOND_CIRCLE_REQUEST_1B, OKUNS_HEAD, 1, 8760),
    (SECOND_CIRCLE_REQUEST_2B, KAKRANS_HEAD, 1, 9380),
    (SECOND_CIRCLE_REQUEST_3B, NARCISSUSS_SOULSTONE, 40, 17820),
    (SECOND_CIRCLE_REQUEST_4B, DEPRIVE_EYE, 20, 17540),
    (SECOND_CIRCLE_REQUEST_5B, UNICORNS_HORN, 20, 14160),
    (SECOND_CIRCLE_REQUEST_6B, KERUNOSS_GOLD_MANE, 1, 15960),
    (SECOND_CIRCLE_REQUEST_1A, SKULL_OF_EXECUTED, 20, 39100),
    (SECOND_CIRCLE_REQUEST_2A, BUST_OF_TRAVIS, 1, 39550),
    (SECOND_CIRCLE_REQUEST_3A, SWORD_OF_CADMUS, 10, 41200),
];

/// `DROPLIST`: (monster, request, item, amount, limit, chance in percent). The amount 0 of the
/// Guardian Basilisk talon row is `(getRandom(100) < 60) ? 2 : 1` of Java's static initialiser.
const DROPLIST: [(i32, i32, i32, i64, i64, i32); 78] = [
    (
        BREKA_ORC_SHAMAN,
        FIRST_CIRCLE_REQUEST_3B,
        COILED_SERPENT_TOTEM,
        1,
        50,
        93,
    ),
    (
        BREKA_ORC_WARRIOR,
        FIRST_CIRCLE_REQUEST_3B,
        COILED_SERPENT_TOTEM,
        1,
        50,
        100,
    ),
    (
        GUARDIAN_BASILISK,
        TEST_INSTRUCTIONS_1,
        GUARDIAN_BASILISK_SCALE,
        1,
        40,
        90,
    ),
    (
        GUARDIAN_BASILISK,
        FIRST_CIRCLE_REQUEST_6C,
        GUARDIAN_BASILISK_TALON,
        0,
        100,
        100,
    ),
    (
        FETTERED_SOUL,
        FIRST_CIRCLE_REQUEST_7C,
        REVENANTS_CHAINS,
        1,
        50,
        100,
    ),
    (WINDSUS, FIRST_CIRCLE_REQUEST_8C, WINDSUS_TUSK, 1, 30, 63),
    (GRANDIS, FIRST_CIRCLE_REQUEST_9C, GRANDISS_SKULL, 2, 100, 100),
    (GIANT_FUNGUS, TEST_INSTRUCTIONS_1, SPORESEA_SEED, 1, 30, 84),
    (
        GIANT_MONSTEREYE,
        FIRST_CIRCLE_REQUEST_4C,
        GIANT_MONSTER_EYE_MEAT,
        1,
        30,
        60,
    ),
    (DIRE_WYRM, FIRST_CIRCLE_REQUEST_5C, DIRE_WYRM_EGG, 1, 40, 90),
    (
        ROTTING_TREE,
        SECOND_CIRCLE_REQUEST_1C,
        ROTTING_TREE_SPORES,
        1,
        40,
        77,
    ),
    (TRISALIM_SPIDER, TEST_INSTRUCTIONS_2, TRISALIM_SILK, 1, 20, 60),
    (
        TRISALIM_SPIDER,
        SECOND_CIRCLE_REQUEST_2C,
        TRISALIM_VENOM_SAC,
        1,
        40,
        76,
    ),
    (TRISALIM_TARANTULA, TEST_INSTRUCTIONS_2, TRISALIM_SILK, 1, 20, 60),
    (
        TRISALIM_TARANTULA,
        SECOND_CIRCLE_REQUEST_2C,
        TRISALIM_VENOM_SAC,
        1,
        40,
        85,
    ),
    (
        SPORE_ZOMBIE,
        SECOND_CIRCLE_REQUEST_2C,
        TRISALIM_VENOM_SAC,
        1,
        30,
        60,
    ),
    (
        MANASHEN_GARGOYLE,
        TEST_INSTRUCTIONS_1,
        INDIGO_SPIRIT_ORE,
        1,
        20,
        60,
    ),
    (
        ENCHANTED_STONE_GOLEM,
        FIRST_CIRCLE_REQUEST_3C,
        ENCHANTED_GOLEM_SHARD,
        1,
        50,
        100,
    ),
    (
        ENCHANTED_STONE_GOLEM,
        TEST_INSTRUCTIONS_1,
        INDIGO_SPIRIT_ORE,
        1,
        20,
        62,
    ),
    (
        ENCHANTED_GARGOYLE,
        FIRST_CIRCLE_REQUEST_2B,
        ENCHANTED_GARGOYLES_HORN,
        1,
        50,
        60,
    ),
    (
        TARLK_BUGBEAR_WARRIOR,
        SECOND_CIRCLE_REQUEST_12C,
        TALK_BUGBEAR_TOTEM,
        1,
        30,
        73,
    ),
    (
        LETO_LIZARDMAN_ARCHER,
        FIRST_CIRCLE_REQUEST_1C,
        CHARM_OF_KADESH,
        1,
        40,
        90,
    ),
    (
        LETO_LIZARDMAN_SOLDIER,
        FIRST_CIRCLE_REQUEST_1C,
        CHARM_OF_KADESH,
        1,
        40,
        93,
    ),
    (LETO_LIZARDMAN_SHAMAN, TEST_INSTRUCTIONS_1, KARUT_WEED, 1, 20, 60),
    (
        LETO_LIZARDMAN_OVERLORD,
        TEST_INSTRUCTIONS_1,
        KARUT_WEED,
        1,
        20,
        60,
    ),
    (
        TIMAK_ORC_WARRIOR,
        FIRST_CIRCLE_REQUEST_2C,
        TIMAK_JADE_NECKLACE,
        1,
        50,
        95,
    ),
    (TIMAK_ORC_WARRIOR, TEST_INSTRUCTIONS_2, TIMAK_ORC_TOTEM, 1, 20, 60),
    (
        TIMAK_ORC_OVERLORD,
        FIRST_CIRCLE_REQUEST_2C,
        TIMAK_JADE_NECKLACE,
        1,
        50,
        100,
    ),
    (FLINE, SECOND_CIRCLE_REQUEST_7C, TEMPEST_SHARD, 1, 40, 59),
    (LIELE, SECOND_CIRCLE_REQUEST_8C, TSUNAMI_SHARD, 1, 40, 61),
    (VALLEY_TREANT, TEST_INSTRUCTIONS_2, AMBROSIUS_FRUIT, 1, 30, 85),
    (SATYR, SECOND_CIRCLE_REQUEST_9C, SATYR_MANE, 1, 40, 90),
    (UNICORN, SECOND_CIRCLE_REQUEST_5B, UNICORNS_HORN, 1, 20, 78),
    (
        FOREST_RUNNER,
        SECOND_CIRCLE_REQUEST_10C,
        HAMADRYAD_SHARD,
        1,
        40,
        74,
    ),
    (
        VALLEY_TREANT_ELDER,
        TEST_INSTRUCTIONS_2,
        AMBROSIUS_FRUIT,
        1,
        30,
        85,
    ),
    (SATYR_ELDER, SECOND_CIRCLE_REQUEST_9C, SATYR_MANE, 1, 40, 100),
    (UNICORN_ELDER, SECOND_CIRCLE_REQUEST_5B, UNICORNS_HORN, 1, 20, 96),
    (
        KARUL_BUGBEAR,
        FIRST_CIRCLE_REQUEST_11C,
        KARUL_BUGBEAR_HEAD,
        1,
        30,
        60,
    ),
    (
        TAMLIN_ORC,
        FIRST_CIRCLE_REQUEST_12C,
        TAMLIN_IVORY_CHARM,
        1,
        40,
        72,
    ),
    (
        TAMLIN_ORC_ARCHER,
        FIRST_CIRCLE_REQUEST_12C,
        TAMLIN_IVORY_CHARM,
        1,
        40,
        90,
    ),
    (
        KRONBE_SPIDER,
        FIRST_CIRCLE_REQUEST_6B,
        KRONBE_VENOM_SAC,
        1,
        30,
        60,
    ),
    (
        TAIK_ORC_ARCHER,
        FIRST_CIRCLE_REQUEST_10C,
        TAIK_OBSIDIAN_AMULET,
        1,
        50,
        100,
    ),
    (
        TAIK_ORC_WARRIOR,
        FIRST_CIRCLE_REQUEST_10C,
        TAIK_OBSIDIAN_AMULET,
        1,
        50,
        93,
    ),
    (
        TAIK_ORC_SHAMAN,
        SECOND_CIRCLE_REQUEST_3C,
        TAIK_ORC_TOTEM,
        1,
        50,
        63,
    ),
    (
        TAIK_ORC_CAPTAIN,
        SECOND_CIRCLE_REQUEST_3C,
        TAIK_ORC_TOTEM,
        1,
        50,
        99,
    ),
    (MIRROR, SECOND_CIRCLE_REQUEST_3B, NARCISSUSS_SOULSTONE, 1, 40, 96),
    (
        HARIT_LIZARDMAN_GRUNT,
        SECOND_CIRCLE_REQUEST_4C,
        HARIT_BARBED_NECKLACE,
        1,
        40,
        98,
    ),
    (
        HARIT_LIZARDMAN_ARCHER,
        SECOND_CIRCLE_REQUEST_4C,
        HARIT_BARBED_NECKLACE,
        1,
        40,
        98,
    ),
    (
        HARIT_LIZARDMAN_WARRIOR,
        SECOND_CIRCLE_REQUEST_4C,
        HARIT_BARBED_NECKLACE,
        1,
        40,
        100,
    ),
    (
        GRAVE_WANDERER,
        SECOND_CIRCLE_REQUEST_1A,
        SKULL_OF_EXECUTED,
        1,
        20,
        83,
    ),
    (
        ARCHER_OF_GREED,
        TEST_INSTRUCTIONS_2,
        IMPERIAL_ARROWHEAD,
        1,
        20,
        60,
    ),
    (
        HATAR_RATMAN_THIEF,
        SECOND_CIRCLE_REQUEST_5C,
        COIN_OF_OLD_EMPIRE,
        1,
        20,
        60,
    ),
    (
        HATAR_RATMAN_BOSS,
        SECOND_CIRCLE_REQUEST_5C,
        COIN_OF_OLD_EMPIRE,
        1,
        20,
        62,
    ),
    (DEPRIVE, SECOND_CIRCLE_REQUEST_4B, DEPRIVE_EYE, 1, 20, 87),
    (FARCRAN, SECOND_CIRCLE_REQUEST_6C, SKIN_OF_FARCRAN, 1, 30, 100),
    (TAIRIM, TEST_INSTRUCTIONS_2, BALEFIRE_CRYSTAL, 1, 20, 60),
    (
        JUDGE_OF_MARSH,
        SECOND_CIRCLE_REQUEST_3A,
        SWORD_OF_CADMUS,
        1,
        10,
        74,
    ),
    (
        VANOR_SILENOS_GRUNT,
        SECOND_CIRCLE_REQUEST_11C,
        VANOR_SILENOS_MANE,
        1,
        30,
        80,
    ),
    (
        VANOR_SILENOS_SCOUT,
        SECOND_CIRCLE_REQUEST_11C,
        VANOR_SILENOS_MANE,
        1,
        30,
        95,
    ),
    (
        VANOR_SILENOS_WARRIOR,
        SECOND_CIRCLE_REQUEST_11C,
        VANOR_SILENOS_MANE,
        1,
        30,
        100,
    ),
    (BREKA_OVERLORD_HAKA, TEST_INSTRUCTIONS_1, HAKAS_HEAD, 1, 1, 100),
    (BREKA_OVERLORD_JAKA, TEST_INSTRUCTIONS_1, JAKAS_HEAD, 1, 1, 100),
    (BREKA_OVERLORD_MARKA, TEST_INSTRUCTIONS_1, MARKAS_HEAD, 1, 1, 100),
    (WINDSUS_ALEPH, TEST_INSTRUCTIONS_1, WINDSUS_ALEPH_SKIN, 1, 1, 100),
    (TARLK_RAIDER_ATHU, TEST_INSTRUCTIONS_2, ATHUS_HEAD, 1, 1, 100),
    (TARLK_RAIDER_LANKA, TEST_INSTRUCTIONS_2, LANKAS_HEAD, 1, 1, 100),
    (TARLK_RAIDER_TRISKA, TEST_INSTRUCTIONS_2, TRISKAS_HEAD, 1, 1, 100),
    (TARLK_RAIDER_MOTURA, TEST_INSTRUCTIONS_2, MOTURAS_HEAD, 1, 1, 100),
    (TARLK_RAIDER_KALATH, TEST_INSTRUCTIONS_2, KALATHS_HEAD, 1, 1, 100),
    (
        LETO_SHAMAN_KETZ,
        FIRST_CIRCLE_REQUEST_4B,
        TOTEM_OF_KADESH,
        1,
        1,
        100,
    ),
    (
        LETO_CHIEF_NARAK,
        FIRST_CIRCLE_REQUEST_1B,
        FANG_OF_NARAK,
        1,
        1,
        100,
    ),
    (
        TIMAK_RAIDER_KAIKEE,
        FIRST_CIRCLE_REQUEST_5B,
        KAIKIS_HEAD,
        1,
        1,
        100,
    ),
    (
        TIMAK_OVERLORD_OKUN,
        SECOND_CIRCLE_REQUEST_1B,
        OKUNS_HEAD,
        1,
        1,
        100,
    ),
    (GOK_MAGOK, FIRST_CIRCLE_REQUEST_2A, TITANS_TABLET, 1, 1, 100),
    (
        TAIK_OVERLORD_KAKRAN,
        SECOND_CIRCLE_REQUEST_2B,
        KAKRANS_HEAD,
        1,
        1,
        100,
    ),
    (
        HATAR_CHIEFTAIN_KUBEL,
        SECOND_CIRCLE_REQUEST_2A,
        BUST_OF_TRAVIS,
        1,
        1,
        100,
    ),
    (
        VANOR_ELDER_KERUNOS,
        SECOND_CIRCLE_REQUEST_6B,
        KERUNOSS_GOLD_MANE,
        1,
        1,
        100,
    ),
    (
        KARUL_CHIEF_OROOTO,
        FIRST_CIRCLE_REQUEST_3A,
        BOOK_OF_SHUNAIMAN,
        1,
        1,
        100,
    ),
];

/// `LINKS` from 33520 on: (page, label).
const LINKS: [(&str, &str); 39] = [
    ("30745-10a.html", "C: 40 Totems of Kadesh"),
    ("30745-10b.html", "C: 50 Jade Necklaces of Timak"),
    ("30745-10c.html", "C: 50 Enchanted Golem Shards"),
    ("30745-10d.html", "C: 30 Pieces Monster Eye Meat"),
    ("30745-10e.html", "C: 40 Eggs of Dire Wyrm"),
    ("30745-10f.html", "C: 100 Claws of Guardian Basilisk"),
    ("30745-10g.html", "C: 50 Revenant Chains "),
    ("30745-10h.html", "C: 30 Windsus Tusks"),
    ("30745-10i.html", "C: 100 Skulls of Grandis"),
    ("30745-10j.html", "C: 50 Taik Obsidian Amulets"),
    ("30745-10k.html", "C: 30 Heads of Karul Bugbear"),
    ("30745-10l.html", "C: 40 Ivory Charms of Tamlin"),
    ("30745-11a.html", "B: Situation Preparation - Leto Chief"),
    ("30745-11b.html", "B: 50 Enchanted Gargoyle Horns"),
    ("30745-11c.html", "B: 50 Coiled Serpent Totems"),
    (
        "30745-11d.html",
        "B: Situation Preparation - Sorcerer Catch of Leto",
    ),
    ("30745-11e.html", "B: Situation Preparation - Timak Raider Kaikee"),
    ("30745-11f.html", "B: 30 Kronbe Venom Sacs"),
    ("30745-12a.html", "A: 30 Charms of Eva"),
    ("30745-12b.html", "A: Titan's Tablet"),
    ("30745-12c.html", "A: Book of Shunaiman"),
    ("30745-13a.html", "C: 40 Rotted Tree Spores"),
    ("30745-13b.html", "C: 40 Trisalim Venom Sacs"),
    ("30745-13c.html", "C: 50 Totems of Taik Orc"),
    ("30745-13d.html", "C: 40 Harit Barbed Necklaces"),
    ("30745-13e.html", "C: 20 Coins of Ancient Empire"),
    ("30745-13f.html", "C: 30 Skins of Farkran"),
    ("30745-13g.html", "C: 40 Tempest Shards"),
    ("30745-13k.html", "C: 30 Vanor Silenos Manes"),
    ("30745-13i.html", "C: 40 Manes of Pan Ruem"),
    ("30745-13j.html", "C: hamadryad shards"),
    ("30745-13k.html", "C: 30 Manes of Vanor Silenos"),
    ("30745-13l.html", "C: 30 Totems of Talk Bugbears"),
    (
        "30745-14a.html",
        "B: Situation Preparation - Overlord Okun of Timak",
    ),
    (
        "30745-14b.html",
        "B: Situation Preparation - Overlord Kakran of Taik",
    ),
    ("30745-14c.html", "B: 40 Narcissus Soulstones"),
    ("30745-14d.html", "B: 20 Eyes of Deprived"),
    ("30745-14e.html", "B: 20 Unicorn Horns"),
    ("30745-14f.html", "B: Kerunos's Gold Mane"),
];

const QUEST_ITEMS: [i32; 120] = [
    CYBELLINS_DAGGER,
    FIRST_CIRCLE_HUNTER_LICENSE,
    SECOND_CIRCLE_HUNTER_LICENSE,
    LAUREL_LEAF_PIN,
    TEST_INSTRUCTIONS_1,
    TEST_INSTRUCTIONS_2,
    CYBELLINS_REQUEST,
    BLOOD_CRYSTAL_PURITY_1,
    BLOOD_CRYSTAL_PURITY_2,
    BLOOD_CRYSTAL_PURITY_3,
    BLOOD_CRYSTAL_PURITY_4,
    BLOOD_CRYSTAL_PURITY_5,
    BLOOD_CRYSTAL_PURITY_6,
    BLOOD_CRYSTAL_PURITY_7,
    BLOOD_CRYSTAL_PURITY_8,
    BLOOD_CRYSTAL_PURITY_9,
    BLOOD_CRYSTAL_PURITY_10,
    BROKEN_BLOOD_CRYSTAL,
    GUARDIAN_BASILISK_SCALE,
    KARUT_WEED,
    HAKAS_HEAD,
    JAKAS_HEAD,
    MARKAS_HEAD,
    WINDSUS_ALEPH_SKIN,
    INDIGO_SPIRIT_ORE,
    SPORESEA_SEED,
    TIMAK_ORC_TOTEM,
    TRISALIM_SILK,
    AMBROSIUS_FRUIT,
    BALEFIRE_CRYSTAL,
    IMPERIAL_ARROWHEAD,
    ATHUS_HEAD,
    LANKAS_HEAD,
    TRISKAS_HEAD,
    MOTURAS_HEAD,
    KALATHS_HEAD,
    FIRST_CIRCLE_REQUEST_1C,
    FIRST_CIRCLE_REQUEST_2C,
    FIRST_CIRCLE_REQUEST_3C,
    FIRST_CIRCLE_REQUEST_4C,
    FIRST_CIRCLE_REQUEST_5C,
    FIRST_CIRCLE_REQUEST_6C,
    FIRST_CIRCLE_REQUEST_7C,
    FIRST_CIRCLE_REQUEST_8C,
    FIRST_CIRCLE_REQUEST_9C,
    FIRST_CIRCLE_REQUEST_10C,
    FIRST_CIRCLE_REQUEST_11C,
    FIRST_CIRCLE_REQUEST_12C,
    FIRST_CIRCLE_REQUEST_1B,
    FIRST_CIRCLE_REQUEST_2B,
    FIRST_CIRCLE_REQUEST_3B,
    FIRST_CIRCLE_REQUEST_4B,
    FIRST_CIRCLE_REQUEST_5B,
    FIRST_CIRCLE_REQUEST_6B,
    FIRST_CIRCLE_REQUEST_1A,
    FIRST_CIRCLE_REQUEST_2A,
    FIRST_CIRCLE_REQUEST_3A,
    SECOND_CIRCLE_REQUEST_1C,
    SECOND_CIRCLE_REQUEST_2C,
    SECOND_CIRCLE_REQUEST_3C,
    SECOND_CIRCLE_REQUEST_4C,
    SECOND_CIRCLE_REQUEST_5C,
    SECOND_CIRCLE_REQUEST_6C,
    SECOND_CIRCLE_REQUEST_7C,
    SECOND_CIRCLE_REQUEST_8C,
    SECOND_CIRCLE_REQUEST_9C,
    SECOND_CIRCLE_REQUEST_10C,
    SECOND_CIRCLE_REQUEST_11C,
    SECOND_CIRCLE_REQUEST_12C,
    SECOND_CIRCLE_REQUEST_1B,
    SECOND_CIRCLE_REQUEST_2B,
    SECOND_CIRCLE_REQUEST_3B,
    SECOND_CIRCLE_REQUEST_4B,
    SECOND_CIRCLE_REQUEST_5B,
    SECOND_CIRCLE_REQUEST_6B,
    SECOND_CIRCLE_REQUEST_1A,
    SECOND_CIRCLE_REQUEST_2A,
    SECOND_CIRCLE_REQUEST_3A,
    CHARM_OF_KADESH,
    TIMAK_JADE_NECKLACE,
    ENCHANTED_GOLEM_SHARD,
    GIANT_MONSTER_EYE_MEAT,
    DIRE_WYRM_EGG,
    GUARDIAN_BASILISK_TALON,
    REVENANTS_CHAINS,
    WINDSUS_TUSK,
    GRANDISS_SKULL,
    TAIK_OBSIDIAN_AMULET,
    KARUL_BUGBEAR_HEAD,
    TAMLIN_IVORY_CHARM,
    FANG_OF_NARAK,
    ENCHANTED_GARGOYLES_HORN,
    COILED_SERPENT_TOTEM,
    TOTEM_OF_KADESH,
    KAIKIS_HEAD,
    KRONBE_VENOM_SAC,
    EVAS_CHARM,
    TITANS_TABLET,
    BOOK_OF_SHUNAIMAN,
    ROTTING_TREE_SPORES,
    TRISALIM_VENOM_SAC,
    TAIK_ORC_TOTEM,
    HARIT_BARBED_NECKLACE,
    COIN_OF_OLD_EMPIRE,
    SKIN_OF_FARCRAN,
    TEMPEST_SHARD,
    TSUNAMI_SHARD,
    SATYR_MANE,
    HAMADRYAD_SHARD,
    VANOR_SILENOS_MANE,
    TALK_BUGBEAR_TOTEM,
    OKUNS_HEAD,
    KAKRANS_HEAD,
    NARCISSUSS_SOULSTONE,
    DEPRIVE_EYE,
    UNICORNS_HORN,
    KERUNOSS_GOLD_MANE,
    SKULL_OF_EXECUTED,
    BUST_OF_TRAVIS,
    SWORD_OF_CADMUS,
];

const NAME: &str = "Q00335_TheSongOfTheHunter";

/// The request items of Tor, first circle, then second circle (Java's `requestCount` list).
const REQUESTS: [i32; 42] = [
    FIRST_CIRCLE_REQUEST_1C,
    FIRST_CIRCLE_REQUEST_2C,
    FIRST_CIRCLE_REQUEST_3C,
    FIRST_CIRCLE_REQUEST_4C,
    FIRST_CIRCLE_REQUEST_5C,
    FIRST_CIRCLE_REQUEST_6C,
    FIRST_CIRCLE_REQUEST_7C,
    FIRST_CIRCLE_REQUEST_8C,
    FIRST_CIRCLE_REQUEST_9C,
    FIRST_CIRCLE_REQUEST_10C,
    FIRST_CIRCLE_REQUEST_11C,
    FIRST_CIRCLE_REQUEST_12C,
    FIRST_CIRCLE_REQUEST_1B,
    FIRST_CIRCLE_REQUEST_2B,
    FIRST_CIRCLE_REQUEST_3B,
    FIRST_CIRCLE_REQUEST_4B,
    FIRST_CIRCLE_REQUEST_5B,
    FIRST_CIRCLE_REQUEST_6B,
    FIRST_CIRCLE_REQUEST_1A,
    FIRST_CIRCLE_REQUEST_2A,
    FIRST_CIRCLE_REQUEST_3A,
    SECOND_CIRCLE_REQUEST_1C,
    SECOND_CIRCLE_REQUEST_2C,
    SECOND_CIRCLE_REQUEST_3C,
    SECOND_CIRCLE_REQUEST_4C,
    SECOND_CIRCLE_REQUEST_5C,
    SECOND_CIRCLE_REQUEST_6C,
    SECOND_CIRCLE_REQUEST_7C,
    SECOND_CIRCLE_REQUEST_8C,
    SECOND_CIRCLE_REQUEST_9C,
    SECOND_CIRCLE_REQUEST_10C,
    SECOND_CIRCLE_REQUEST_11C,
    SECOND_CIRCLE_REQUEST_12C,
    SECOND_CIRCLE_REQUEST_1B,
    SECOND_CIRCLE_REQUEST_2B,
    SECOND_CIRCLE_REQUEST_3B,
    SECOND_CIRCLE_REQUEST_4B,
    SECOND_CIRCLE_REQUEST_5B,
    SECOND_CIRCLE_REQUEST_6B,
    SECOND_CIRCLE_REQUEST_1A,
    SECOND_CIRCLE_REQUEST_2A,
    SECOND_CIRCLE_REQUEST_3A,
];

/// The first circle requests of the `30744-09.html` test.
// NOTE(java-quirk): Java adds FIRST_CIRCLE_REQUEST_4C twice and leaves out 5C.
const FIRST_CIRCLE_SUM: [i32; 21] = [
    FIRST_CIRCLE_REQUEST_1C,
    FIRST_CIRCLE_REQUEST_2C,
    FIRST_CIRCLE_REQUEST_3C,
    FIRST_CIRCLE_REQUEST_4C,
    FIRST_CIRCLE_REQUEST_4C,
    FIRST_CIRCLE_REQUEST_6C,
    FIRST_CIRCLE_REQUEST_7C,
    FIRST_CIRCLE_REQUEST_8C,
    FIRST_CIRCLE_REQUEST_9C,
    FIRST_CIRCLE_REQUEST_10C,
    FIRST_CIRCLE_REQUEST_11C,
    FIRST_CIRCLE_REQUEST_12C,
    FIRST_CIRCLE_REQUEST_1B,
    FIRST_CIRCLE_REQUEST_2B,
    FIRST_CIRCLE_REQUEST_3B,
    FIRST_CIRCLE_REQUEST_4B,
    FIRST_CIRCLE_REQUEST_5B,
    FIRST_CIRCLE_REQUEST_6B,
    FIRST_CIRCLE_REQUEST_1A,
    FIRST_CIRCLE_REQUEST_2A,
    FIRST_CIRCLE_REQUEST_3A,
];

/// The blood crystals, purity 1 to 10.
const BLOOD_CRYSTALS: [i32; 10] = [
    BLOOD_CRYSTAL_PURITY_1,
    BLOOD_CRYSTAL_PURITY_2,
    BLOOD_CRYSTAL_PURITY_3,
    BLOOD_CRYSTAL_PURITY_4,
    BLOOD_CRYSTAL_PURITY_5,
    BLOOD_CRYSTAL_PURITY_6,
    BLOOD_CRYSTAL_PURITY_7,
    BLOOD_CRYSTAL_PURITY_8,
    BLOOD_CRYSTAL_PURITY_9,
    BLOOD_CRYSTAL_PURITY_10,
];

/// The adena Cybellin pays for the crystals of purity 2 to 10 (`30746-06.html`).
const CRYSTAL_ADENA: [i64; 9] = [3400, 6800, 13600, 27200, 54400, 108800, 217600, 435200, 870400];

/// The requests of `30745-10a.html`..`30745-14f.html` and the page that hands each out.
// NOTE(java-quirk): there is no case for `30745-13h.html`, so the Tsunami Shard request (8C) cannot be taken.
const REQUEST_PAGES: [(&str, i32); 38] = [
    ("30745-10a.html", FIRST_CIRCLE_REQUEST_1C),
    ("30745-10b.html", FIRST_CIRCLE_REQUEST_2C),
    ("30745-10c.html", FIRST_CIRCLE_REQUEST_3C),
    ("30745-10d.html", FIRST_CIRCLE_REQUEST_4C),
    ("30745-10e.html", FIRST_CIRCLE_REQUEST_5C),
    ("30745-10f.html", FIRST_CIRCLE_REQUEST_6C),
    ("30745-10g.html", FIRST_CIRCLE_REQUEST_7C),
    ("30745-10h.html", FIRST_CIRCLE_REQUEST_8C),
    ("30745-10i.html", FIRST_CIRCLE_REQUEST_9C),
    ("30745-10j.html", FIRST_CIRCLE_REQUEST_10C),
    ("30745-10k.html", FIRST_CIRCLE_REQUEST_11C),
    ("30745-10l.html", FIRST_CIRCLE_REQUEST_12C),
    ("30745-11a.html", FIRST_CIRCLE_REQUEST_1B),
    ("30745-11b.html", FIRST_CIRCLE_REQUEST_2B),
    ("30745-11c.html", FIRST_CIRCLE_REQUEST_3B),
    ("30745-11d.html", FIRST_CIRCLE_REQUEST_4B),
    ("30745-11e.html", FIRST_CIRCLE_REQUEST_5B),
    ("30745-11f.html", FIRST_CIRCLE_REQUEST_6B),
    ("30745-12a.html", FIRST_CIRCLE_REQUEST_1A),
    ("30745-12b.html", FIRST_CIRCLE_REQUEST_2A),
    ("30745-12c.html", FIRST_CIRCLE_REQUEST_3A),
    ("30745-13a.html", SECOND_CIRCLE_REQUEST_1C),
    ("30745-13b.html", SECOND_CIRCLE_REQUEST_2C),
    ("30745-13c.html", SECOND_CIRCLE_REQUEST_3C),
    ("30745-13d.html", SECOND_CIRCLE_REQUEST_4C),
    ("30745-13e.html", SECOND_CIRCLE_REQUEST_5C),
    ("30745-13f.html", SECOND_CIRCLE_REQUEST_6C),
    ("30745-13g.html", SECOND_CIRCLE_REQUEST_7C),
    ("30745-13k.html", SECOND_CIRCLE_REQUEST_11C),
    ("30745-13i.html", SECOND_CIRCLE_REQUEST_9C),
    ("30745-13j.html", SECOND_CIRCLE_REQUEST_10C),
    ("30745-13l.html", SECOND_CIRCLE_REQUEST_12C),
    ("30745-14a.html", SECOND_CIRCLE_REQUEST_1B),
    ("30745-14b.html", SECOND_CIRCLE_REQUEST_2B),
    ("30745-14c.html", SECOND_CIRCLE_REQUEST_3B),
    ("30745-14d.html", SECOND_CIRCLE_REQUEST_4B),
    ("30745-14e.html", SECOND_CIRCLE_REQUEST_5B),
    ("30745-14f.html", SECOND_CIRCLE_REQUEST_6B),
];

/// What the script keeps (Java's `static final int[][] DROPLIST` has the amount of the basilisk
/// talon row rolled once in the static initialiser).
#[derive(Default)]
struct SongState {
    /// `(getRandom(100) < 60) ? 2 : 1`, 0 until it is rolled.
    talon_amount: i64,
}

/// The `DROPLIST` amount of a row: the talon row has the amount Java rolled at class load.
fn drop_amount(ctx: &mut ScriptCtx<'_>, amount: i64) -> i64 {
    if amount != 0 {
        return amount;
    }
    if ctx.state::<SongState>().talon_amount == 0 {
        // NOTE(java-quirk): the roll is made once when the class loads, not for each kill.
        let rolled = if ctx.get_random(100) < 60 { 2 } else { 1 };
        ctx.state::<SongState>().talon_amount = rolled;
    }
    ctx.state::<SongState>().talon_amount
}

/// The sum of the counts of `items`, one item after the other (so a repeated item counts twice).
fn sum(ctx: &ScriptCtx<'_>, player: ObjectId, items: &[i32]) -> i64 {
    items.iter().map(|&i| ctx.get_quest_items_count(player, i)).sum()
}

/// `LINKS.get(id)` as the page text.
fn link(id: i32) -> String {
    let index = usize::try_from(id - 33520).unwrap_or(usize::MAX);
    match LINKS.get(index) {
        Some((page, label)) => {
            format!("<a action=\"bypass -h Script {NAME} {page}\">{label}</a><br>")
        }
        // Java: `html.replace(.., null)` throws on a missing entry.
        None => String::new(),
    }
}

/// Java `getHtml(player, htmlName, i0..i4)`.
fn get_html(ctx: &ScriptCtx<'_>, player: ObjectId, html_name: &str, ids: [i32; 5]) -> Option<String> {
    let mut html = ctx.get_htm(player, html_name)?;
    for (n, id) in ids.into_iter().enumerate() {
        html = html.replace(&format!("<?reply{}?>", n + 1), &link(id));
    }
    Some(html)
}

/// The five list entries of a memo state: 5 bits each, the last pick in the low bits.
fn pack(i: [i32; 5]) -> i32 {
    i[0] * 32 * 32 * 32 * 32 + i[1] * 32 * 32 * 32 + i[2] * 32 * 32 + i[3] * 32 + i[4]
}

/// Java's `while ((i0 == i1) || (i1 == i2) || ...)`: any two of the five are equal.
fn has_duplicates(i: [i32; 5]) -> bool {
    i[0] == i[1]
        || i[1] == i[2]
        || i[2] == i[3]
        || i[3] == i[4]
        || i[0] == i[4]
        || i[0] == i[2]
        || i[0] == i[3]
        || i[1] == i[3]
        || i[1] == i[4]
        || i[2] == i[4]
}

/// Java `reward(player, qs, REWARDS)`: the highest request in the list that the player holds.
fn reward(ctx: &mut ScriptCtx<'_>, player: ObjectId, qs: l2_world::script::Qs) -> Option<String> {
    for &(request, item, count, adena) in REWARDS.iter().rev() {
        if ctx.has_quest_items(player, request) {
            if ctx.get_quest_items_count(player, item) >= count {
                ctx.give_items(player, LAUREL_LEAF_PIN, 1);
                ctx.give_adena(player, adena, true);
                ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
                qs.set_memo_state(ctx, 0);
                ctx.take_items(player, request, -1);
                if item == GIANT_MONSTER_EYE_MEAT {
                    ctx.take_items(player, item, count);
                } else {
                    ctx.take_items(player, item, -1);
                }

                return Some(
                    if ctx.has_quest_items(player, FIRST_CIRCLE_HUNTER_LICENSE) {
                        "30745-06a.html"
                    } else {
                        "30745-06b.html"
                    }
                    .to_owned(),
                );
            }

            return Some("30745-05.html".to_owned());
        }
    }

    None
}

/// Java `evolveBloodCrystal(player)`.
fn evolve_blood_crystal(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    if ctx.active_weapon_item_id(player) != Some(CYBELLINS_DAGGER) {
        return;
    }
    if !(ctx.has_quest_items(player, FIRST_CIRCLE_HUNTER_LICENSE)
        || ctx.has_quest_items(player, SECOND_CIRCLE_HUNTER_LICENSE))
    {
        return;
    }
    if ctx.get_random(100) < 60 {
        if ctx.has_quest_items(player, CYBELLINS_REQUEST) {
            // The first crystal the player holds becomes the next one; purity 6 and up also jackpot.
            for n in 0..9 {
                if ctx.has_quest_items(player, BLOOD_CRYSTALS[n]) {
                    ctx.give_items(player, BLOOD_CRYSTALS[n + 1], 1);
                    if n >= 5 {
                        ctx.play_sound(player, QuestSound::ItemsoundQuestJackpot);
                    }
                    ctx.take_items(player, BLOOD_CRYSTALS[n], -1);
                    break;
                }
            }
        }
    } else if ctx.has_quest_items(player, CYBELLINS_REQUEST)
        && ctx.has_at_least_one_quest_item(player, &BLOOD_CRYSTALS[..9])
    {
        ctx.take_items_many(player, -1, &BLOOD_CRYSTALS[..9]);
        ctx.give_items(player, BROKEN_BLOOD_CRYSTAL, 1);
    }
}

/// `addSpawn(id, npc.getX(), npc.getY(), npc.getZ() + 20, npc.getHeading(), false, 0, true)`.
fn spawn_near(ctx: &mut ScriptCtx<'_>, id: i32, npc: ScriptNpc) {
    if let Some((x, y, z, heading)) = ctx.npc_position(npc) {
        ctx.add_spawn(id, x, y, z + 20, heading, false, 0, 0);
    }
}

/// The gremlin filcher spawn of the "collect" requests: `(getRandom(20) < 2)`.
fn spawn_filcher(
    ctx: &mut ScriptCtx<'_>,
    member: ObjectId,
    npc: ScriptNpc,
    request: i32,
    item: i32,
    limit: i64,
) {
    if ctx.has_quest_items(member, request)
        && ctx.get_quest_items_count(member, item) < limit
        && ctx.get_random(20) < 2
    {
        spawn_near(ctx, GREMLIN_FILCHER, npc);
        // NOTE(java-quirk): Java sends a `CreatureSay` built from the NPC; it is shown as speech.
        ctx.broadcast_npc_say(
            npc,
            ChatType::NpcGeneral,
            NpcStringId::ShowMeThePrettySparklingThingsTheyReAllMine,
            &[],
        );
    }
}

/// The Black Legion pair a boss calls with half the kills.
fn stormtroopers(ctx: &mut ScriptCtx<'_>, member: ObjectId, npc: ScriptNpc, (request, item): (i32, i32)) {
    if ctx.has_quest_items(member, request) && !ctx.has_quest_items(member, item) && ctx.get_random(2) == 0 {
        spawn_near(ctx, BLACK_LEGION_STORMTROOPER, npc);
        spawn_near(ctx, BLACK_LEGION_STORMTROOPER, npc);
        ctx.broadcast_npc_say(
            npc,
            ChatType::NpcGeneral,
            NpcStringId::WeLlTakeThePropertyOfTheAncientEmpire,
            &[],
        );
    }
}

/// Java `Q00335_TheSongOfTheHunter`.
pub struct Q00335TheSongOfTheHunter;

impl Script for Q00335TheSongOfTheHunter {
    fn name(&self) -> &'static str {
        NAME
    }

    fn path(&self) -> &'static str {
        "quests/Q00335_TheSongOfTheHunter"
    }

    fn id(&self) -> i32 {
        335
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[GREY]);
        s.add_talk_id(&[GREY, TOR, CYBELLIN]);
        s.add_kill_id(&[
            BREKA_ORC_SHAMAN,
            BREKA_ORC_WARRIOR,
            GUARDIAN_BASILISK,
            FETTERED_SOUL,
            WINDSUS,
            GRANDIS,
            GIANT_FUNGUS,
            GIANT_MONSTEREYE,
            DIRE_WYRM,
            ROTTING_TREE,
            TRISALIM_SPIDER,
            TRISALIM_TARANTULA,
            SPORE_ZOMBIE,
            MANASHEN_GARGOYLE,
            ENCHANTED_STONE_GOLEM,
            ENCHANTED_GARGOYLE,
            TARLK_BUGBEAR_WARRIOR,
            LETO_LIZARDMAN_ARCHER,
            LETO_LIZARDMAN_SOLDIER,
            LETO_LIZARDMAN_SHAMAN,
            LETO_LIZARDMAN_OVERLORD,
            TIMAK_ORC_WARRIOR,
            TIMAK_ORC_OVERLORD,
            FLINE,
            LIELE,
            VALLEY_TREANT,
            SATYR,
            UNICORN,
            FOREST_RUNNER,
            VALLEY_TREANT_ELDER,
            SATYR_ELDER,
            UNICORN_ELDER,
            KARUL_BUGBEAR,
            TAMLIN_ORC,
            TAMLIN_ORC_ARCHER,
            KRONBE_SPIDER,
            TAIK_ORC_ARCHER,
            TAIK_ORC_WARRIOR,
            TAIK_ORC_SHAMAN,
            TAIK_ORC_CAPTAIN,
            MIRROR,
            HARIT_LIZARDMAN_GRUNT,
            HARIT_LIZARDMAN_ARCHER,
            HARIT_LIZARDMAN_WARRIOR,
            GRAVE_WANDERER,
            ARCHER_OF_GREED,
            HATAR_RATMAN_THIEF,
            HATAR_RATMAN_BOSS,
            DEPRIVE,
            FARCRAN,
            TAIRIM,
            JUDGE_OF_MARSH,
            VANOR_SILENOS_GRUNT,
            VANOR_SILENOS_SCOUT,
            VANOR_SILENOS_WARRIOR,
            BREKA_OVERLORD_HAKA,
            BREKA_OVERLORD_JAKA,
            BREKA_OVERLORD_MARKA,
            WINDSUS_ALEPH,
            TARLK_RAIDER_ATHU,
            TARLK_RAIDER_LANKA,
            TARLK_RAIDER_TRISKA,
            TARLK_RAIDER_MOTURA,
            TARLK_RAIDER_KALATH,
            GREMLIN_FILCHER,
            LETO_SHAMAN_KETZ,
            LETO_CHIEF_NARAK,
            TIMAK_RAIDER_KAIKEE,
            TIMAK_OVERLORD_OKUN,
            GOK_MAGOK,
            TAIK_OVERLORD_KAKRAN,
            HATAR_CHIEFTAIN_KUBEL,
            VANOR_ELDER_KERUNOS,
            KARUL_CHIEF_OROOTO,
            VANOR_SILENOS_CHIEFTAIN,
        ]);
        s.register_quest_items(&QUEST_ITEMS);
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = Some(ctx.get_no_quest_msg(player));
        let set = |s: &str| Some(s.to_owned());
        if qs.is_created(ctx) {
            htmltext = if ctx.player_level(player) < MIN_LEVEL {
                set("30744-01.htm")
            } else {
                set("30744-02.htm")
            };
        } else if qs.state(ctx) == STARTED {
            match npc.id {
                GREY => {
                    if ctx.has_quest_items(player, TEST_INSTRUCTIONS_1) {
                        let count = [
                            ctx.get_quest_items_count(player, GUARDIAN_BASILISK_SCALE) >= 40,
                            ctx.get_quest_items_count(player, KARUT_WEED) >= 20,
                            sum(ctx, player, &[HAKAS_HEAD, JAKAS_HEAD, MARKAS_HEAD]) >= 3,
                            ctx.has_quest_items(player, WINDSUS_ALEPH_SKIN),
                            ctx.get_quest_items_count(player, INDIGO_SPIRIT_ORE) >= 20,
                            ctx.get_quest_items_count(player, SPORESEA_SEED) >= 30,
                        ]
                        .into_iter()
                        .filter(|&b| b)
                        .count();
                        if count < 3 {
                            htmltext = set("30744-05.html");
                        } else {
                            qs.set_cond(ctx, 2, true);
                            ctx.give_items(player, FIRST_CIRCLE_HUNTER_LICENSE, 1);
                            for item in [
                                GUARDIAN_BASILISK_SCALE,
                                KARUT_WEED,
                                HAKAS_HEAD,
                                JAKAS_HEAD,
                                MARKAS_HEAD,
                                WINDSUS_ALEPH_SKIN,
                                INDIGO_SPIRIT_ORE,
                                SPORESEA_SEED,
                                TEST_INSTRUCTIONS_1,
                            ] {
                                ctx.take_items(player, item, -1);
                            }
                            htmltext = set("30744-06.html");
                        }
                    }

                    if ctx.has_quest_items(player, FIRST_CIRCLE_HUNTER_LICENSE) {
                        if ctx.player_level(player) < MAX_LEVEL {
                            htmltext = set("30744-07.html");
                        } else if !ctx.has_quest_items(player, TEST_INSTRUCTIONS_2) {
                            htmltext = set("30744-08.html");
                        }
                    }

                    if ctx.has_quest_items(player, TEST_INSTRUCTIONS_2) {
                        let count = [
                            ctx.get_quest_items_count(player, TIMAK_ORC_TOTEM) >= 20,
                            ctx.get_quest_items_count(player, TRISALIM_SILK) >= 20,
                            ctx.get_quest_items_count(player, AMBROSIUS_FRUIT) >= 30,
                            ctx.get_quest_items_count(player, BALEFIRE_CRYSTAL) >= 20,
                            ctx.get_quest_items_count(player, IMPERIAL_ARROWHEAD) >= 20,
                            sum(
                                ctx,
                                player,
                                &[ATHUS_HEAD, LANKAS_HEAD, TRISKAS_HEAD, MOTURAS_HEAD, KALATHS_HEAD],
                            ) >= 5,
                        ]
                        .into_iter()
                        .filter(|&b| b)
                        .count();
                        if count < 3 {
                            htmltext = set("30744-11.html");
                        } else {
                            qs.set_cond(ctx, 3, true);
                            ctx.give_items(player, SECOND_CIRCLE_HUNTER_LICENSE, 1);
                            for item in [
                                TRISALIM_SILK,
                                TIMAK_ORC_TOTEM,
                                AMBROSIUS_FRUIT,
                                BALEFIRE_CRYSTAL,
                                IMPERIAL_ARROWHEAD,
                                ATHUS_HEAD,
                                LANKAS_HEAD,
                                TRISKAS_HEAD,
                                MOTURAS_HEAD,
                                KALATHS_HEAD,
                                TEST_INSTRUCTIONS_2,
                                FIRST_CIRCLE_HUNTER_LICENSE,
                            ] {
                                ctx.take_items(player, item, -1);
                            }
                            htmltext = set("30744-12.html");
                        }
                    }

                    if ctx.has_quest_items(player, SECOND_CIRCLE_HUNTER_LICENSE) {
                        htmltext = set("30744-14.html");
                    }
                }
                CYBELLIN => {
                    let crystals_2_to_9 = &BLOOD_CRYSTALS[1..9];
                    if !ctx.has_quest_items(player, SECOND_CIRCLE_HUNTER_LICENSE)
                        && !ctx.has_quest_items(player, FIRST_CIRCLE_HUNTER_LICENSE)
                    {
                        htmltext = set("30746-01.html");
                    } else if !ctx.has_quest_items(player, CYBELLINS_REQUEST) {
                        htmltext = set("30746-02.html");
                    } else if ctx.has_quest_items(player, BLOOD_CRYSTAL_PURITY_1) {
                        htmltext = set("30746-04.html");
                    } else if ctx.has_at_least_one_quest_item(player, crystals_2_to_9) {
                        htmltext = set("30746-05.html");
                    } else if ctx.has_quest_items(player, BLOOD_CRYSTAL_PURITY_10) {
                        ctx.give_adena(player, 870400, true);
                        ctx.take_items(player, BLOOD_CRYSTAL_PURITY_10, -1);
                        htmltext = set("30746-05a.html");
                    } else if !ctx.has_quest_items(player, BROKEN_BLOOD_CRYSTAL) {
                        htmltext = set("30746-08.html");
                    } else {
                        ctx.take_items(player, BROKEN_BLOOD_CRYSTAL, -1);
                        htmltext = set("30746-09.html");
                    }
                }
                TOR => {
                    if !ctx.has_quest_items(player, SECOND_CIRCLE_HUNTER_LICENSE)
                        && !ctx.has_quest_items(player, FIRST_CIRCLE_HUNTER_LICENSE)
                    {
                        htmltext = set("30745-01a.html");
                    } else {
                        let request_count = ctx.get_quest_items_count_of(player, &REQUESTS);
                        if ctx.has_quest_items(player, FIRST_CIRCLE_HUNTER_LICENSE) {
                            if request_count == 0 {
                                if ctx.player_level(player) < MAX_LEVEL {
                                    htmltext = set("30745-01b.html");
                                } else if ctx.has_quest_items(player, SECOND_CIRCLE_HUNTER_LICENSE) {
                                    htmltext = set("30745-03.html");
                                } else {
                                    htmltext = set("30745-03a.html");
                                }
                            } else {
                                htmltext = reward(ctx, player, qs);
                            }
                        } else if ctx.has_quest_items(player, SECOND_CIRCLE_HUNTER_LICENSE) {
                            if request_count == 0 {
                                htmltext = set("30745-03b.html");
                            } else {
                                htmltext = reward(ctx, player, qs);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        htmltext
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let qs = ctx.get_quest_state(player, false)?;
        let mut htmltext = None;
        let event_page = Some(event.to_owned());
        match event {
            "30744-03.htm" => {
                qs.start_quest(ctx);
                if !ctx.has_quest_items(player, TEST_INSTRUCTIONS_1) {
                    ctx.give_items(player, TEST_INSTRUCTIONS_1, 1);
                }

                qs.set_memo_state(ctx, 0);
                htmltext = event_page;
            }
            "30744-04.html" | "30744-04a.html" | "30744-04b.html" | "30744-04c.html" | "30744-04d.html"
            | "30744-04e.html" | "30744-04f.html" | "30744-07.html" | "30744-07a.html" | "30744-07b.html"
            | "30744-08.html" | "30744-08a.html" | "30744-10.html" | "30744-10a.html" | "30744-10b.html"
            | "30744-10c.html" | "30744-10d.html" | "30744-10e.html" | "30744-10f.html" | "30744-14.html"
            | "30744-14a.html" | "30744-15.html" | "30744-18.html" | "30745-09.html" | "30746-03a.html"
            | "30746-07.html" | "30745-04.html" | "30745-05a.html" | "30745-05c.html" => {
                htmltext = event_page;
            }
            "30744-09.html" => {
                if sum(ctx, player, &FIRST_CIRCLE_SUM) == 0 {
                    ctx.give_items(player, TEST_INSTRUCTIONS_2, 1);
                    htmltext = event_page;
                } else {
                    htmltext = Some("30744-09a.html".to_owned());
                }
            }
            "30744-16.html" => {
                qs.exit_quest(ctx, true, true);
                let mut items: Vec<i32> = vec![
                    FIRST_CIRCLE_HUNTER_LICENSE,
                    SECOND_CIRCLE_HUNTER_LICENSE,
                    LAUREL_LEAF_PIN,
                    TEST_INSTRUCTIONS_1,
                    TEST_INSTRUCTIONS_2,
                    CYBELLINS_REQUEST,
                ];
                items.extend_from_slice(&BLOOD_CRYSTALS);
                items.extend_from_slice(&[
                    BROKEN_BLOOD_CRYSTAL,
                    CYBELLINS_DAGGER,
                    GUARDIAN_BASILISK_SCALE,
                    KARUT_WEED,
                    HAKAS_HEAD,
                    JAKAS_HEAD,
                    MARKAS_HEAD,
                    WINDSUS_ALEPH_SKIN,
                    INDIGO_SPIRIT_ORE,
                    SPORESEA_SEED,
                    TIMAK_ORC_TOTEM,
                    TRISALIM_SILK,
                    AMBROSIUS_FRUIT,
                    BALEFIRE_CRYSTAL,
                    IMPERIAL_ARROWHEAD,
                    ATHUS_HEAD,
                    LANKAS_HEAD,
                    TRISKAS_HEAD,
                    MOTURAS_HEAD,
                    KALATHS_HEAD,
                ]);
                items.extend_from_slice(&REQUESTS);
                items.extend_from_slice(&QUEST_ITEMS[QUEST_ITEMS.len() - 41..]);
                ctx.take_items_many(player, -1, &items);

                // TODO(Zoey76): This is dead code.
                if ctx.get_quest_items_count(player, LAUREL_LEAF_PIN) < 20 {
                    htmltext = event_page;
                } else {
                    ctx.give_adena(player, 20000, true);
                    htmltext = Some("30744-17.html".to_owned());
                }
            }
            "30745-02.html" => {
                if !ctx.has_quest_items(player, TEST_INSTRUCTIONS_2) {
                    htmltext = event_page;
                } else {
                    htmltext = Some("30745-03.html".to_owned());
                }
            }
            "LIST_1" => {
                let mut i = [0i32; 5];
                if qs.is_memo_state(ctx, 0) {
                    while has_duplicates(i) {
                        if !ctx.has_quest_items(player, LAUREL_LEAF_PIN) {
                            i = [
                                ctx.get_random(12),
                                ctx.get_random(12),
                                ctx.get_random(12),
                                ctx.get_random(12),
                                ctx.get_random(12),
                            ];
                            qs.set_memo_state(ctx, pack(i));
                        } else if ctx.get_quest_items_count(player, LAUREL_LEAF_PIN) < 4 {
                            if ctx.get_random(100) < 20 {
                                i[0] = ctx.get_random(6) + 12;
                                i[1] = ctx.get_random(12);
                                i[2] = ctx.get_random(6);
                                i[3] = ctx.get_random(6) + 6;
                                i[4] = ctx.get_random(12);
                                qs.set_memo_state(ctx, pack(i));
                            } else {
                                i = [
                                    ctx.get_random(12),
                                    ctx.get_random(12),
                                    ctx.get_random(12),
                                    ctx.get_random(12),
                                    ctx.get_random(12),
                                ];
                                qs.set_memo_state(ctx, pack(i));
                            }
                        } else if ctx.get_random(100) < 20 {
                            i[0] = ctx.get_random(6) + 12;
                            i[1] = if ctx.get_random(20) == 0 {
                                ctx.get_random(2) + 18
                            } else {
                                ctx.get_random(12)
                            };
                            i[2] = ctx.get_random(6);
                            i[3] = ctx.get_random(6) + 6;
                            i[4] = ctx.get_random(12);
                            qs.set_memo_state(ctx, pack(i));
                        } else {
                            i[0] = ctx.get_random(12);
                            i[1] = if ctx.get_random(20) == 0 {
                                ctx.get_random(2) + 18
                            } else {
                                ctx.get_random(12)
                            };
                            i[2] = ctx.get_random(6);
                            i[3] = ctx.get_random(6) + 6;
                            i[4] = ctx.get_random(12);
                            qs.set_memo_state(ctx, pack(i));
                        }
                    }

                    htmltext = get_html(ctx, player, "30745-16.html", i.map(|n| n + 33520));
                } else {
                    let mut i5 = qs.get_memo_state(ctx);
                    let mut decoded = [0i32; 5];
                    for slot in &mut decoded {
                        *slot = i5 % 32;
                        i5 /= 32;
                    }
                    let [i0, i1, i2, i3, i4] = decoded.map(|n| n + 33520);
                    htmltext = get_html(ctx, player, "30745-16.html", [i4, i3, i2, i1, i0]);
                }
            }
            "LIST_2" => {
                let mut i = [0i32; 5];
                if qs.is_memo_state(ctx, 0) {
                    while has_duplicates(i) {
                        if !ctx.has_quest_items(player, LAUREL_LEAF_PIN) {
                            i[0] = ctx.get_random(10);
                            i[1] = ctx.get_random(10);
                            i[2] = ctx.get_random(5);
                            i[3] = ctx.get_random(5) + 5;
                            i[4] = ctx.get_random(10);
                            qs.set_memo_state(ctx, pack(i));
                        } else if ctx.get_quest_items_count(player, LAUREL_LEAF_PIN) < 4 {
                            if ctx.get_random(100) < 20 {
                                i[0] = ctx.get_random(6) + 10;
                                i[1] = ctx.get_random(10);
                                i[2] = ctx.get_random(5);
                                i[3] = ctx.get_random(5) + 5;
                                i[4] = ctx.get_random(10);
                                qs.set_memo_state(ctx, pack(i));
                            } else {
                                i[0] = ctx.get_random(10);
                                i[1] = ctx.get_random(10);
                                i[2] = ctx.get_random(5);
                                i[3] = ctx.get_random(5) + 5;
                                i[4] = ctx.get_random(10);
                                qs.set_memo_state(ctx, pack(i));
                            }
                        } else if ctx.get_random(100) < 20 {
                            i[0] = ctx.get_random(6) + 10;
                            i[1] = if ctx.get_random(20) == 0 {
                                ctx.get_random(3) + 16
                            } else {
                                ctx.get_random(10)
                            };
                            i[2] = ctx.get_random(5);
                            i[3] = ctx.get_random(5) + 5;
                            i[4] = ctx.get_random(10);
                            qs.set_memo_state(ctx, pack(i));
                        } else {
                            i[0] = ctx.get_random(10);
                            i[1] = if ctx.get_random(20) == 0 {
                                ctx.get_random(3) + 16
                            } else {
                                ctx.get_random(10)
                            };
                            i[2] = ctx.get_random(5);
                            i[3] = ctx.get_random(5) + 5;
                            i[4] = ctx.get_random(10);
                            qs.set_memo_state(ctx, pack(i));
                        }
                    }

                    htmltext = get_html(ctx, player, "30745-16.html", i.map(|n| 33520 + (n + 20)));
                } else {
                    let mut i5 = qs.get_memo_state(ctx);
                    let mut decoded = [0i32; 5];
                    for slot in &mut decoded {
                        *slot = i5 % 32;
                        i5 /= 32;
                    }
                    let [i0, i1, i2, i3, i4] = decoded.map(|n| 33520 + (n + 20));
                    htmltext = get_html(ctx, player, "30745-16.html", [i4, i3, i2, i1, i0]);
                }
            }
            "30746-03.html" => {
                if !ctx.has_quest_items(player, CYBELLINS_DAGGER) {
                    ctx.give_items(player, CYBELLINS_DAGGER, 1);
                }

                if ctx.get_quest_items_count(player, CYBELLINS_REQUEST) == 0 {
                    ctx.give_items(player, CYBELLINS_REQUEST, 1);
                }

                ctx.give_items(player, BLOOD_CRYSTAL_PURITY_1, 1);
                if ctx.has_quest_items(player, BROKEN_BLOOD_CRYSTAL) {
                    ctx.take_items(player, BROKEN_BLOOD_CRYSTAL, -1);
                }

                htmltext = event_page;
            }
            "30746-06.html" => {
                // The crystals of purity 2 to 10, the first one the player holds pays.
                for (n, &adena) in CRYSTAL_ADENA.iter().enumerate() {
                    if ctx.has_quest_items(player, BLOOD_CRYSTALS[n + 1]) {
                        ctx.give_adena(player, adena, true);
                        break;
                    }
                }

                ctx.take_items_many(player, -1, &BLOOD_CRYSTALS[1..]);
                htmltext = event_page;
            }
            "30746-10.html" => {
                ctx.take_items(player, BLOOD_CRYSTAL_PURITY_1, -1);
                ctx.take_items(player, CYBELLINS_DAGGER, -1);
                ctx.take_items(player, CYBELLINS_REQUEST, -1);
                htmltext = event_page;
            }
            "30745-05b.html" => {
                htmltext = event_page;
                if ctx.has_quest_items(player, LAUREL_LEAF_PIN) {
                    ctx.take_items(player, LAUREL_LEAF_PIN, 1);
                }

                ctx.take_items_many(player, -1, &REQUESTS);
                ctx.take_items_many(player, -1, &QUEST_ITEMS[QUEST_ITEMS.len() - 41..]);
            }
            _ => {
                // The request pages ("30745-10a.html" ...) hand out their request.
                if let Some(&(_, request)) = REQUEST_PAGES
                    .iter()
                    .find(|(page, _)| !page.is_empty() && *page == event)
                {
                    ctx.give_items(player, request, 1);
                    htmltext = event_page;
                }
            }
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.random_party_member_state(player, -1, 3, Some(npc)) else {
            return;
        };
        let member = qs.player();
        for &(monster, request, item, amount, limit, chance) in DROPLIST.iter().rev() {
            if npc.id == monster && ctx.has_quest_items(member, request) {
                let amount = drop_amount(ctx, amount);
                if ctx.give_item_randomly(
                    member,
                    Some(npc),
                    item,
                    amount,
                    amount,
                    limit,
                    f64::from(chance) / 100.0,
                    true,
                ) {
                    ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
                }
            }
        }

        match npc.id {
            BREKA_ORC_WARRIOR => {
                if ctx.has_quest_items(member, TEST_INSTRUCTIONS_1)
                    && sum(ctx, member, &[HAKAS_HEAD, JAKAS_HEAD, MARKAS_HEAD]) < 3
                    && ctx.get_random(10) < 2
                {
                    if !ctx.has_quest_items(member, HAKAS_HEAD) {
                        spawn_near(ctx, BREKA_OVERLORD_HAKA, npc);
                    } else if !ctx.has_quest_items(member, JAKAS_HEAD) {
                        spawn_near(ctx, BREKA_OVERLORD_JAKA, npc);
                    } else if !ctx.has_quest_items(member, MARKAS_HEAD) {
                        spawn_near(ctx, BREKA_OVERLORD_MARKA, npc);
                    }
                }
            }
            WINDSUS => {
                if ctx.has_quest_items(member, TEST_INSTRUCTIONS_1)
                    && !ctx.has_quest_items(member, WINDSUS_ALEPH_SKIN)
                    && ctx.get_random(10) < 2
                {
                    spawn_near(ctx, WINDSUS_ALEPH, npc);
                }
            }
            GRANDIS => {
                let roll = !ctx.has_quest_items(member, TITANS_TABLET)
                    && ctx.has_quest_items(member, FIRST_CIRCLE_REQUEST_2A)
                    && ctx.get_random(10) < 2;
                if roll {
                    spawn_near(ctx, GOK_MAGOK, npc);
                }
            }
            TARLK_BUGBEAR_WARRIOR => {
                if ctx.has_quest_items(member, TEST_INSTRUCTIONS_2)
                    && sum(
                        ctx,
                        member,
                        &[ATHUS_HEAD, LANKAS_HEAD, TRISKAS_HEAD, MOTURAS_HEAD, KALATHS_HEAD],
                    ) < 5
                    && ctx.get_random(10) < 2
                {
                    if !ctx.has_quest_items(member, ATHUS_HEAD) {
                        spawn_near(ctx, TARLK_RAIDER_ATHU, npc);
                    } else if !ctx.has_quest_items(member, LANKAS_HEAD) {
                        spawn_near(ctx, TARLK_RAIDER_LANKA, npc);
                    } else if !ctx.has_quest_items(member, TRISKAS_HEAD) {
                        spawn_near(ctx, TARLK_RAIDER_TRISKA, npc);
                    } else if !ctx.has_quest_items(member, MOTURAS_HEAD) {
                        spawn_near(ctx, TARLK_RAIDER_MOTURA, npc);
                    } else if !ctx.has_quest_items(member, KALATHS_HEAD) {
                        spawn_near(ctx, TARLK_RAIDER_KALATH, npc);
                    }
                }
            }
            LETO_LIZARDMAN_SHAMAN => {
                // The request and item are tested before the roll, as in Java's `&&` chain.
                if ctx.has_quest_items(member, FIRST_CIRCLE_REQUEST_4B)
                    && !ctx.has_quest_items(member, TOTEM_OF_KADESH)
                    && ctx.get_random(10) < 2
                {
                    spawn_near(ctx, LETO_SHAMAN_KETZ, npc);
                }

                evolve_blood_crystal(ctx, member);
            }
            LETO_LIZARDMAN_OVERLORD => {
                if ctx.has_quest_items(member, FIRST_CIRCLE_REQUEST_1B)
                    && !ctx.has_quest_items(member, FANG_OF_NARAK)
                    && ctx.get_random(10) < 2
                {
                    spawn_near(ctx, LETO_CHIEF_NARAK, npc);
                }

                evolve_blood_crystal(ctx, member);
            }
            TIMAK_ORC_WARRIOR => {
                if ctx.has_quest_items(member, FIRST_CIRCLE_REQUEST_5B)
                    && !ctx.has_quest_items(member, KAIKIS_HEAD)
                    && ctx.get_random(10) < 2
                {
                    spawn_near(ctx, TIMAK_RAIDER_KAIKEE, npc);
                }
            }
            TIMAK_ORC_OVERLORD => {
                if ctx.has_quest_items(member, SECOND_CIRCLE_REQUEST_1B)
                    && !ctx.has_quest_items(member, OKUNS_HEAD)
                    && ctx.get_random(10) == 0
                {
                    spawn_near(ctx, TIMAK_OVERLORD_OKUN, npc);
                }
            }
            FLINE | LIELE => {
                spawn_filcher(ctx, member, npc, SECOND_CIRCLE_REQUEST_7C, TEMPEST_SHARD, 40);
            }
            FOREST_RUNNER => {
                spawn_filcher(ctx, member, npc, SECOND_CIRCLE_REQUEST_10C, HAMADRYAD_SHARD, 40);
            }
            KARUL_BUGBEAR => {
                if ctx.has_quest_items(member, FIRST_CIRCLE_REQUEST_3A)
                    && !ctx.has_quest_items(member, BOOK_OF_SHUNAIMAN)
                    && ctx.get_random(10) < 2
                {
                    spawn_near(ctx, KARUL_CHIEF_OROOTO, npc);
                }
            }
            TAIK_ORC_CAPTAIN => {
                if ctx.has_quest_items(member, SECOND_CIRCLE_REQUEST_2B)
                    && !ctx.has_quest_items(member, KAKRANS_HEAD)
                    && ctx.get_random(10) < 2
                {
                    spawn_near(ctx, TAIK_OVERLORD_KAKRAN, npc);
                }
            }
            MIRROR => {
                spawn_filcher(
                    ctx,
                    member,
                    npc,
                    SECOND_CIRCLE_REQUEST_3B,
                    NARCISSUSS_SOULSTONE,
                    40,
                );
            }
            HATAR_RATMAN_THIEF => {
                spawn_filcher(ctx, member, npc, SECOND_CIRCLE_REQUEST_5C, COIN_OF_OLD_EMPIRE, 20);
            }
            HATAR_RATMAN_BOSS => {
                spawn_filcher(ctx, member, npc, SECOND_CIRCLE_REQUEST_5C, COIN_OF_OLD_EMPIRE, 20);

                if ctx.has_quest_items(member, SECOND_CIRCLE_REQUEST_2A)
                    && !ctx.has_quest_items(member, BUST_OF_TRAVIS)
                    && ctx.get_random(10) < 2
                {
                    spawn_near(ctx, HATAR_CHIEFTAIN_KUBEL, npc);
                }
            }
            VANOR_SILENOS_CHIEFTAIN => {
                if ctx.has_quest_items(member, SECOND_CIRCLE_REQUEST_6B)
                    && !ctx.has_quest_items(member, KERUNOSS_GOLD_MANE)
                    && ctx.get_random(10) < 2
                {
                    spawn_near(ctx, VANOR_ELDER_KERUNOS, npc);
                }
            }
            GREMLIN_FILCHER => {
                for (request, item, amount, limit) in [
                    (SECOND_CIRCLE_REQUEST_7C, TEMPEST_SHARD, 5, 40),
                    (SECOND_CIRCLE_REQUEST_8C, TSUNAMI_SHARD, 5, 40),
                    (SECOND_CIRCLE_REQUEST_10C, HAMADRYAD_SHARD, 5, 40),
                    (SECOND_CIRCLE_REQUEST_3B, NARCISSUSS_SOULSTONE, 5, 40),
                    (SECOND_CIRCLE_REQUEST_5C, COIN_OF_OLD_EMPIRE, 3, 20),
                ] {
                    if ctx.has_quest_items(member, request) && ctx.get_quest_items_count(member, item) < limit
                    {
                        if ctx.give_item_randomly(member, Some(npc), item, amount, amount, limit, 1.0, true) {
                            ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
                        }

                        ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, NpcStringId::PrettyGood, &[]);
                    }
                }
            }
            GOK_MAGOK => {
                stormtroopers(ctx, member, npc, (FIRST_CIRCLE_REQUEST_2A, TITANS_TABLET));
            }
            HATAR_CHIEFTAIN_KUBEL => {
                stormtroopers(ctx, member, npc, (SECOND_CIRCLE_REQUEST_2A, BUST_OF_TRAVIS));
            }
            KARUL_CHIEF_OROOTO => {
                stormtroopers(ctx, member, npc, (FIRST_CIRCLE_REQUEST_3A, BOOK_OF_SHUNAIMAN));
            }
            LETO_LIZARDMAN_ARCHER
            | LETO_LIZARDMAN_SOLDIER
            | HARIT_LIZARDMAN_GRUNT
            | HARIT_LIZARDMAN_ARCHER
            | HARIT_LIZARDMAN_WARRIOR => {
                evolve_blood_crystal(ctx, member);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NPCS: &str = r#"
        <npc id="30744" type="Folk" level="20" name="Grey"/>
        <npc id="30745" type="Folk" level="20" name="Tor"/>
        <npc id="30746" type="Folk" level="20" name="Cybellin"/>
        <npc id="20550" type="Monster" level="40" name="Guardian Basilisk"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="3692" type="EtcItem" name="First Circle Hunter License"><set name="is_questitem" val="true"/></item>
        <item id="3694" type="EtcItem" name="Laurel Leaf Pin"><set name="is_questitem" val="true"/></item>
        <item id="3695" type="EtcItem" name="Test Instructions 1"><set name="is_questitem" val="true"/></item>
        <item id="3709" type="EtcItem" name="Guardian Basilisk Scale"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="3710" type="EtcItem" name="Karut Weed"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="3716" type="EtcItem" name="Sporesea Seed"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="3727" type="EtcItem" name="First Circle Request 1C"><set name="is_questitem" val="true"/></item>
        <item id="3769" type="EtcItem" name="Charm of Kadesh"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00335TheSongOfTheHunter);
        let pages = Pages::of(&Q00335TheSongOfTheHunter)
            .page("30744-02.htm", &["30744-03.htm"])
            .page("30744-03.htm", &["30744-16.html"])
            .plain(&[
                "30744-01.htm",
                "30744-16.html",
                "30745-06a.html",
                "30745-10a.html",
            ])
            .page("30745-03a.html", &["LIST_1", "30745-10a.html"])
            .file(
                "data/scripts/quests/Q00335_TheSongOfTheHunter/30745-16.html",
                "<html><body><?reply1?><?reply2?><?reply3?><?reply4?><?reply5?></body></html>",
            )
            .npcs(&[30744, 30745, 30746]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let grey = t.spawn(30744, 20);
        let tor = t.spawn(30745, 40);
        (t, grey, tor)
    }

    #[test]
    fn a_low_level_character_is_refused() {
        let (mut t, grey, _) = setup();
        t.enter(1, 34, vec![]);
        assert!(t.quest_page(1, grey).contains("30744-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn grey_starts_the_quest_with_the_first_test() {
        let (mut t, grey, _) = setup();
        t.enter(1, 35, vec![]);
        assert!(t.quest_page(1, grey).contains("30744-02.htm"));
        assert!(t.event(1, NAME, "30744-03.htm").contains("30744-03.htm"));
        assert_eq!(t.count(1, TEST_INSTRUCTIONS_1), 1);
        assert_eq!(
            t.take_writes()[..2],
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
    }

    #[test]
    fn the_first_test_is_passed_with_three_of_six_parts() {
        let (mut t, grey, _) = setup();
        t.enter(1, 35, vec![]);
        t.quest_page(1, grey);
        t.event(1, NAME, "30744-03.htm");
        t.take_writes();
        // Three parts in the bag: the basilisk scales and the other two the test wants.
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), GUARDIAN_BASILISK_SCALE, 40);
            ctx.give_items(Oid(1), KARUT_WEED, 20);
            ctx.give_items(Oid(1), SPORESEA_SEED, 30);
        });
        t.quest_page(1, grey);
        assert_eq!(t.count(1, FIRST_CIRCLE_HUNTER_LICENSE), 1);
        assert_eq!(t.count(1, TEST_INSTRUCTIONS_1), 0);
        assert_eq!(t.count(1, GUARDIAN_BASILISK_SCALE), 0);
        assert!(t.take_writes().contains(&rows::update(NAME, "cond", "2")));
    }

    #[test]
    fn tor_pays_a_filled_request_with_a_laurel_leaf_pin() {
        let (mut t, grey, tor) = setup();
        t.enter(1, 45, vec![]);
        t.quest_page(1, grey);
        t.event(1, NAME, "30744-03.htm");
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), FIRST_CIRCLE_HUNTER_LICENSE, 1);
            ctx.give_items(Oid(1), FIRST_CIRCLE_REQUEST_1C, 1);
            ctx.give_items(Oid(1), CHARM_OF_KADESH, 40);
        });
        t.take_writes();
        t.quest_page(1, tor);
        assert_eq!(t.count(1, LAUREL_LEAF_PIN), 1);
        assert_eq!(t.count(1, FIRST_CIRCLE_REQUEST_1C), 0);
        assert_eq!(t.count(1, CHARM_OF_KADESH), 0);
        assert_eq!(t.count(1, 57), 2090);
    }

    #[test]
    fn the_basilisk_gives_scales_for_the_first_test() {
        let (mut t, grey, _) = setup();
        t.enter(1, 35, vec![item(10, TEST_INSTRUCTIONS_1, 1)]);
        t.quest_page(1, grey);
        t.event(1, NAME, "30744-03.htm");
        for _ in 0..60 {
            let m = t.spawn(20550, 20);
            t.kill(1, m);
        }
        assert!(t.count(1, GUARDIAN_BASILISK_SCALE) > 0);
    }

    #[test]
    fn tor_lists_five_different_requests_and_remembers_them() {
        let (mut t, grey, tor) = setup();
        t.enter(1, 45, vec![]);
        t.quest_page(1, grey);
        t.event(1, NAME, "30744-03.htm");
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), FIRST_CIRCLE_HUNTER_LICENSE, 1);
        });
        assert!(t.quest_page(1, tor).contains("30745-03a.html"));
        t.take_writes();
        let page = t.event(1, NAME, "LIST_1");
        assert_eq!(
            page.matches("bypass -h Script Q00335_TheSongOfTheHunter 30745-1")
                .count(),
            5
        );
        let mut memo = 0;
        t.world.run_as_script(NAME, |ctx| {
            memo = ctx.get_quest_state(Oid(1), false).unwrap().get_memo_state(ctx);
        });
        assert_ne!(memo, 0);
    }

    #[test]
    fn the_exit_takes_everything_and_ends_the_quest() {
        let (mut t, grey, _) = setup();
        t.enter(1, 35, vec![]);
        t.quest_page(1, grey);
        t.event(1, NAME, "30744-03.htm");
        t.take_writes();
        assert!(t.event(1, NAME, "30744-16.html").contains("30744-16.html"));
        assert_eq!(t.count(1, TEST_INSTRUCTIONS_1), 0);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
