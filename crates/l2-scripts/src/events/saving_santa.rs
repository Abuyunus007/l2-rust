//! Christmas Event, Saving Santa: Thomas D. Turkey keeps Santa and the Santa Trainee sells trees and hats.
// Java: dist/game/data/scripts/events/SavingSanta/SavingSanta.java
// Java: dist/game/data/scripts/events/SavingSanta/config.xml
//
// http://legacy.lineage2.com/archive/2008/12/saving_santa_ev.html
//
// Java's constructor registers everything only `if (isEventPeriod())`. Here `on_load` notes whether
// the event was running at that moment (`State::registered`) and every hook asks that, so an event
// that starts later (the `ScheduleStart` timer) is as inert as Java's: it takes a restart.
//
// Left out (dead code in Java):
// - `SantaBlessings`: `SANTAS_HELPER_AUTOBUFF` is a `false` constant and `_santaHelpers` is never
//   filled, so the loop has nothing to bless. The timer is never started.
// - `onAggroRangeEnter`: Thomas is never registered with `addAggroRangeEnterId`, so Java never
//   calls it. It is ported but not registered.
// - `unload()`: scripts are compiled in (D-16), nothing unloads.

use std::collections::BTreeMap;

use l2_protocol_game::server::{ChatType, SocialAction, SystemMessage};
use l2_protocol_game::{NpcStringId, SystemMessageId};
use l2_util::ObjectId;
use l2_world::effects::{SkillFinishType, SkillKey};
use l2_world::script::QuestSound;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, ZoneId};
use tracing::warn;

use super::long_time_event::{self, DropSpec, EventConfig, SpawnSpec};

// Is Saving Santa event used?
const SAVING_SANTA: bool = true;

// Use Santa's Helpers Auto Buff?
const SANTAS_HELPER_AUTOBUFF: bool = false;

/// Java `ItemHolder(id, count)`.
const TREE_REQUIRED_ITEMS: [(i32, i64); 4] = [(5556, 4), (5557, 4), (5558, 10), (5559, 1)];

const SANTA_TRAINEE_ID: i32 = 31863;
const SPECIAL_CHRISTMAS_TREE_ID: i32 = 13007;
const HOLIDAY_SANTA_ID: i32 = 104;
const HOLIDAY_SLED_ID: i32 = 105;
const THOMAS_D_TURKEY_ID: i32 = 13183;
const MIN_TIME_BETWEEN_2_REWARDS: i64 = 43_200_000;
const BR_XMAS_PRESENT_NORMAL: i32 = 20101;
const BR_XMAS_PRESENT_JACKPOT: i32 = 20102;
const BR_XMAS_WPN_TICKET_NORMAL: i32 = 20107;
const BR_XMAS_WPN_TICKET_JACKPOT: i32 = 20108;
const BR_XMAS_REWARD_BUFF: i32 = 23017;
const BR_XMAS_GAWIBAWIBO_CAP: i32 = 20100;
const X_MAS_TREE1: i32 = 5560;
const X_MAS_TREE2: i32 = 5561;
const SANTAS_HAT_ID: i32 = 7836;
const WEAPON_REWARDS: [i32; 14] = [
    14621, 14622, 14625, 14623, 14624, 14630, 14631, 14628, 14627, 14626, 14674, 14632, 14634, 14633,
];
/// Java `THOMAS_SPAWN`: x, y, z, heading.
const THOMAS_SPAWN: (i32, i32, i32, i32) = (117935, -126003, -2585, 54625);

const NPC_STRINGS: [NpcStringId; 29] = [
    NpcStringId::ItSHurtingIMInPainWhatCanIDoForThePain,
    NpcStringId::NoWhenILoseThatOneILlBeInMorePain,
    NpcStringId::HahahahICapturedSantaClausThereWillBeNoGiftsThisYear,
    NpcStringId::NowWhyDonTYouTakeUpTheChallenge,
    NpcStringId::ComeOnILlTakeAllOfYouOn,
    NpcStringId::HowAboutItIThinkIWon,
    NpcStringId::NowThoseOfYouWhoLostGoAway,
    NpcStringId::WhatABunchOfLosers,
    NpcStringId::IGuessYouCameToRescueSantaButYouPickedTheWrongPerson,
    NpcStringId::AhOkay,
    NpcStringId::AghIWasnTGoingToDoThat,
    NpcStringId::YouReCursedOhWhat,
    NpcStringId::StopItNoMoreIDidItBecauseIWasTooLonely,
    NpcStringId::IHaveToReleaseSantaHowInfuriating,
    NpcStringId::IHateHappyMerryChristmas,
    NpcStringId::OhIMBored,
    NpcStringId::ShallIGoToTakeALookIfSantaIsStillThereHehe,
    NpcStringId::OhHoHoMerryChristmas,
    NpcStringId::SantaCouldGiveNicePresentsOnlyIfHeSReleasedFromTheTurkey,
    NpcStringId::OhHoHoOhHoHoThankYouLadiesAndGentlemenIWillRepayYouForSure,
    NpcStringId::MerryChristmasYouReDoingAGoodJob,
    NpcStringId::MerryChristmasThankYouForRescuingMeFromThatWretchedTurkey,
    NpcStringId::S1IHavePreparedAGiftForYou,
    NpcStringId::IHaveAGiftForS1,
    NpcStringId::TakeALookAtTheInventoryIHopeYouLikeTheGiftIGaveYou,
    NpcStringId::TakeALookAtTheInventoryPerhapsThereMightBeABigPresent,
    NpcStringId::IMTiredOfDealingWithYouIMLeaving,
    NpcStringId::WhenAreYouGoingToStopISlowlyStartedToBeTiredOfIt,
    NpcStringId::MessageFromSantaClausManyBlessingsToS1WhoSavedMe,
];

