//! The Energy Seeds of the Seeds of Destruction and Annihilation: their spawns, the collection
//! skill and the doors, exits and zone checks of the Seed of Destruction.
// Java: dist/game/data/scripts/ai/areas/Gracia/AI/EnergySeeds.java
//
// NOT PORTED: the `soi*Spawn()` / `soi*Stop()` functions for the Seed of Infinity. Java's
// `SeedOfInfinityManager` calls them only in comments (`// EnergySeeds.SoiSeedSpawn();`), and
// `isSeedActive(INFINITY)` is false, so nothing ever runs them. Their tables are not copied.

use std::collections::HashMap;

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{ActionFailed, SystemMessage};
use l2_util::ObjectId;
use l2_world::{QuestSound, Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

use crate::ai::areas::gracia::seeds::{sod_state, soi_current_stage, soi_opened_time};
use crate::ai::areas::gracia::util::players_in_zone;

/// Java `Q00692_HowtoOpposeEvil.class.getSimpleName()`.
const HOW_TO_OPPOSE_EVIL: &str = "Q00692_HowtoOpposeEvil";

const HOW_TO_OPPOSE_EVIL_CHANCE: i32 = 60;
const RATE: i32 = 1;
const RESPAWN: i64 = 480_000;
const RANDOM_RESPAWN_OFFSET: i32 = 180_000;

const TEMPORARY_TELEPORTER: i32 = 32602;
const SEED_IDS: [i32; 6] = [18678, 18679, 18680, 18681, 18682, 18683];

const ANNIHILATION_SUPRISE_MOB_IDS: [&[i32]; 3] = [
    &[22746, 22747, 22748, 22749],
    &[22754, 22755, 22756],
    &[22760, 22761, 22762],
];

const SEED_OF_DESTRUCTION_DOORS: [i32; 29] = [
    12240003, 12240004, 12240005, 12240006, 12240007, 12240008, 12240009, 12240010, 12240011, 12240012,
    12240013, 12240014, 12240015, 12240016, 12240017, 12240018, 12240019, 12240020, 12240021, 12240022,
    12240023, 12240024, 12240025, 12240026, 12240027, 12240028, 12240029, 12240030, 12240031,
];
const SOI_EXIT_POINT: (i32, i32, i32) = (-183285, 205996, -12896);
const SOD_EXIT_POINT: (i32, i32, i32) = (-248717, 250260, 4337);

const SOD_ZONE: i32 = 60009;
const SOI_ZONE: i32 = 60010;

/// Java `GraciaSeeds`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GraciaSeeds {
    #[allow(dead_code)] // `isSeedActive(INFINITY)` is false and no spawn names it
    Infinity,
    Destruction,
    AnnihilationBistakon,
    AnnihilationReptilikon,
    AnnihilationCokrakon,
}

