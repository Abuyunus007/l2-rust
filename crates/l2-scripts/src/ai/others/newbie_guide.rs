//! The Newbie Guides of the starting villages, Gludin and Gludio, and the Adventurers' Guide: the
//! beginner buffs, the coupons, the village teleports and the tutorial's guide missions.
// Java: dist/game/data/scripts/ai/others/NewbieGuide/NewbieGuide.java
//
// The six `eventGuide...` functions of Java (one `switch` per guide, every `case` a radar and a
// page) are the tables below, generated from the Java `switch`es case by case; `talkGuide` is the
// Java text with its repeated blocks (`switch (race)` radars, reward tiers, the memo bookkeeping)
// as small functions.

use l2_protocol_game::NpcStringId;
use l2_util::ObjectId;
use l2_world::script::{STARTED, player_class};
use l2_world::{Qs, Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// Suffix
const SUFFIX_FIGHTER_5_LEVEL: &str = "-f05.htm";
const SUFFIX_FIGHTER_10_LEVEL: &str = "-f10.htm";
const SUFFIX_FIGHTER_15_LEVEL: &str = "-f15.htm";
const SUFFIX_FIGHTER_20_LEVEL: &str = "-f20.htm";
const SUFFIX_MAGE_7_LEVEL: &str = "-m07.htm";
const SUFFIX_MAGE_14_LEVEL: &str = "-m14.htm";
const SUFFIX_MAGE_20_LEVEL: &str = "-m20.htm";

// Vars
const FIRST_COUPON_SIZE: i64 = 5;
const SECOND_COUPON_SIZE: i64 = 1;

// Newbie helpers
const NEWBIE_GUIDE_HUMAN: i32 = 30598;
const NEWBIE_GUIDE_ELF: i32 = 30599;
const NEWBIE_GUIDE_DARK_ELF: i32 = 30600;
const NEWBIE_GUIDE_DWARF: i32 = 30601;
const NEWBIE_GUIDE_ORC: i32 = 30602;
const NEWBIE_GUIDE_KAMAEL: i32 = 32135;
const NEWBIE_GUIDE_GLUDIN: i32 = 31076;
const NEWBIE_GUIDE_GLUDIO: i32 = 31077;
const ADVENTURERS_GUIDE: i32 = 32327;

const GUIDE_MISSION: i32 = 41;

// Item
const SOULSHOT_NO_GRADE_FOR_BEGINNERS: i32 = 5789;
const SPIRITSHOT_NO_GRADE_FOR_BEGINNERS: i32 = 5790;
const SCROLL_RECOVERY_NO_GRADE: i32 = 8594;

const APPRENTICE_ADVENTURERS_WEAPON_EXCHANGE_COUPON: i32 = 7832;
const ADVENTURERS_MAGIC_ACCESSORY_EXCHANGE_COUPON: i32 = 7833;

// Buffs
const WIND_WALK_FOR_BEGINNERS: SkillKey = SkillKey { id: 4322, level: 1 };
const SHIELD_FOR_BEGINNERS: SkillKey = SkillKey { id: 4323, level: 1 };
const BLESS_THE_BODY_FOR_BEGINNERS: SkillKey = SkillKey { id: 4324, level: 1 };
const VAMPIRIC_RAGE_FOR_BEGINNERS: SkillKey = SkillKey { id: 4325, level: 1 };
const REGENERATION_FOR_BEGINNERS: SkillKey = SkillKey { id: 4326, level: 1 };
const HASTE_FOR_BEGINNERS: SkillKey = SkillKey { id: 4327, level: 1 };
const BLESS_THE_SOUL_FOR_BEGINNERS: SkillKey = SkillKey { id: 4328, level: 1 };
const ACUMEN_FOR_BEGINNERS: SkillKey = SkillKey { id: 4329, level: 1 };
const CONCENTRATION_FOR_BEGINNERS: SkillKey = SkillKey { id: 4330, level: 1 };
const EMPOWER_FOR_BEGINNERS: SkillKey = SkillKey { id: 4331, level: 1 };
const LIFE_CUBIC_FOR_BEGINNERS: SkillKey = SkillKey { id: 4338, level: 1 };
const BLESSING_OF_PROTECTION: SkillKey = SkillKey { id: 5182, level: 1 };
const ADVENTURERS_HASTE: SkillKey = SkillKey { id: 5632, level: 1 };
const ADVENTURERS_MAGIC_BARRIER: SkillKey = SkillKey { id: 5637, level: 1 };

// Buylist
const WEAPON_MULTISELL: i32 = 305986001;
const ACCESORIES_MULTISELL: i32 = 305986002;

type Loc = (i32, i32, i32);

const TALKING_ISLAND_VILLAGE: Loc = (-84081, 243227, -3723);
const DARK_ELF_VILLAGE: Loc = (12111, 16686, -4582);
const DWARVEN_VILLAGE: Loc = (115632, -177996, -905);
const ELVEN_VILLAGE: Loc = (45475, 48359, -3060);
const ORC_VILLAGE: Loc = (-45032, -113598, -192);
const KAMAEL_VILLAGE: Loc = (-119697, 44532, 380);

/// Java `TELEPORT_MAP.get(npcId)`.
fn teleport_list(npc_id: i32) -> Option<[Loc; 5]> {
    match npc_id {
        NEWBIE_GUIDE_HUMAN => Some([DARK_ELF_VILLAGE, DWARVEN_VILLAGE, ELVEN_VILLAGE, ORC_VILLAGE, KAMAEL_VILLAGE]),
        NEWBIE_GUIDE_ELF => Some([DARK_ELF_VILLAGE, DWARVEN_VILLAGE, TALKING_ISLAND_VILLAGE, ORC_VILLAGE, KAMAEL_VILLAGE]),
        NEWBIE_GUIDE_DARK_ELF => Some([DWARVEN_VILLAGE, TALKING_ISLAND_VILLAGE, ELVEN_VILLAGE, ORC_VILLAGE, KAMAEL_VILLAGE]),
        NEWBIE_GUIDE_DWARF => Some([DARK_ELF_VILLAGE, TALKING_ISLAND_VILLAGE, ELVEN_VILLAGE, ORC_VILLAGE, KAMAEL_VILLAGE]),
        NEWBIE_GUIDE_ORC => Some([DARK_ELF_VILLAGE, DWARVEN_VILLAGE, TALKING_ISLAND_VILLAGE, ELVEN_VILLAGE, KAMAEL_VILLAGE]),
        NEWBIE_GUIDE_KAMAEL => Some([TALKING_ISLAND_VILLAGE, DARK_ELF_VILLAGE, ELVEN_VILLAGE, DWARVEN_VILLAGE, ORC_VILLAGE]),
        _ => None,
    }
}

type EventTable<const N: usize> = [(i32, Option<Loc>, &'static str); N];

/// Java `eventGuideHumanCnacelot`: (event, radar, page).
const HUMAN_CNACELOT: EventTable<45> = [
    (10, None, "30598-04.htm"),
    (11, None, "30598-04a.htm"),
    (12, None, "30598-04b.htm"),
    (13, None, "30598-04c.htm"),
    (14, None, "30598-04d.htm"),
    (15, None, "30598-04e.htm"),
    (16, None, "30598-04f.htm"),
    (17, None, "30598-04g.htm"),
    (18, None, "30598-04h.htm"),
    (19, None, "30598-04i.htm"),
    (31, Some((-84108, 244604, -3729)), "30598-05.htm"),
    (32, Some((-82236, 241573, -3728)), "30598-05.htm"),
    (33, Some((-82515, 241221, -3728)), "30598-05.htm"),
    (34, Some((-82319, 244709, -3727)), "30598-05.htm"),
    (35, Some((-82659, 244992, -3717)), "30598-05.htm"),
    (36, Some((-86114, 244682, -3727)), "30598-05.htm"),
    (37, Some((-86328, 244448, -3724)), "30598-05.htm"),
    (38, Some((-86322, 241215, -3727)), "30598-05.htm"),
    (39, Some((-85964, 240947, -3727)), "30598-05.htm"),
    (40, Some((-85026, 242689, -3729)), "30598-05.htm"),
    (41, Some((-83789, 240799, -3717)), "30598-05.htm"),
    (42, Some((-84204, 240403, -3717)), "30598-05.htm"),
    (43, Some((-86385, 243267, -3717)), "30598-05.htm"),
    (44, Some((-86733, 242918, -3717)), "30598-05.htm"),
    (45, Some((-84516, 245449, -3714)), "30598-05.htm"),
    (46, Some((-84729, 245001, -3726)), "30598-05.htm"),
    (47, Some((-84965, 245222, -3726)), "30598-05.htm"),
    (48, Some((-84981, 244764, -3726)), "30598-05.htm"),
    (49, Some((-85186, 245001, -3726)), "30598-05.htm"),
    (50, Some((-83326, 242964, -3718)), "30598-05.htm"),
    (51, Some((-83020, 242553, -3718)), "30598-05.htm"),
    (52, Some((-83175, 243065, -3718)), "30598-05.htm"),
    (53, Some((-82809, 242751, -3718)), "30598-05.htm"),
    (54, Some((-81895, 243917, -3721)), "30598-05.htm"),
    (55, Some((-81840, 243534, -3721)), "30598-05.htm"),
    (56, Some((-81512, 243424, -3720)), "30598-05.htm"),
    (57, Some((-84436, 242793, -3729)), "30598-05.htm"),
    (58, Some((-78939, 240305, -3443)), "30598-05.htm"),
    (59, Some((-85301, 244587, -3725)), "30598-05.htm"),
    (60, Some((-83163, 243560, -3728)), "30598-05.htm"),
    (61, Some((-97131, 258946, -3622)), "30598-05.htm"),
    (62, Some((-114685, 222291, -2925)), "30598-05.htm"),
    (63, Some((-84057, 242832, -3729)), "30598-05.htm"),
    (64, Some((-100332, 238019, -3573)), "30598-05.htm"),
    (65, Some((-82041, 242718, -3725)), "30598-05.htm"),
];

/// Java `eventGuideElfRoios`: (event, radar, page).
const ELF_ROIOS: EventTable<40> = [
    (10, None, "30599-04.htm"),
    (11, None, "30599-04a.htm"),
    (12, None, "30599-04b.htm"),
    (13, None, "30599-04c.htm"),
    (14, None, "30599-04d.htm"),
    (15, None, "30599-04e.htm"),
    (16, None, "30599-04f.htm"),
    (17, None, "30599-04g.htm"),
    (18, None, "30599-04h.htm"),
    (31, Some((46926, 51511, -2977)), "30599-05.htm"),
    (32, Some((44995, 51706, -2803)), "30599-05.htm"),
    (33, Some((45727, 51721, -2803)), "30599-05.htm"),
    (34, Some((42812, 51138, -2996)), "30599-05.htm"),
    (35, Some((45487, 46511, -2996)), "30599-05.htm"),
    (36, Some((47401, 51764, -2996)), "30599-05.htm"),
    (37, Some((42971, 51372, -2996)), "30599-05.htm"),
    (38, Some((47595, 51569, -2996)), "30599-05.htm"),
    (39, Some((45778, 46534, -2996)), "30599-05.htm"),
    (40, Some((44476, 47153, -2984)), "30599-05.htm"),
    (41, Some((42700, 50057, -2984)), "30599-05.htm"),
    (42, Some((42766, 50037, -2984)), "30599-05.htm"),
    (43, Some((44683, 46952, -2981)), "30599-05.htm"),
    (44, Some((44667, 46896, -2982)), "30599-05.htm"),
    (45, Some((45725, 52105, -2795)), "30599-05.htm"),
    (46, Some((44823, 52414, -2795)), "30599-05.htm"),
    (47, Some((45000, 52101, -2795)), "30599-05.htm"),
    (48, Some((45919, 52414, -2795)), "30599-05.htm"),
    (49, Some((44692, 52261, -2795)), "30599-05.htm"),
    (50, Some((47780, 49568, -2983)), "30599-05.htm"),
    (51, Some((47912, 50170, -2983)), "30599-05.htm"),
    (52, Some((47868, 50167, -2983)), "30599-05.htm"),
    (53, Some((28928, 74248, -3773)), "30599-05.htm"),
    (54, Some((43673, 49683, -3046)), "30599-05.htm"),
    (55, Some((45610, 49008, -3059)), "30599-05.htm"),
    (56, Some((50592, 54986, -3376)), "30599-05.htm"),
    (57, Some((42978, 49115, -2994)), "30599-05.htm"),
    (58, Some((46475, 50495, -3058)), "30599-05.htm"),
    (59, Some((45859, 50827, -3058)), "30599-05.htm"),
    (60, Some((51210, 82474, -3283)), "30599-05.htm"),
    (61, Some((49262, 53607, -3216)), "30599-05.htm"),
];

/// Java `eventGuideDelfFrankia`: (event, radar, page).
const DELF_FRANKIA: EventTable<44> = [
    (10, None, "30600-04.htm"),
    (11, None, "30600-04a.htm"),
    (12, None, "30600-04b.htm"),
    (13, None, "30600-04c.htm"),
    (14, None, "30600-04d.htm"),
    (15, None, "30600-04e.htm"),
    (16, None, "30600-04f.htm"),
    (17, None, "30600-04g.htm"),
    (18, None, "30600-04h.htm"),
    (31, Some((9670, 15537, -4574)), "30600-05.htm"),
    (32, Some((15120, 15656, -4376)), "30600-05.htm"),
    (33, Some((17306, 13592, -3724)), "30600-05.htm"),
    (34, Some((15272, 16310, -4377)), "30600-05.htm"),
    (35, Some((6449, 19619, -3694)), "30600-05.htm"),
    (36, Some((-15404, 71131, -3445)), "30600-05.htm"),
    (37, Some((7496, 17388, -4377)), "30600-05.htm"),
    (38, Some((17102, 13002, -3743)), "30600-05.htm"),
    (39, Some((6532, 19903, -3693)), "30600-05.htm"),
    (40, Some((-15648, 71405, -3451)), "30600-05.htm"),
    (41, Some((7644, 18048, -4377)), "30600-05.htm"),
    (42, Some((-1301, 75883, -3566)), "30600-05.htm"),
    (43, Some((-1152, 76125, -3566)), "30600-05.htm"),
    (44, Some((10580, 17574, -4554)), "30600-05.htm"),
    (45, Some((12009, 15704, -4554)), "30600-05.htm"),
    (46, Some((11951, 15661, -4554)), "30600-05.htm"),
    (47, Some((10761, 17970, -4554)), "30600-05.htm"),
    (48, Some((10823, 18013, -4554)), "30600-05.htm"),
    (49, Some((11283, 14226, -4242)), "30600-05.htm"),
    (50, Some((10447, 14620, -4242)), "30600-05.htm"),
    (51, Some((11258, 14431, -4242)), "30600-05.htm"),
    (52, Some((10344, 14445, -4242)), "30600-05.htm"),
    (53, Some((10315, 14293, -4242)), "30600-05.htm"),
    (54, Some((10775, 14190, -4242)), "30600-05.htm"),
    (55, Some((11235, 14078, -4242)), "30600-05.htm"),
    (56, Some((11012, 14128, -4242)), "30600-05.htm"),
    (57, Some((13380, 17430, -4542)), "30600-05.htm"),
    (58, Some((13464, 17751, -4541)), "30600-05.htm"),
    (59, Some((13763, 17501, -4542)), "30600-05.htm"),
    (60, Some((-44225, 79721, -3652)), "30600-05.htm"),
    (61, Some((-44015, 79683, -3652)), "30600-05.htm"),
    (62, Some((25856, 10832, -3724)), "30600-05.htm"),
    (63, Some((12328, 14947, -4574)), "30600-05.htm"),
    (64, Some((13081, 18444, -4573)), "30600-05.htm"),
    (65, Some((12311, 17470, -4574)), "30600-05.htm"),
];

/// Java `eventGuideDwarfGullin`: (event, radar, page).
const DWARF_GULLIN: EventTable<45> = [
    (10, None, "30601-04.htm"),
    (11, None, "30601-04a.htm"),
    (12, None, "30601-04b.htm"),
    (13, None, "30601-04c.htm"),
    (14, None, "30601-04d.htm"),
    (15, None, "30601-04e.htm"),
    (16, None, "30601-04f.htm"),
    (17, None, "30601-04g.htm"),
    (18, None, "30601-04h.htm"),
    (31, Some((115072, -178176, -906)), "30601-05.htm"),
    (32, Some((117847, -182339, -1537)), "30601-05.htm"),
    (33, Some((116617, -184308, -1569)), "30601-05.htm"),
    (34, Some((117826, -182576, -1537)), "30601-05.htm"),
    (35, Some((116378, -184308, -1571)), "30601-05.htm"),
    (36, Some((115183, -176728, -791)), "30601-05.htm"),
    (37, Some((114969, -176752, -790)), "30601-05.htm"),
    (38, Some((117366, -178725, -1118)), "30601-05.htm"),
    (39, Some((117378, -178914, -1120)), "30601-05.htm"),
    (40, Some((116226, -178529, -948)), "30601-05.htm"),
    (41, Some((116190, -178441, -948)), "30601-05.htm"),
    (42, Some((116016, -178615, -948)), "30601-05.htm"),
    (43, Some((116190, -178615, -948)), "30601-05.htm"),
    (44, Some((116103, -178407, -948)), "30601-05.htm"),
    (45, Some((116103, -178653, -948)), "30601-05.htm"),
    (46, Some((115468, -182446, -1434)), "30601-05.htm"),
    (47, Some((115315, -182155, -1444)), "30601-05.htm"),
    (48, Some((115271, -182692, -1445)), "30601-05.htm"),
    (49, Some((115900, -177316, -915)), "30601-05.htm"),
    (50, Some((116268, -177524, -914)), "30601-05.htm"),
    (51, Some((115741, -181645, -1344)), "30601-05.htm"),
    (52, Some((116192, -181072, -1344)), "30601-05.htm"),
    (53, Some((115205, -180024, -870)), "30601-05.htm"),
    (54, Some((114716, -180018, -871)), "30601-05.htm"),
    (55, Some((114832, -179520, -871)), "30601-05.htm"),
    (56, Some((115717, -183488, -1483)), "30601-05.htm"),
    (57, Some((115618, -183265, -1483)), "30601-05.htm"),
    (58, Some((114348, -178537, -813)), "30601-05.htm"),
    (59, Some((114990, -177294, -854)), "30601-05.htm"),
    (60, Some((114426, -178672, -812)), "30601-05.htm"),
    (61, Some((114409, -178415, -812)), "30601-05.htm"),
    (62, Some((117061, -181867, -1413)), "30601-05.htm"),
    (63, Some((116164, -184029, -1507)), "30601-05.htm"),
    (64, Some((115563, -182923, -1448)), "30601-05.htm"),
    (65, Some((112656, -174864, -611)), "30601-05.htm"),
    (66, Some((116852, -183595, -1566)), "30601-05.htm"),
];

/// Java `eventGuideOrcTanai`: (event, radar, page).
const ORC_TANAI: EventTable<36> = [
    (10, None, "30602-04.htm"),
    (11, None, "30602-04a.htm"),
    (12, None, "30602-04b.htm"),
    (13, None, "30602-04c.htm"),
    (14, None, "30602-04d.htm"),
    (15, None, "30602-04e.htm"),
    (16, None, "30602-04f.htm"),
    (17, None, "30602-04g.htm"),
    (18, None, "30602-04h.htm"),
    (19, None, "30602-04i.htm"),
    (31, Some((-45264, -112512, -235)), "30602-05.htm"),
    (32, Some((-46576, -117311, -242)), "30602-05.htm"),
    (33, Some((-47360, -113791, -237)), "30602-05.htm"),
    (34, Some((-47360, -113424, -235)), "30602-05.htm"),
    (35, Some((-45744, -117165, -236)), "30602-05.htm"),
    (36, Some((-46528, -109968, -250)), "30602-05.htm"),
    (37, Some((-45808, -110055, -255)), "30602-05.htm"),
    (38, Some((-45731, -113844, -237)), "30602-05.htm"),
    (39, Some((-45728, -113360, -237)), "30602-05.htm"),
    (40, Some((-45952, -114784, -199)), "30602-05.htm"),
    (41, Some((-45952, -114496, -199)), "30602-05.htm"),
    (42, Some((-45863, -112621, -200)), "30602-05.htm"),
    (43, Some((-45864, -112540, -199)), "30602-05.htm"),
    (44, Some((-43264, -112532, -220)), "30602-05.htm"),
    (45, Some((-43910, -115518, -194)), "30602-05.htm"),
    (46, Some((-43950, -115457, -194)), "30602-05.htm"),
    (47, Some((-44416, -111486, -222)), "30602-05.htm"),
    (48, Some((-43926, -111794, -222)), "30602-05.htm"),
    (49, Some((-43109, -113770, -221)), "30602-05.htm"),
    (50, Some((-43114, -113404, -221)), "30602-05.htm"),
    (51, Some((-46768, -113610, -3)), "30602-05.htm"),
    (52, Some((-46802, -114011, -112)), "30602-05.htm"),
    (53, Some((-46247, -113866, -21)), "30602-05.htm"),
    (54, Some((-46808, -113184, -112)), "30602-05.htm"),
    (55, Some((-45328, -114736, -237)), "30602-05.htm"),
    (56, Some((-44624, -111873, -238)), "30602-05.htm"),
];

/// Java `eventGuideKrenisk`: (event, radar, page).
const KAMAEL_KRENISK: EventTable<56> = [
    (10, None, "32135-04.htm"),
    (11, None, "32135-04a.htm"),
    (12, None, "32135-04b.htm"),
    (13, None, "32135-04c.htm"),
    (14, None, "32135-04d.htm"),
    (15, None, "32135-04e.htm"),
    (16, None, "32135-04f.htm"),
    (17, None, "32135-04g.htm"),
    (18, None, "32135-04h.htm"),
    (19, None, "32135-04i.htm"),
    (20, None, "32135-04j.htm"),
    (21, None, "32135-04k.htm"),
    (22, None, "32135-04l.htm"),
    (31, Some((-116879, 46591, 380)), "32135-05.htm"),
    (32, Some((-119378, 49242, 22)), "32135-05.htm"),
    (33, Some((-119774, 49245, 22)), "32135-05.htm"),
    (34, Some((-119830, 51860, -787)), "32135-05.htm"),
    (35, Some((-119362, 51862, -780)), "32135-05.htm"),
    (36, Some((-112872, 46850, 68)), "32135-05.htm"),
    (37, Some((-112352, 47392, 68)), "32135-05.htm"),
    (38, Some((-110544, 49040, -1124)), "32135-05.htm"),
    (39, Some((-110536, 45162, -1132)), "32135-05.htm"),
    (40, Some((-115888, 43568, 524)), "32135-05.htm"),
    (41, Some((-115486, 43567, 525)), "32135-05.htm"),
    (42, Some((-116920, 47792, 464)), "32135-05.htm"),
    (43, Some((-116749, 48077, 462)), "32135-05.htm"),
    (44, Some((-117153, 48075, 463)), "32135-05.htm"),
    (45, Some((-119104, 43280, 559)), "32135-05.htm"),
    (46, Some((-119104, 43152, 559)), "32135-05.htm"),
    (47, Some((-117056, 43168, 559)), "32135-05.htm"),
    (48, Some((-117060, 43296, 559)), "32135-05.htm"),
    (49, Some((-118192, 42384, 838)), "32135-05.htm"),
    (50, Some((-117968, 42384, 838)), "32135-05.htm"),
    (51, Some((-118132, 42788, 723)), "32135-05.htm"),
    (52, Some((-118028, 42788, 720)), "32135-05.htm"),
    (53, Some((-114802, 44821, 524)), "32135-05.htm"),
    (54, Some((-114975, 44658, 524)), "32135-05.htm"),
    (55, Some((-114801, 45031, 525)), "32135-05.htm"),
    (56, Some((-120432, 45296, 416)), "32135-05.htm"),
    (57, Some((-120706, 45079, 419)), "32135-05.htm"),
    (58, Some((-120356, 45293, 416)), "32135-05.htm"),
    (59, Some((-120604, 44960, 423)), "32135-05.htm"),
    (60, Some((-120294, 46013, 384)), "32135-05.htm"),
    (61, Some((-120157, 45813, 355)), "32135-05.htm"),
    (62, Some((-120158, 46221, 354)), "32135-05.htm"),
    (63, Some((-120400, 46921, 415)), "32135-05.htm"),
    (64, Some((-120407, 46755, 423)), "32135-05.htm"),
    (65, Some((-120442, 47125, 422)), "32135-05.htm"),
    (66, Some((-118720, 48062, 473)), "32135-05.htm"),
    (67, Some((-118918, 47956, 474)), "32135-05.htm"),
    (68, Some((-118527, 47955, 473)), "32135-05.htm"),
    (69, Some((-117605, 48079, 472)), "32135-05.htm"),
    (70, Some((-117824, 48080, 476)), "32135-05.htm"),
    (71, Some((-118030, 47930, 465)), "32135-05.htm"),
    (72, Some((-119221, 46981, 380)), "32135-05.htm"),
    (73, Some((-118080, 42835, 720)), "32135-05.htm"),
];

/// Java `eventGuide...(reply, qs)`: `clearRadar` + `addRadar` when the case has a radar, then the
/// page; `None` is Java's empty string.
fn guide_event<const N: usize>(
    ctx: &mut ScriptCtx<'_>,
    player: ObjectId,
    table: &EventTable<N>,
    event: i32,
) -> Option<String> {
    let &(_, radar, page) = table.iter().find(|(e, _, _)| *e == event)?;
    if let Some((x, y, z)) = radar {
        ctx.clear_radar(player);
        ctx.add_radar(player, x, y, z);
    }
    Some(page.to_owned())
}

/// The guide table of an NPC (Java's second `switch (npc.getId())` of `onEvent`).
fn guide_table_event(ctx: &mut ScriptCtx<'_>, npc_id: i32, player: ObjectId, reply: i32) -> Option<String> {
    match npc_id {
        NEWBIE_GUIDE_HUMAN => guide_event(ctx, player, &HUMAN_CNACELOT, reply),
        NEWBIE_GUIDE_ELF => guide_event(ctx, player, &ELF_ROIOS, reply),
        NEWBIE_GUIDE_DARK_ELF => guide_event(ctx, player, &DELF_FRANKIA, reply),
        NEWBIE_GUIDE_DWARF => guide_event(ctx, player, &DWARF_GULLIN, reply),
        NEWBIE_GUIDE_ORC => guide_event(ctx, player, &ORC_TANAI, reply),
        NEWBIE_GUIDE_KAMAEL => guide_event(ctx, player, &KAMAEL_KRENISK, reply),
        _ => None,
    }
}

// ---------------------------------------------------------------------- talkGuide helpers

/// Java `if (!haveNRMemo(qs, GUIDE_MISSION)) { setNRMemo(qs, GUIDE_MISSION); setNRMemoState(qs, GUIDE_MISSION, 0); }`.
fn memo_init(ctx: &mut ScriptCtx<'_>, qs: Qs) {
    if !ctx.have_nr_memo(qs, GUIDE_MISSION) {
        ctx.set_nr_memo(qs, GUIDE_MISSION);
        ctx.set_nr_memo_state(qs, GUIDE_MISSION, 0);
    }
}

/// Java `if (!haveNRMemo(..)) { setNRMemo(..); setNRMemoState(.., n); } else { setNRMemoState(.., get + n); }`.
fn memo_add(ctx: &mut ScriptCtx<'_>, qs: Qs, n: i32) {
    if !ctx.have_nr_memo(qs, GUIDE_MISSION) {
        ctx.set_nr_memo(qs, GUIDE_MISSION);
        ctx.set_nr_memo_state(qs, GUIDE_MISSION, n);
    } else {
        let state = ctx.get_nr_memo_state(qs, GUIDE_MISSION);
        ctx.set_nr_memo_state(qs, GUIDE_MISSION, state + n);
    }
}

/// Java `giveAdena(talker, adena, true); addExpAndSp(player, exp, sp);`.
fn reward(ctx: &mut ScriptCtx<'_>, player: ObjectId, adena: i64, exp: i64, sp: i32) {
    ctx.give_adena(player, adena, true);
    ctx.add_exp_and_sp(player, exp, sp);
}

/// The reward of the levels 1 to 5 (first mission).
fn reward_low(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    let level = ctx.player_level(player);
    if level >= 5 {
        reward(ctx, player, 695, 3154, 127);
    } else if level >= 4 {
        reward(ctx, player, 1041, 4870, 195);
    } else if level >= 3 {
        reward(ctx, player, 1186, 5675, 227);
    } else {
        reward(ctx, player, 1240, 5970, 239);
    }
}

/// The reward of the levels 6 to 9.
fn reward_6_to_9(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    let level = ctx.player_level(player);
    if level >= 9 {
        reward(ctx, player, 5563, 16851, 711);
    } else if level >= 8 {
        reward(ctx, player, 9290, 28806, 1207);
    } else if level >= 7 {
        reward(ctx, player, 11567, 36942, 1541);
    } else {
        reward(ctx, player, 12928, 42191, 1753);
    }
}

/// The reward of the levels 10 to 14.
fn reward_10_to_14(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    let level = ctx.player_level(player);
    if level >= 14 {
        reward(ctx, player, 13002, 62876, 2891);
    } else if level >= 13 {
        reward(ctx, player, 23468, 113137, 5161);
    } else if level >= 12 {
        reward(ctx, player, 31752, 152653, 6914);
    } else if level >= 11 {
        reward(ctx, player, 38180, 183128, 8242);
    } else {
        reward(ctx, player, 43054, 206101, 9227);
    }
}

/// The reward of the levels 15 to 17.
fn reward_15_to_17(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    let level = ctx.player_level(player);
    if level >= 17 {
        reward(ctx, player, 22996, 113712, 5518);
    } else if level >= 16 {
        reward(ctx, player, 10018, 208133, 42237);
    } else {
        reward(ctx, player, 13648, 285670, 58155);
    }
}

/// Java `switch (npc.getRace())`: the radar of the NPC's race and the page `newbie-guide-<prefix>a..f`
/// (human, elf, dark elf, orc, dwarf, kamael); no page for any other race.
fn npc_race_radar(ctx: &mut ScriptCtx<'_>, player: ObjectId, npc: ScriptNpc, table: [Loc; 6], prefix: &str) -> Option<String> {
    let race = usize::try_from(ctx.world().npc_race_ordinal(npc.object_id)).ok()?;
    let (x, y, z) = *table.get(race)?;
    ctx.add_radar(player, x, y, z);
    Some(format!("newbie-guide-{prefix}{}.htm", ['a', 'b', 'c', 'd', 'e', 'f'][race]))
}

/// Table A of Java: the village of the first lessons (`01x` and `07x`).
const RADAR_A: [Loc; 6] = [
    (-84436, 242793, -3729),
    (42978, 49115, 2994),
    (25790, 10844, -3727),
    (-47360, -113791, -237),
    (112656, -174864, -611),
    (-119378, 49242, 22),
];
/// Table C of Java (`04x` and `09x`).
const RADAR_C: [Loc; 6] = [
    (-82236, 241573, -3728),
    (42812, 51138, -2996),
    (7644, 18048, -4377),
    (-46802, -114011, -112),
    (116103, -178407, -948),
    (-119378, 49242, 22),
];
/// Table D of Java (`11x`).
const RADAR_D: [Loc; 6] = [
    (-84057, 242832, -3729),
    (45859, 50827, -3058),
    (11258, 14431, -4242),
    (-45863, -112621, -200),
    (116268, -177524, -914),
    (-125872, 38208, 1251),
];

/// Java `switch (talker.getRace())` with the mage splits of the humans and the dark elves
/// (`05x` / `10x`). `orc` and `kamael` are the two places where the first and the second copy of
/// the Java `switch` differ.
fn talker_race_radar(ctx: &mut ScriptCtx<'_>, player: ObjectId, prefix: &str, orc: Loc, kamael: Loc) -> Option<String> {
    let mage = ctx.is_mage_class(player);
    let (loc, letter) = match ctx.player_race(player) {
        0 if !mage => ((-71384, 258304, -3109), 'a'),
        0 => ((-91008, 248016, -3568), 'b'),
        1 => ((47595, 51569, -2996), 'c'),
        2 if !mage => ((10580, 17574, -4554), 'd'),
        2 => ((10775, 14190, -4242), 'e'),
        3 => (orc, 'f'),
        4 => ((115717, -183488, -1483), 'g'),
        5 => (kamael, 'h'),
        _ => return None,
    };
    ctx.add_radar(player, loc.0, loc.1, loc.2);
    Some(format!("newbie-guide-{prefix}{letter}.htm"))
}

/// Java `ai.others.NewbieGuide.NewbieGuide`.
pub struct NewbieGuide;

impl Script for NewbieGuide {
    fn name(&self) -> &'static str {
        "NewbieGuide"
    }

    fn path(&self) -> &'static str {
        "ai/others/NewbieGuide"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        let newbie_list = [
            NEWBIE_GUIDE_HUMAN,
            NEWBIE_GUIDE_ELF,
            NEWBIE_GUIDE_DARK_ELF,
            NEWBIE_GUIDE_DWARF,
            NEWBIE_GUIDE_ORC,
            NEWBIE_GUIDE_KAMAEL,
            NEWBIE_GUIDE_GLUDIN,
            NEWBIE_GUIDE_GLUDIO,
            ADVENTURERS_GUIDE,
        ];

        s.add_start_npc(&newbie_list);
        s.add_first_talk_id(&newbie_list);
        s.add_talk_id(&newbie_list);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        talker: Option<ObjectId>,
    ) -> Option<String> {
        if event.ends_with(".htm") {
            return Some(event.to_owned());
        }
        let (npc, talker) = (npc?, talker?);

        if event.starts_with("teleport") {
            let tel: Vec<&str> = event.split('_').collect();
            if tel.len() != 2 {
                return teleport_request(ctx, talker, npc, -1);
            }

            // NOTE(java-quirk): `Integer.parseInt` throws for a bad number; nothing happens here.
            return teleport_request(ctx, talker, npc, tel[1].parse().ok()?);
        }

        let mut htmltext: Option<String> = None;
        let qs = ctx.get_quest_state(talker, true)?;
        // `Integer.parseInt(event.split(";")[0])` and `[1]`: a bad event throws.
        let mut parts = event.split(';');
        let ask: i32 = parts.next()?.parse().ok()?;
        let reply: i32 = parts.next()?.parse().ok()?;
        match ask {
            -7 => match reply {
                1 => {
                    if ctx.player_race(talker) == 5 {
                        if ctx.player_race(talker) != ctx.world().npc_race_ordinal(npc.object_id) {
                            htmltext = Some("32135-003.htm".to_owned());
                        } else if ctx.player_level(talker) > 20 || ctx.player_class_level(talker) != 0 {
                            // (the race is Kamael here: the `getRace() != KAMAEL` of Java is false)
                            htmltext = Some("32135-002.htm".to_owned());
                        } else if ctx.player_class_id(talker) == player_class::MALE_SOLDIER {
                            let level = ctx.player_level(talker);
                            htmltext = Some(
                                if level <= 5 {
                                    "32135-kmf05.htm"
                                } else if level <= 10 {
                                    "32135-kmf10.htm"
                                } else if level <= 15 {
                                    "32135-kmf15.htm"
                                } else {
                                    "32135-kmf20.htm"
                                }
                                .to_owned(),
                            );
                        } else if ctx.player_class_id(talker) == player_class::FEMALE_SOLDIER {
                            let level = ctx.player_level(talker);
                            htmltext = Some(
                                if level <= 5 {
                                    "32135-kff05.htm"
                                } else if level <= 10 {
                                    "32135-kff10.htm"
                                } else if level <= 15 {
                                    "32135-kff15.htm"
                                } else {
                                    "32135-kff20.htm"
                                }
                                .to_owned(),
                            );
                        }
                    } else if ctx.player_race(talker) != ctx.world().npc_race_ordinal(npc.object_id) {
                        return Some(format!("{}-003.htm", npc.id));
                    } else if ctx.player_level(talker) > 20 || ctx.player_class_level(talker) != 0 {
                        return Some(format!("{}-002.htm", npc.id));
                    } else if !ctx.is_mage_class(talker) {
                        let level = ctx.player_level(talker);
                        htmltext = Some(format!(
                            "{}{}",
                            npc.id,
                            if level <= 5 {
                                SUFFIX_FIGHTER_5_LEVEL
                            } else if level <= 10 {
                                SUFFIX_FIGHTER_10_LEVEL
                            } else if level <= 15 {
                                SUFFIX_FIGHTER_15_LEVEL
                            } else {
                                SUFFIX_FIGHTER_20_LEVEL
                            }
                        ));
                    } else {
                        let level = ctx.player_level(talker);
                        htmltext = Some(format!(
                            "{}{}",
                            npc.id,
                            if level <= 7 {
                                SUFFIX_MAGE_7_LEVEL
                            } else if level <= 14 {
                                SUFFIX_MAGE_14_LEVEL
                            } else {
                                SUFFIX_MAGE_20_LEVEL
                            }
                        ));
                    }
                }
                2 => {
                    let level = ctx.player_level(talker);
                    if level <= 75 {
                        let class_level = ctx.player_class_level(talker);
                        let mage = ctx.is_mage_class(talker);
                        if level < 6 {
                            htmltext = Some("buffs-low-level.htm".to_owned());
                        } else if !mage && class_level < 3 {
                            ctx.cast_skill(npc, talker, WIND_WALK_FOR_BEGINNERS);
                            ctx.cast_skill(npc, talker, WIND_WALK_FOR_BEGINNERS);
                            ctx.cast_skill(npc, talker, SHIELD_FOR_BEGINNERS);
                            ctx.cast_skill(npc, talker, ADVENTURERS_MAGIC_BARRIER);
                            ctx.cast_skill(npc, talker, BLESS_THE_BODY_FOR_BEGINNERS);
                            ctx.cast_skill(npc, talker, VAMPIRIC_RAGE_FOR_BEGINNERS);
                            ctx.cast_skill(npc, talker, REGENERATION_FOR_BEGINNERS);
                            if (6..=39).contains(&level) {
                                ctx.cast_skill(npc, talker, HASTE_FOR_BEGINNERS);
                            }

                            if (40..=75).contains(&level) {
                                ctx.cast_skill(npc, talker, ADVENTURERS_HASTE);
                            }

                            if (16..=34).contains(&level) {
                                ctx.world_mut().do_simultaneous_cast(talker, LIFE_CUBIC_FOR_BEGINNERS);
                            }
                        } else if mage && class_level < 3 {
                            ctx.cast_skill(npc, talker, WIND_WALK_FOR_BEGINNERS);
                            ctx.cast_skill(npc, talker, SHIELD_FOR_BEGINNERS);
                            ctx.cast_skill(npc, talker, ADVENTURERS_MAGIC_BARRIER);
                            ctx.cast_skill(npc, talker, BLESS_THE_SOUL_FOR_BEGINNERS);
                            ctx.cast_skill(npc, talker, ACUMEN_FOR_BEGINNERS);
                            ctx.cast_skill(npc, talker, CONCENTRATION_FOR_BEGINNERS);
                            ctx.cast_skill(npc, talker, EMPOWER_FOR_BEGINNERS);
                            if (16..=34).contains(&level) {
                                ctx.world_mut().do_simultaneous_cast(talker, LIFE_CUBIC_FOR_BEGINNERS);
                            }
                        }
                    } else {
                        htmltext = Some("buffs-big-level.htm".to_owned());
                    }
                }
                3 => {
                    if ctx.player_level(talker) <= 39 && ctx.player_class_level(talker) < 3 {
                        ctx.cast_skill(npc, talker, BLESSING_OF_PROTECTION);
                    } else {
                        htmltext = Some("pk-protection-002.htm".to_owned());
                    }
                }
                4 => {
                    let summon = ctx.summon_of(talker).filter(|&s| !ctx.world().is_pet(s));
                    if let Some(summon) = summon {
                        let level = ctx.player_level(talker);
                        if !(6..=75).contains(&level) {
                            htmltext = Some("buffs-big-level.htm".to_owned());
                        } else {
                            ctx.cast_skill(npc, summon, WIND_WALK_FOR_BEGINNERS);
                            ctx.cast_skill(npc, summon, SHIELD_FOR_BEGINNERS);
                            ctx.cast_skill(npc, summon, ADVENTURERS_MAGIC_BARRIER);
                            ctx.cast_skill(npc, summon, BLESS_THE_BODY_FOR_BEGINNERS);
                            ctx.cast_skill(npc, summon, VAMPIRIC_RAGE_FOR_BEGINNERS);
                            ctx.cast_skill(npc, summon, REGENERATION_FOR_BEGINNERS);
                            ctx.cast_skill(npc, summon, BLESS_THE_SOUL_FOR_BEGINNERS);
                            ctx.cast_skill(npc, summon, ACUMEN_FOR_BEGINNERS);
                            ctx.cast_skill(npc, summon, CONCENTRATION_FOR_BEGINNERS);
                            ctx.cast_skill(npc, summon, EMPOWER_FOR_BEGINNERS);
                            if (6..=39).contains(&level) {
                                ctx.cast_skill(npc, summon, HASTE_FOR_BEGINNERS);
                            }

                            if (40..=75).contains(&level) {
                                ctx.cast_skill(npc, summon, ADVENTURERS_HASTE);
                            }
                        }
                    } else {
                        htmltext = Some("buffs-no-pet.htm".to_owned());
                    }
                }
                _ => {}
            },
            -1000 => match reply {
                1 => {
                    if ctx.player_level(talker) > 5 {
                        if ctx.player_level(talker) < 20 && ctx.player_class_level(talker) == 0 {
                            if ctx.get_one_time_quest_flag(talker, 207) == 0 {
                                ctx.give_items(talker, APPRENTICE_ADVENTURERS_WEAPON_EXCHANGE_COUPON, FIRST_COUPON_SIZE);
                                ctx.set_one_time_quest_flag(talker, 207, 1);
                                htmltext = Some("newbie-guide-002.htm".to_owned());
                                let state = ctx.get_nr_memo_state(qs, GUIDE_MISSION);
                                ctx.set_nr_memo_state(qs, GUIDE_MISSION, state + 100);
                                ctx.show_on_screen_npc_string(
                                    talker,
                                    NpcStringId::AcquisitionOfWeaponExchangeCouponForBeginnersCompleteNGoSpeakWithTheNewbieGuide,
                                    2,
                                    5000,
                                    &[""],
                                );

                                // Needed for retrieving NewbieGuide quest after playerEnter.
                                qs.set_state(ctx, STARTED);
                            } else {
                                htmltext = Some("newbie-guide-004.htm".to_owned());
                            }
                        } else {
                            htmltext = Some("newbie-guide-003.htm".to_owned());
                        }
                    } else {
                        htmltext = Some("newbie-guide-003.htm".to_owned());
                    }
                }
                2 => {
                    if ctx.player_class_level(talker) == 1 {
                        if ctx.player_level(talker) < 40 {
                            if ctx.get_one_time_quest_flag(talker, 208) == 0 {
                                ctx.give_items(talker, ADVENTURERS_MAGIC_ACCESSORY_EXCHANGE_COUPON, SECOND_COUPON_SIZE);
                                ctx.set_one_time_quest_flag(talker, 208, 1);
                                htmltext = Some("newbie-guide-011.htm".to_owned());
                            } else {
                                htmltext = Some("newbie-guide-013.htm".to_owned());
                            }
                        } else {
                            htmltext = Some("newbie-guide-012.htm".to_owned());
                        }
                    } else {
                        htmltext = Some("newbie-guide-012.htm".to_owned());
                    }
                }
                _ => {}
            },
            -303 => match reply {
                528 => {
                    if ctx.player_level(talker) > 5 {
                        if ctx.player_level(talker) < 20 && ctx.player_class_level(talker) == 0 {
                            ctx.world_mut().multisell_separate_and_send(
                                WEAPON_MULTISELL,
                                talker,
                                Some(npc.object_id),
                                false,
                                1.0,
                                1.0,
                            );
                        } else {
                            htmltext = Some("newbie-guide-005.htm".to_owned());
                        }
                    } else {
                        htmltext = Some("newbie-guide-005.htm".to_owned());
                    }
                }
                529 => {
                    if ctx.player_level(talker) > 5 {
                        if ctx.player_level(talker) < 40 && ctx.player_class_level(talker) == 1 {
                            ctx.world_mut().multisell_separate_and_send(
                                ACCESORIES_MULTISELL,
                                talker,
                                Some(npc.object_id),
                                false,
                                1.0,
                                1.0,
                            );
                        } else {
                            htmltext = Some("newbie-guide-014.htm".to_owned());
                        }
                    } else {
                        htmltext = Some("newbie-guide-014.htm".to_owned());
                    }
                }
                _ => {}
            },
            _ => {}
        }

        if let Some(answer) = guide_table_event(ctx, npc.id, talker, reply) {
            return Some(answer);
        }

        htmltext
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.quest_state_of("Q00255_Tutorial", player, false);
        if let Some(qs) = qs {
            match npc.id {
                ADVENTURERS_GUIDE => return Some("32327.htm".to_owned()),
                NEWBIE_GUIDE_GLUDIN => return Some("31076.htm".to_owned()),
                NEWBIE_GUIDE_GLUDIO => return Some("31077.htm".to_owned()),
                _ => {}
            }

            return talk_guide(ctx, player, npc, qs);
        }

        // Java `super.onFirstTalk`.
        None
    }
}

/// Java `teleportRequest(talker, npc, teleportId)`.
fn teleport_request(ctx: &mut ScriptCtx<'_>, talker: ObjectId, npc: ScriptNpc, teleport_id: i32) -> Option<String> {
    let mut htmltext = None;
    let transformation = ctx.world().transformation_id(talker);
    if ctx.player_level(talker) >= 20 {
        htmltext = Some("teleport-big-level.htm".to_owned());
    } else if transformation == 111 || transformation == 112 || transformation == 124 {
        htmltext = Some("frog-teleport.htm".to_owned());
    } else if !(0..=5).contains(&teleport_id) {
        htmltext = Some(format!("{}-teleport.htm", npc.id));
    } else if let Some(list) = teleport_list(npc.id)
        && let Some(&(x, y, z)) = usize::try_from(teleport_id).ok().and_then(|i| list.get(i))
    {
        ctx.teleport_player(talker, x, y, z, 0, false);
    }

    htmltext
}

/// Java `talkGuide(talker, npc, tutorialQS)`.
fn talk_guide(ctx: &mut ScriptCtx<'_>, talker: ObjectId, npc: ScriptNpc, tutorial_qs: Qs) -> Option<String> {
    let mut hmltext: Option<String> = None;
    let qs = ctx.get_quest_state(talker, true)?;
    if tutorial_qs.get_memo_state_ex(ctx, 1) < 5 && ctx.get_one_time_quest_flag(talker, GUIDE_MISSION) == 0 {
        if !ctx.is_mage_class(talker) {
            ctx.play_sound_named(talker, "tutorial_voice_026");
            ctx.give_items(talker, SOULSHOT_NO_GRADE_FOR_BEGINNERS, 200);
            ctx.give_items(talker, SCROLL_RECOVERY_NO_GRADE, 2);
            tutorial_qs.set_memo_state_ex(ctx, 1, 5);
            if ctx.player_level(talker) <= 1 {
                ctx.add_exp_and_sp(talker, 68, 50);
            } else {
                ctx.add_exp_and_sp(talker, 0, 50);
            }
        }

        if ctx.is_mage_class(talker) {
            if ctx.player_class_id(talker) == player_class::ORC_MAGE {
                ctx.play_sound_named(talker, "tutorial_voice_026");
                ctx.give_items(talker, SOULSHOT_NO_GRADE_FOR_BEGINNERS, 200);
            } else {
                ctx.play_sound_named(talker, "tutorial_voice_027");
                ctx.give_items(talker, SPIRITSHOT_NO_GRADE_FOR_BEGINNERS, 100);
            }

            ctx.give_items(talker, SCROLL_RECOVERY_NO_GRADE, 2);
            tutorial_qs.set_memo_state_ex(ctx, 1, 5);
            if ctx.player_level(talker) <= 1 {
                ctx.add_exp_and_sp(talker, 68, 50);
            } else {
                ctx.add_exp_and_sp(talker, 0, 50);
            }
        }

        let level = ctx.player_level(talker);
        if level < 6 {
            if ctx.get_nr_memo_state(qs, GUIDE_MISSION) % 10 == 1 {
                reward_low(ctx, talker);
                memo_add(ctx, qs, 10);
                hmltext = Some("newbie-guide-02.htm".to_owned());
            } else {
                hmltext = npc_race_radar(ctx, talker, npc, RADAR_A, "01");
                memo_init(ctx, qs);
            }
        } else if level < 10 {
            let memo = ctx.get_nr_memo_state(qs, GUIDE_MISSION);
            // NOTE(java-quirk): the second test divides `% 10000` by 100, not by 1000.
            if (memo % 1000) / 100 == 1 && (memo % 10000) / 100 == 1 {
                // NOTE(java-quirk): the Orc radar of this copy has a positive x, and the Kamael
                // radar is the Dwarf's.
                hmltext = talker_race_radar(ctx, talker, "05", (46808, -113184, -112), (115717, -183488, -1483));
                reward_6_to_9(ctx, talker);
                memo_add(ctx, qs, 10000);
            } else if (memo % 1000) / 100 == 1 && (memo % 10000) / 100 != 1 {
                hmltext = npc_race_radar(ctx, talker, npc, RADAR_C, "04");
                memo_init(ctx, qs);
            } else {
                memo_init(ctx, qs);
                hmltext = Some("newbie-guide-03.htm".to_owned());
            }
        } else {
            ctx.set_one_time_quest_flag(talker, GUIDE_MISSION, 1);
            memo_init(ctx, qs);
            hmltext = Some("newbie-guide-06.htm".to_owned());
        }
    } else if tutorial_qs.get_memo_state_ex(ctx, 1) >= 5 && ctx.get_one_time_quest_flag(talker, GUIDE_MISSION) == 0 {
        let level = ctx.player_level(talker);
        if level < 6 {
            if ctx.get_nr_memo_state(qs, GUIDE_MISSION) % 10 == 1 {
                reward_low(ctx, talker);
                memo_add(ctx, qs, 10);
                hmltext = Some("newbie-guide-08.htm".to_owned());
            } else {
                hmltext = npc_race_radar(ctx, talker, npc, RADAR_A, "07");
                memo_init(ctx, qs);
            }
        } else if level < 10 {
            let memo = ctx.get_nr_memo_state(qs, GUIDE_MISSION);
            if (memo % 100000) / 10000 == 1 {
                hmltext = Some("newbie-guide-09g.htm".to_owned());
            } else if (memo % 1000) / 100 == 1 && (memo % 10000) / 1000 == 1 && (memo % 100000) / 10000 != 1 {
                hmltext = talker_race_radar(ctx, talker, "10", (-46808, -113184, -112), (-118080, 42835, 720));
                reward_6_to_9(ctx, talker);
                memo_add(ctx, qs, 10000);
            } else if (memo % 1000) / 100 == 1 && (memo % 10000) / 1000 != 1 {
                hmltext = npc_race_radar(ctx, talker, npc, RADAR_C, "09");
                memo_init(ctx, qs);
            } else {
                memo_init(ctx, qs);
                hmltext = Some("newbie-guide-08.htm".to_owned());
            }
        } else if level < 15 {
            let memo = ctx.get_nr_memo_state(qs, GUIDE_MISSION);
            if (memo % 1000000) / 100000 == 1 && (memo % 10000000) / 1000000 == 1 {
                hmltext = Some("newbie-guide-15.htm".to_owned());
            } else if (memo % 1000000) / 100000 == 1 && (memo % 10000000) / 1000000 != 1 {
                hmltext = npc_race_radar(ctx, talker, npc, RADAR_D, "11");
                reward_10_to_14(ctx, talker);
                memo_add(ctx, qs, 1000000);
            } else if (memo % 1000000) / 100000 != 1 {
                hmltext = talker_race_radar(ctx, talker, "10", (-46808, -113184, -112), (-118080, 42835, 720));
                memo_init(ctx, qs);
            }
        } else if level < 18 {
            let memo = ctx.get_nr_memo_state(qs, GUIDE_MISSION);
            if (memo % 100000000) / 10000000 == 1 && (memo % 1000000000) / 100000000 == 1 {
                ctx.set_one_time_quest_flag(talker, GUIDE_MISSION, 1);
                hmltext = Some("newbie-guide-13.htm".to_owned());
            } else if (memo % 100000000) / 10000000 == 1 && (memo % 1000000000) / 100000000 != 1 {
                reward_15_to_17(ctx, talker);
                memo_add(ctx, qs, 100000000);
                ctx.set_one_time_quest_flag(talker, GUIDE_MISSION, 1);
                hmltext = Some("newbie-guide-12.htm".to_owned());
            } else if (memo % 100000000) / 10000000 != 1 {
                hmltext = npc_race_radar(ctx, talker, npc, RADAR_D, "11");
            }

            memo_init(ctx, qs);
        } else if ctx.player_class_level(talker) == 1 {
            ctx.set_one_time_quest_flag(talker, GUIDE_MISSION, 1);
            memo_init(ctx, qs);
            hmltext = Some("newbie-guide-13.htm".to_owned());
        } else {
            ctx.set_one_time_quest_flag(talker, GUIDE_MISSION, 1);
            memo_init(ctx, qs);
            hmltext = Some("newbie-guide-14.htm".to_owned());
        }
    }

    hmltext
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;
    use l2_world::script::race;

    use super::*;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"<npc id="30598" type="Folk" level="10" name="Guide Cnacelot"><race>HUMAN</race></npc>
        <npc id="32135" type="Folk" level="10" name="Guide Krenisk"><race>KAMAEL</race></npc>"#;
    const ITEMS: &str = r#"<item id="7832" type="EtcItem" name="Weapon Coupon"><set name="is_stackable" val="true"/></item>
        <item id="5789" type="EtcItem" name="Soulshot"><set name="is_stackable" val="true"/></item>
        <item id="8594" type="EtcItem" name="Scroll"><set name="is_stackable" val="true"/></item>
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>"#;

    /// Stand-in for the tutorial quest the guide asks about.
    struct Tutorial;

    impl Script for Tutorial {
        fn name(&self) -> &'static str {
            "Q00255_Tutorial"
        }
        fn path(&self) -> &'static str {
            "quests/Q00255_Tutorial"
        }
        fn id(&self) -> i32 {
            255
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    /// Stand-in for a quest the one-time flags of the guide read (Java ids 207, 208 and 41).
    struct Flag(i32, &'static str);

    impl Script for Flag {
        fn name(&self) -> &'static str {
            self.1
        }
        fn path(&self) -> &'static str {
            "quests"
        }
        fn id(&self) -> i32 {
            self.0
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn setup() -> (ScriptTest, ScriptNpc) {
        let mut registry = ScriptRegistry::new();
        registry.add(NewbieGuide);
        registry.add(Tutorial);
        registry.add(Flag(207, "Q00207_Flag"));
        registry.add(Flag(208, "Q00208_Flag"));
        registry.add(Flag(41, "Q00041_Flag"));
        let mut t = ScriptTest::new(registry, NPCS, ITEMS, &[]);
        let guide = t.spawn(30598, 20);
        t.enter_as(1, 3, race::HUMAN, player_class::FIGHTER, vec![]);
        let npc = ScriptNpc { object_id: guide, id: 30598 };
        (t, npc)
    }

    fn event(t: &mut ScriptTest, npc: ScriptNpc, name: &str) -> Option<String> {
        let mut out = None;
        t.world.run_as_script("NewbieGuide", |ctx| {
            out = NewbieGuide.on_event(ctx, name, Some(npc), Some(ObjectId(1)));
        });
        out
    }

    #[test]
    fn a_page_event_is_echoed_and_the_guide_tables_answer_their_cases() {
        let (mut t, npc) = setup();
        assert_eq!(event(&mut t, npc, "30598-001.htm").as_deref(), Some("30598-001.htm"));
        assert_eq!(event(&mut t, npc, "0;10").as_deref(), Some("30598-04.htm"));
        assert_eq!(event(&mut t, npc, "0;31").as_deref(), Some("30598-05.htm"));
        assert_eq!(event(&mut t, npc, "0;9999"), None);
        // A bad event throws in Java (`parseInt`); nothing happens.
        assert_eq!(event(&mut t, npc, "x;y"), None);
    }

    fn event_as(t: &mut ScriptTest, player: i32, npc: ScriptNpc, name: &str) -> Option<String> {
        let mut out = None;
        t.world.run_as_script("NewbieGuide", |ctx| {
            out = NewbieGuide.on_event(ctx, name, Some(npc), Some(ObjectId(player)));
        });
        out
    }

    #[test]
    fn the_first_lesson_page_follows_race_level_and_class() {
        let (mut t, npc) = setup();
        t.enter_as(2, 12, race::HUMAN, player_class::FIGHTER, vec![]);
        t.enter_as(3, 21, race::HUMAN, player_class::FIGHTER, vec![]);
        t.enter_as(4, 8, race::HUMAN, player_class::MAGE, vec![]);
        t.enter_as(5, 8, race::ELF, player_class::ELVEN_FIGHTER, vec![]);
        assert_eq!(event_as(&mut t, 1, npc, "-7;1").as_deref(), Some("30598-f05.htm"));
        assert_eq!(event_as(&mut t, 2, npc, "-7;1").as_deref(), Some("30598-f15.htm"));
        assert_eq!(event_as(&mut t, 3, npc, "-7;1").as_deref(), Some("30598-002.htm"));
        assert_eq!(event_as(&mut t, 4, npc, "-7;1").as_deref(), Some("30598-m14.htm"));
        assert_eq!(event_as(&mut t, 5, npc, "-7;1").as_deref(), Some("30598-003.htm"));
    }

    #[test]
    fn the_weapon_coupons_are_given_once() {
        let (mut t, npc) = setup();
        t.enter_as(2, 8, race::HUMAN, player_class::FIGHTER, vec![]);
        t.take_writes();
        assert_eq!(event_as(&mut t, 2, npc, "-1000;1").as_deref(), Some("newbie-guide-002.htm"));
        assert_eq!(t.count(2, 7832), 5);
        assert_eq!(event_as(&mut t, 2, npc, "-1000;1").as_deref(), Some("newbie-guide-004.htm"));
        assert_eq!(t.count(2, 7832), 5);
        // A level 3 player gets the refusal page.
        assert_eq!(event_as(&mut t, 1, npc, "-1000;1").as_deref(), Some("newbie-guide-003.htm"));
        assert!(!t.take_writes().is_empty());
    }

    #[test]
    fn the_first_talk_gives_the_starter_pack_once_and_sends_to_the_first_lesson() {
        let (mut t, npc) = setup();
        let mut page = Some(String::new());
        t.world.run_as_script("NewbieGuide", |ctx| {
            // Without the tutorial's state there is no page (Java `super.onFirstTalk`).
            page = NewbieGuide.on_first_talk(ctx, npc, ObjectId(1));
        });
        assert_eq!(page, None);
        t.world.run_as_script("NewbieGuide", |ctx| {
            // The tutorial is started (the memo slots of a state that is not started read 0).
            let tutorial = ctx.quest_state_of("Q00255_Tutorial", ObjectId(1), true).unwrap();
            tutorial.set_state(ctx, STARTED);
            page = NewbieGuide.on_first_talk(ctx, npc, ObjectId(1));
        });
        // A level 3 human fighter: soulshots and scrolls, then the Talking Island lesson page.
        assert_eq!(page.as_deref(), Some("newbie-guide-01a.htm"));
        assert_eq!(t.count(1, 5789), 200);
        assert_eq!(t.count(1, 8594), 2);
        // The memo of the tutorial says the pack was given: a second talk gives nothing.
        t.world.run_as_script("NewbieGuide", |ctx| {
            page = NewbieGuide.on_first_talk(ctx, npc, ObjectId(1));
        });
        // (the second lesson text of the same village: the section for memo state 5 and more)
        assert_eq!(page.as_deref(), Some("newbie-guide-07a.htm"));
        assert_eq!(t.count(1, 5789), 200);
    }

    #[test]
    fn the_table_sizes_are_javas() {
        assert_eq!(HUMAN_CNACELOT.len(), 45);
        assert_eq!(KAMAEL_KRENISK.len(), 56);
        assert_eq!(teleport_list(NEWBIE_GUIDE_KAMAEL).unwrap()[0], TALKING_ISLAND_VILLAGE);
    }

    #[test]
    fn teleports_refuse_the_experienced_and_send_the_rest() {
        let (mut t, npc) = setup();
        assert_eq!(event(&mut t, npc, "teleport_9").as_deref(), Some("30598-teleport.htm"));
        assert_eq!(event(&mut t, npc, "teleport_0"), None);
        let at = t.world.position_of(ObjectId(1)).unwrap();
        assert_eq!((at.0, at.1), (DARK_ELF_VILLAGE.0, DARK_ELF_VILLAGE.1));
        t.enter_as(2, 20, race::HUMAN, player_class::FIGHTER, vec![]);
        assert_eq!(event_as(&mut t, 2, npc, "teleport_0").as_deref(), Some("teleport-big-level.htm"));
    }
}