/// `config.xml` of the event.
const CONFIG: EventConfig = EventConfig {
    name: "Saving Santa",
    active: "21 12 2019-11 01 2020",
    drop_period: None,
    drops: &[
        DropSpec {
            item: 5556,
            min: 1,
            max: 1,
            chance: 4.0,
            min_level: 1,
            max_level: i32::MAX,
            monster_ids: &[],
        },
        DropSpec {
            item: 5557,
            min: 1,
            max: 1,
            chance: 4.0,
            min_level: 1,
            max_level: i32::MAX,
            monster_ids: &[],
        },
        DropSpec {
            item: 5558,
            min: 1,
            max: 1,
            chance: 8.0,
            min_level: 1,
            max_level: i32::MAX,
            monster_ids: &[],
        },
        DropSpec {
            item: 5559,
            min: 1,
            max: 1,
            chance: 1.0,
            min_level: 1,
            max_level: i32::MAX,
            monster_ids: &[],
        },
        DropSpec {
            item: 5562,
            min: 1,
            max: 1,
            chance: 2.0,
            min_level: 1,
            max_level: i32::MAX,
            monster_ids: &[],
        },
        DropSpec {
            item: 5563,
            min: 1,
            max: 1,
            chance: 2.0,
            min_level: 1,
            max_level: i32::MAX,
            monster_ids: &[],
        },
        DropSpec {
            item: 5564,
            min: 1,
            max: 1,
            chance: 2.0,
            min_level: 1,
            max_level: i32::MAX,
            monster_ids: &[],
        },
        DropSpec {
            item: 5565,
            min: 1,
            max: 1,
            chance: 2.0,
            min_level: 1,
            max_level: i32::MAX,
            monster_ids: &[],
        },
        DropSpec {
            item: 5566,
            min: 1,
            max: 1,
            chance: 2.0,
            min_level: 1,
            max_level: i32::MAX,
            monster_ids: &[],
        },
    ],
    spawns: &[
        SpawnSpec::at(31863, 147698, -56025, -2775, 48000, 0),
        SpawnSpec::at(31863, 147453, 26968, -2204, 49545, 0),
        SpawnSpec::at(31863, 82218, 148605, -3470, 0, 0),
        SpawnSpec::at(31863, 82754, 53573, -1496, 0, 0),
        SpawnSpec::at(31863, 15064, 143254, -2668, 0, 0),
        SpawnSpec::at(31863, 111067, 218933, -3543, 0, 0),
        SpawnSpec::at(31863, -12965, 122914, -3117, 0, 0),
        SpawnSpec::at(31863, 87362, -143166, -1293, 16383, 0),
        SpawnSpec::at(31863, -81037, 150092, -3044, 0, 0),
        SpawnSpec::at(31863, 117412, 76642, -2695, 37861, 0),
        SpawnSpec::at(31863, 43983, -47758, -797, 50000, 0),
        SpawnSpec::at(31863, 47643, 50786, -2999, 31509, 0),
        SpawnSpec::at(31863, 12153, 16753, -4584, 0, 0),
        SpawnSpec::at(31863, -84458, 244761, -3730, 56462, 0),
        SpawnSpec::at(31863, 114750, -178692, -820, 0, 0),
        SpawnSpec::at(31863, -45656, -113119, -240, 0, 0),
        SpawnSpec::at(31863, -117195, 46837, 36, 0, 0),
        SpawnSpec::at(13006, 84304, 149133, -3402, 0, 0),
        SpawnSpec::at(13006, 84311, 148101, -3402, 0, 0),
        SpawnSpec::at(13006, 82948, 147658, -3469, 0, 0),
        SpawnSpec::at(13006, 80905, 147659, -3469, 0, 0),
        SpawnSpec::at(13006, 80908, 149556, -3469, 0, 0),
        SpawnSpec::at(13006, 82957, 149554, -3469, 0, 0),
        SpawnSpec::at(13006, 147849, -55119, -2734, 0, 0),
        SpawnSpec::at(13006, 147580, -55117, -2734, 0, 0),
        SpawnSpec::at(13006, 147581, -57244, -2781, 0, 0),
        SpawnSpec::at(13006, 147847, -57261, -2781, 0, 0),
        SpawnSpec::at(13006, 149085, -55826, -2781, 0, 0),
        SpawnSpec::at(13006, 146340, -55829, -2781, 0, 0),
        SpawnSpec::at(13006, 147826, -54095, -2735, 0, 0),
        SpawnSpec::at(13006, 147584, -54070, -2735, 0, 0),
        SpawnSpec::at(13006, 146235, 25921, -2013, 0, 0),
        SpawnSpec::at(13006, 147840, 25568, -2013, 0, 0),
        SpawnSpec::at(13006, 147055, 25568, -2013, 0, 0),
        SpawnSpec::at(13006, 148694, 25929, -2013, 0, 0),
        SpawnSpec::at(13006, 147733, 27366, -2205, 0, 0),
        SpawnSpec::at(13006, 147197, 27364, -2205, 0, 0),
        SpawnSpec::at(13006, 147266, 29065, -2269, 0, 0),
        SpawnSpec::at(13006, 147646, 29065, -2269, 0, 0),
        SpawnSpec::at(13006, 147456, 27664, -2204, 0, 0),
        SpawnSpec::at(13006, 148078, -55960, -2781, 0, 0),
        SpawnSpec::at(13006, 147348, -55939, -2781, 0, 0),
        SpawnSpec::at(13006, 117056, 75627, -2726, 0, 0),
        SpawnSpec::at(13006, 116473, 75352, -2712, 0, 0),
        SpawnSpec::at(13006, 115785, 76111, -2715, 0, 0),
        SpawnSpec::at(13006, 115939, 76544, -2719, 0, 0),
        SpawnSpec::at(13006, 116833, 77400, -2697, 0, 0),
        SpawnSpec::at(13006, 116666, 76210, -2730, 0, 0),
        SpawnSpec::at(13006, -13130, 122533, -3117, 0, 0),
        SpawnSpec::at(13006, -13165, 122425, -2989, 0, 0),
        SpawnSpec::at(13006, -13126, 122806, -3117, 0, 0),
        SpawnSpec::at(13006, 15733, 142767, -2706, 0, 0),
        SpawnSpec::at(13006, 16208, 142710, -2706, 0, 0),
        SpawnSpec::at(13006, 83807, 149325, -3408, 0, 0),
        SpawnSpec::at(13006, 83690, 147987, -3403, 0, 0),
        SpawnSpec::at(13006, 83896, 147865, -3403, 0, 0),
        SpawnSpec::at(13006, 83794, 147924, -3403, 0, 0),
        SpawnSpec::at(13006, 83700, 149249, -3403, 0, 0),
        SpawnSpec::at(13006, 83895, 149379, -3403, 0, 0),
        SpawnSpec::at(13006, 84329, 149395, -3403, 0, 0),
        SpawnSpec::at(13006, 84550, 149415, -3403, 0, 0),
        SpawnSpec::at(13006, 84797, 149410, -3403, 0, 0),
        SpawnSpec::at(13006, 85051, 149459, -3403, 0, 0),
        SpawnSpec::at(13006, 85050, 149757, -3390, 0, 0),
        SpawnSpec::at(13006, 85945, 149865, -3390, 0, 0),
        SpawnSpec::at(13006, 85688, 149875, -3359, 0, 0),
        SpawnSpec::at(13006, 85631, 150244, -3359, 0, 0),
        SpawnSpec::at(13006, 85988, 150243, -3495, 0, 0),
        SpawnSpec::at(13006, 86455, 152599, -3495, 0, 0),
        SpawnSpec::at(13006, 86253, 152603, -3495, 0, 0),
        SpawnSpec::at(13006, 85189, 152606, -3495, 0, 0),
        SpawnSpec::at(13006, 85407, 152603, -3390, 0, 0),
        SpawnSpec::at(13006, 85182, 149752, -3390, 0, 0),
        SpawnSpec::at(13006, 85184, 149498, -3467, 0, 0),
        SpawnSpec::at(13006, 83200, 147872, -3368, 0, 0),
        SpawnSpec::at(13006, 83172, 148400, -3370, 0, 0),
        SpawnSpec::at(13006, 83177, 148845, -3370, 0, 0),
        SpawnSpec::at(13006, 83171, 149325, -3370, 0, 0),
        SpawnSpec::at(13006, 83171, 147913, -3370, 0, 0),
        SpawnSpec::at(13006, 83203, 149379, -3403, 0, 0),
        SpawnSpec::at(13006, 83681, 149691, -3403, 0, 0),
        SpawnSpec::at(13006, 84268, 149746, -3403, 0, 0),
        SpawnSpec::at(13006, 83680, 147535, -3403, 0, 0),
        SpawnSpec::at(13006, 84320, 147840, -3403, 0, 0),
        SpawnSpec::at(13006, 84545, 147839, -3403, 0, 0),
        SpawnSpec::at(13006, 85749, 147241, -3403, 0, 0),
        SpawnSpec::at(13006, 85747, 147492, -3403, 0, 0),
        SpawnSpec::at(13006, 87421, 147486, -3403, 0, 0),
        SpawnSpec::at(13006, 85916, 147483, -3403, 0, 0),
        SpawnSpec::at(13006, 85918, 147238, -3403, 0, 0),
        SpawnSpec::at(13006, 83776, 147120, -3403, 0, 0),
        SpawnSpec::at(13006, 84032, 147120, -3403, 0, 0),
        SpawnSpec::at(13006, 84024, 146970, -3403, 0, 0),
        SpawnSpec::at(13006, 83778, 146971, -3403, 0, 0),
        SpawnSpec::at(13006, 84030, 143571, -3403, 0, 0),
        SpawnSpec::at(13006, 83752, 143572, -3408, 0, 0),
        SpawnSpec::at(13006, 82848, 147760, -3403, 0, 0),
        SpawnSpec::at(13006, 82847, 149456, -3403, 0, 0),
        SpawnSpec::at(13006, 81042, 149458, -3403, 0, 0),
        SpawnSpec::at(13006, 81039, 147792, -3403, 0, 0),
        SpawnSpec::at(13006, 82022, 149592, -3467, 0, 0),
        SpawnSpec::at(13006, 81600, 149744, -3468, 0, 0),
        SpawnSpec::at(13006, 81472, 149750, -3467, 0, 0),
        SpawnSpec::at(13006, 82197, 147530, -3467, 0, 0),
        SpawnSpec::at(13006, 82694, 146601, -3463, 0, 0),
        SpawnSpec::at(13006, 80706, 146406, -3463, 0, 0),
        SpawnSpec::at(13006, 79242, 149588, -3467, 0, 0),
        SpawnSpec::at(13006, 82355, 149905, -3464, 0, 0),
        SpawnSpec::at(13006, 83482, 147543, -3536, 0, 0),
        SpawnSpec::at(13006, 87888, 147808, -3531, 0, 0),
        SpawnSpec::at(13006, 88097, 148165, -3531, 0, 0),
        SpawnSpec::at(13006, 88160, 148630, -3403, 0, 0),
        SpawnSpec::at(13006, 88080, 149120, -3403, 0, 0),
        SpawnSpec::at(13006, 87856, 149456, -3403, 0, 0),
        SpawnSpec::at(13006, 87440, 149744, -3403, 0, 0),
        SpawnSpec::at(13006, 86517, 149584, -3403, 0, 0),
        SpawnSpec::at(13006, 86768, 149574, -3403, 0, 0),
        SpawnSpec::at(13006, 87030, 149568, -3403, 0, 0),
        SpawnSpec::at(13006, 87312, 149440, -3403, 0, 0),
        SpawnSpec::at(13006, 87600, 149248, -3403, 0, 0),
        SpawnSpec::at(13006, 87776, 148976, -3403, 0, 0),
        SpawnSpec::at(13006, 87826, 148629, -3403, 0, 0),
        SpawnSpec::at(13006, 87792, 148288, -3403, 0, 0),
        SpawnSpec::at(13006, 87616, 148000, -3403, 0, 0),
        SpawnSpec::at(13006, 87312, 147776, -3403, 0, 0),
        SpawnSpec::at(13006, 87029, 147667, -3403, 0, 0),
        SpawnSpec::at(13006, 88427, 147174, -3403, 0, 0),
        SpawnSpec::at(13006, 88426, 147533, -3362, 0, 0),
        SpawnSpec::at(13006, 84065, 143505, -3362, 0, 0),
        SpawnSpec::at(13006, 83706, 143501, -3362, 0, 0),
        SpawnSpec::at(13006, 81777, 143495, -3361, 0, 0),
        SpawnSpec::at(13006, 81278, 143478, -3530, 0, 0),
        SpawnSpec::at(13006, 77164, 148545, -3526, 0, 0),
        SpawnSpec::at(13006, 77166, 148715, -3597, 0, 0),
        SpawnSpec::at(13006, 77609, 148380, -3596, 0, 0),
        SpawnSpec::at(13006, 77470, 148381, -3579, 0, 0),
        SpawnSpec::at(13006, 78945, 148767, -3579, 0, 0),
        SpawnSpec::at(13006, 80463, 148539, -3595, 0, 0),
        SpawnSpec::at(13006, 80462, 148699, -3531, 0, 0),
        SpawnSpec::at(13006, 81460, 149988, -3531, 0, 0),
        SpawnSpec::at(13006, 81621, 149991, -3531, 0, 0),
        SpawnSpec::at(13006, 81297, 152955, -3531, 0, 0),
        SpawnSpec::at(13006, 81804, 152934, -3531, 0, 0),
        SpawnSpec::at(13006, 81420, 152319, -3531, 0, 0),
        SpawnSpec::at(13006, 80077, 151798, -3531, 0, 0),
        SpawnSpec::at(13006, 81055, 152545, -3531, 0, 0),
        SpawnSpec::at(13006, 81073, 150734, -3531, 0, 0),
        SpawnSpec::at(13006, 80059, 147482, -3531, 0, 0),
        SpawnSpec::at(13006, 80152, 145485, -3531, 0, 0),
        SpawnSpec::at(13006, 81461, 147253, -3531, 0, 0),
        SpawnSpec::at(13006, 81622, 147253, -3531, 0, 0),
        SpawnSpec::at(13006, 81154, 145454, -3531, 0, 0),
        SpawnSpec::at(13006, 81158, 145633, -3531, 0, 0),
        SpawnSpec::at(13006, 81282, 145456, -3531, 0, 0),
        SpawnSpec::at(13006, 81283, 145636, -3531, 0, 0),
        SpawnSpec::at(13006, 82841, 144687, -3531, 0, 0),
        SpawnSpec::at(13006, 80426, 144439, -3531, 0, 0),
        SpawnSpec::at(13006, 82841, 144327, -3531, 0, 0),
        SpawnSpec::at(13006, 85073, 146404, -3460, 0, 0),
        SpawnSpec::at(13006, 81774, 146488, -3460, 0, 0),
        SpawnSpec::at(13006, 81776, 146669, -3460, 0, 0),
        SpawnSpec::at(13006, 81896, 146671, -3460, 0, 0),
        SpawnSpec::at(13006, 81905, 146488, -3460, 0, 0),
        SpawnSpec::at(13006, 81287, 146823, -3460, 0, 0),
        SpawnSpec::at(13006, 81285, 147005, -3460, 0, 0),
        SpawnSpec::at(13006, 81150, 146821, -3460, 0, 0),
        SpawnSpec::at(13006, 81151, 147003, -3460, 0, 0),
        SpawnSpec::at(13006, 79386, 146898, -3460, 0, 0),
        SpawnSpec::at(13006, 80152, 146901, -3494, 0, 0),
        SpawnSpec::at(13006, 80063, 149310, -3494, 0, 0),
        SpawnSpec::at(13006, 79415, 150246, -3529, 0, 0),
        SpawnSpec::at(13006, 79717, 150251, -3529, 0, 0),
        SpawnSpec::at(13006, 79758, 150263, -3531, 0, 0),
        SpawnSpec::at(13006, 80531, 149746, -3531, 0, 0),
        SpawnSpec::at(13006, 80496, 149774, -3531, 0, 0),
        SpawnSpec::at(13006, 80487, 149729, -3531, 0, 0),
        SpawnSpec::at(13006, 80542, 149786, -3531, 0, 0),
        SpawnSpec::at(13006, 80454, 149724, -3531, 0, 0),
        SpawnSpec::at(13006, 80506, 149806, -3531, 0, 0),
        SpawnSpec::at(13006, 82774, 150512, -3531, 0, 0),
        SpawnSpec::at(13006, 81918, 150748, -3531, 0, 0),
        SpawnSpec::at(13006, 81919, 150568, -3531, 0, 0),
        SpawnSpec::at(13006, 81281, 150401, -3531, 0, 0),
        SpawnSpec::at(13006, 81282, 150221, -3531, 0, 0),
        SpawnSpec::at(13006, 81777, 150568, -3531, 0, 0),
        SpawnSpec::at(13006, 81777, 150751, -3531, 0, 0),
        SpawnSpec::at(13006, 80180, 150722, -3531, 0, 0),
        SpawnSpec::at(13006, 79711, 148955, -3531, 0, 0),
        SpawnSpec::at(13006, 79893, 148957, -3531, 0, 0),
        SpawnSpec::at(13006, 79710, 148815, -3531, 0, 0),
        SpawnSpec::at(13006, 79894, 148821, -3531, 0, 0),
        SpawnSpec::at(13006, 79891, 148403, -3531, 0, 0),
        SpawnSpec::at(13006, 79708, 148397, -3531, 0, 0),
        SpawnSpec::at(13006, 82419, 146424, -3531, 0, 0),
        SpawnSpec::at(13006, 82416, 146912, -3531, 0, 0),
        SpawnSpec::at(13006, 81768, 145442, -3531, 0, 0),
        SpawnSpec::at(13006, 81277, 151482, -3531, 0, 0),
        SpawnSpec::at(13006, 81802, 152013, -3531, 0, 0),
        SpawnSpec::at(13006, 81147, 150219, -3531, 0, 0),
        SpawnSpec::at(13006, 81144, 150400, -3531, 0, 0),
        SpawnSpec::at(13006, 78690, 148400, -3531, 0, 0),
        SpawnSpec::at(13006, 79892, 148262, -3531, 0, 0),
        SpawnSpec::at(13006, 79712, 148256, -3531, 0, 0),
        SpawnSpec::at(13006, 80063, 148293, -3531, 0, 0),
        SpawnSpec::at(13006, 83686, 145210, -3531, 0, 0),
        SpawnSpec::at(13006, 86042, 146906, -3531, 0, 0),
        SpawnSpec::at(13006, 85918, 145889, -3531, 0, 0),
        SpawnSpec::at(13006, 84308, 145932, -3531, 0, 0),
    ],
    on_enter_msg: "Saving Santa: Event ongoing!",
    on_end_msg: "Saving Santa: Event end!",
    destroy_items_on_end: &[],
};

