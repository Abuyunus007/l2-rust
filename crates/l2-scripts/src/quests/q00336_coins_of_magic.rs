//! Quest 336: Coins of Magic (Warehouse Keeper Sorint in the Town of Gludin).
// Java: dist/game/data/scripts/quests/Q00336_CoinsOfMagic/Q00336_CoinsOfMagic.java
//
// Pattern: a long exchange quest. The nine "coin" NPCs (Pano, Collob, Rapin, Hagger, Stan, Lorain,
// Duning, Page, Ferris) all run the same three-step game, with their own item tables: the player
// pays coins for a number to guess (`param1` packs three base-4 digits, `flag` collects the
// guesses), trades coins, and Sorint pays rewards in exchange for the finished coin sets.
// The Java `shortFirstSteps`/`shortSecondStep*`/`shortThirdStep` helpers are the free functions
// below; each NPC's arguments are its row in `config`.

use l2_util::ObjectId;
use l2_world::script::{QuestSound, STARTED};
use l2_world::{Qs, Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const PANO: i32 = 30078;
const COLLOB: i32 = 30092;
const RAPIN: i32 = 30165;
const HAGGER: i32 = 30183;
const STAN: i32 = 30200;
const WAREHOUSE_KEEPER_SORINT: i32 = 30232;
const RESEARCHER_LORAIN: i32 = 30673;
const BLACKSMITH_DUNING: i32 = 30688;
const MAGISTER_PAGE: i32 = 30696;
const UNION_PRESIDENT_BERNARD: i32 = 30702;
const HEAD_BLACKSMITH_FERRIS: i32 = 30847;

// Items
const Q_BLOOD_MEDUSA: i32 = 3472;
const Q_BLOOD_WEREWOLF: i32 = 3473;
const Q_BLOOD_BASILISK: i32 = 3474;
const Q_BLOOD_DREVANUL: i32 = 3475;
const Q_BLOOD_SUCCUBUS: i32 = 3476;
const Q_BLOOD_DRAGON: i32 = 3477;
const Q_BERETHS_BLOOD_DRAGON: i32 = 3478;
const Q_MANAKS_BLOOD_WEREWOLF: i32 = 3479;
const Q_NIAS_BLOOD_MEDUSA: i32 = 3480;
const Q_GOLD_DRAGON: i32 = 3481;
const Q_GOLD_WYVERN: i32 = 3482;
const Q_GOLD_KNIGHT: i32 = 3483;
const Q_GOLD_GIANT: i32 = 3484;
const Q_GOLD_DRAKE: i32 = 3485;
const Q_GOLD_WYRM: i32 = 3486;
const Q_BERETHS_GOLD_DRAGON: i32 = 3487;
const Q_MANAKS_GOLD_GIANT: i32 = 3488;
const Q_NIAS_GOLD_WYVERN: i32 = 3489;
const Q_SILVER_UNICORN: i32 = 3490;
const Q_SILVER_FAIRY: i32 = 3491;
const Q_SILVER_DRYAD: i32 = 3492;
const Q_SILVER_DRAGON: i32 = 3493;
const Q_SILVER_GOLEM: i32 = 3494;
const Q_SILVER_UNDINE: i32 = 3495;
const Q_BERETHS_SILVER_DRAGON: i32 = 3496;
const Q_MANAKS_SILVER_DRYAD: i32 = 3497;
const Q_NIAS_SILVER_FAIRY: i32 = 3498;
const Q_COIN_DIAGRAM: i32 = 3811;
const Q_KALDIS_GOLD_DRAGON: i32 = 3812;
const Q_CC_MEMBERSHIP_1: i32 = 3813;
const Q_CC_MEMBERSHIP_2: i32 = 3814;
const Q_CC_MEMBERSHIP_3: i32 = 3815;

// Monsters
const HEADLESS_KNIGHT: i32 = 20146;
const OEL_MAHUM: i32 = 20161;
const SHACKLE: i32 = 20235;
const ROYAL_CAVE_SERVANT: i32 = 20240;
const MALRUK_SUCCUBUS_TUREN: i32 = 20245;
const ROYAL_CAVE_SERVANT_HOLD: i32 = 20276;
const SHACKLE_HOLD: i32 = 20279;
const HEADLESS_KNIGHT_HOLD: i32 = 20280;
const H_MALRUK_SUCCUBUS_TUREN: i32 = 20284;
const BYFOOT: i32 = 20568;
const BYFOOT_SIGEL: i32 = 20569;
const TARLK_BUGBEAR_BOSS: i32 = 20572;
const OEL_MAHUM_WARRIOR: i32 = 20575;
const OEL_MAHUM_WITCH_DOCTOR: i32 = 20576;
const TIMAK_ORC: i32 = 20583;
const TIMAK_ORC_ARCHER: i32 = 20584;
const TIMAK_ORC_SOLDIER: i32 = 20585;
const TIMAK_ORC_SHAMAN: i32 = 20587;
const LAKIN: i32 = 20604;
const HARIT_LIZARDMAN_SHAMAN: i32 = 20644;
const HARIT_LIZARDM_MATRIARCH: i32 = 20645;
const HATAR_HANISHEE: i32 = 20663;
const DOOM_KNIGHT: i32 = 20674;
const PUNISHMENT_OF_UNDEAD: i32 = 20678;
const VANOR_SILENOS_SHAMAN: i32 = 20685;
const HUNGRY_CORPSE: i32 = 20954;
const NIHIL_INVADER: i32 = 20957;
const DARK_GUARD: i32 = 20959;
const BLOODY_GHOST: i32 = 20960;
const FLOAT_OF_GRAVE: i32 = 21003;
const DOOM_SERVANT: i32 = 21006;
const DOOM_ARCHER: i32 = 21008;
const KUKABURO: i32 = 21274;
const KUKABURO_A: i32 = 21275;
const KUKABURO_B: i32 = 21276;
const ANTELOPE: i32 = 21278;
const ANTELOPE_A: i32 = 21279;
const ANTELOPE_B: i32 = 21280;
const BANDERSNATCH: i32 = 21282;
const BANDERSNATCH_A: i32 = 21283;
const BANDERSNATCH_B: i32 = 21284;
const BUFFALO: i32 = 21286;
const BUFFALO_A: i32 = 21287;
const BUFFALO_B: i32 = 21288;
const BRILLIANT_CLAW: i32 = 21521;
const BRILLIANT_CLAW_1: i32 = 21522;
const BRILLIANT_WISDOM: i32 = 21526;
const BRILLIANT_VENGEANCE: i32 = 21531;
const BRILLIANT_VENGEANCE_1: i32 = 21658;
const BRILLIANT_ANGUISH: i32 = 21539;
const BRILLIANT_ANGUISH_1: i32 = 21540;

// Rewards
const DEMON_STAFF: i32 = 206;
const DARK_SCREAMER: i32 = 233;
const WIDOW_MAKER: i32 = 303;
const SWORD_OF_LIMIT: i32 = 132;
const DEMONS_BOOTS: i32 = 2435;
const DEMONS_HOSE: i32 = 472;
const DEMONS_GLOVES: i32 = 2459;
const FULL_PLATE_HELMET: i32 = 2414;
const MOONSTONE_EARING: i32 = 852;
const NASSENS_EARING: i32 = 855;
const RING_OF_BINDING: i32 = 886;
const NECKLACE_OF_PROTECTION: i32 = 916;

// Variables name
const WEIGHT_POINT: &str = "weight_point";
const PARAM_1: &str = "param1";
const PARAM_2: &str = "param2";
const PARAM_3: &str = "param3";
const FLAG: &str = "flag";

const MOBS: &[i32] = &[
    HEADLESS_KNIGHT,
    OEL_MAHUM,
    SHACKLE,
    ROYAL_CAVE_SERVANT,
    MALRUK_SUCCUBUS_TUREN,
    ROYAL_CAVE_SERVANT_HOLD,
    SHACKLE_HOLD,
    HEADLESS_KNIGHT_HOLD,
    H_MALRUK_SUCCUBUS_TUREN,
    BYFOOT,
    BYFOOT_SIGEL,
    TARLK_BUGBEAR_BOSS,
    OEL_MAHUM_WARRIOR,
    OEL_MAHUM_WITCH_DOCTOR,
    TIMAK_ORC,
    TIMAK_ORC_ARCHER,
    TIMAK_ORC_SOLDIER,
    TIMAK_ORC_SHAMAN,
    LAKIN,
    HARIT_LIZARDMAN_SHAMAN,
    HARIT_LIZARDM_MATRIARCH,
    HATAR_HANISHEE,
    DOOM_KNIGHT,
    PUNISHMENT_OF_UNDEAD,
    VANOR_SILENOS_SHAMAN,
    HUNGRY_CORPSE,
    NIHIL_INVADER,
    DARK_GUARD,
    BLOODY_GHOST,
    FLOAT_OF_GRAVE,
    DOOM_SERVANT,
    DOOM_ARCHER,
    KUKABURO,
    KUKABURO_A,
    KUKABURO_B,
    ANTELOPE,
    ANTELOPE_A,
    ANTELOPE_B,
    BANDERSNATCH,
    BANDERSNATCH_A,
    BANDERSNATCH_B,
    BUFFALO,
    BUFFALO_A,
    BUFFALO_B,
    BRILLIANT_CLAW,
    BRILLIANT_CLAW_1,
    BRILLIANT_WISDOM,
    BRILLIANT_VENGEANCE,
    BRILLIANT_VENGEANCE_1,
    BRILLIANT_ANGUISH,
    BRILLIANT_ANGUISH_1,
];

/// Java `Q00336_CoinsOfMagic`.
pub struct Q00336CoinsOfMagic;

/// The arguments one coin NPC gives the Java `short*` helpers.
struct Cfg {
    /// `shortFirstSteps` `base` for the events 6, 7, 8.
    base: [i32; 3],
    /// `item1a, item1b, item1Mul, item2, item3, item4`.
    first: [i32; 6],
    second: Second,
    /// `shortThirdStep` `item1..item4`.
    third: [i32; 4],
}

enum Second {
    /// `shortSecondStepTwoItems`: `item1a, item1b, reward1, ..., item4a, item4b, reward4`.
    Two([i32; 12]),
    /// `shortSecondStepOneItem`: `item1, item1Mul, reward1, item2, reward2, item3, reward3,
    /// item4, reward4`.
    One([i32; 9]),
}

fn config(npc_id: i32) -> Option<Cfg> {
    let cfg = match npc_id {
        PANO => Cfg {
            base: [4, 8, 9],
            first: [
                Q_SILVER_DRYAD,
                Q_SILVER_UNDINE,
                1,
                Q_GOLD_GIANT,
                Q_SILVER_DRYAD,
                Q_BLOOD_BASILISK,
            ],
            second: Second::Two([
                Q_SILVER_DRYAD,
                Q_SILVER_UNDINE,
                Q_BERETHS_SILVER_DRAGON,
                Q_GOLD_GIANT,
                Q_GOLD_WYRM,
                Q_GOLD_DRAGON,
                Q_SILVER_DRYAD,
                Q_SILVER_UNDINE,
                Q_SILVER_DRAGON,
                Q_BLOOD_BASILISK,
                Q_BLOOD_SUCCUBUS,
                Q_BLOOD_DRAGON,
            ]),
            // NOTE(java-quirk): the fourth item is Q_SILVER_DRAGON again, as in Java.
            third: [
                Q_BERETHS_SILVER_DRAGON,
                Q_GOLD_DRAGON,
                Q_SILVER_DRAGON,
                Q_SILVER_DRAGON,
            ],
        },
        COLLOB => Cfg {
            base: [4, 8, 9],
            first: [
                Q_GOLD_WYRM,
                Q_GOLD_GIANT,
                1,
                Q_GOLD_WYRM,
                Q_SILVER_UNDINE,
                Q_BLOOD_SUCCUBUS,
            ],
            second: Second::Two([
                Q_GOLD_GIANT,
                Q_GOLD_WYRM,
                Q_BERETHS_GOLD_DRAGON,
                Q_GOLD_GIANT,
                Q_GOLD_WYRM,
                Q_GOLD_DRAGON,
                Q_SILVER_DRYAD,
                Q_SILVER_UNDINE,
                Q_SILVER_DRAGON,
                Q_BLOOD_BASILISK,
                Q_BLOOD_SUCCUBUS,
                Q_BLOOD_DRAGON,
            ]),
            third: [
                Q_BERETHS_GOLD_DRAGON,
                Q_GOLD_DRAGON,
                Q_SILVER_DRAGON,
                Q_BLOOD_DRAGON,
            ],
        },
        RAPIN => Cfg {
            base: [3, 7, 9],
            first: [
                Q_BLOOD_WEREWOLF,
                Q_BLOOD_DREVANUL,
                1,
                Q_SILVER_GOLEM,
                Q_SILVER_FAIRY,
                Q_GOLD_DRAKE,
            ],
            second: Second::Two([
                Q_BLOOD_WEREWOLF,
                Q_BLOOD_DREVANUL,
                Q_MANAKS_BLOOD_WEREWOLF,
                Q_SILVER_GOLEM,
                Q_SILVER_FAIRY,
                Q_SILVER_UNDINE,
                Q_SILVER_GOLEM,
                Q_SILVER_FAIRY,
                Q_SILVER_DRYAD,
                Q_GOLD_DRAKE,
                Q_GOLD_KNIGHT,
                Q_GOLD_WYRM,
            ]),
            third: [
                Q_MANAKS_BLOOD_WEREWOLF,
                Q_SILVER_UNDINE,
                Q_SILVER_DRYAD,
                Q_GOLD_WYRM,
            ],
        },
        HAGGER => Cfg {
            base: [4, 8, 9],
            first: [
                Q_SILVER_UNICORN,
                0,
                2,
                Q_BLOOD_MEDUSA,
                Q_SILVER_UNICORN,
                Q_GOLD_WYVERN,
            ],
            second: Second::One([
                Q_SILVER_UNICORN,
                2,
                Q_NIAS_SILVER_FAIRY,
                Q_BLOOD_MEDUSA,
                Q_BLOOD_WEREWOLF,
                Q_SILVER_UNICORN,
                Q_SILVER_GOLEM,
                Q_GOLD_WYVERN,
                Q_GOLD_DRAKE,
            ]),
            third: [
                Q_NIAS_SILVER_FAIRY,
                Q_BLOOD_WEREWOLF,
                Q_SILVER_GOLEM,
                Q_GOLD_DRAKE,
            ],
        },
        STAN => Cfg {
            base: [3, 7, 9],
            first: [
                Q_SILVER_FAIRY,
                Q_SILVER_GOLEM,
                1,
                Q_SILVER_FAIRY,
                Q_BLOOD_WEREWOLF,
                Q_GOLD_KNIGHT,
            ],
            second: Second::Two([
                Q_SILVER_FAIRY,
                Q_SILVER_GOLEM,
                Q_MANAKS_SILVER_DRYAD,
                Q_SILVER_FAIRY,
                Q_SILVER_GOLEM,
                Q_SILVER_DRYAD,
                Q_BLOOD_WEREWOLF,
                Q_BLOOD_DREVANUL,
                Q_BLOOD_BASILISK,
                Q_GOLD_KNIGHT,
                Q_GOLD_DRAKE,
                Q_GOLD_GIANT,
            ]),
            third: [
                Q_MANAKS_SILVER_DRYAD,
                Q_SILVER_DRYAD,
                Q_BLOOD_BASILISK,
                Q_GOLD_GIANT,
            ],
        },
        RESEARCHER_LORAIN => Cfg {
            base: [4, 8, 9],
            first: [
                Q_GOLD_WYVERN,
                0,
                2,
                Q_BLOOD_MEDUSA,
                Q_SILVER_UNICORN,
                Q_GOLD_WYVERN,
            ],
            second: Second::One([
                Q_GOLD_WYVERN,
                2,
                Q_NIAS_GOLD_WYVERN,
                Q_BLOOD_MEDUSA,
                Q_BLOOD_DREVANUL,
                Q_SILVER_UNICORN,
                Q_SILVER_GOLEM,
                Q_GOLD_WYVERN,
                Q_GOLD_KNIGHT,
            ]),
            third: [
                Q_NIAS_GOLD_WYVERN,
                Q_BLOOD_DREVANUL,
                Q_SILVER_GOLEM,
                Q_GOLD_KNIGHT,
            ],
        },
        BLACKSMITH_DUNING => Cfg {
            base: [3, 7, 9],
            first: [
                Q_GOLD_DRAKE,
                Q_GOLD_KNIGHT,
                1,
                Q_SILVER_GOLEM,
                Q_BLOOD_DREVANUL,
                Q_GOLD_DRAKE,
            ],
            second: Second::Two([
                Q_GOLD_KNIGHT,
                Q_GOLD_DRAKE,
                Q_MANAKS_GOLD_GIANT,
                Q_SILVER_GOLEM,
                Q_SILVER_FAIRY,
                Q_SILVER_UNDINE,
                Q_BLOOD_DREVANUL,
                Q_BLOOD_WEREWOLF,
                Q_BLOOD_SUCCUBUS,
                Q_GOLD_DRAKE,
                Q_GOLD_KNIGHT,
                Q_GOLD_GIANT,
            ]),
            third: [
                Q_MANAKS_GOLD_GIANT,
                Q_SILVER_UNDINE,
                Q_BLOOD_SUCCUBUS,
                Q_GOLD_GIANT,
            ],
        },
        MAGISTER_PAGE => Cfg {
            base: [4, 8, 9],
            first: [
                Q_BLOOD_MEDUSA,
                0,
                2,
                Q_BLOOD_MEDUSA,
                Q_SILVER_UNICORN,
                Q_GOLD_WYVERN,
            ],
            second: Second::One([
                Q_BLOOD_MEDUSA,
                2,
                Q_NIAS_BLOOD_MEDUSA,
                Q_BLOOD_MEDUSA,
                Q_BLOOD_WEREWOLF,
                Q_SILVER_UNICORN,
                Q_SILVER_FAIRY,
                Q_GOLD_WYVERN,
                Q_GOLD_KNIGHT,
            ]),
            third: [
                Q_NIAS_BLOOD_MEDUSA,
                Q_BLOOD_WEREWOLF,
                Q_SILVER_FAIRY,
                Q_GOLD_KNIGHT,
            ],
        },
        HEAD_BLACKSMITH_FERRIS => Cfg {
            base: [4, 8, 9],
            first: [
                Q_BLOOD_BASILISK,
                Q_BLOOD_SUCCUBUS,
                1,
                Q_GOLD_GIANT,
                Q_SILVER_DRYAD,
                Q_BLOOD_BASILISK,
            ],
            second: Second::Two([
                Q_BLOOD_BASILISK,
                Q_BLOOD_SUCCUBUS,
                Q_BERETHS_BLOOD_DRAGON,
                Q_GOLD_GIANT,
                Q_GOLD_WYRM,
                Q_GOLD_DRAGON,
                Q_SILVER_DRYAD,
                Q_SILVER_UNDINE,
                Q_SILVER_DRAGON,
                Q_BLOOD_BASILISK,
                Q_BLOOD_SUCCUBUS,
                Q_BLOOD_DRAGON,
            ]),
            third: [
                Q_BERETHS_BLOOD_DRAGON,
                Q_GOLD_DRAGON,
                Q_SILVER_DRAGON,
                Q_BLOOD_DRAGON,
            ],
        },
        _ => return None,
    };
    Some(cfg)
}

/// Java `resetParams(qs)`.
fn reset_params(ctx: &mut ScriptCtx<'_>, qs: Qs) {
    qs.set_int(ctx, WEIGHT_POINT, 0);
    qs.set_int(ctx, PARAM_1, 0);
    qs.set_int(ctx, PARAM_2, 0);
    qs.set_int(ctx, PARAM_3, 0);
    qs.set_int(ctx, FLAG, 0);
}

/// The number the player has to guess: three base-4 digits, each 1..=3 (Java computes it the
/// same way in all four branches of `shortFirstSteps`).
fn roll_param_1(ctx: &mut ScriptCtx<'_>) -> i32 {
    let mut param1 = ctx.get_random(3) + 1;
    param1 += (ctx.get_random(3) + 1) * 4;
    param1 += (ctx.get_random(3) + 1) * 16;
    param1
}

/// Java `shortFirstSteps`.
#[allow(clippy::too_many_arguments)]
fn short_first_steps(
    ctx: &mut ScriptCtx<'_>,
    qs: Qs,
    npc_id: i32,
    weight_point: i32,
    base: i32,
    first: [i32; 6],
) -> String {
    let player = qs.player();
    let [item1a, item1b, item1_mul, item2, item3, item4] = first;
    // The paying item of each `param2` value.
    let pay = match qs.get_int(ctx, PARAM_2) {
        42 => {
            if ctx.get_quest_items_count(player, item1a) >= i64::from(base * item1_mul)
                && (item1b == 0 || ctx.get_quest_items_count(player, item1b) >= i64::from(base))
            {
                qs.set_int(ctx, FLAG, 1);
                ctx.take_items(player, item1a, i64::from(base * item1_mul));
                if item1b > 0 {
                    ctx.take_items(player, item1b, i64::from(base));
                }
                true
            } else {
                false
            }
        }
        31 | 21 | 11 => {
            let item = match qs.get_int(ctx, PARAM_2) {
                31 => item2,
                21 => item3,
                _ => item4,
            };
            if ctx.get_quest_items_count(player, item) >= i64::from(base) {
                qs.set_int(ctx, FLAG, 1);
                ctx.take_items(player, item, i64::from(base));
                true
            } else {
                false
            }
        }
        _ => false,
    };
    if pay {
        qs.set_int(ctx, WEIGHT_POINT, weight_point);
        let param1 = roll_param_1(ctx);
        qs.set_int(ctx, PARAM_1, param1);
        return format!("{npc_id}-11.html");
    }
    format!("{npc_id}-10.html")
}

/// Java `shortSecondStepOneItem`.
fn short_second_step_one_item(ctx: &mut ScriptCtx<'_>, qs: Qs, npc_id: i32, mul: i32, a: [i32; 9]) -> String {
    let player = qs.player();
    let [
        item1,
        item1_mul,
        reward1,
        item2,
        reward2,
        item3,
        reward3,
        item4,
        reward4,
    ] = a;
    let mul = i64::from(mul);
    let (item, need, reward) = match qs.get_int(ctx, PARAM_2) {
        42 => (item1, 10 * mul * i64::from(item1_mul), reward1),
        31 => (item2, 5 * mul, reward2),
        21 => (item3, 5 * mul, reward3),
        11 => (item4, 5 * mul, reward4),
        _ => return format!("{npc_id}-10.html"),
    };
    if ctx.get_quest_items_count(player, item) >= need {
        ctx.take_items(player, item, need);
        ctx.give_items(player, reward, mul);
        ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
        return format!("{npc_id}-07.html");
    }
    format!("{npc_id}-10.html")
}

/// Java `shortSecondStepTwoItems`.
fn short_second_step_two_items(
    ctx: &mut ScriptCtx<'_>,
    qs: Qs,
    npc_id: i32,
    mul: i32,
    a: [i32; 12],
) -> String {
    let player = qs.player();
    let mul = i64::from(mul);
    let (first, second, reward, need) = match qs.get_int(ctx, PARAM_2) {
        42 => (a[0], a[1], a[2], 10 * mul),
        31 => (a[3], a[4], a[5], 5 * mul),
        21 => (a[6], a[7], a[8], 5 * mul),
        11 => (a[9], a[10], a[11], 5 * mul),
        _ => return format!("{npc_id}-10.html"),
    };
    if ctx.get_quest_items_count(player, first) >= need && ctx.get_quest_items_count(player, second) >= need {
        ctx.take_items(player, first, need);
        ctx.take_items(player, second, need);
        ctx.give_items(player, reward, mul);
        ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
        return format!("{npc_id}-07.html");
    }
    format!("{npc_id}-10.html")
}

/// Java `shortThirdStep`.
fn short_third_step(
    ctx: &mut ScriptCtx<'_>,
    qs: Qs,
    npc_id: i32,
    flag: i32,
    items: [i32; 4],
) -> Option<String> {
    let player = qs.player();
    qs.set_int(ctx, PARAM_3, 0);
    qs.set_int(ctx, FLAG, qs.get_int(ctx, FLAG) + flag);
    if qs.get_int(ctx, PARAM_1) == qs.get_int(ctx, FLAG) && qs.get_int(ctx, WEIGHT_POINT) >= 0 {
        qs.set_int(ctx, WEIGHT_POINT, 0);
        match qs.get_int(ctx, PARAM_2) {
            42 => ctx.give_items(player, items[0], 1),
            31 => ctx.give_items(player, items[1], 1),
            21 => ctx.give_items(player, items[2], 1),
            11 => ctx.give_items(player, items[3], 1),
            _ => {}
        }
        qs.set_int(ctx, PARAM_1, 0);
        return Some(format!("{npc_id}-20.html"));
    }

    if qs.get_int(ctx, WEIGHT_POINT) == 0 {
        // The page number by the secret number.
        let page = match qs.get_int(ctx, PARAM_1) {
            21 => Some(23),
            25 => Some(24),
            37 => Some(25),
            41 => Some(26),
            61 => Some(27),
            29 => Some(28),
            45 => Some(29),
            53 => Some(30),
            57 => Some(31),
            22 => Some(32),
            26 => Some(33),
            38 => Some(34),
            42 => Some(35),
            62 => Some(36),
            30 => Some(37),
            46 => Some(38),
            54 => Some(39),
            58 => Some(40),
            23 => Some(41),
            27 => Some(42),
            39 => Some(43),
            43 => Some(44),
            63 => Some(45),
            31 => Some(46),
            47 => Some(47),
            55 => Some(48),
            59 => Some(49),
            _ => None,
        };
        if let Some(page) = page {
            return Some(format!("{npc_id}-{page}.html"));
        }
        qs.set_int(ctx, PARAM_1, 0);
    } else {
        let param1 = qs.get_int(ctx, PARAM_1);
        let i0 = param1 % 4;
        let mut i1 = param1 / 4;
        let i2 = i1 / 4;
        i1 %= 4;

        let flag_value = qs.get_int(ctx, FLAG);
        let i3 = flag_value % 4;
        let mut i4 = flag_value / 4;
        let i5 = i4 / 4;
        i4 %= 4;
        if i0 == i3 {
            qs.set_int(ctx, PARAM_3, qs.get_int(ctx, PARAM_3) + 1);
        }
        if i1 == i4 {
            qs.set_int(ctx, PARAM_3, qs.get_int(ctx, PARAM_3) + 1);
        }
        if i2 == i5 {
            qs.set_int(ctx, PARAM_3, qs.get_int(ctx, PARAM_3) + 1);
        }

        qs.set_int(ctx, FLAG, 1);
        qs.set_int(ctx, WEIGHT_POINT, qs.get_int(ctx, WEIGHT_POINT) - 1);
        match qs.get_int(ctx, PARAM_3) {
            0 => return Some(format!("{npc_id}-52.html")),
            1 => return Some(format!("{npc_id}-50.html")),
            2 => return Some(format!("{npc_id}-51.html")),
            _ => {}
        }
    }
    None
}

/// Java `showQuestionMark(336)` followed by the middle sound.
fn mark_and_sound(ctx: &mut ScriptCtx<'_>, qs: Qs, player: ObjectId) {
    qs.show_question_mark(ctx, 336);
    ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
}

/// Java `getRandomPlayerFromParty` (`coin` = `getRandomPlayerFromPartyCoin`): the killer counts
/// twice, then once more as a party member.
fn random_player_from_party(
    ctx: &mut ScriptCtx<'_>,
    player: ObjectId,
    npc: ScriptNpc,
    memo_state: i32,
    coin: bool,
) -> Option<Qs> {
    let qs = ctx.get_quest_state(player, false);
    let mut candidates: Vec<Qs> = Vec::new();
    if let Some(qs) = qs
        && qs.is_started(ctx)
        && qs.is_memo_state(ctx, memo_state)
        && (!coin || !ctx.has_quest_items(player, Q_KALDIS_GOLD_DRAGON))
    {
        candidates.push(qs);
        candidates.push(qs);
    }
    if let Some(party) = ctx.world().party_of(player) {
        for pm in ctx.world().party_members(party) {
            let qss = ctx.get_quest_state(pm, false);
            if let Some(qss) = qss
                && qss.is_started(ctx)
                && qss.is_memo_state(ctx, memo_state)
                // NOTE(java-quirk): the coin variant checks the killer's coin, not the member's.
                && (!coin || !ctx.has_quest_items(player, Q_KALDIS_GOLD_DRAGON))
                && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, pm, true)
            {
                candidates.push(qss);
            }
        }
    }
    if candidates.is_empty() {
        return None;
    }
    let index = ctx.get_random(candidates.len() as i32) as usize;
    candidates.get(index).copied()
}