/// One `ESSpawn`: `(spawn id, seed, location, the NPC ids one of which spawns there)`.
type EsSpawn = (i32, GraciaSeeds, (i32, i32, i32), &'static [i32]);

/// Java `SPAWNS`.
#[rustfmt::skip]
const SPAWNS: &[EsSpawn] = &[
    (1, GraciaSeeds::Destruction, (-245790, 220320, -12104), &[TEMPORARY_TELEPORTER]),
    (2, GraciaSeeds::Destruction, (-249770, 207300, -11952), &[TEMPORARY_TELEPORTER]),
    (3, GraciaSeeds::Destruction, (-248360, 219272, -12448), &[18678, 18679, 18680]),
    (4, GraciaSeeds::Destruction, (-249448, 219256, -12448), &[18678, 18679, 18680]),
    (5, GraciaSeeds::Destruction, (-249432, 220872, -12448), &[18678, 18679, 18680]),
    (6, GraciaSeeds::Destruction, (-248360, 220888, -12448), &[18678, 18679, 18680]),
    (7, GraciaSeeds::Destruction, (-250088, 219256, -12448), &[18681, 18682]),
    (8, GraciaSeeds::Destruction, (-250600, 219272, -12448), &[18681, 18682]),
    (9, GraciaSeeds::Destruction, (-250584, 220904, -12448), &[18681, 18682]),
    (10, GraciaSeeds::Destruction, (-250072, 220888, -12448), &[18681, 18682]),
    (11, GraciaSeeds::Destruction, (-253096, 217704, -12296), &[18683, 18678]),
    (12, GraciaSeeds::Destruction, (-253112, 217048, -12288), &[18683, 18678]),
    (13, GraciaSeeds::Destruction, (-251448, 217032, -12288), &[18683, 18678]),
    (14, GraciaSeeds::Destruction, (-251416, 217672, -12296), &[18683, 18678]),
    (15, GraciaSeeds::Destruction, (-251416, 217672, -12296), &[18679, 18680]),
    (16, GraciaSeeds::Destruction, (-251416, 217016, -12280), &[18679, 18680]),
    (17, GraciaSeeds::Destruction, (-249752, 217016, -12280), &[18679, 18680]),
    (18, GraciaSeeds::Destruction, (-249736, 217688, -12296), &[18679, 18680]),
    (19, GraciaSeeds::Destruction, (-252472, 215208, -12120), &[18681, 18682]),
    (20, GraciaSeeds::Destruction, (-252552, 216760, -12248), &[18681, 18682]),
    (21, GraciaSeeds::Destruction, (-253160, 216744, -12248), &[18681, 18682]),
    (22, GraciaSeeds::Destruction, (-253128, 215160, -12096), &[18681, 18682]),
    (23, GraciaSeeds::Destruction, (-250392, 215208, -12120), &[18683, 18678]),
    (24, GraciaSeeds::Destruction, (-250264, 216744, -12248), &[18683, 18678]),
    (25, GraciaSeeds::Destruction, (-249720, 216744, -12248), &[18683, 18678]),
    (26, GraciaSeeds::Destruction, (-249752, 215128, -12096), &[18683, 18678]),
    (27, GraciaSeeds::Destruction, (-250280, 216760, -12248), &[18679, 18680, 18681]),
    (28, GraciaSeeds::Destruction, (-250344, 216152, -12248), &[18679, 18680, 18681]),
    (29, GraciaSeeds::Destruction, (-252504, 216152, -12248), &[18679, 18680, 18681]),
    (30, GraciaSeeds::Destruction, (-252520, 216792, -12248), &[18679, 18680, 18681]),
    (31, GraciaSeeds::Destruction, (-242520, 217272, -12384), &[18681, 18682, 18683]),
    (32, GraciaSeeds::Destruction, (-241432, 217288, -12384), &[18681, 18682, 18683]),
    (33, GraciaSeeds::Destruction, (-241432, 218936, -12384), &[18681, 18682, 18683]),
    (34, GraciaSeeds::Destruction, (-242536, 218936, -12384), &[18681, 18682, 18683]),
    (35, GraciaSeeds::Destruction, (-240808, 217272, -12384), &[18678, 18679]),
    (36, GraciaSeeds::Destruction, (-240280, 217272, -12384), &[18678, 18679]),
    (37, GraciaSeeds::Destruction, (-240280, 218952, -12384), &[18678, 18679]),
    (38, GraciaSeeds::Destruction, (-240792, 218936, -12384), &[18678, 18679]),
    (39, GraciaSeeds::Destruction, (-239576, 217240, -12640), &[18680, 18681, 18682]),
    (40, GraciaSeeds::Destruction, (-239560, 216168, -12640), &[18680, 18681, 18682]),
    (41, GraciaSeeds::Destruction, (-237896, 216152, -12640), &[18680, 18681, 18682]),
    (42, GraciaSeeds::Destruction, (-237912, 217256, -12640), &[18680, 18681, 18682]),
    (43, GraciaSeeds::Destruction, (-237896, 215528, -12640), &[18683, 18678]),
    (44, GraciaSeeds::Destruction, (-239560, 215528, -12640), &[18683, 18678]),
    (45, GraciaSeeds::Destruction, (-239560, 214984, -12640), &[18683, 18678]),
    (46, GraciaSeeds::Destruction, (-237896, 215000, -12640), &[18683, 18678]),
    (47, GraciaSeeds::Destruction, (-237896, 213640, -12768), &[18678, 18679, 18680]),
    (48, GraciaSeeds::Destruction, (-239560, 213640, -12768), &[18678, 18679, 18680]),
    (49, GraciaSeeds::Destruction, (-239544, 212552, -12768), &[18678, 18679, 18680]),
    (50, GraciaSeeds::Destruction, (-237912, 212552, -12768), &[18678, 18679, 18680]),
    (51, GraciaSeeds::Destruction, (-237912, 211912, -12768), &[18681, 18682]),
    (52, GraciaSeeds::Destruction, (-237912, 211400, -12768), &[18681, 18682]),
    (53, GraciaSeeds::Destruction, (-239560, 211400, -12768), &[18681, 18682]),
    (54, GraciaSeeds::Destruction, (-239560, 211912, -12768), &[18681, 18682]),
    (55, GraciaSeeds::Destruction, (-241960, 214536, -12512), &[18683, 18678, 18679]),
    (56, GraciaSeeds::Destruction, (-241976, 213448, -12512), &[18683, 18678, 18679]),
    (57, GraciaSeeds::Destruction, (-243624, 213448, -12512), &[18683, 18678, 18679]),
    (58, GraciaSeeds::Destruction, (-243624, 214520, -12512), &[18683, 18678, 18679]),
    (59, GraciaSeeds::Destruction, (-241976, 212808, -12504), &[18680, 18681]),
    (60, GraciaSeeds::Destruction, (-241960, 212280, -12504), &[18680, 18681]),
    (61, GraciaSeeds::Destruction, (-243624, 212264, -12504), &[18680, 18681]),
    (62, GraciaSeeds::Destruction, (-243624, 212792, -12504), &[18680, 18681]),
    (63, GraciaSeeds::Destruction, (-243640, 210920, -12640), &[18682, 18683, 18678]),
    (64, GraciaSeeds::Destruction, (-243624, 209832, -12640), &[18682, 18683, 18678]),
    (65, GraciaSeeds::Destruction, (-241976, 209832, -12640), &[18682, 18683, 18678]),
    (66, GraciaSeeds::Destruction, (-241976, 210920, -12640), &[18682, 18683, 18678]),
    (67, GraciaSeeds::Destruction, (-241976, 209192, -12640), &[18679, 18680]),
    (68, GraciaSeeds::Destruction, (-241976, 208664, -12640), &[18679, 18680]),
    (69, GraciaSeeds::Destruction, (-243624, 208664, -12640), &[18679, 18680]),
    (70, GraciaSeeds::Destruction, (-243624, 209192, -12640), &[18679, 18680]),
    (71, GraciaSeeds::Destruction, (-241256, 208664, -12896), &[18681, 18682, 18683]),
    (72, GraciaSeeds::Destruction, (-240168, 208648, -12896), &[18681, 18682, 18683]),
    (73, GraciaSeeds::Destruction, (-240168, 207000, -12896), &[18681, 18682, 18683]),
    (74, GraciaSeeds::Destruction, (-241256, 207000, -12896), &[18681, 18682, 18683]),
    (75, GraciaSeeds::Destruction, (-239528, 208648, -12896), &[18678, 18679]),
    (76, GraciaSeeds::Destruction, (-238984, 208664, -12896), &[18678, 18679]),
    (77, GraciaSeeds::Destruction, (-239000, 207000, -12896), &[18678, 18679]),
    (78, GraciaSeeds::Destruction, (-239512, 207000, -12896), &[18678, 18679]),
    (79, GraciaSeeds::Destruction, (-245064, 213144, -12384), &[18680, 18681, 18682]),
    (80, GraciaSeeds::Destruction, (-245064, 212072, -12384), &[18680, 18681, 18682]),
    (81, GraciaSeeds::Destruction, (-246696, 212072, -12384), &[18680, 18681, 18682]),
    (82, GraciaSeeds::Destruction, (-246696, 213160, -12384), &[18680, 18681, 18682]),
    (83, GraciaSeeds::Destruction, (-245064, 211416, -12384), &[18683, 18678]),
    (84, GraciaSeeds::Destruction, (-245048, 210904, -12384), &[18683, 18678]),
    (85, GraciaSeeds::Destruction, (-246712, 210888, -12384), &[18683, 18678]),
    (86, GraciaSeeds::Destruction, (-246712, 211416, -12384), &[18683, 18678]),
    (87, GraciaSeeds::Destruction, (-245048, 209544, -12512), &[18679, 18680, 18681]),
    (88, GraciaSeeds::Destruction, (-245064, 208456, -12512), &[18679, 18680, 18681]),
    (89, GraciaSeeds::Destruction, (-246696, 208456, -12512), &[18679, 18680, 18681]),
    (90, GraciaSeeds::Destruction, (-246712, 209544, -12512), &[18679, 18680, 18681]),
    (91, GraciaSeeds::Destruction, (-245048, 207816, -12512), &[18682, 18683]),
    (92, GraciaSeeds::Destruction, (-245048, 207288, -12512), &[18682, 18683]),
    (93, GraciaSeeds::Destruction, (-246696, 207304, -12512), &[18682, 18683]),
    (94, GraciaSeeds::Destruction, (-246712, 207816, -12512), &[18682, 18683]),
    (95, GraciaSeeds::Destruction, (-244328, 207272, -12768), &[18678, 18679, 18680]),
    (96, GraciaSeeds::Destruction, (-243256, 207256, -12768), &[18678, 18679, 18680]),
    (97, GraciaSeeds::Destruction, (-243256, 205624, -12768), &[18678, 18679, 18680]),
    (98, GraciaSeeds::Destruction, (-244328, 205608, -12768), &[18678, 18679, 18680]),
    (99, GraciaSeeds::Destruction, (-242616, 207272, -12768), &[18681, 18682]),
    (100, GraciaSeeds::Destruction, (-242104, 207272, -12768), &[18681, 18682]),
    (101, GraciaSeeds::Destruction, (-242088, 205624, -12768), &[18681, 18682]),
    (102, GraciaSeeds::Destruction, (-242600, 205608, -12768), &[18681, 18682]),
    (103, GraciaSeeds::AnnihilationBistakon, (-184519, 183007, -10456), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (104, GraciaSeeds::AnnihilationBistakon, (-184873, 181445, -10488), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (105, GraciaSeeds::AnnihilationBistakon, (-184009, 180962, -10488), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (106, GraciaSeeds::AnnihilationBistakon, (-185321, 181641, -10448), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (107, GraciaSeeds::AnnihilationBistakon, (-184035, 182775, -10512), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (108, GraciaSeeds::AnnihilationBistakon, (-185433, 181935, -10424), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (109, GraciaSeeds::AnnihilationBistakon, (-183309, 183007, -10560), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (110, GraciaSeeds::AnnihilationBistakon, (-184929, 181886, -10488), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (111, GraciaSeeds::AnnihilationBistakon, (-184009, 180392, -10424), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (112, GraciaSeeds::AnnihilationBistakon, (-183793, 183239, -10488), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (113, GraciaSeeds::AnnihilationBistakon, (-184245, 180848, -10464), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (114, GraciaSeeds::AnnihilationBistakon, (-182704, 183761, -10528), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (115, GraciaSeeds::AnnihilationBistakon, (-184705, 181886, -10504), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (116, GraciaSeeds::AnnihilationBistakon, (-184304, 181076, -10488), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (117, GraciaSeeds::AnnihilationBistakon, (-183596, 180430, -10424), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (118, GraciaSeeds::AnnihilationBistakon, (-184422, 181038, -10480), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (119, GraciaSeeds::AnnihilationBistakon, (-184929, 181543, -10496), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (120, GraciaSeeds::AnnihilationBistakon, (-184398, 182891, -10472), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (121, GraciaSeeds::AnnihilationBistakon, (-177606, 182848, -10584), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (122, GraciaSeeds::AnnihilationBistakon, (-178104, 183224, -10560), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (123, GraciaSeeds::AnnihilationBistakon, (-177274, 182284, -10600), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (124, GraciaSeeds::AnnihilationBistakon, (-177772, 183224, -10560), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (125, GraciaSeeds::AnnihilationBistakon, (-181532, 180364, -10504), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (126, GraciaSeeds::AnnihilationBistakon, (-181802, 180276, -10496), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (127, GraciaSeeds::AnnihilationBistakon, (-178429, 180444, -10512), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (128, GraciaSeeds::AnnihilationBistakon, (-177606, 182190, -10600), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (129, GraciaSeeds::AnnihilationBistakon, (-177357, 181908, -10576), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (130, GraciaSeeds::AnnihilationBistakon, (-178747, 179534, -10408), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (131, GraciaSeeds::AnnihilationBistakon, (-178429, 179534, -10392), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (132, GraciaSeeds::AnnihilationBistakon, (-178853, 180094, -10472), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (133, GraciaSeeds::AnnihilationBistakon, (-181937, 179660, -10416), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (134, GraciaSeeds::AnnihilationBistakon, (-180992, 179572, -10416), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (135, GraciaSeeds::AnnihilationBistakon, (-185552, 179252, -10368), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (136, GraciaSeeds::AnnihilationBistakon, (-184572, 178913, -10400), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (137, GraciaSeeds::AnnihilationBistakon, (-184768, 178348, -10312), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (138, GraciaSeeds::AnnihilationBistakon, (-184572, 178574, -10352), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (139, GraciaSeeds::AnnihilationBistakon, (-185062, 178913, -10384), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (140, GraciaSeeds::AnnihilationBistakon, (-181397, 179484, -10416), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (141, GraciaSeeds::AnnihilationBistakon, (-181667, 179044, -10408), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (142, GraciaSeeds::AnnihilationBistakon, (-185258, 177896, -10240), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (143, GraciaSeeds::AnnihilationBistakon, (-183506, 176570, -10280), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (144, GraciaSeeds::AnnihilationBistakon, (-183719, 176804, -10240), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (145, GraciaSeeds::AnnihilationBistakon, (-183648, 177116, -10240), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (146, GraciaSeeds::AnnihilationBistakon, (-183932, 176492, -10240), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (147, GraciaSeeds::AnnihilationBistakon, (-183861, 176570, -10240), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (148, GraciaSeeds::AnnihilationBistakon, (-183790, 175946, -10240), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (149, GraciaSeeds::AnnihilationBistakon, (-178641, 179604, -10416), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (150, GraciaSeeds::AnnihilationBistakon, (-178959, 179814, -10432), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (151, GraciaSeeds::AnnihilationBistakon, (-176367, 178456, -10376), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (152, GraciaSeeds::AnnihilationBistakon, (-175845, 177172, -10264), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (153, GraciaSeeds::AnnihilationBistakon, (-175323, 177600, -10248), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (154, GraciaSeeds::AnnihilationBistakon, (-174975, 177172, -10216), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (155, GraciaSeeds::AnnihilationBistakon, (-176019, 178242, -10352), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (156, GraciaSeeds::AnnihilationBistakon, (-174801, 178456, -10264), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (157, GraciaSeeds::AnnihilationReptilikon, (-185648, 183384, -15680), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (158, GraciaSeeds::AnnihilationReptilikon, (-186740, 180908, -15528), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (159, GraciaSeeds::AnnihilationReptilikon, (-185297, 184658, -15680), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (160, GraciaSeeds::AnnihilationReptilikon, (-185697, 181601, -15488), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (161, GraciaSeeds::AnnihilationReptilikon, (-186684, 182744, -15536), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (162, GraciaSeeds::AnnihilationReptilikon, (-184908, 183384, -15616), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (163, GraciaSeeds::AnnihilationReptilikon, (-184994, 185572, -15784), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (164, GraciaSeeds::AnnihilationReptilikon, (-185796, 182616, -15608), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (165, GraciaSeeds::AnnihilationReptilikon, (-184970, 184385, -15648), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (166, GraciaSeeds::AnnihilationReptilikon, (-185995, 180809, -15512), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (167, GraciaSeeds::AnnihilationReptilikon, (-185352, 182872, -15632), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (168, GraciaSeeds::AnnihilationReptilikon, (-185624, 184294, -15680), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (169, GraciaSeeds::AnnihilationReptilikon, (-184486, 185774, -15816), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (170, GraciaSeeds::AnnihilationReptilikon, (-186496, 184112, -15680), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (171, GraciaSeeds::AnnihilationReptilikon, (-184232, 185976, -15816), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (172, GraciaSeeds::AnnihilationReptilikon, (-184994, 185673, -15792), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (173, GraciaSeeds::AnnihilationReptilikon, (-185733, 184203, -15680), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (174, GraciaSeeds::AnnihilationReptilikon, (-185079, 184294, -15680), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (175, GraciaSeeds::AnnihilationReptilikon, (-184803, 180710, -15528), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (176, GraciaSeeds::AnnihilationReptilikon, (-186293, 180413, -15528), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (177, GraciaSeeds::AnnihilationReptilikon, (-185352, 182936, -15632), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (178, GraciaSeeds::AnnihilationReptilikon, (-184356, 180611, -15496), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (179, GraciaSeeds::AnnihilationReptilikon, (-185375, 186784, -15816), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (180, GraciaSeeds::AnnihilationReptilikon, (-184867, 186784, -15816), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (181, GraciaSeeds::AnnihilationReptilikon, (-180553, 180454, -15152), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (182, GraciaSeeds::AnnihilationReptilikon, (-180422, 180454, -15152), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (183, GraciaSeeds::AnnihilationReptilikon, (-181863, 181138, -15120), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (184, GraciaSeeds::AnnihilationReptilikon, (-181732, 180454, -15152), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (185, GraciaSeeds::AnnihilationReptilikon, (-180684, 180397, -15152), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (186, GraciaSeeds::AnnihilationReptilikon, (-182256, 180682, -15112), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (187, GraciaSeeds::AnnihilationReptilikon, (-185492, 179492, -15392), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (188, GraciaSeeds::AnnihilationReptilikon, (-185894, 178538, -15336), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (189, GraciaSeeds::AnnihilationReptilikon, (-186028, 178856, -15336), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (190, GraciaSeeds::AnnihilationReptilikon, (-185224, 179068, -15336), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (191, GraciaSeeds::AnnihilationReptilikon, (-185492, 178538, -15336), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (192, GraciaSeeds::AnnihilationReptilikon, (-185894, 178538, -15336), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (193, GraciaSeeds::AnnihilationReptilikon, (-180619, 178855, -15152), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (194, GraciaSeeds::AnnihilationReptilikon, (-180255, 177892, -15152), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (195, GraciaSeeds::AnnihilationReptilikon, (-185804, 176472, -15336), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (196, GraciaSeeds::AnnihilationReptilikon, (-184580, 176370, -15320), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (197, GraciaSeeds::AnnihilationReptilikon, (-184308, 176166, -15320), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (198, GraciaSeeds::AnnihilationReptilikon, (-183764, 177186, -15304), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (199, GraciaSeeds::AnnihilationReptilikon, (-180801, 177571, -15144), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (200, GraciaSeeds::AnnihilationReptilikon, (-184716, 176064, -15320), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (201, GraciaSeeds::AnnihilationReptilikon, (-184444, 175452, -15296), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (202, GraciaSeeds::AnnihilationReptilikon, (-180164, 177464, -15152), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (203, GraciaSeeds::AnnihilationReptilikon, (-180164, 178213, -15152), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (204, GraciaSeeds::AnnihilationReptilikon, (-179982, 178320, -15152), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (205, GraciaSeeds::AnnihilationReptilikon, (-176925, 177757, -15824), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (206, GraciaSeeds::AnnihilationReptilikon, (-176164, 179282, -15720), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (207, GraciaSeeds::AnnihilationReptilikon, (-175692, 177613, -15800), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (208, GraciaSeeds::AnnihilationReptilikon, (-175418, 178117, -15824), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (209, GraciaSeeds::AnnihilationReptilikon, (-176103, 177829, -15824), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (210, GraciaSeeds::AnnihilationReptilikon, (-175966, 177325, -15792), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (211, GraciaSeeds::AnnihilationReptilikon, (-174778, 179732, -15664), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (212, GraciaSeeds::AnnihilationReptilikon, (-175692, 178261, -15824), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (213, GraciaSeeds::AnnihilationReptilikon, (-176038, 179192, -15736), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (214, GraciaSeeds::AnnihilationReptilikon, (-175660, 179462, -15680), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (215, GraciaSeeds::AnnihilationReptilikon, (-175912, 179732, -15664), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (216, GraciaSeeds::AnnihilationReptilikon, (-175156, 180182, -15680), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (217, GraciaSeeds::AnnihilationReptilikon, (-174240, 182059, -15664), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (218, GraciaSeeds::AnnihilationReptilikon, (-175590, 181478, -15640), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (219, GraciaSeeds::AnnihilationReptilikon, (-174510, 181561, -15616), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (220, GraciaSeeds::AnnihilationReptilikon, (-174240, 182391, -15688), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (221, GraciaSeeds::AnnihilationReptilikon, (-174105, 182806, -15672), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (222, GraciaSeeds::AnnihilationReptilikon, (-174645, 182806, -15712), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (223, GraciaSeeds::AnnihilationCokrakon, (-214962, 182403, -10992), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (224, GraciaSeeds::AnnihilationCokrakon, (-215019, 182493, -11000), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (225, GraciaSeeds::AnnihilationCokrakon, (-211374, 180793, -11672), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (226, GraciaSeeds::AnnihilationCokrakon, (-211198, 180661, -11680), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (227, GraciaSeeds::AnnihilationCokrakon, (-213097, 178936, -12720), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (228, GraciaSeeds::AnnihilationCokrakon, (-213517, 178936, -12712), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (229, GraciaSeeds::AnnihilationCokrakon, (-214105, 179191, -12720), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (230, GraciaSeeds::AnnihilationCokrakon, (-213769, 179446, -12720), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (231, GraciaSeeds::AnnihilationCokrakon, (-214021, 179344, -12720), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (232, GraciaSeeds::AnnihilationCokrakon, (-210582, 180595, -11672), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (233, GraciaSeeds::AnnihilationCokrakon, (-210934, 180661, -11696), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (234, GraciaSeeds::AnnihilationCokrakon, (-207058, 178460, -12656), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (235, GraciaSeeds::AnnihilationCokrakon, (-207454, 179151, -11368), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (236, GraciaSeeds::AnnihilationCokrakon, (-207422, 181365, -11344), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (237, GraciaSeeds::AnnihilationCokrakon, (-207358, 180627, -11344), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (238, GraciaSeeds::AnnihilationCokrakon, (-207230, 180996, -11344), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (239, GraciaSeeds::AnnihilationCokrakon, (-208515, 184160, -11344), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (240, GraciaSeeds::AnnihilationCokrakon, (-207613, 184000, -11344), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (241, GraciaSeeds::AnnihilationCokrakon, (-208597, 183760, -11352), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (242, GraciaSeeds::AnnihilationCokrakon, (-206710, 176142, -12656), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (243, GraciaSeeds::AnnihilationCokrakon, (-206361, 178136, -11336), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (244, GraciaSeeds::AnnihilationCokrakon, (-206178, 178630, -12672), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (245, GraciaSeeds::AnnihilationCokrakon, (-205738, 178715, -12656), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (246, GraciaSeeds::AnnihilationCokrakon, (-206442, 178205, -12648), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (247, GraciaSeeds::AnnihilationCokrakon, (-206585, 178874, -11336), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (248, GraciaSeeds::AnnihilationCokrakon, (-206073, 179366, -11344), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (249, GraciaSeeds::AnnihilationCokrakon, (-206009, 178628, -11344), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (250, GraciaSeeds::AnnihilationCokrakon, (-206155, 181301, -12656), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (251, GraciaSeeds::AnnihilationCokrakon, (-206595, 181641, -12656), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (252, GraciaSeeds::AnnihilationCokrakon, (-206507, 181641, -12656), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (253, GraciaSeeds::AnnihilationCokrakon, (-206507, 181471, -12640), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (254, GraciaSeeds::AnnihilationCokrakon, (-206974, 175972, -12672), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (255, GraciaSeeds::AnnihilationCokrakon, (-206304, 175130, -11344), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (256, GraciaSeeds::AnnihilationCokrakon, (-206886, 175802, -12672), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (257, GraciaSeeds::AnnihilationCokrakon, (-207238, 175972, -12672), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (258, GraciaSeeds::AnnihilationCokrakon, (-206386, 174857, -11328), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (259, GraciaSeeds::AnnihilationCokrakon, (-206386, 175039, -11336), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (260, GraciaSeeds::AnnihilationCokrakon, (-205976, 174584, -11344), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (261, GraciaSeeds::AnnihilationCokrakon, (-207367, 184320, -11336), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (262, GraciaSeeds::AnnihilationCokrakon, (-219002, 180419, -12608), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (263, GraciaSeeds::AnnihilationCokrakon, (-218853, 182790, -12592), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (264, GraciaSeeds::AnnihilationCokrakon, (-218853, 183343, -12600), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (265, GraciaSeeds::AnnihilationCokrakon, (-218358, 186247, -11344), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (266, GraciaSeeds::AnnihilationCokrakon, (-218358, 186083, -11344), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (267, GraciaSeeds::AnnihilationCokrakon, (-217574, 185796, -11352), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (268, GraciaSeeds::AnnihilationCokrakon, (-219178, 181051, -12592), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (269, GraciaSeeds::AnnihilationCokrakon, (-220171, 180313, -11344), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (270, GraciaSeeds::AnnihilationCokrakon, (-219293, 183738, -12592), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (271, GraciaSeeds::AnnihilationCokrakon, (-219381, 182553, -12584), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (272, GraciaSeeds::AnnihilationCokrakon, (-219600, 183024, -11336), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (273, GraciaSeeds::AnnihilationCokrakon, (-219940, 182680, -11344), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (274, GraciaSeeds::AnnihilationCokrakon, (-219260, 183884, -11344), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (275, GraciaSeeds::AnnihilationCokrakon, (-219855, 183540, -11344), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (276, GraciaSeeds::AnnihilationCokrakon, (-218946, 186575, -11344), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (277, GraciaSeeds::AnnihilationCokrakon, (-219882, 180103, -12592), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (278, GraciaSeeds::AnnihilationCokrakon, (-219266, 179787, -12584), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (279, GraciaSeeds::AnnihilationCokrakon, (-219201, 178337, -12592), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (280, GraciaSeeds::AnnihilationCokrakon, (-219716, 179875, -11336), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (281, GraciaSeeds::AnnihilationCokrakon, (-219716, 180021, -11328), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (282, GraciaSeeds::AnnihilationCokrakon, (-219989, 179437, -11336), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (283, GraciaSeeds::AnnihilationCokrakon, (-219078, 178298, -12592), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (284, GraciaSeeds::AnnihilationCokrakon, (-218684, 178954, -11328), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (285, GraciaSeeds::AnnihilationCokrakon, (-219089, 178456, -11328), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (286, GraciaSeeds::AnnihilationCokrakon, (-220266, 177623, -12608), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (287, GraciaSeeds::AnnihilationCokrakon, (-219201, 178025, -12592), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (288, GraciaSeeds::AnnihilationCokrakon, (-219142, 177044, -12592), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (289, GraciaSeeds::AnnihilationCokrakon, (-219690, 177895, -12592), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (290, GraciaSeeds::AnnihilationCokrakon, (-219754, 177623, -12584), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (291, GraciaSeeds::AnnihilationCokrakon, (-218791, 177830, -12584), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (292, GraciaSeeds::AnnihilationCokrakon, (-218904, 176219, -12592), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (293, GraciaSeeds::AnnihilationCokrakon, (-218768, 176384, -12584), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (294, GraciaSeeds::AnnihilationCokrakon, (-218774, 177626, -11320), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (295, GraciaSeeds::AnnihilationCokrakon, (-218774, 177792, -11328), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (296, GraciaSeeds::AnnihilationCokrakon, (-219880, 175901, -11344), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (297, GraciaSeeds::AnnihilationCokrakon, (-219210, 176054, -12592), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (298, GraciaSeeds::AnnihilationCokrakon, (-219850, 175991, -12608), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (299, GraciaSeeds::AnnihilationCokrakon, (-219079, 175021, -11336), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (300, GraciaSeeds::AnnihilationCokrakon, (-218812, 174229, -11344), &[18678, 18679, 18680, 18681, 18682, 18683]),
    (301, GraciaSeeds::AnnihilationCokrakon, (-218723, 174669, -11336), &[18678, 18679, 18680, 18681, 18682, 18683]),
];

/// Java `_spawnedNpcs`: the energy NPCs standing now and the `SPAWNS` id they came from.
#[derive(Default)]
struct EnergySeedsState {
    spawned_npcs: HashMap<ObjectId, i32>,
}

/// Where an energy NPC stood when it was gathered, for the surprise monster.
struct SeedSpot {
    x: i32,
    y: i32,
    z: i32,
    heading: i32,
    instance_id: i32,
}

/// Java `SPAWNS.get(spawnId)`.
fn spawn_of(spawn_id: i32) -> Option<&'static EsSpawn> {
    SPAWNS.iter().find(|s| s.0 == spawn_id)
}

/// Java `ai.areas.Gracia.AI.EnergySeeds`.
pub struct EnergySeeds;

impl EnergySeeds {
    /// Java `isSeedActive(seed)`.
    fn is_seed_active(ctx: &ScriptCtx<'_>, seed: GraciaSeeds) -> bool {
        match seed {
            GraciaSeeds::Infinity => false,
            GraciaSeeds::Destruction => sod_state(ctx) == 2,
            GraciaSeeds::AnnihilationBistakon
            | GraciaSeeds::AnnihilationReptilikon
            | GraciaSeeds::AnnihilationCokrakon => true,
        }
    }

    /// Java `ESSpawn.scheduleRespawn(waitTime)`: the Java lambda is a timer event of this script.
    fn schedule_respawn(ctx: &mut ScriptCtx<'_>, spawn_id: i32, wait_time: i64) {
        ctx.start_quest_timer(&format!("ESRespawn_{spawn_id}"), wait_time, None, None, false);
    }

    /// The body of Java's `ESSpawn.scheduleRespawn` lambda.
    fn respawn(ctx: &mut ScriptCtx<'_>, spawn_id: i32) {
        let Some(&(_, seed, (x, y, z), npc_ids)) = spawn_of(spawn_id) else {
            return;
        };
        // If the AI is inactive, do not spawn the NPC.
        if Self::is_seed_active(ctx, seed) {
            // Get a random NPC that should spawn at this location.
            let Some(npc_id) = ctx.get_random_entry(npc_ids) else {
                return;
            };
            if let Some(npc) = ctx.add_spawn(npc_id, x, y, z, 0, false, 0, 0) {
                ctx.state::<EnergySeedsState>()
                    .spawned_npcs
                    .insert(npc.object_id, spawn_id);
            }
        }
    }

    /// Java `startAI()`: spawn all NPCs of the active seeds.
    fn start_ai(ctx: &mut ScriptCtx<'_>) {
        for &(spawn_id, seed, _, _) in SPAWNS {
            if Self::is_seed_active(ctx, seed) {
                Self::schedule_respawn(ctx, spawn_id, 0);
            }
        }
    }

    /// Java `startAI(type)`.
    fn start_ai_of(ctx: &mut ScriptCtx<'_>, seed: GraciaSeeds) {
        for &(spawn_id, spawn_seed, _, _) in SPAWNS {
            if spawn_seed == seed {
                Self::schedule_respawn(ctx, spawn_id, 0);
            }
        }
    }

    /// Java `stopAI(type)`.
    fn stop_ai(ctx: &mut ScriptCtx<'_>, seed: GraciaSeeds) {
        let entries: Vec<(ObjectId, i32)> = ctx
            .state::<EnergySeedsState>()
            .spawned_npcs
            .iter()
            .map(|(npc, spawn_id)| (*npc, *spawn_id))
            .collect();
        for (npc, spawn_id) in entries {
            if spawn_of(spawn_id).is_some_and(|s| s.1 == seed) {
                ctx.world_mut().delete_npc(npc);
            }
        }
    }

    /// Java `handleQuestDrop(player, itemId)`.
    fn handle_quest_drop(ctx: &mut ScriptCtx<'_>, player: ObjectId, item_id: i32) {
        let mut chance = HOW_TO_OPPOSE_EVIL_CHANCE;
        let mut num_items = chance / 100;
        chance %= 100;
        if ctx.get_random(100) < chance {
            num_items += 1;
        }

        if num_items > 0 {
            ctx.give_items(player, item_id, i64::from(num_items));
            ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
        }
    }

    /// Java `spawnSupriseMob(energy, npcId)` and what `seedCollectEvent` does to it.
    fn surprise_mob(ctx: &mut ScriptCtx<'_>, player: ObjectId, spot: &SeedSpot, list: usize) {
        let ids = ANNIHILATION_SUPRISE_MOB_IDS[list];
        let Some(npc_id) = ctx.get_random_entry(ids) else {
            return;
        };
        // `new Monster(template)`, full HP/MP, the energy's heading and instance, `spawnMe(x, y, z)`.
        let Some(mob) = ctx.add_spawn(
            npc_id,
            spot.x,
            spot.y,
            spot.z,
            spot.heading,
            false,
            0,
            spot.instance_id,
        ) else {
            return;
        };
        ctx.start_quest_timer("DeSpawnTask", 30000, Some(mob), None, false);
        // `mob.setRunning(); mob.addDamageHate(player, 0, 999); mob.getAI().setIntentionAttack(player);`
        ctx.add_attack_desire(mob, player, 999);
    }

    /// Java `seedCollectEvent(player, seedEnergy, seedType)`.
    fn seed_collect_event(
        ctx: &mut ScriptCtx<'_>,
        player: ObjectId,
        spot: &SeedSpot,
        seed_type: GraciaSeeds,
    ) {
        let cond_3 = ctx
            .player_quest_state(player, HOW_TO_OPPOSE_EVIL)
            .is_some_and(|qs| qs.is_cond(3));
        match seed_type {
            GraciaSeeds::Infinity => {
                if cond_3 {
                    Self::handle_quest_drop(ctx, player, 13798);
                }
            }
            GraciaSeeds::Destruction => {
                if cond_3 {
                    Self::handle_quest_drop(ctx, player, 13867);
                }
            }
            GraciaSeeds::AnnihilationBistakon
            | GraciaSeeds::AnnihilationReptilikon
            | GraciaSeeds::AnnihilationCokrakon => {
                if cond_3 {
                    Self::handle_quest_drop(ctx, player, 15535);
                }

                if ctx.get_random(100) < 50 {
                    let list = match seed_type {
                        GraciaSeeds::AnnihilationBistakon => 0,
                        GraciaSeeds::AnnihilationReptilikon => 1,
                        _ => 2,
                    };
                    Self::surprise_mob(ctx, player, spot, list);
                }
            }
        }
    }
}

impl Script for EnergySeeds {
    fn name(&self) -> &'static str {
        "EnergySeeds"
    }

    fn path(&self) -> &'static str {
        "ai/areas/Gracia/AI"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_kill_id(&SEED_IDS);
        s.add_skill_see_id(&SEED_IDS);
        s.add_first_talk_id(&SEED_IDS);
        s.add_first_talk_id(&[TEMPORARY_TELEPORTER]);
        s.add_enter_zone_id(&[SOD_ZONE]);
        s.add_enter_zone_id(&[SOI_ZONE]);
    }

    // The Java constructor ends with `startAI()`.
    fn on_load(&self, ctx: &mut ScriptCtx<'_>) {
        Self::start_ai(ctx);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        if event.eq_ignore_ascii_case("StartSoDAi") {
            for door_id in SEED_OF_DESTRUCTION_DOORS {
                ctx.open_door(door_id, 0);
            }

            Self::start_ai_of(ctx, GraciaSeeds::Destruction);
        } else if event.eq_ignore_ascii_case("StopSoDAi") {
            for door_id in SEED_OF_DESTRUCTION_DOORS {
                ctx.close_door(door_id, 0);
            }

            for ch in players_in_zone(ctx, SOD_ZONE) {
                ctx.teleport_player(ch, SOD_EXIT_POINT.0, SOD_EXIT_POINT.1, SOD_EXIT_POINT.2, 0, false);
            }

            Self::stop_ai(ctx, GraciaSeeds::Destruction);
        } else if event.eq_ignore_ascii_case("DeSpawnTask") {
            let npc = npc?;
            if ctx.world().is_in_combat(npc.object_id) {
                ctx.start_quest_timer("DeSpawnTask", 30000, Some(npc), None, false);
            } else {
                ctx.world_mut().delete_npc(npc.object_id);
            }
        } else if let Some(spawn_id) = event.strip_prefix("ESRespawn_").and_then(|id| id.parse().ok()) {
            Self::respawn(ctx, spawn_id);
        }

        None
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        if npc.id == TEMPORARY_TELEPORTER {
            ctx.teleport_player(
                player,
                SOD_EXIT_POINT.0,
                SOD_EXIT_POINT.1,
                SOD_EXIT_POINT.2,
                0,
                false,
            );
        }

        ctx.world().send(player, &ActionFailed);
        None
    }

    fn on_skill_see(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        caster: ObjectId,
        skill: SkillKey,
        targets: &[ObjectId],
        _is_summon: bool,
    ) {
        if !targets.contains(&npc.object_id) || skill.id != 5780 {
            return;
        }

        // `deleteMe()` first: the place is read before the NPC is gone.
        let spot = {
            let world = ctx.world();
            let (x, y, z) = world.position_of(npc.object_id).unwrap_or_default();
            SeedSpot {
                x,
                y,
                z,
                heading: world.npc(npc.object_id).map_or(0, |n| n.core.heading),
                instance_id: world.instance_id_of(npc.object_id),
            }
        };
        ctx.world_mut().delete_npc(npc.object_id);

        let spawn_id = ctx
            .state::<EnergySeedsState>()
            .spawned_npcs
            .get(&npc.object_id)
            .copied();
        if let Some((spawn_id, spawn)) = spawn_id.and_then(|id| spawn_of(id).map(|s| (id, s))) {
            let wait = RESPAWN + i64::from(ctx.get_random(RANDOM_RESPAWN_OFFSET));
            Self::schedule_respawn(ctx, spawn_id, wait);
            ctx.state::<EnergySeedsState>()
                .spawned_npcs
                .remove(&npc.object_id);
            if Self::is_seed_active(ctx, spawn.1) {
                let item_id = match npc.id {
                    18678 => 14016, // Water
                    18679 => 14015, // Fire
                    18680 => 14017, // Wind
                    18681 => 14018, // Earth
                    18682 => 14020, // Divinity
                    18683 => 14019, // Darkness
                    _ => return,
                };

                if ctx.get_random(100) < 33 {
                    ctx.world().send(
                        caster,
                        &SystemMessage::new(SystemMessageId::YourCollectionHasSucceeded),
                    );
                    let count = ctx.get_random_range(RATE + 1, 2 * RATE);
                    ctx.give_items(caster, item_id, i64::from(count));
                } else {
                    ctx.world().send(
                        caster,
                        &SystemMessage::new(SystemMessageId::YourCollectionHasSucceeded),
                    );
                    let count = ctx.get_random_range(1, RATE);
                    ctx.give_items(caster, item_id, i64::from(count));
                }

                Self::seed_collect_event(ctx, caster, &spot, spawn.1);
            }
        }
    }

    fn on_enter_zone(&self, ctx: &mut ScriptCtx<'_>, creature: ObjectId, zone_id: i32) {
        if ctx.world().instance_id_of(creature) != 0 {
            return;
        }

        if ctx.world().player(creature).is_some() {
            match zone_id {
                SOD_ZONE => {
                    if !Self::is_seed_active(ctx, GraciaSeeds::Destruction) && !ctx.is_gm(creature) {
                        ctx.teleport_player(
                            creature,
                            SOD_EXIT_POINT.0,
                            SOD_EXIT_POINT.1,
                            SOD_EXIT_POINT.2,
                            0,
                            false,
                        );
                    }
                }
                SOI_ZONE => {
                    // NOTE(java-quirk): outside stage 3 the seed counts as closed only when it is
                    // not open either; `getOpenedTime()` is 0 outside stage 3, so both say the same.
                    if soi_current_stage(ctx) != 3 && soi_opened_time(ctx, ctx.now_ms()) <= 0 {
                        ctx.teleport_player(
                            creature,
                            SOI_EXIT_POINT.0,
                            SOI_EXIT_POINT.1,
                            SOI_EXIT_POINT.2,
                            0,
                            false,
                        );
                    }
                }
                _ => {}
            }
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _player: ObjectId, _is_summon: bool) {
        let spawn_id = ctx
            .state::<EnergySeedsState>()
            .spawned_npcs
            .get(&npc.object_id)
            .copied();
        if let Some(spawn_id) = spawn_id.filter(|id| spawn_of(*id).is_some()) {
            let wait = RESPAWN + i64::from(ctx.get_random(RANDOM_RESPAWN_OFFSET));
            Self::schedule_respawn(ctx, spawn_id, wait);
            ctx.state::<EnergySeedsState>()
                .spawned_npcs
                .remove(&npc.object_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{ScriptTest, names_to};

    const NPCS: &str = r#"<npc id="18678" type="Monster" level="80" name="Water Seed"/>
        <npc id="18679" type="Monster" level="80" name="Fire Seed"/>
        <npc id="18680" type="Monster" level="80" name="Wind Seed"/>
        <npc id="18681" type="Monster" level="80" name="Earth Seed"/>
        <npc id="18682" type="Monster" level="80" name="Divinity Seed"/>
        <npc id="18683" type="Monster" level="80" name="Darkness Seed"/>
        <npc id="32602" type="Folk" level="80" name="Teleporter"/>
        <npc id="22746" type="Monster" level="80" name="Surprise"/>
        <npc id="22747" type="Monster" level="80" name="Surprise"/>
        <npc id="22748" type="Monster" level="80" name="Surprise"/>
        <npc id="22749" type="Monster" level="80" name="Surprise"/>
        <npc id="22754" type="Monster" level="80" name="Surprise"/>
        <npc id="22755" type="Monster" level="80" name="Surprise"/>
        <npc id="22756" type="Monster" level="80" name="Surprise"/>
        <npc id="22760" type="Monster" level="80" name="Surprise"/>
        <npc id="22761" type="Monster" level="80" name="Surprise"/>
        <npc id="22762" type="Monster" level="80" name="Surprise"/>"#;
    const ITEMS: &str = r#"<item id="14016" type="EtcItem" name="Water"><set name="is_stackable" val="true"/></item>
        <item id="14015" type="EtcItem" name="Fire"><set name="is_stackable" val="true"/></item>"#;

    fn annihilation_spawns() -> usize {
        SPAWNS
            .iter()
            .filter(|s| {
                matches!(
                    s.1,
                    GraciaSeeds::AnnihilationBistakon
                        | GraciaSeeds::AnnihilationReptilikon
                        | GraciaSeeds::AnnihilationCokrakon
                )
            })
            .count()
    }

    fn world(vars: &[(&str, &str)]) -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(EnergySeeds);
        let mut t = ScriptTest::new(registry, NPCS, ITEMS, &[]);
        t.world.set_global_variables(
            vars.iter()
                .map(|(k, v)| ((*k).to_owned(), Some((*v).to_owned())))
                .collect(),
        );
        t.enter(1, 80, vec![]);
        t
    }

    fn started(t: &mut ScriptTest) {
        // The load ran `startAI()` (the Java constructor): the respawn timers fire at once.
        t.ticks(5);
    }

    #[test]
    fn the_table_is_complete() {
        assert_eq!(SPAWNS.len(), 301);
        assert!(SPAWNS.iter().all(|s| !s.3.is_empty()));
        assert_eq!(spawn_of(1).unwrap().3, &[TEMPORARY_TELEPORTER]);
    }

    #[test]
    fn the_annihilation_seeds_spawn_and_the_destruction_ones_wait_for_the_state() {
        let mut t = world(&[]);
        let before = t.world.npc_count();
        started(&mut t);
        assert_eq!(t.world.npc_count() - before, annihilation_spawns());

        // State 2 of the Seed of Destruction: `StartSoDAi` spawns its energy.
        t.world
            .set_global_variables(vec![("SoDState".to_owned(), Some("2".to_owned()))]);
        let before = t.world.npc_count();
        t.world
            .run_as_script("EnergySeeds", |ctx| ctx.notify_event("StartSoDAi", None, None));
        t.ticks(5);
        let destruction = SPAWNS.iter().filter(|s| s.1 == GraciaSeeds::Destruction).count();
        assert_eq!(t.world.npc_count() - before, destruction);
        // `StopSoDAi` takes them away again.
        t.world
            .run_as_script("EnergySeeds", |ctx| ctx.notify_event("StopSoDAi", None, None));
        assert_eq!(t.world.npc_count(), before);
    }

    #[test]
    fn a_collected_seed_gives_its_item_and_comes_back() {
        let mut t = world(&[]);
        started(&mut t);
        let seed = t
            .world
            .npc_ids()
            .into_iter()
            .find(|id| t.world.npc(*id).is_some_and(|n| n.npc_id == 18679))
            .unwrap();
        let before = t.world.npc_count();
        t.take();
        t.world.run_as_script("EnergySeeds", |ctx| {
            let n = ctx.script_npc(seed).unwrap();
            EnergySeeds.on_skill_see(ctx, n, ObjectId(1), SkillKey::new(5780, 1), &[seed], false);
        });
        // The energy is gone; half of the Annihilation ones bring a surprise monster.
        assert!(t.world.npc_count() == before - 1 || t.world.npc_count() == before);
        // Fire energy gives 14015: 1 or 2 pieces; the message went out.
        assert!((1..=2).contains(&t.count(1, 14015)), "{}", t.count(1, 14015));
        assert!(names_to(&t.take(), 1).contains(&"SYSTEM_MESSAGE".to_owned()));
        // 480 to 660 seconds later it is back.
        t.ticks(13_300);
        assert!(t.world.npc_count() >= before);
    }

    #[test]
    fn a_seed_nobody_targets_stays() {
        let mut t = world(&[]);
        started(&mut t);
        let seed = t
            .world
            .npc_ids()
            .into_iter()
            .find(|id| t.world.npc(*id).is_some_and(|n| n.npc_id == 18679))
            .unwrap();
        let before = t.world.npc_count();
        t.world.run_as_script("EnergySeeds", |ctx| {
            let n = ctx.script_npc(seed).unwrap();
            EnergySeeds.on_skill_see(ctx, n, ObjectId(1), SkillKey::new(5780, 1), &[], false);
            EnergySeeds.on_skill_see(ctx, n, ObjectId(1), SkillKey::new(1, 1), &[seed], false);
        });
        assert_eq!(t.world.npc_count(), before);
    }
}