/// What Java keeps in the fields of `SavingSanta` and in its `static` collections.
struct State {
    /// The constructor ran inside the event period (see the head of the file).
    registered: bool,
    is_santa_free: bool,
    is_jack_pot: bool,
    is_waiting_for_player_skill: bool,
    special_trees: Vec<ObjectId>,
    rewarded_players: BTreeMap<String, i64>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            registered: false,
            is_santa_free: true,
            is_jack_pot: false,
            is_waiting_for_player_skill: false,
            special_trees: Vec::new(),
            rewarded_players: BTreeMap::new(),
        }
    }
}

/// `NpcSay(npc, NPC_GENERAL, NPC_STRINGS[index])`.
fn say(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, index: usize) {
    if let Some(string) = NPC_STRINGS.get(index) {
        ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, *string, &[]);
    }
}

/// `new SystemMessage(id)` to every online player.
fn broadcast_system_message(ctx: &ScriptCtx<'_>, id: SystemMessageId) {
    ctx.world()
        .broadcast_to_all_online_players(&SystemMessage::new(id));
}

/// The head of every page of the event.
fn page_head(title: &str, icon: &str) -> String {
    format!(
        "<html><title>Christmas Event</title><body><br><br><table width=260><tr><td></td><td width=40></td><td width=40></td></tr><tr><td><font color=LEVEL>{title}</font></td><td width=40><img src=\"{icon}\" width=32 height=32></td><td width=40></td></tr></table><br><br><table width=260>"
    )
}