/// `(drop item, chance out of 1000)` of a monster of the second kill table.
fn drop_of(npc_id: i32) -> Option<(i32, i32)> {
    let drop = match npc_id {
        SHACKLE | SHACKLE_HOLD => (Q_GOLD_WYVERN, 70),
        HEADLESS_KNIGHT | TIMAK_ORC => (Q_GOLD_WYVERN, 80),
        HEADLESS_KNIGHT_HOLD => (Q_GOLD_WYVERN, 85),
        ROYAL_CAVE_SERVANT
        | MALRUK_SUCCUBUS_TUREN
        | ROYAL_CAVE_SERVANT_HOLD
        | KUKABURO_B
        | ANTELOPE
        | ANTELOPE_A
        | ANTELOPE_B
        | H_MALRUK_SUCCUBUS_TUREN => (Q_GOLD_WYVERN, 100),
        BUFFALO | BUFFALO_A | BUFFALO_B | KUKABURO | KUKABURO_A => (Q_GOLD_WYVERN, 110),
        DOOM_SERVANT => (Q_GOLD_WYVERN, 140),
        DOOM_KNIGHT => (Q_GOLD_WYVERN, 210),
        VANOR_SILENOS_SHAMAN => (Q_SILVER_UNICORN, 70),
        BLOODY_GHOST | TARLK_BUGBEAR_BOSS | OEL_MAHUM => (Q_SILVER_UNICORN, 80),
        OEL_MAHUM_WARRIOR => (Q_SILVER_UNICORN, 90),
        HUNGRY_CORPSE => (Q_SILVER_UNICORN, 100),
        BYFOOT => (Q_SILVER_UNICORN, 110),
        BYFOOT_SIGEL => (Q_SILVER_UNICORN, 120),
        DARK_GUARD | BRILLIANT_CLAW | BRILLIANT_CLAW_1 => (Q_SILVER_UNICORN, 150),
        OEL_MAHUM_WITCH_DOCTOR => (Q_SILVER_UNICORN, 200),
        BRILLIANT_ANGUISH | BRILLIANT_ANGUISH_1 => (Q_SILVER_UNICORN, 210),
        LAKIN => (Q_BLOOD_MEDUSA, 60),
        HATAR_HANISHEE => (Q_BLOOD_MEDUSA, 70),
        PUNISHMENT_OF_UNDEAD => (Q_BLOOD_MEDUSA, 80),
        FLOAT_OF_GRAVE | BANDERSNATCH_A | BANDERSNATCH_B => (Q_BLOOD_MEDUSA, 90),
        BANDERSNATCH => (Q_BLOOD_MEDUSA, 100),
        NIHIL_INVADER => (Q_BLOOD_MEDUSA, 110),
        TIMAK_ORC_SHAMAN => (Q_BLOOD_MEDUSA, 130),
        TIMAK_ORC_ARCHER | TIMAK_ORC_SOLDIER => (Q_BLOOD_MEDUSA, 140),
        DOOM_ARCHER | BRILLIANT_WISDOM | BRILLIANT_VENGEANCE | BRILLIANT_VENGEANCE_1 => (Q_BLOOD_MEDUSA, 160),
        _ => return None,
    };
    Some(drop)
}