/// One row of the list: the name, what the player has and whether it is enough.
fn page_row(name: &str, count: i64, ok: bool) -> String {
    let (color, mark) = if ok { ("0FF000", "OK") } else { ("8ae2ffb", "NO") };
    format!(
        "<tr><td>{name}</td><td width=40>{count}</td><td width=40><font color={color}>{mark}</font></td></tr>"
    )
}

const PAGE_SPACE: &str =
    "<tr><td><br></td><td width=40></td><td width=40></td></tr></table><table width=260>";

fn page_button(label: &str, bypass: &str) -> String {
    format!(
        "<tr><td><center><button value=\"{label}\" action=\"bypass -h Script SavingSanta {bypass}\" width=110 height=20 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></center></td></tr></table></body></html>"
    )
}

const PAGE_NOT_ENOUGH: &str = "</table><br><br>You do not have enough items.</center></body></html>";

/// `Inventory.getInventoryItemCount(itemId, -1)`.
fn count_of(ctx: &ScriptCtx<'_>, player: ObjectId, item_id: i32) -> i64 {
    ctx.world().inventory_item_count(player, item_id)
}

/// Java `events.SavingSanta.SavingSanta`.
pub struct SavingSanta;

impl Script for SavingSanta {
    fn name(&self) -> &'static str {
        "SavingSanta"
    }

    fn path(&self) -> &'static str {
        "events/SavingSanta"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[SANTA_TRAINEE_ID]);
        s.add_first_talk_id(&[SANTA_TRAINEE_ID]);
        s.add_talk_id(&[SANTA_TRAINEE_ID]);
        s.add_first_talk_id(&[THOMAS_D_TURKEY_ID]);
        s.add_first_talk_id(&[HOLIDAY_SANTA_ID]);
        s.add_first_talk_id(&[HOLIDAY_SLED_ID]);
        s.add_skill_see_id(&[THOMAS_D_TURKEY_ID]);
        s.add_spell_finished_id(&[THOMAS_D_TURKEY_ID]);
        s.add_spawn_id(&[SPECIAL_CHRISTMAS_TREE_ID]);
    }

    fn on_load(&self, ctx: &mut ScriptCtx<'_>) {
        long_time_event::load(ctx, &CONFIG);

        if !long_time_event::is_event_period(ctx) {
            return;
        }
        ctx.shared_state::<State>().registered = true;

        ctx.start_quest_timer("SpecialTreeHeal", 5000, None, None, false);
        if SAVING_SANTA {
            ctx.start_quest_timer("ThomasQuest", 1000, None, None, false);
        }

        if SANTAS_HELPER_AUTOBUFF {
            ctx.start_quest_timer("SantaBlessings", 5000, None, None, false);
        }
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        let state = ctx.shared_state::<State>();
        if state.registered {
            state.special_trees.push(npc.object_id);
        }
    }

    fn on_skill_see(
        &self,
        ctx: &mut ScriptCtx<'_>,
        _npc: ScriptNpc,
        caster: ObjectId,
        skill: SkillKey,
        _targets: &[ObjectId],
        _is_summon: bool,
    ) {
        let state = ctx.shared_state::<State>();
        if !state.registered {
            return;
        }
        if state.is_waiting_for_player_skill && skill.id > 21013 && skill.id < 21017 {
            let level = skill.id - 21013;
            ctx.broadcast_magic_skill_use(caster, caster, caster, 23019, level, 3000, 1);
            ctx.world_mut()
                .apply_effects(caster, caster, SkillKey::new(23019, level));
        }
    }

    fn on_spell_finished(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        _player: Option<ObjectId>,
        skill: SkillKey,
    ) {
        if !ctx.shared_state::<State>().registered {
            return;
        }
        // Turkey's Choice
        // Level 1: Scissors
        // Level 2: Rock
        // Level 3: Paper
        if skill.id == 6100 {
            ctx.shared_state::<State>().is_waiting_for_player_skill = false;
            let in_range: Vec<ObjectId> = ctx
                .world()
                .visible_objects_in_range(npc.object_id, 600)
                .into_iter()
                .filter(|object| ctx.world().player(*object).is_some())
                .collect();
            for pl in in_range {
                // Level 1: Scissors
                // Level 2: Rock
                // Level 3: Paper
                if !ctx.world().is_affected_by_skill(pl, 23019) {
                    continue;
                }

                let result = ctx.world().effect_skill_level(pl, 23019).unwrap_or(0) - skill.level;

                if result == 0 {
                    // Oh. I'm bored.
                    say(ctx, npc, 15);
                } else if result == -1 || result == 2 {
                    // Now!! Those of you who lost, go away!
                    // What a bunch of losers.
                    let pick = ctx.get_random(2);
                    say(ctx, npc, 6 + usize::try_from(pick).unwrap_or(0));
                    ctx.broadcast_magic_skill_use(pl, pl, pl, 23023, 1, 3000, 1);
                    ctx.world_mut()
                        .stop_skill_effects(pl, SkillFinishType::Removed, 23022);
                } else if result == 1 || result == -2 {
                    let level = ctx
                        .world()
                        .effect_skill_level(pl, 23022)
                        .map_or(1, |level| level + 1);
                    ctx.broadcast_magic_skill_use(pl, pl, pl, 23022, level, 3000, 1);
                    ctx.world_mut().apply_effects(pl, pl, SkillKey::new(23022, level));

                    if level == 1 || level == 2 {
                        // Ah, okay...
                        // Agh!! I wasn't going to do that!
                        // You're cursed!! Oh.. What?
                        // Have you done nothing but rock-paper-scissors??
                        let pick = ctx.get_random(4);
                        say(ctx, npc, 10 + usize::try_from(pick).unwrap_or(0));
                    } else if level == 3 {
                        ctx.world_mut().apply_effects(pl, pl, SkillKey::new(23018, 1));

                        // Stop it, no more... I did it because I was too lonely...
                        say(ctx, npc, 13);
                    } else if level == 4 {
                        broadcast_system_message(
                            ctx,
                            SystemMessageId::YouHaveDefeatedThomasDTurkeyAndRescuedSanta,
                        );

                        // I have to release Santa... How infuriating!!!
                        // I hate happy Merry Christmas!!!
                        let pick = ctx.get_random(2);
                        say(ctx, npc, 14 + usize::try_from(pick).unwrap_or(0));

                        ctx.start_quest_timer("SantaSpawn", 120_000, None, None, false);
                        let holiday_sled =
                            ctx.add_spawn(HOLIDAY_SLED_ID, 117935, -126003, -2585, 54625, false, 12_000, 0);

                        if let Some(holiday_sled) = holiday_sled {
                            // Message from Santa Claus: Many blessings to $s1, who saved me~
                            let name = ctx.player_name(pl);
                            ctx.broadcast_npc_say(
                                holiday_sled,
                                ChatType::NpcShout,
                                NPC_STRINGS[28],
                                &[&name],
                            );

                            // Oh ho ho.... Merry Christmas!!
                            say(ctx, holiday_sled, 17);
                        }

                        if ctx.get_random(100) > 90 {
                            ctx.shared_state::<State>().is_jack_pot = true;
                            ctx.give_items(pl, BR_XMAS_PRESENT_JACKPOT, 1);
                        } else {
                            ctx.give_items(pl, BR_XMAS_PRESENT_NORMAL, 1);
                        }

                        // `ThreadPool.schedule(new SledAnimation(holidaySled), 7000)`
                        ctx.start_quest_timer("SledAnimation", 7000, holiday_sled, None, false);
                        // `npc.decayMe()`
                        ctx.delete_npc(npc);
                        ctx.shared_state::<State>().is_santa_free = true;
                        break;
                    }
                }
            }
        }
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        if long_time_event::on_event(ctx, &CONFIG, event, npc) {
            return None;
        }
        // `SledAnimation.run()`: the sled's last animation. (A `Runnable`, not an event.)
        if event == "SledAnimation" {
            if let Some(sled) = npc {
                ctx.world_mut().script_set_intention_idle(sled.object_id);
                ctx.world().broadcast_packet(
                    sled.object_id,
                    &SocialAction {
                        object_id: sled.object_id.0,
                        action_id: 1,
                    },
                );
            }
            return None;
        }
        if !long_time_event::is_event_period(ctx) {
            return None;
        }

        if event.eq_ignore_ascii_case("ThomasQuest") {
            ctx.start_quest_timer("ThomasQuest", 14_400_000, None, None, false);
            let thomas = ctx.add_spawn(
                THOMAS_D_TURKEY_ID,
                THOMAS_SPAWN.0,
                THOMAS_SPAWN.1,
                THOMAS_SPAWN.2,
                THOMAS_SPAWN.3,
                false,
                1_800_000,
                0,
            );
            broadcast_system_message(ctx, SystemMessageId::ThomasDTurkeyHasAppearedPleaseSaveSanta);
            ctx.start_quest_timer("ThomasCast1", 15_000, thomas, None, false);
            ctx.shared_state::<State>().is_santa_free = false;
        } else if event.eq_ignore_ascii_case("SantaSpawn") {
            if ctx.shared_state::<State>().is_santa_free {
                ctx.start_quest_timer("SantaSpawn", 120_000, None, None, false);

                for pl in ctx.online_players() {
                    if ctx.player_level(pl) >= 20
                        && ctx.is_in_combat(pl)
                        && !ctx.world().is_inside_zone(pl, ZoneId::Peace)
                        && !ctx.world().script_is_flying_mounted(pl)
                    {
                        let account = ctx
                            .world()
                            .player(pl)
                            .map(|p| p.data.account.clone())
                            .unwrap_or_default();
                        let rewarded = ctx
                            .shared_state::<State>()
                            .rewarded_players
                            .get(&account)
                            .copied();
                        if let Some(last) = rewarded {
                            let elapsed_time_since_last_rewarded = ctx.now_ms() - last;
                            if elapsed_time_since_last_rewarded < MIN_TIME_BETWEEN_2_REWARDS {
                                continue;
                            }
                        } else {
                            // NOTE(java-quirk): `player` is the player of the timer, which is
                            // `null` (`startQuestTimer("SantaSpawn", .., null, null)`), so Java
                            // throws a `NullPointerException` here, ends the event and never
                            // gives a reward: `_rewardedPlayers` stays empty.
                            let Some(player) = player else {
                                warn!("SavingSanta: java.lang.NullPointerException in onEvent(SantaSpawn)");
                                return None;
                            };
                            let time: i64 = ctx
                                .player_variable(player, "LAST_SANTA_REWARD")
                                .and_then(|v| v.trim().parse().ok())
                                .unwrap_or(0);
                            if ctx.now_ms() - time < MIN_TIME_BETWEEN_2_REWARDS {
                                ctx.shared_state::<State>().rewarded_players.insert(account, time);
                                continue;
                            }
                        }

                        let Some((px, py, pz)) = ctx.world().position_of(pl) else {
                            continue;
                        };
                        // `MathUtil.pow(-1, getRandom(1, 2)) * 50`
                        let sign_x = if ctx.get_random_range(1, 2) == 1 { -50 } else { 50 };
                        let locx = px + sign_x;
                        let sign_y = if ctx.get_random_range(1, 2) == 1 { -50 } else { 50 };
                        let locy = py + sign_y;
                        let heading = l2_world::movement::heading_from_points(locx, locy, px, py);
                        let santa =
                            ctx.add_spawn(HOLIDAY_SANTA_ID, locx, locy, pz, heading, false, 30_000, 0);
                        let now = ctx.now_ms();
                        ctx.shared_state::<State>().rewarded_players.insert(account, now);
                        if let Some(player) = player {
                            ctx.set_player_variable(player, "LAST_SANTA_REWARD", &now.to_string());
                        }
                        ctx.start_quest_timer("SantaRewarding0", 500, santa, Some(pl), false);
                    }
                }
            }
        } else if event.eq_ignore_ascii_case("ThomasCast1") {
            let npc = npc?;
            if !ctx.world().npc(npc.object_id).is_some_and(|n| n.decayed) {
                ctx.shared_state::<State>().is_waiting_for_player_skill = true;
                ctx.start_quest_timer("ThomasCast2", 4000, Some(npc), None, false);
                ctx.world_mut().do_cast(npc.object_id, SkillKey::new(6116, 1));

                // It's hurting... I'm in pain... What can I do for the pain...
                // No... When I lose that one... I'll be in more pain...
                // Hahahah!!! I captured Santa Claus!! There will be no gifts this year!!!
                // Now! Why don't you take up the challenge?
                // Come on, I'll take all of you on!
                // How about it? I think I won?
                let pick = ctx.get_random(6);
                say(ctx, npc, usize::try_from(pick).unwrap_or(0));
            } else if !ctx.shared_state::<State>().is_santa_free {
                broadcast_system_message(
                    ctx,
                    SystemMessageId::YouDidNotRescueSantaAndThomasDTurkeyHasDisappeared,
                );
                ctx.shared_state::<State>().is_waiting_for_player_skill = false;
            }
        } else if event.eq_ignore_ascii_case("ThomasCast2") {
            let npc = npc?;
            if !ctx.world().npc(npc.object_id).is_some_and(|n| n.decayed) {
                ctx.start_quest_timer("ThomasCast1", 13_000, Some(npc), None, false);
                let level = ctx.get_random_range(1, 3);
                ctx.world_mut().do_cast(npc.object_id, SkillKey::new(6100, level));

                // (the same six lines as in ThomasCast1)
                let pick = ctx.get_random(6);
                say(ctx, npc, usize::try_from(pick).unwrap_or(0));
            } else if !ctx.shared_state::<State>().is_santa_free {
                broadcast_system_message(
                    ctx,
                    SystemMessageId::YouDidNotRescueSantaAndThomasDTurkeyHasDisappeared,
                );
                ctx.shared_state::<State>().is_waiting_for_player_skill = false;
            }
        } else if event.eq_ignore_ascii_case("SantaRewarding0") {
            let npc = npc?;
            ctx.start_quest_timer("SantaRewarding1", 9500, Some(npc), player, false);
            social_action(ctx, npc, 3);
        } else if event.eq_ignore_ascii_case("SantaRewarding1") {
            let npc = npc?;
            ctx.start_quest_timer("SantaRewarding2", 5000, Some(npc), player, false);
            social_action(ctx, npc, 1);

            // Merry Christmas~ Thank you for rescuing me from that wretched Turkey.
            say(ctx, npc, 21);
        } else if event.eq_ignore_ascii_case("SantaRewarding2") {
            let npc = npc?;
            ctx.start_quest_timer("SantaRewarding3", 5000, Some(npc), player, false);

            // I have a gift for $s1.
            let name = ctx.player_name(player?);
            ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, NPC_STRINGS[23], &[&name]);
        } else if event.eq_ignore_ascii_case("SantaRewarding3") {
            let npc = npc?;
            let player = player?;
            social_action(ctx, npc, 2);
            if ctx.shared_state::<State>().is_jack_pot {
                // Take a look at the inventory. Perhaps there might be a big present~
                say(ctx, npc, 25);
                ctx.give_items(player, BR_XMAS_PRESENT_JACKPOT, 1);
                ctx.shared_state::<State>().is_jack_pot = false;
            } else {
                // Take a look at the inventory. I hope you like the gift I gave you.
                say(ctx, npc, 24);
                ctx.give_items(player, BR_XMAS_PRESENT_NORMAL, 1);
            }
        } else if event.eq_ignore_ascii_case("SpecialTreeHeal") {
            ctx.start_quest_timer("SpecialTreeHeal", 9000, None, None, false);
            for tree in ctx.shared_state::<State>().special_trees.clone() {
                let Some((tree_x, tree_y, _)) = ctx.world().position_of(tree) else {
                    continue;
                };
                for playerr in ctx.world().visible_players(tree) {
                    let (xx_min, yy_min) = (tree_x - 60, tree_y - 60);
                    let (xx_max, yy_max) = (tree_x + 60, tree_y + 60);
                    let Some((player_x, player_y, _)) = ctx.world().position_of(playerr) else {
                        continue;
                    };

                    if player_x > xx_min && player_x < xx_max && player_y > yy_min && player_y < yy_max {
                        ctx.world_mut()
                            .apply_effects(tree, playerr, SkillKey::new(2139, 1));
                    }
                }
            }
        } else if let Some(player) = player {
            // FIXME: Unhardcore html!
            if event.eq_ignore_ascii_case("Tree") {
                let mut items_ok = 0;
                let mut html = page_head("Christmas Tree", "Icon.etc_x_mas_tree_i00");

                for (item_id, item_count) in TREE_REQUIRED_ITEMS {
                    let piece_count = count_of(ctx, player, item_id);
                    let name = ctx.world().item_template_name(item_id).unwrap_or_default();
                    if piece_count >= item_count {
                        items_ok += 1;
                        html.push_str(&page_row(&name, piece_count, true));
                    } else {
                        html.push_str(&page_row(&name, piece_count, false));
                    }
                }

                if items_ok == 4 {
                    html.push_str(PAGE_SPACE);
                    html.push_str(&page_button("Get the tree", "buyTree"));
                } else if items_ok < 4 {
                    html.push_str(PAGE_NOT_ENOUGH);
                }

                return Some(html);
            } else if event.eq_ignore_ascii_case("buyTree") {
                ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
                for (item_id, item_count) in TREE_REQUIRED_ITEMS {
                    if count_of(ctx, player, item_id) < item_count {
                        ctx.send_system_message(player, SystemMessageId::IncorrectItemCount2);
                        return None;
                    }
                }

                for (item_id, item_count) in TREE_REQUIRED_ITEMS {
                    ctx.world_mut()
                        .destroy_item_by_item_id(player, item_id, item_count, true);
                }

                ctx.give_items(player, X_MAS_TREE1, 1);
            } else if event.eq_ignore_ascii_case("SpecialTree") {
                let mut html = page_head("Special Christmas Tree", "Icon.etc_x_mas_tree_i00");
                let piece_count = count_of(ctx, player, X_MAS_TREE1);
                let mut items_ok = 0;

                if piece_count >= 10 {
                    items_ok = 1;
                    html.push_str(&page_row("Christmas Tree", piece_count, true));
                } else {
                    html.push_str(&page_row("Christmas Tree", piece_count, false));
                }

                if items_ok == 1 {
                    // NOTE(java-quirk): the spacer row is added twice.
                    html.push_str(PAGE_SPACE);
                    html.push_str(PAGE_SPACE);
                    html.push_str(&page_button("Get the tree", "buySpecialTree"));
                } else if items_ok == 0 {
                    html.push_str(PAGE_NOT_ENOUGH);
                }

                return Some(html);
            } else if event.eq_ignore_ascii_case("buySpecialTree") {
                ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
                if count_of(ctx, player, X_MAS_TREE1) < 10 {
                    ctx.send_system_message(player, SystemMessageId::IncorrectItemCount2);
                    return None;
                }

                ctx.world_mut()
                    .destroy_item_by_item_id(player, X_MAS_TREE1, 10, true);
                ctx.give_items(player, X_MAS_TREE2, 1);
            } else if event.eq_ignore_ascii_case("SantaHat") {
                let mut html = page_head("Santa's Hat", "Icon.Accessory_santas_cap_i00");
                let piece_count = count_of(ctx, player, X_MAS_TREE1);
                let mut items_ok = 0;

                if piece_count >= 10 {
                    items_ok = 1;
                    html.push_str(&page_row("Christmas Tree", piece_count, true));
                } else {
                    html.push_str(&page_row("Christmas Tree", piece_count, false));
                }

                if items_ok == 1 {
                    html.push_str(PAGE_SPACE);
                    html.push_str(&page_button("Get the hat", "buyHat"));
                } else if items_ok == 0 {
                    html.push_str(PAGE_NOT_ENOUGH);
                }

                return Some(html);
            } else if event.eq_ignore_ascii_case("buyHat") {
                ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
                if count_of(ctx, player, X_MAS_TREE1) < 10 {
                    ctx.send_system_message(player, SystemMessageId::IncorrectItemCount2);
                    return None;
                }

                ctx.world_mut()
                    .destroy_item_by_item_id(player, X_MAS_TREE1, 10, true);
                ctx.give_items(player, SANTAS_HAT_ID, 1);
            } else if event.eq_ignore_ascii_case("SavingSantaHat") {
                let mut html = page_head("Saving Santa's Hat", "Icon.Accessory_santas_cap_i00");
                let piece_count = ctx.adena_of(player);
                let mut items_ok = 0;

                if piece_count >= 50_000 {
                    items_ok = 1;
                    html.push_str(&page_row("Adena", piece_count, true));
                } else {
                    html.push_str(&page_row("Adena", piece_count, false));
                }

                if items_ok == 1 {
                    html.push_str(PAGE_SPACE);
                    html.push_str(&page_button("Get the hat", "buySavingHat"));
                } else if items_ok == 0 {
                    html.push_str("</table><br><br>You do not have enough Adena.</center></body></html>");
                }

                return Some(html);
            } else if event.eq_ignore_ascii_case("buySavingHat") {
                ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
                if ctx.adena_of(player) < 50_000 {
                    return None;
                }

                ctx.world_mut().reduce_adena(player, 50_000, true);
                ctx.give_items(player, BR_XMAS_GAWIBAWIBO_CAP, 1);
            } else if event.eq_ignore_ascii_case("HolidayFestival") {
                if ctx.shared_state::<State>().is_santa_free {
                    let npc = npc?;
                    ctx.broadcast_magic_skill_use(
                        npc.object_id,
                        npc.object_id,
                        player,
                        BR_XMAS_REWARD_BUFF,
                        1,
                        2000,
                        1,
                    );
                    ctx.world_mut()
                        .apply_effects(player, player, SkillKey::new(BR_XMAS_REWARD_BUFF, 1));
                } else {
                    return Some("savingsanta-nobuff.htm".to_owned());
                }
            } else if event.eq_ignore_ascii_case("getWeapon") {
                if count_of(ctx, player, BR_XMAS_WPN_TICKET_NORMAL) > 0
                    && count_of(ctx, player, BR_XMAS_WPN_TICKET_JACKPOT) > 0
                {
                    return Some("savingsanta-noweapon.htm".to_owned());
                }

                return Some("savingsanta-weapon.htm".to_owned());
            } else if let Some(number) = event.strip_prefix("weapon_") {
                // NOTE(java-quirk): `Integer.parseInt(event.split("weapon_")[1]) - 1` and the array
                // index throw for a bad number; the event ends there.
                let Some(item_id) = number
                    .parse::<i32>()
                    .ok()
                    .and_then(|n| usize::try_from(n - 1).ok())
                    .and_then(|index| WEAPON_REWARDS.get(index).copied())
                else {
                    warn!("SavingSanta: bad weapon event {event}");
                    return None;
                };
                if count_of(ctx, player, BR_XMAS_WPN_TICKET_JACKPOT) > 0 {
                    ctx.world_mut()
                        .destroy_item_by_item_id(player, BR_XMAS_WPN_TICKET_JACKPOT, 1, true);
                    ctx.give_items_enchanted(player, item_id, 1, 10);
                    // NOTE(java-quirk): the jackpot ticket also tells "incorrect item count".
                    ctx.send_system_message(player, SystemMessageId::IncorrectItemCount2);
                } else if count_of(ctx, player, BR_XMAS_WPN_TICKET_NORMAL) > 0 {
                    ctx.world_mut()
                        .destroy_item_by_item_id(player, BR_XMAS_WPN_TICKET_NORMAL, 1, true);
                    let enchant = ctx.get_random_range(4, 16);
                    ctx.give_items_enchanted(player, item_id, 1, enchant);
                }
            }
        }

        None
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        if !ctx.shared_state::<State>().registered {
            return None;
        }
        match npc.id {
            THOMAS_D_TURKEY_ID | HOLIDAY_SANTA_ID | HOLIDAY_SLED_ID => {
                ctx.world().send(player, &l2_protocol_game::server::ActionFailed);
                None
            }
            SANTA_TRAINEE_ID => Some(
                if SAVING_SANTA {
                    "savingsanta.htm"
                } else {
                    "santa.htm"
                }
                .to_owned(),
            ),
            _ => None,
        }
    }

    fn on_aggro_range_enter(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        _player: ObjectId,
        _is_summon: bool,
    ) {
        // FIXME: Increase Thomas D. Turkey aggro rage.
        if npc.id == THOMAS_D_TURKEY_ID {
            // I guess you came to rescue Santa. But you picked the wrong person.
            say(ctx, npc, 8);
        }
    }
}

/// `npc.broadcastPacket(new SocialAction(npc.getObjectId(), actionId))`.
fn social_action(ctx: &ScriptCtx<'_>, npc: ScriptNpc, action_id: i32) {
    ctx.world().broadcast_packet(
        npc.object_id,
        &SocialAction {
            object_id: npc.object_id.0,
            action_id,
        },
    );
}

#[cfg(test)]
mod tests {
    use l2_world::{ItemData, ScriptRegistry};

    use super::*;
    use crate::events::long_time_event::TEST_CLOCK;
    use crate::testing::{Pages, ScriptTest, item, names_to};

    const NPCS: &str = r#"
        <npc id="31863" type="Folk" level="70" name="Santa Trainee"/>
        <npc id="13183" type="Monster" level="70" name="Thomas D. Turkey"/>
        <npc id="104" type="Folk" level="70" name="Holiday Santa"/>
        <npc id="105" type="Folk" level="70" name="Holiday Sled"/>
        <npc id="13007" type="Folk" level="70" name="Special Christmas Tree"/>"#;
    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="5556" type="EtcItem" name="Star Ornament"><set name="is_stackable" val="true"/></item>
        <item id="5557" type="EtcItem" name="Bead Ornament"><set name="is_stackable" val="true"/></item>
        <item id="5558" type="EtcItem" name="Fir Tree Branch"><set name="is_stackable" val="true"/></item>
        <item id="5559" type="EtcItem" name="Flower Pot"><set name="is_stackable" val="true"/></item>
        <item id="5560" type="EtcItem" name="Christmas Tree"><set name="is_stackable" val="true"/></item>
        <item id="5561" type="EtcItem" name="Special Christmas Tree"><set name="is_stackable" val="true"/></item>
        <item id="7836" type="EtcItem" name="Santa's Hat"/>
        <item id="20100" type="EtcItem" name="Gawibawibo Cap"/>
        <item id="20101" type="EtcItem" name="Present"><set name="is_stackable" val="true"/></item>
        <item id="20102" type="EtcItem" name="Jackpot Present"><set name="is_stackable" val="true"/></item>
        <item id="20107" type="EtcItem" name="Weapon Ticket"><set name="is_stackable" val="true"/></item>
        <item id="20108" type="EtcItem" name="Jackpot Weapon Ticket"><set name="is_stackable" val="true"/></item>
        <item id="14621" type="Weapon" name="Santa Weapon 1"/>
        <item id="14622" type="Weapon" name="Santa Weapon 2"/>"#;
    /// 2019-12-25 12:00 UTC, inside the event (21 12 2019 to 11 01 2020).
    const INSIDE: i64 = 1_577_275_200_000;
    /// Java `Inventory.ADENA_ID`.
    const ADENA_ID: i32 = 57;