/// One Sorint exchange (events 20..=31): the items and counts the player pays, the reward and
/// the page it gets; Java checks all of them with `hasQuestItems`/`getQuestItemsCount`.
fn sorint_trade(
    ctx: &mut ScriptCtx<'_>,
    player: ObjectId,
    cost: &[(i32, i64)],
    reward: i32,
    page: &str,
) -> String {
    if cost
        .iter()
        .all(|&(item, count)| ctx.get_quest_items_count(player, item) >= count)
    {
        for &(item, count) in cost {
            ctx.take_items(player, item, count);
        }
        ctx.give_items(player, reward, 1);
        ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
        return page.to_owned();
    }
    "30232-24.html".to_owned()
}

impl Script for Q00336CoinsOfMagic {
    fn name(&self) -> &'static str {
        "Q00336_CoinsOfMagic"
    }

    fn path(&self) -> &'static str {
        "quests/Q00336_CoinsOfMagic"
    }

    fn id(&self) -> i32 {
        336
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[WAREHOUSE_KEEPER_SORINT]);
        s.add_talk_id(&[
            PANO,
            COLLOB,
            RAPIN,
            HAGGER,
            STAN,
            RESEARCHER_LORAIN,
            BLACKSMITH_DUNING,
            MAGISTER_PAGE,
            UNION_PRESIDENT_BERNARD,
            HEAD_BLACKSMITH_FERRIS,
        ]);
        s.add_kill_id(MOBS);
        s.register_quest_items(&[
            Q_COIN_DIAGRAM,
            Q_KALDIS_GOLD_DRAGON,
            Q_CC_MEMBERSHIP_1,
            Q_CC_MEMBERSHIP_2,
            Q_CC_MEMBERSHIP_3,
        ]);
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            PANO | COLLOB | HEAD_BLACKSMITH_FERRIS => {
                if ctx.has_quest_items(player, Q_CC_MEMBERSHIP_1) {
                    reset_params(ctx, qs);
                    return Some(format!("{}-01.html", npc.id));
                }
                if ctx.has_quest_items(player, Q_CC_MEMBERSHIP_2)
                    || ctx.has_quest_items(player, Q_CC_MEMBERSHIP_3)
                {
                    return Some(format!("{}-54.html", npc.id));
                }
            }
            RAPIN | STAN | BLACKSMITH_DUNING => {
                if ctx.has_quest_items(player, Q_CC_MEMBERSHIP_1)
                    || ctx.has_quest_items(player, Q_CC_MEMBERSHIP_2)
                {
                    reset_params(ctx, qs);
                    return Some(format!("{}-01.html", npc.id));
                }
                if ctx.has_quest_items(player, Q_CC_MEMBERSHIP_3) {
                    return Some(format!("{}-54.html", npc.id));
                }
            }
            HAGGER | MAGISTER_PAGE | RESEARCHER_LORAIN => {
                if ctx.has_quest_items(player, Q_CC_MEMBERSHIP_1)
                    || ctx.has_quest_items(player, Q_CC_MEMBERSHIP_2)
                    || ctx.has_quest_items(player, Q_CC_MEMBERSHIP_3)
                {
                    reset_params(ctx, qs);
                    return Some(format!("{}-01.html", npc.id));
                }
            }
            UNION_PRESIDENT_BERNARD => {
                if qs.is_memo_state(ctx, 1) && ctx.has_quest_items(player, Q_COIN_DIAGRAM) {
                    return Some("30702-01.html".to_owned());
                }
                if qs.get_memo_state(ctx) >= 3 {
                    return Some("30702-05.html".to_owned());
                }
                if qs.is_memo_state(ctx, 2) {
                    return Some("30702-02a.html".to_owned());
                }
            }
            WAREHOUSE_KEEPER_SORINT => {
                if qs.is_created(ctx) {
                    return Some(
                        if ctx.player_level(player) < 40 {
                            "30232-01.htm"
                        } else {
                            "30232-02.htm"
                        }
                        .to_owned(),
                    );
                }
                if qs.state(ctx) == STARTED {
                    let memo = qs.get_memo_state(ctx);
                    if !ctx.has_quest_items(player, Q_KALDIS_GOLD_DRAGON) && (memo == 1 || memo == 2) {
                        return Some("30232-06.html".to_owned());
                    }
                    if ctx.has_quest_items(player, Q_KALDIS_GOLD_DRAGON) && (memo == 1 || memo == 2) {
                        ctx.give_items(player, Q_CC_MEMBERSHIP_3, 1);
                        ctx.take_items(player, Q_COIN_DIAGRAM, -1);
                        ctx.take_items(player, Q_KALDIS_GOLD_DRAGON, 1);
                        qs.set_memo_state(ctx, 3);
                        qs.set_cond(ctx, 4, false);
                        mark_and_sound(ctx, qs, player);
                        return Some("30232-07.html".to_owned());
                    }
                    if ctx.has_quest_items(player, Q_CC_MEMBERSHIP_3) && memo == 3 {
                        return Some("30232-10.html".to_owned());
                    }
                    if ctx.has_quest_items(player, Q_CC_MEMBERSHIP_2) && memo == 3 {
                        return Some("30232-11.html".to_owned());
                    }
                    if ctx.has_quest_items(player, Q_CC_MEMBERSHIP_1) && memo == 3 {
                        return Some("30232-12.html".to_owned());
                    }
                }
            }
            _ => {}
        }
        Some(htmltext)
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let qs = ctx.get_quest_state(player, false)?;
        if event == "QUEST_ACCEPTED" {
            ctx.play_sound(player, QuestSound::ItemsoundQuestAccept);
            if !ctx.has_quest_items(player, Q_COIN_DIAGRAM) {
                ctx.give_items(player, Q_COIN_DIAGRAM, 1);
            }
            qs.set_memo_state(ctx, 1);
            qs.start_quest(ctx);
            mark_and_sound(ctx, qs, player);
            return Some("30232-05.htm".to_owned());
        }
        if event.contains(".htm") {
            return Some(event.to_owned());
        }

        // NOTE(java-quirk): Java dereferences the NPC and parses the event with
        // `Integer.parseInt`; an event that is no number throws there, here it ends the call.
        let npc_id = npc?.id;
        let event_id: i32 = event.parse().ok()?;

        if let Some(cfg) = config(npc_id) {
            match event_id {
                1 => {
                    qs.set_int(ctx, PARAM_2, 11);
                    return Some(format!("{npc_id}-02.html"));
                }
                2 => {
                    qs.set_int(ctx, PARAM_2, 21);
                    return Some(format!("{npc_id}-03.html"));
                }
                3 => {
                    qs.set_int(ctx, PARAM_2, 31);
                    return Some(format!("{npc_id}-04.html"));
                }
                4 => {
                    qs.set_int(ctx, PARAM_2, 42);
                    return Some(format!("{npc_id}-05.html"));
                }
                5 => return Some(format!("{npc_id}-06.html")),
                9 => return Some(format!("{npc_id}-53.html")),
                13 => {
                    if qs.get_int(ctx, FLAG) == 1 {
                        qs.set_int(ctx, FLAG, 16);
                        return Some(format!("{npc_id}-14.html"));
                    }
                }
                14 => {
                    if qs.get_int(ctx, FLAG) == 1 {
                        qs.set_int(ctx, FLAG, 32);
                        return Some(format!("{npc_id}-15.html"));
                    }
                }
                15 => {
                    if qs.get_int(ctx, FLAG) == 1 {
                        qs.set_int(ctx, FLAG, 48);
                        return Some(format!("{npc_id}-16.html"));
                    }
                }
                16 => {
                    qs.set_int(ctx, FLAG, qs.get_int(ctx, FLAG) + 4);
                    return Some(format!("{npc_id}-17.html"));
                }
                17 => {
                    qs.set_int(ctx, FLAG, qs.get_int(ctx, FLAG) + 8);
                    return Some(format!("{npc_id}-18.html"));
                }
                18 => {
                    qs.set_int(ctx, FLAG, qs.get_int(ctx, FLAG) + 12);
                    return Some(format!("{npc_id}-19.html"));
                }
                22 => return Some(format!("{npc_id}-01.html")),
                _ => {}
            }
            // The second switch: the three steps of the game.
            match event_id {
                6..=8 => {
                    let i = (event_id - 6) as usize;
                    let weight_point = event_id - 5;
                    return Some(short_first_steps(
                        ctx,
                        qs,
                        npc_id,
                        weight_point,
                        cfg.base[i],
                        cfg.first,
                    ));
                }
                10..=12 => {
                    let mul = match event_id {
                        10 => 1,
                        11 => 5,
                        _ => 10,
                    };
                    return Some(match cfg.second {
                        Second::Two(a) => short_second_step_two_items(ctx, qs, npc_id, mul, a),
                        Second::One(a) => short_second_step_one_item(ctx, qs, npc_id, mul, a),
                    });
                }
                19..=21 => {
                    return short_third_step(ctx, qs, npc_id, event_id - 18, cfg.third);
                }
                _ => {}
            }
            return None;
        }

        match npc_id {
            UNION_PRESIDENT_BERNARD => match event_id {
                1 => return Some("30702-02.html".to_owned()),
                2 => {
                    qs.set_memo_state(ctx, 2);
                    qs.set_cond(ctx, 2, false);
                    mark_and_sound(ctx, qs, player);
                    return Some("30702-03.html".to_owned());
                }
                3 => {
                    qs.set_memo_state(ctx, 2);
                    qs.set_cond(ctx, 2, false);
                    mark_and_sound(ctx, qs, player);
                    return Some("30702-04.html".to_owned());
                }
                4 => {
                    qs.set_cond(ctx, 7, false);
                    mark_and_sound(ctx, qs, player);
                    return Some("30702-06.html".to_owned());
                }
                _ => {}
            },
            WAREHOUSE_KEEPER_SORINT => match event_id {
                1 => return Some("30232-03.html".to_owned()),
                2 => return Some("30232-04.html".to_owned()),
                3 => return Some("30232-08.html".to_owned()),
                4 => return Some("30232-09.html".to_owned()),
                5 => {
                    if ctx.has_quest_items(player, Q_CC_MEMBERSHIP_3) {
                        if ctx.has_all_quest_items(
                            player,
                            &[
                                Q_BLOOD_DREVANUL,
                                Q_BLOOD_WEREWOLF,
                                Q_GOLD_KNIGHT,
                                Q_GOLD_DRAKE,
                                Q_SILVER_FAIRY,
                                Q_SILVER_GOLEM,
                            ],
                        ) {
                            qs.set_cond(ctx, 9, false);
                            mark_and_sound(ctx, qs, player);
                            ctx.take_items(player, Q_CC_MEMBERSHIP_3, -1);
                            ctx.take_items(player, Q_BLOOD_DREVANUL, 1);
                            ctx.take_items(player, Q_BLOOD_WEREWOLF, 1);
                            ctx.take_items(player, Q_GOLD_KNIGHT, 1);
                            ctx.take_items(player, Q_GOLD_DRAKE, 1);
                            ctx.take_items(player, Q_SILVER_FAIRY, 1);
                            ctx.take_items(player, Q_SILVER_GOLEM, 1);
                            ctx.give_items(player, Q_CC_MEMBERSHIP_2, 1);
                            return Some("30232-16.html".to_owned());
                        }
                        qs.set_cond(ctx, 8, false);
                        mark_and_sound(ctx, qs, player);
                        return Some("30232-13.html".to_owned());
                    }
                    if ctx.has_quest_items(player, Q_CC_MEMBERSHIP_2) {
                        if ctx.has_all_quest_items(
                            player,
                            &[
                                Q_BLOOD_BASILISK,
                                Q_BLOOD_SUCCUBUS,
                                Q_GOLD_GIANT,
                                Q_GOLD_WYRM,
                                Q_SILVER_UNDINE,
                                Q_SILVER_DRYAD,
                            ],
                        ) {
                            qs.set_cond(ctx, 11, false);
                            mark_and_sound(ctx, qs, player);
                            ctx.take_items(player, Q_CC_MEMBERSHIP_2, -1);
                            ctx.take_items(player, Q_BLOOD_BASILISK, 1);
                            ctx.take_items(player, Q_BLOOD_SUCCUBUS, 1);
                            ctx.take_items(player, Q_GOLD_GIANT, 1);
                            ctx.take_items(player, Q_GOLD_WYRM, 1);
                            ctx.take_items(player, Q_SILVER_UNDINE, 1);
                            ctx.take_items(player, Q_SILVER_DRYAD, 1);
                            ctx.give_items(player, Q_CC_MEMBERSHIP_1, 1);
                            return Some("30232-17.html".to_owned());
                        }
                        qs.set_cond(ctx, 10, false);
                        mark_and_sound(ctx, qs, player);
                        return Some("30232-14.html".to_owned());
                    }
                    if ctx.has_quest_items(player, Q_CC_MEMBERSHIP_1) {
                        return Some("30232-15.html".to_owned());
                    }
                }
                6 => return Some("30232-18.html".to_owned()),
                7 => return Some("30232-19.html".to_owned()),
                8 => return Some("30232-20.html".to_owned()),
                9 => return Some("30232-21.html".to_owned()),
                10 => {
                    qs.set_cond(ctx, 6, false);
                    mark_and_sound(ctx, qs, player);
                    return Some("30232-22.html".to_owned());
                }
                11 => {
                    qs.set_cond(ctx, 5, false);
                    mark_and_sound(ctx, qs, player);
                    return Some("30232-23.html".to_owned());
                }
                20 => {
                    return Some(sorint_trade(
                        ctx,
                        player,
                        &[
                            (Q_BERETHS_BLOOD_DRAGON, 1),
                            (Q_SILVER_DRAGON, 1),
                            (Q_GOLD_WYRM, 13),
                        ],
                        DEMON_STAFF,
                        "30232-24a.html",
                    ));
                }
                21 => {
                    return Some(sorint_trade(
                        ctx,
                        player,
                        &[
                            (Q_BERETHS_GOLD_DRAGON, 1),
                            (Q_BLOOD_DRAGON, 1),
                            (Q_SILVER_DRYAD, 1),
                            (Q_GOLD_GIANT, 1),
                        ],
                        DARK_SCREAMER,
                        "30232-24b.html",
                    ));
                }
                22 => {
                    return Some(sorint_trade(
                        ctx,
                        player,
                        &[
                            (Q_BERETHS_SILVER_DRAGON, 1),
                            (Q_GOLD_DRAGON, 1),
                            (Q_BLOOD_SUCCUBUS, 1),
                            (Q_BLOOD_BASILISK, 2),
                        ],
                        WIDOW_MAKER,
                        "30232-24c.html",
                    ));
                }
                23 => {
                    return Some(sorint_trade(
                        ctx,
                        player,
                        &[
                            (Q_GOLD_DRAGON, 1),
                            (Q_SILVER_DRAGON, 1),
                            (Q_BLOOD_DRAGON, 1),
                            (Q_SILVER_UNDINE, 1),
                        ],
                        SWORD_OF_LIMIT,
                        "30232-24d.html",
                    ));
                }
                24 => {
                    return Some(sorint_trade(
                        ctx,
                        player,
                        &[(Q_MANAKS_GOLD_GIANT, 1)],
                        DEMONS_BOOTS,
                        "30232-24e.html",
                    ));
                }
                25 => {
                    return Some(sorint_trade(
                        ctx,
                        player,
                        &[(Q_MANAKS_SILVER_DRYAD, 1), (Q_SILVER_DRYAD, 1)],
                        DEMONS_HOSE,
                        "30232-24f.html",
                    ));
                }
                26 => {
                    return Some(sorint_trade(
                        ctx,
                        player,
                        &[(Q_MANAKS_GOLD_GIANT, 1)],
                        DEMONS_GLOVES,
                        "30232-24g.html",
                    ));
                }
                27 => {
                    return Some(sorint_trade(
                        ctx,
                        player,
                        &[(Q_MANAKS_BLOOD_WEREWOLF, 1), (Q_GOLD_GIANT, 1), (Q_GOLD_WYRM, 1)],
                        FULL_PLATE_HELMET,
                        "30232-24h.html",
                    ));
                }
                28 => {
                    return Some(sorint_trade(
                        ctx,
                        player,
                        &[
                            (Q_NIAS_BLOOD_MEDUSA, 2),
                            (Q_GOLD_DRAKE, 2),
                            (Q_BLOOD_DREVANUL, 2),
                            (Q_GOLD_KNIGHT, 3),
                        ],
                        MOONSTONE_EARING,
                        "30232-24i.html",
                    ));
                }
                29 => {
                    return Some(sorint_trade(
                        ctx,
                        player,
                        &[
                            (Q_NIAS_BLOOD_MEDUSA, 7),
                            (Q_GOLD_KNIGHT, 5),
                            (Q_BLOOD_DREVANUL, 5),
                            (Q_SILVER_GOLEM, 5),
                        ],
                        NASSENS_EARING,
                        "30232-24j.html",
                    ));
                }
                30 => {
                    return Some(sorint_trade(
                        ctx,
                        player,
                        &[
                            (Q_NIAS_GOLD_WYVERN, 5),
                            (Q_SILVER_GOLEM, 4),
                            (Q_GOLD_DRAKE, 4),
                            (Q_BLOOD_DREVANUL, 4),
                        ],
                        RING_OF_BINDING,
                        "30232-24k.html",
                    ));
                }
                31 => {
                    return Some(sorint_trade(
                        ctx,
                        player,
                        &[
                            (Q_NIAS_SILVER_FAIRY, 5),
                            (Q_SILVER_FAIRY, 3),
                            (Q_GOLD_KNIGHT, 3),
                            (Q_BLOOD_DREVANUL, 3),
                        ],
                        NECKLACE_OF_PROTECTION,
                        "30232-24l.html",
                    ));
                }
                100 => {
                    ctx.take_items(player, Q_CC_MEMBERSHIP_1, -1);
                    ctx.take_items(player, Q_CC_MEMBERSHIP_2, -1);
                    ctx.take_items(player, Q_CC_MEMBERSHIP_3, -1);
                    ctx.play_sound(player, QuestSound::ItemsoundQuestFinish);
                    qs.exit_quest(ctx, true, false);
                    return Some("30232-18a.html".to_owned());
                }
                _ => {}
            },
            _ => {}
        }
        None
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        if matches!(npc.id, HARIT_LIZARDMAN_SHAMAN | HARIT_LIZARDM_MATRIARCH) {
            let qs = random_player_from_party(ctx, killer, npc, 2, true);
            if let Some(qs) = qs
                && ctx.get_random(1000) < 63
            {
                ctx.give_item_randomly(qs.player(), Some(npc), Q_KALDIS_GOLD_DRAGON, 1, 1, 0, 1.0, true);
                qs.set_cond(ctx, 3, false);
                qs.show_question_mark(ctx, 336);
            }
            return;
        }

        let qs = random_player_from_party(ctx, killer, npc, 3, false);
        if let Some(qs) = qs
            && let Some((item, chance)) = drop_of(npc.id)
            && ctx.get_random(1000) < chance
        {
            ctx.give_item_randomly(qs.player(), Some(npc), item, 1, 1, 0, 1.0, true);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::{QuestLoad, ScriptRegistry};

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, player_data, rows};

    const NAME: &str = "Q00336_CoinsOfMagic";

    const NPCS: &str = r#"
        <npc id="30078" type="Folk" level="50" name="Pano"/>
        <npc id="30232" type="Folk" level="50" name="Sorint"/>
        <npc id="30702" type="Folk" level="50" name="Bernard"/>
        <npc id="20644" type="Monster" level="40" name="Harit Lizardman Shaman"/>
        <npc id="20604" type="Monster" level="40" name="Lakin"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="3472" type="EtcItem" name="Blood Medusa"><set name="is_stackable" val="true"/></item>
        <item id="3484" type="EtcItem" name="Gold Giant"><set name="is_stackable" val="true"/></item>
        <item id="3492" type="EtcItem" name="Silver Dryad"><set name="is_stackable" val="true"/></item>
        <item id="3474" type="EtcItem" name="Blood Basilisk"><set name="is_stackable" val="true"/></item>
        <item id="3495" type="EtcItem" name="Silver Undine"><set name="is_stackable" val="true"/></item>
        <item id="3496" type="EtcItem" name="Bereth's Silver Dragon"><set name="is_stackable" val="true"/></item>
        <item id="3486" type="EtcItem" name="Gold Wyrm"><set name="is_stackable" val="true"/></item>
        <item id="3493" type="EtcItem" name="Silver Dragon"><set name="is_stackable" val="true"/></item>
        <item id="3478" type="EtcItem" name="Bereth's Blood Dragon"><set name="is_stackable" val="true"/></item>
        <item id="3811" type="EtcItem" name="Coin Diagram"><set name="is_questitem" val="true"/></item>
        <item id="3812" type="EtcItem" name="Kaldis' Gold Dragon"><set name="is_questitem" val="true"/></item>
        <item id="3813" type="EtcItem" name="CC Membership 1"><set name="is_questitem" val="true"/></item>
        <item id="3814" type="EtcItem" name="CC Membership 2"><set name="is_questitem" val="true"/></item>
        <item id="3815" type="EtcItem" name="CC Membership 3"><set name="is_questitem" val="true"/></item>
        <item id="206" type="Weapon" name="Demon Staff"/>"#;

    fn setup() -> (ScriptTest, Oid, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00336CoinsOfMagic);
        let pages = Pages::of(&Q00336CoinsOfMagic)
            .page("30232-02.htm", &["QUEST_ACCEPTED", "30232-03.html"])
            .page("30232-12.html", &["20", "100"])
            .plain(&["30232-18a.html"])
            .plain(&["30232-01.htm", "30232-03.html", "30232-05.htm", "30232-06.html"])
            .page("30232-07.html", &["5", "20"])
            .plain(&[
                "30232-16.html",
                "30232-13.html",
                "30232-24.html",
                "30232-24a.html",
            ])
            .page("30702-01.html", &["1", "2", "3", "4"])
            .plain(&[
                "30702-02.html",
                "30702-03.html",
                "30702-04.html",
                "30702-02a.html",
                "30702-05.html",
            ])
            .page(
                "30078-01.html",
                &[
                    "1", "2", "3", "4", "6", "7", "8", "10", "11", "12", "19", "20", "21",
                ],
            )
            .page("30078-03.html", &["6", "10", "11"])
            .plain(&[
                "30078-02.html",
                "30078-05.html",
                "30078-07.html",
                "30078-10.html",
                "30078-11.html",
                "30078-20.html",
            ])
            .npcs(&[PANO, WAREHOUSE_KEEPER_SORINT, UNION_PRESIDENT_BERNARD]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let sorint = t.spawn(WAREHOUSE_KEEPER_SORINT, 20);
        let bernard = t.spawn(UNION_PRESIDENT_BERNARD, 40);
        let pano = t.spawn(PANO, 60);
        (t, sorint, bernard, pano)
    }

    fn started(
        memo: i32,
        level: i32,
        vars: &[(&str, &str)],
        items: Vec<l2_world::ItemData>,
    ) -> l2_world::PlayerData {
        let mut data = player_data(1, level, items);
        let mut variables = vec![
            (NAME.to_owned(), "cond".to_owned(), Some("2".to_owned())),
            (NAME.to_owned(), "memoState".to_owned(), Some(memo.to_string())),
        ];
        for (k, v) in vars {
            variables.push((NAME.to_owned(), (*k).to_owned(), Some((*v).to_owned())));
        }
        data.profile.quests = QuestLoad {
            states: vec![(NAME.to_owned(), Some("Started".to_owned()))],
            variables,
        };
        data
    }

    #[test]
    fn a_level_39_character_is_sent_away() {
        let (mut t, sorint, ..) = setup();
        t.enter(1, 39, vec![]);
        assert!(t.quest_page(1, sorint).contains("30232-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn accept_and_the_lizardman_coin() {
        let (mut t, sorint, bernard, _) = setup();
        t.enter(1, 40, vec![]);
        assert!(t.quest_page(1, sorint).contains("30232-02.htm"));
        assert!(t.event(1, NAME, "QUEST_ACCEPTED").contains("30232-05.htm"));
        assert_eq!(t.count(1, Q_COIN_DIAGRAM), 1);
        let writes = t.take_writes();
        assert!(writes.contains(&rows::insert(NAME, "memoState", "1")));

        // Bernard (memo 1, diagram) -> memo 2, cond 2.
        assert!(t.quest_page(1, bernard).contains("30702-01.html"));
        assert!(t.event(1, NAME, "2").contains("30702-03.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "2"),
                rows::update(NAME, "cond", "2")
            ]
        );

        // Harit lizardmen drop the Kaldis coin at memo 2 (6.3 %), then Sorint hands the membership.
        for _ in 0..400 {
            if t.count(1, Q_KALDIS_GOLD_DRAGON) > 0 {
                break;
            }
            let mob = t.spawn(HARIT_LIZARDMAN_SHAMAN, 80);
            t.kill(1, mob);
        }
        assert_eq!(t.count(1, Q_KALDIS_GOLD_DRAGON), 1);
        t.take_writes();
        assert!(t.quest_page(1, sorint).contains("30232-07.html"));
        assert_eq!(t.count(1, Q_CC_MEMBERSHIP_3), 1);
        assert_eq!(t.count(1, Q_COIN_DIAGRAM), 0);
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "3"),
                rows::update(NAME, "cond", "4")
            ]
        );
    }

    #[test]
    fn pano_game_from_payment_to_a_swap() {
        let (mut t, _, _, pano) = setup();
        t.enter_with(started(
            3,
            50,
            &[],
            vec![
                item(5, Q_SILVER_DRYAD, 20),
                item(6, Q_SILVER_UNDINE, 20),
                item(7, Q_CC_MEMBERSHIP_1, 1),
            ],
        ));
        // Talking to Pano with a membership resets the game variables.
        assert!(t.quest_page(1, pano).contains("30078-01.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "weight_point", "0"),
                rows::insert(NAME, "param1", "0"),
                rows::insert(NAME, "param2", "0"),
                rows::insert(NAME, "param3", "0"),
                rows::insert(NAME, "flag", "0"),
            ]
        );
        // Event 2 picks the second coin kind (param2 21); event 6 pays 4 silver dryads for a
        // secret number and weight 1.
        assert!(t.event(1, NAME, "2").contains("30078-03.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "param2", "21")]);
        assert!(t.event(1, NAME, "6").contains("30078-11.html"));
        assert_eq!(t.count(1, Q_SILVER_DRYAD), 16);
        let writes = t.take_writes();
        assert_eq!(writes[0], rows::update(NAME, "flag", "1"));
        assert_eq!(writes[1], rows::update(NAME, "weight_point", "1"));
        assert_eq!(writes.len(), 3);

        // The swap: event 10 (mul 1) pays 5 dryads and 5 undines for a silver dragon.
        t.quest_page(1, pano);
        t.event(1, NAME, "2");
        assert!(t.event(1, NAME, "10").contains("30078-07.html"));
        assert_eq!(t.count(1, Q_SILVER_DRAGON), 1);
        assert_eq!(t.count(1, Q_SILVER_DRYAD), 11);
        assert_eq!(t.count(1, Q_SILVER_UNDINE), 15);
    }

    #[test]
    fn sorint_trades_a_set_for_a_demon_staff() {
        let (mut t, sorint, ..) = setup();
        t.enter_with(started(
            3,
            50,
            &[],
            vec![
                item(5, Q_BERETHS_BLOOD_DRAGON, 1),
                item(6, Q_SILVER_DRAGON, 1),
                item(7, Q_GOLD_WYRM, 13),
                item(8, Q_CC_MEMBERSHIP_1, 1),
            ],
        ));
        assert!(t.quest_page(1, sorint).contains("30232-12.html"));
        assert!(t.event(1, NAME, "20").contains("30232-24a.html"));
        assert_eq!(t.count(1, 206), 1);
        assert_eq!(t.count(1, Q_GOLD_WYRM), 0);
        // Without the set, the same trade gets the refusal page.
        t.quest_page(1, sorint);
        assert!(t.event(1, NAME, "20").contains("30232-24.html"));
        // 100 ends the quest, repeatable.
        t.take_writes();
        t.quest_page(1, sorint);
        assert!(t.event(1, NAME, "100").contains("30232-18a.html"));
        assert_eq!(t.count(1, Q_CC_MEMBERSHIP_1), 0);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }

    #[test]
    fn lakin_drops_blood_at_memo_3() {
        let (mut t, ..) = setup();
        t.enter_with(started(3, 50, &[], vec![]));
        for _ in 0..60 {
            let mob = t.spawn(LAKIN, 80);
            t.kill(1, mob);
        }
        // 6 % each: 60 kills give at least one with the test seed.
        assert!(t.count(1, Q_BLOOD_MEDUSA) >= 1);
    }
}