    fn setup(now: Option<i64>) -> ScriptTest {
        TEST_CLOCK.set(now);
        let mut registry = ScriptRegistry::new();
        registry.add(SavingSanta);
        let pages = Pages::of(&SavingSanta)
            .plain(&["savingsanta.htm", "santa.htm", "savingsanta-nobuff.htm"])
            .page("savingsanta-weapon.htm", &["weapon_1", "weapon_2", "weapon_99"])
            .plain(&["savingsanta-noweapon.htm"])
            .npcs(&[SANTA_TRAINEE_ID]);
        ScriptTest::with_pages(registry, NPCS, ITEMS, pages)
    }

    fn event(t: &mut ScriptTest, player: i32, event: &str) -> Option<String> {
        let mut page = None;
        t.world.run_as_script("SavingSanta", |ctx| {
            let npc = ctx.script_npc(ctx.world().npc_ids()[0]);
            page = SavingSanta.on_event(ctx, event, npc, Some(ObjectId(player)));
        });
        page
    }

    #[test]
    fn inside_the_dates_the_timers_run_and_outside_nothing_does() {
        let mut inside = setup(Some(INSIDE));
        // The trainee has no spawn in the config (it is a spawn of the datapack); the timers do.
        assert!(inside.world.has_script("SavingSanta"));
        inside.wait_ms(1_100);
        // `ThomasQuest` ran: Thomas stands at his place and everybody was told.
        assert_eq!(inside.npcs_of(THOMAS_D_TURKEY_ID).len(), 1);

        let mut outside = setup(None);
        outside.wait_ms(1_100);
        assert!(outside.npcs_of(THOMAS_D_TURKEY_ID).is_empty());
        // The hooks do nothing either: the trainee gets the default page.
        outside.enter(1, 40, vec![]);
        let trainee = outside.spawn(SANTA_TRAINEE_ID, 20);
        outside.world.run_as_script("SavingSanta", |ctx| {
            assert_eq!(
                SavingSanta.on_first_talk(ctx, ctx.script_npc(trainee).unwrap(), ObjectId(1)),
                None
            );
        });
    }

    #[test]
    fn the_trainee_shows_the_saving_santa_page() {
        let mut t = setup(Some(INSIDE));
        t.enter(1, 40, vec![]);
        let trainee = t.spawn(SANTA_TRAINEE_ID, 20);
        t.world.run_as_script("SavingSanta", |ctx| {
            let npc = ctx.script_npc(trainee).unwrap();
            assert_eq!(
                SavingSanta.on_first_talk(ctx, npc, ObjectId(1)).as_deref(),
                Some("savingsanta.htm")
            );
        });
    }

    #[test]
    fn thomas_shows_nothing_to_talk_to() {
        let mut t = setup(Some(INSIDE));
        t.enter(1, 40, vec![]);
        let thomas = t.spawn(THOMAS_D_TURKEY_ID, 20);
        t.take();
        t.world.run_as_script("SavingSanta", |ctx| {
            let npc = ctx.script_npc(thomas).unwrap();
            assert_eq!(SavingSanta.on_first_talk(ctx, npc, ObjectId(1)), None);
        });
        assert!(names_to(&t.take(), 1).contains(&"ACTION_FAIL".to_owned()));
    }

    #[test]
    fn the_tree_page_lists_the_pieces_and_the_purchase_takes_them() {
        let mut t = setup(Some(INSIDE));
        let pieces: Vec<ItemData> = TREE_REQUIRED_ITEMS
            .iter()
            .enumerate()
            .map(|(i, (id, count))| item(1000 + i as i32, *id, *count))
            .collect();
        t.enter(1, 40, pieces);
        t.enter(2, 40, vec![item(2000, 5556, 4)]);
        t.spawn(SANTA_TRAINEE_ID, 20);

        let page = event(&mut t, 1, "Tree").unwrap();
        assert!(page.contains("Christmas Tree"));
        assert_eq!(page.matches("OK</font>").count(), 4);
        assert!(page.contains("bypass -h Script SavingSanta buyTree"));
        // Missing pieces: no button.
        let page = event(&mut t, 2, "Tree").unwrap();
        assert_eq!(page.matches("OK</font>").count(), 1);
        assert_eq!(page.matches("NO</font>").count(), 3);
        assert!(page.ends_with("You do not have enough items.</center></body></html>"));

        assert_eq!(event(&mut t, 1, "buyTree"), None);
        assert_eq!(t.count(1, X_MAS_TREE1), 1);
        assert_eq!(t.count(1, 5558), 0);
        // Without the pieces nothing is taken or given.
        assert_eq!(event(&mut t, 2, "buyTree"), None);
        assert_eq!(t.count(2, X_MAS_TREE1), 0);
        assert_eq!(t.count(2, 5556), 4);
    }

    #[test]
    fn ten_trees_make_a_special_tree_or_a_hat() {
        let mut t = setup(Some(INSIDE));
        t.enter(1, 40, vec![item(1000, X_MAS_TREE1, 25)]);
        t.spawn(SANTA_TRAINEE_ID, 20);
        let page = event(&mut t, 1, "SpecialTree").unwrap();
        // NOTE(java-quirk): the spacer row twice.
        assert_eq!(page.matches(PAGE_SPACE).count(), 2);
        assert!(page.contains("buySpecialTree"));
        event(&mut t, 1, "buySpecialTree");
        assert_eq!((t.count(1, X_MAS_TREE1), t.count(1, X_MAS_TREE2)), (15, 1));

        assert!(event(&mut t, 1, "SantaHat").unwrap().contains("buyHat"));
        event(&mut t, 1, "buyHat");
        assert_eq!((t.count(1, X_MAS_TREE1), t.count(1, SANTAS_HAT_ID)), (5, 1));
        // Five trees are not enough for the next one.
        assert!(
            event(&mut t, 1, "SantaHat")
                .unwrap()
                .contains("You do not have enough items.")
        );
        event(&mut t, 1, "buyHat");
        assert_eq!(t.count(1, SANTAS_HAT_ID), 1);
    }

    #[test]
    fn the_saving_santa_hat_costs_50000_adena() {
        let mut t = setup(Some(INSIDE));
        t.enter(1, 40, vec![item(1000, ADENA_ID, 60_000)]);
        t.enter(2, 40, vec![item(2000, ADENA_ID, 49_999)]);
        t.spawn(SANTA_TRAINEE_ID, 20);
        assert!(
            event(&mut t, 1, "SavingSantaHat")
                .unwrap()
                .contains("buySavingHat")
        );
        assert!(
            event(&mut t, 2, "SavingSantaHat")
                .unwrap()
                .contains("You do not have enough Adena.")
        );
        event(&mut t, 1, "buySavingHat");
        assert_eq!(
            (t.count(1, ADENA_ID), t.count(1, BR_XMAS_GAWIBAWIBO_CAP)),
            (10_000, 1)
        );
        event(&mut t, 2, "buySavingHat");
        assert_eq!(t.count(2, BR_XMAS_GAWIBAWIBO_CAP), 0);
    }

    #[test]
    fn a_weapon_ticket_buys_a_santa_weapon() {
        let mut t = setup(Some(INSIDE));
        t.enter(1, 40, vec![item(1000, BR_XMAS_WPN_TICKET_NORMAL, 2)]);
        t.enter(2, 40, vec![item(2000, BR_XMAS_WPN_TICKET_JACKPOT, 1)]);
        t.spawn(SANTA_TRAINEE_ID, 20);
        // A ticket of one kind only: the weapon page.
        assert_eq!(
            event(&mut t, 1, "getWeapon").as_deref(),
            Some("savingsanta-weapon.htm")
        );
        event(&mut t, 1, "weapon_1");
        assert_eq!((t.count(1, 14621), t.count(1, BR_XMAS_WPN_TICKET_NORMAL)), (1, 1));
        let enchant = t
            .world
            .player(ObjectId(1))
            .and_then(|p| {
                p.data
                    .items
                    .iter()
                    .find(|i| i.item_id == 14621)
                    .map(|i| i.enchant_level)
            })
            .unwrap();
        assert!((4..=16).contains(&enchant), "{enchant}");

        // The jackpot ticket gives +10 and the odd "incorrect item count" message.
        t.take();
        event(&mut t, 2, "weapon_2");
        assert_eq!(t.count(2, 14622), 1);
        assert!(names_to(&t.take(), 2).contains(&"SYSTEM_MESSAGE".to_owned()));
        let enchant = t
            .world
            .player(ObjectId(2))
            .and_then(|p| {
                p.data
                    .items
                    .iter()
                    .find(|i| i.item_id == 14622)
                    .map(|i| i.enchant_level)
            })
            .unwrap();
        assert_eq!(enchant, 10);

        // A number past the list ends the event with nothing.
        t.enter(3, 40, vec![item(3000, BR_XMAS_WPN_TICKET_NORMAL, 1)]);
        assert_eq!(event(&mut t, 3, "weapon_99"), None);
        assert_eq!(t.count(3, BR_XMAS_WPN_TICKET_NORMAL), 1);
    }

    #[test]
    fn both_tickets_close_the_weapon_page() {
        let mut t = setup(Some(INSIDE));
        t.enter(
            1,
            40,
            vec![
                item(1000, BR_XMAS_WPN_TICKET_NORMAL, 1),
                item(1001, BR_XMAS_WPN_TICKET_JACKPOT, 1),
            ],
        );
        t.spawn(SANTA_TRAINEE_ID, 20);
        assert_eq!(
            event(&mut t, 1, "getWeapon").as_deref(),
            Some("savingsanta-noweapon.htm")
        );
    }

    #[test]
    fn santa_spawn_without_a_timer_player_ends_in_the_java_exception() {
        let mut t = setup(Some(INSIDE));
        t.enter(1, 40, vec![]);
        t.spawn(SANTA_TRAINEE_ID, 20);
        t.world.run_as_script("SavingSanta", |ctx| {
            // The player must be in combat and above level 20; the combat flag is not set in the
            // test world, so the loop finds nobody and the timer is simply restarted.
            assert_eq!(SavingSanta.on_event(ctx, "SantaSpawn", None, None), None);
        });
        assert!(t.npcs_of(HOLIDAY_SANTA_ID).is_empty());
    }

    #[test]
    fn thomas_waits_for_the_skills_until_he_is_gone() {
        let mut t = setup(Some(INSIDE));
        t.enter(1, 40, vec![]);
        t.wait_ms(1_100);
        let thomas = t.npcs_of(THOMAS_D_TURKEY_ID)[0];
        assert!(!santa_is_free(&mut t));
        // `ThomasCast1` (15 s later): the players' skills count from now on.
        t.wait_ms(15_000);
        assert!(is_waiting(&mut t));
        // Thomas is despawned after 30 minutes (`addSpawn(.., 1800000)`). His quest timers stop
        // with him (`Npc.onDecay`), so the "Santa was not rescued" branch of `ThomasCast1/2` is
        // never reached and Java keeps waiting for the skills.
        t.take();
        t.wait_ms(1_800_000);
        assert!(!t.in_world(thomas));
        assert!(is_waiting(&mut t));
        assert!(!names_to(&t.take(), 1).contains(&"SYSTEM_MESSAGE".to_owned()));
    }

    fn is_waiting(t: &mut ScriptTest) -> bool {
        let mut waiting = true;
        t.world.run_as_script("SavingSanta", |ctx| {
            waiting = ctx.shared_state::<State>().is_waiting_for_player_skill;
        });
        waiting
    }

    fn santa_is_free(t: &mut ScriptTest) -> bool {
        let mut free = true;
        t.world.run_as_script("SavingSanta", |ctx| {
            free = ctx.shared_state::<State>().is_santa_free;
        });
        free
    }
}
