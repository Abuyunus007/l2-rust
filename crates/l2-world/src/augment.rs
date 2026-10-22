//! Augmentation: the life stone windows, the "Augment" button, rolling a new augmentation and
//! taking one off again (M6-AUGMENT-01).
// Java: network/clientpackets/{AbstractRefinePacket,RequestConfirmTargetItem,RequestConfirmRefinerItem,
//       RequestConfirmGemStone,RequestRefine,RequestConfirmCancelItem,RequestRefineCancel}.java
// Java: data/AugmentationData.java (the rolling; the tables are `l2_data::augment`)
// Java: dist/game/data/scripts/handlers/bypass/npc/Augment.java
// Java: network/serverpackets/{ExShowVariationMakeWindow,ExShowVariationCancelWindow,
//       ExPutItemResultForVariationMake,ExPutIntensiveResultForVariationMake,
//       ExPutCommissionResultForVariationMake,ExVariationResult,ExPutItemResultForVariationCancel,
//       ExVariationCancelResult}.java
//
// # Model
// An augmentation is the `i32` in `ItemData::augmentation` (`effects/item_extras.rs`) and the
// `item_attributes` row it writes. What its two option ids give the wearer is applied by the
// equip code (`stats/skills.rs`, `Options.apply`), so this module only rolls, stores and removes
// the id. Both Java packets that change an item (`RequestRefine`, `RequestRefineCancel`) take the
// item off first, so no bonus is ever added or removed here (`Augmentation.applyBonus` is the
// equip listener's).
//
// # Left out on purpose
// - `isFishing()` and `isCursedWeaponEquipped()` of `AbstractRefinePacket.isValid(player)`: neither
//   fishing nor cursed weapons exist yet, so they never refuse.
// - The `OnPlayerAugment` script event (`Item.setAugmentation` / `removeAugmentation`): scripts come
//   after the world is complete (D-05).
// - `PunishmentManager.handleIllegalPlayerAction` for an item of another owner: the items come from
//   the player's own inventory, so the owner check cannot fail.

use std::sync::Arc;

use l2_data::AugmentationData;
use l2_data::augment::{
    ACC_EAR_BLOCKSIZE, ACC_EAR_SKILLS, ACC_EAR_START, ACC_NECK_BLOCKSIZE, ACC_NECK_SKILLS, ACC_NECK_START,
    ACC_RING_BLOCKSIZE, ACC_RING_SKILLS, ACC_RING_START, ACC_STAT_SUBBLOCKSIZE, AugmentationChance,
    BASESTAT_MEN, BASESTAT_STR, STAT_BLOCKSIZE, STAT_SUBBLOCKSIZE, SkillColor,
};
use l2_data::item::ItemTemplates;
use l2_data::item::types::{BodyPart, CrystalType, WeaponType};
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{
    ExPutCommissionResultForVariationMake, ExPutIntensiveResultForVariationMake,
    ExPutItemResultForVariationCancel, ExPutItemResultForVariationMake, ExShowVariationCancelWindow,
    ExShowVariationMakeWindow, ExVariationCancelResult, ExVariationResult, SystemMessage,
};
use l2_util::{ObjectId, WorldRng};
use tracing::warn;

use crate::inventory::{LOC_INVENTORY, LOC_PAPERDOLL};
use crate::player::ItemData;
use crate::world::World;

// ---------------------------------------------------------------------------------------------
// Life stones and gemstones (AbstractRefinePacket)
// ---------------------------------------------------------------------------------------------

/// Java `GRADE_NONE`.
pub const GRADE_NONE: i32 = 0;
/// Java `GRADE_MID`.
pub const GRADE_MID: i32 = 1;
/// Java `GRADE_HIGH`.
pub const GRADE_HIGH: i32 = 2;
/// Java `GRADE_TOP`.
pub const GRADE_TOP: i32 = 3;
/// Java `GRADE_ACC`: the accessory life stones.
pub const GRADE_ACC: i32 = 4;

/// Java `GEMSTONE_D`.
pub const GEMSTONE_D: i32 = 2130;
/// Java `GEMSTONE_C`.
pub const GEMSTONE_C: i32 = 2131;
/// Java `GEMSTONE_B`.
pub const GEMSTONE_B: i32 = 2132;

/// Java `LifeStone.LEVELS`: the character level each life stone level needs.
const LIFE_STONE_PLAYER_LEVELS: [i32; 14] = [46, 49, 52, 55, 58, 61, 64, 67, 70, 76, 80, 82, 84, 85];

/// Java `LifeStone`: the grade and level of a life stone item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LifeStone {
    /// `GRADE_NONE` to `GRADE_ACC`.
    pub grade: i32,
    /// 0 to 13.
    pub level: i32,
}

impl LifeStone {
    /// Java `getPlayerLevel()`.
    pub fn player_level(self) -> i32 {
        usize::try_from(self.level)
            .ok()
            .and_then(|l| LIFE_STONE_PLAYER_LEVELS.get(l))
            .copied()
            .unwrap_or(0)
    }
}

/// Java `_lifeStones`: item id, grade, level.
const LIFE_STONES: [(i32, i32, i32); 87] = [
    (8723, GRADE_NONE, 0),
    (8724, GRADE_NONE, 1),
    (8725, GRADE_NONE, 2),
    (8726, GRADE_NONE, 3),
    (8727, GRADE_NONE, 4),
    (8728, GRADE_NONE, 5),
    (8729, GRADE_NONE, 6),
    (8730, GRADE_NONE, 7),
    (8731, GRADE_NONE, 8),
    (8732, GRADE_NONE, 9),
    (8733, GRADE_MID, 0),
    (8734, GRADE_MID, 1),
    (8735, GRADE_MID, 2),
    (8736, GRADE_MID, 3),
    (8737, GRADE_MID, 4),
    (8738, GRADE_MID, 5),
    (8739, GRADE_MID, 6),
    (8740, GRADE_MID, 7),
    (8741, GRADE_MID, 8),
    (8742, GRADE_MID, 9),
    (8743, GRADE_HIGH, 0),
    (8744, GRADE_HIGH, 1),
    (8745, GRADE_HIGH, 2),
    (8746, GRADE_HIGH, 3),
    (8747, GRADE_HIGH, 4),
    (8748, GRADE_HIGH, 5),
    (8749, GRADE_HIGH, 6),
    (8750, GRADE_HIGH, 7),
    (8751, GRADE_HIGH, 8),
    (8752, GRADE_HIGH, 9),
    (8753, GRADE_TOP, 0),
    (8754, GRADE_TOP, 1),
    (8755, GRADE_TOP, 2),
    (8756, GRADE_TOP, 3),
    (8757, GRADE_TOP, 4),
    (8758, GRADE_TOP, 5),
    (8759, GRADE_TOP, 6),
    (8760, GRADE_TOP, 7),
    (8761, GRADE_TOP, 8),
    (8762, GRADE_TOP, 9),
    (9573, GRADE_NONE, 10),
    (9574, GRADE_MID, 10),
    (9575, GRADE_HIGH, 10),
    (9576, GRADE_TOP, 10),
    (10483, GRADE_NONE, 11),
    (10484, GRADE_MID, 11),
    (10485, GRADE_HIGH, 11),
    (10486, GRADE_TOP, 11),
    (12754, GRADE_ACC, 0),
    (12755, GRADE_ACC, 1),
    (12756, GRADE_ACC, 2),
    (12757, GRADE_ACC, 3),
    (12758, GRADE_ACC, 4),
    (12759, GRADE_ACC, 5),
    (12760, GRADE_ACC, 6),
    (12761, GRADE_ACC, 7),
    (12762, GRADE_ACC, 8),
    (12763, GRADE_ACC, 9),
    (12821, GRADE_ACC, 10),
    (12822, GRADE_ACC, 11),
    (12840, GRADE_ACC, 0),
    (12841, GRADE_ACC, 1),
    (12842, GRADE_ACC, 2),
    (12843, GRADE_ACC, 3),
    (12844, GRADE_ACC, 4),
    (12845, GRADE_ACC, 5),
    (12846, GRADE_ACC, 6),
    (12847, GRADE_ACC, 7),
    (12848, GRADE_ACC, 8),
    (12849, GRADE_ACC, 9),
    (12850, GRADE_ACC, 10),
    (12851, GRADE_ACC, 11),
    (14008, GRADE_ACC, 12),
    (14166, GRADE_NONE, 12),
    (14167, GRADE_MID, 12),
    (14168, GRADE_HIGH, 12),
    (14169, GRADE_TOP, 12),
    (16160, GRADE_NONE, 13),
    (16161, GRADE_MID, 13),
    (16162, GRADE_HIGH, 13),
    (16163, GRADE_TOP, 13),
    (16177, GRADE_ACC, 13),
    (16164, GRADE_NONE, 13),
    (16165, GRADE_MID, 13),
    (16166, GRADE_HIGH, 13),
    (16167, GRADE_TOP, 13),
    (16178, GRADE_ACC, 13),
];

/// Java `getLifeStone(itemId)`: `None` for an item that is no life stone.
pub fn life_stone(item_id: i32) -> Option<LifeStone> {
    LIFE_STONES
        .iter()
        .find(|(id, _, _)| *id == item_id)
        .map(|&(_, grade, level)| LifeStone { grade, level })
}

/// Java `getGemStoneId(itemGrade)`: the gemstone item the grade of the item needs, 0 for a grade
/// that cannot be augmented.
pub fn gemstone_id(grade: CrystalType) -> i32 {
    match grade {
        CrystalType::C | CrystalType::B => GEMSTONE_D,
        CrystalType::A | CrystalType::S => GEMSTONE_C,
        CrystalType::S80 | CrystalType::S84 => GEMSTONE_B,
        _ => 0,
    }
}

/// Java `getGemStoneCount(itemGrade, lifeStoneGrade)`: different for weapons and accessories.
pub fn gemstone_count(grade: CrystalType, life_stone_grade: i32) -> i32 {
    if life_stone_grade == GRADE_ACC {
        match grade {
            CrystalType::C => 200,
            CrystalType::B => 300,
            CrystalType::A => 200,
            CrystalType::S => 250,
            CrystalType::S80 => 360,
            CrystalType::S84 => 480,
            _ => 0,
        }
    } else {
        match grade {
            CrystalType::C => 20,
            CrystalType::B => 30,
            CrystalType::A => 20,
            CrystalType::S => 25,
            CrystalType::S80 | CrystalType::S84 => 36,
            _ => 0,
        }
    }
}

/// What taking an augmentation off costs: the same table in `RequestConfirmCancelItem` and
/// `RequestRefineCancel`. `None` for a grade that cannot be augmented.
pub fn removal_price(grade: CrystalType, crystal_count: i32) -> Option<i64> {
    let price = match grade {
        CrystalType::C => {
            if crystal_count < 1720 {
                95_000
            } else if crystal_count < 2452 {
                150_000
            } else {
                210_000
            }
        }
        CrystalType::B => {
            if crystal_count < 1746 {
                240_000
            } else {
                270_000
            }
        }
        CrystalType::A => {
            if crystal_count < 2160 {
                330_000
            } else if crystal_count < 2824 {
                390_000
            } else {
                420_000
            }
        }
        CrystalType::S => 480_000,
        CrystalType::S80 | CrystalType::S84 => 920_000,
        // any other item type is not augmentable
        _ => return None,
    };
    Some(price)
}

// ---------------------------------------------------------------------------------------------
// Settings and world state
// ---------------------------------------------------------------------------------------------

/// The `Player.ini` values the augmentation reads (`PlayerConfig.AUGMENTATION_*`,
/// `RETAIL_LIKE_AUGMENTATION*`, `ALT_ALLOW_AUGMENT_PVP_ITEMS`). `AltAllowAugmentTrade` and
/// `AltAllowAugmentDestroy` are the inventory settings'. `Default` is the Java default of each key.
#[derive(Debug, Clone, PartialEq)]
pub struct AugmentSettings {
    /// `AugmentationNGSkillChance`.
    pub ng_skill_chance: i32,
    /// `AugmentationNGGlowChance`.
    pub ng_glow_chance: i32,
    /// `AugmentationMidSkillChance`.
    pub mid_skill_chance: i32,
    /// `AugmentationMidGlowChance`.
    pub mid_glow_chance: i32,
    /// `AugmentationHighSkillChance`.
    pub high_skill_chance: i32,
    /// `AugmentationHighGlowChance`.
    pub high_glow_chance: i32,
    /// `AugmentationTopSkillChance`.
    pub top_skill_chance: i32,
    /// `AugmentationTopGlowChance`.
    pub top_glow_chance: i32,
    /// `AugmentationBaseStatChance`.
    pub basestat_chance: i32,
    /// `AugmentationAccSkillChance`.
    pub acc_skill_chance: i32,
    /// `RetailLikeAugmentation`.
    pub retail_like: bool,
    /// `RetailLikeAugmentationNoGradeChance`, `MidGradeChance`, `HighGradeChance`,
    /// `TopGradeChance` (yellow, blue, purple, red), indexed by life stone grade.
    pub retail_like_chances: [[i32; 4]; 4],
    /// `RetailLikeAugmentationAccessory`.
    pub retail_like_accessory: bool,
    /// `AugmentationBlackList`, sorted (Java `Arrays.sort`).
    pub blacklist: Vec<i32>,
    /// `AltAllowAugmentPvPItems`.
    pub alt_allow_augment_pvp_items: bool,
}

impl Default for AugmentSettings {
    fn default() -> Self {
        Self {
            ng_skill_chance: 15,
            ng_glow_chance: 0,
            mid_skill_chance: 30,
            mid_glow_chance: 40,
            high_skill_chance: 45,
            high_glow_chance: 70,
            top_skill_chance: 60,
            top_glow_chance: 100,
            basestat_chance: 1,
            acc_skill_chance: 0,
            retail_like: true,
            retail_like_chances: [[55, 35, 7, 3]; 4],
            retail_like_accessory: true,
            blacklist: vec![
                6656, 6657, 6658, 6659, 6660, 6661, 6662, 8191, 10170, 10314, 13740, 13741, 13742, 13743,
                13744, 13745, 13746, 13747, 13748, 14592, 14593, 14594, 14595, 14596, 14597, 14598, 14599,
                14600, 14664, 14665, 14666, 14667, 14668, 14669, 14670, 14671, 14672, 14801, 14802, 14803,
                14804, 14805, 14806, 14807, 14808, 14809, 15282, 15283, 15284, 15285, 15286, 15287, 15288,
                15289, 15290, 15291, 15292, 15293, 15294, 15295, 15296, 15297, 15298, 15299, 16025, 16026,
                21712, 22173, 22174, 22175,
            ],
            alt_allow_augment_pvp_items: false,
        }
    }
}

/// The loaded augmentation tables.
#[derive(Debug, Default)]
pub struct AugmentWorld {
    pub(crate) data: Option<Arc<AugmentationData>>,
}

/// The augmentation packets, as the world gets them (see `ClientInput::Augment`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AugmentInput {
    /// Java `RequestConfirmTargetItem`.
    ConfirmTargetItem { object_id: i32 },
    /// Java `RequestConfirmRefinerItem`.
    ConfirmRefinerItem {
        target_item_object_id: i32,
        refiner_item_object_id: i32,
    },
    /// Java `RequestConfirmGemStone`.
    ConfirmGemStone {
        target_item_object_id: i32,
        refiner_item_object_id: i32,
        gemstone_item_object_id: i32,
        gemstone_count: i64,
    },
    /// Java `RequestRefine`.
    Refine {
        target_item_object_id: i32,
        refiner_item_object_id: i32,
        gemstone_item_object_id: i32,
        gemstone_count: i64,
    },
    /// Java `RequestConfirmCancelItem`.
    ConfirmCancelItem { object_id: i32 },
    /// Java `RequestRefineCancel`.
    RefineCancel { target_item_object_id: i32 },
}

// ---------------------------------------------------------------------------------------------
// Rolling an augmentation (AugmentationData.generateRandomAugmentation)
// ---------------------------------------------------------------------------------------------

/// What one roll needs to know about the item and the life stone.
#[derive(Debug, Clone, Copy)]
pub struct RollInput {
    /// `LifeStone.getLevel()`.
    pub life_stone_level: i32,
    /// `LifeStone.getGrade()`.
    pub life_stone_grade: i32,
    /// The life stone item id.
    pub life_stone_id: i32,
    /// `item.getTemplate().getBodyPart()`.
    pub body_part: BodyPart,
    /// `item.getTemplate().isMagicWeapon()`.
    pub magic_weapon: bool,
}

/// Java `AugmentationData.generateRandomAugmentation(...)`: the augmentation id (the second option
/// id in the high 16 bits, the first in the low ones).
///
/// `option_has_skill(id)` is Java's `OptionData.getOptions(id)` followed by "has an active, a
/// passive or an activation skill" (false when there is no such option).
///
/// `None` is where Java throws (`IndexOutOfBoundsException` on an empty skill list of the skill
/// map): `RequestRefine` has used up the stones by then and ends without a result.
pub fn generate_random_augmentation(
    rng: &mut WorldRng,
    tables: &AugmentationData,
    settings: &AugmentSettings,
    input: RollInput,
    option_has_skill: &dyn Fn(i32) -> bool,
) -> Option<i32> {
    match input.body_part {
        BodyPart::BothFingers | BodyPart::BothEars | BodyPart::Neck => Some(generate_accessory(
            rng,
            tables,
            settings,
            input.life_stone_level,
            input.body_part,
            input.life_stone_id,
            option_has_skill,
        )),
        _ => generate_weapon(rng, tables, settings, input),
    }
}

/// Java's `s > r` loop of the retail tables: walks the candidates and keeps the last one visited
/// while the running sum is still above the roll.
// NOTE(java-quirk): the loop never leaves early and the running sum is a `float` that shrinks by
// `chance * 100` at each visited entry, so the entries are not weighted by their own chance the
// way a cumulative table would be. The sum and the products are `f32`, as in Java.
fn pick_retail(rng: &mut WorldRng, candidates: &[&AugmentationChance]) -> i32 {
    let r = rng.below(10000);
    let mut s: f32 = 10000.0;
    let mut picked = 0;
    for candidate in candidates {
        if s > r as f32 {
            s -= candidate.augment_chance * 100.0;
            picked = candidate.augment_id;
        }
    }
    picked
}

/// Java's colour category roll of the retail weapon table: 55, 35, 7 or 3.
fn retail_category(rng: &mut WorldRng, chances: [i32; 4]) -> i32 {
    let c = rng.below(100);
    if c < chances[0] {
        55
    } else if c < chances[0] + chances[1] {
        35
    } else if c < chances[0] + chances[1] + chances[2] {
        7
    } else {
        3
    }
}

fn generate_weapon(
    rng: &mut WorldRng,
    tables: &AugmentationData,
    settings: &AugmentSettings,
    input: RollInput,
) -> Option<i32> {
    let grade = input.life_stone_grade;
    if settings.retail_like {
        // The mage and warrior branches of Java are the same code with another weapon type.
        let weapon_type = if input.magic_weapon { "mage" } else { "warrior" };
        let mut selected12: Vec<&AugmentationChance> = Vec::new();
        let mut selected34: Vec<&AugmentationChance> = Vec::new();
        for ac in tables.chances() {
            if ac.weapon_type == weapon_type && ac.stone_id == input.life_stone_id {
                if ac.variation_id == 1 {
                    selected12.push(ac);
                } else {
                    selected34.push(ac);
                }
            }
        }
        let stat12 = pick_retail(rng, &selected12);
        // The default of Java's switch is the no-grade chances.
        let grade_chance = usize::try_from(grade)
            .ok()
            .and_then(|g| settings.retail_like_chances.get(g))
            .copied()
            .unwrap_or(settings.retail_like_chances[0]);
        let category = retail_category(rng, grade_chance);
        let selected34_final: Vec<&AugmentationChance> = selected34
            .into_iter()
            .filter(|ac| ac.category_chance == category)
            .collect();
        let stat34 = pick_retail(rng, &selected34_final);
        return Some(stat34.wrapping_shl(16).wrapping_add(stat12));
    }

    let mut generate_skill = false;
    let mut generate_glow = false;
    // life stone level is used for stat Id and skill level, but here the max level is 9
    let life_stone_level = input.life_stone_level.min(9);
    match grade {
        GRADE_NONE => {
            generate_skill = rng.get(1, 100) <= settings.ng_skill_chance;
            generate_glow = rng.get(1, 100) <= settings.ng_glow_chance;
        }
        GRADE_MID => {
            generate_skill = rng.get(1, 100) <= settings.mid_skill_chance;
            generate_glow = rng.get(1, 100) <= settings.mid_glow_chance;
        }
        GRADE_HIGH => {
            generate_skill = rng.get(1, 100) <= settings.high_skill_chance;
            generate_glow = rng.get(1, 100) <= settings.high_glow_chance;
        }
        GRADE_TOP => {
            generate_skill = rng.get(1, 100) <= settings.top_skill_chance;
            generate_glow = rng.get(1, 100) <= settings.top_glow_chance;
        }
        GRADE_ACC => {
            generate_skill = rng.get(1, 100) <= settings.acc_skill_chance;
        }
        _ => {}
    }

    let mut stat34 = 0;
    if !generate_skill && rng.get(1, 100) <= settings.basestat_chance {
        stat34 = rng.get(BASESTAT_STR, BASESTAT_MEN);
    }

    // Second: decide which grade the augmentation result is going to have:
    // 0:yellow, 1:blue, 2:purple, 3:red
    // The chances used here are most likely custom,
    // what's known is: you can't have yellow with skill(or baseStatModifier)
    // noGrade stone can not have glow, mid only with skill, high has a chance(custom), top allways glow
    let mut result_color = rng.get(0, 100);
    if stat34 == 0 && !generate_skill {
        result_color = if result_color <= (15 * grade) + 40 { 1 } else { 0 };
    } else if result_color <= (10 * grade) + 5 || stat34 != 0 {
        result_color = 3;
    } else if result_color <= (10 * grade) + 10 {
        result_color = 1;
    } else {
        result_color = 2;
    }

    // generate a skill if necessary
    if generate_skill {
        let color = match result_color {
            1 => Some(SkillColor::Blue),
            2 => Some(SkillColor::Purple),
            3 => Some(SkillColor::Red),
            _ => None,
        };
        if let Some(color) = color {
            let list = tables.skills(color, usize::try_from(life_stone_level).unwrap_or(0));
            // `Rnd.get(0, size - 1)`, then `List.get`: an empty list throws.
            let upper = i32::try_from(list.len()).unwrap_or(i32::MAX) - 1;
            let index = usize::try_from(rng.get(0, upper)).unwrap_or(0);
            stat34 = *list.get(index)?;
        }
    }

    // Third: Calculate the subblock offset for the chosen color, and the level of the lifeStone
    // from large number of retail augmentations:
    // no skill part
    // Id for stat12:
    // A:1-910 B:911-1820 C:1821-2730 D:2731-3640 E:3641-4550 F:4551-5460 G:5461-6370 H:6371-7280
    // Id for stat34(this defines the color):
    // I:7281-8190(yellow) K:8191-9100(blue) L:10921-11830(yellow) M:11831-12740(blue)
    // you can combine I-K with A-D and L-M with E-H
    // using C-D or G-H Id you will get a glow effect
    // there seems no correlation in which grade use which Id except for the glowing restriction
    // skill part
    // Id for stat12:
    // same for no skill part
    // A same as E, B same as F, C same as G, D same as H
    // A - no glow, no grade LS
    // B - weak glow, mid grade LS?
    // C - glow, high grade LS?
    // D - strong glow, top grade LS?

    // Is neither a skill nor basestat used for stat34? Then generate a normal stat.
    let offset;
    if stat34 == 0 {
        let temp = rng.get(2, 3);
        let color_offset = (result_color * (10 * STAT_SUBBLOCKSIZE)) + (temp * STAT_BLOCKSIZE) + 1;
        let first = (life_stone_level * STAT_SUBBLOCKSIZE) + color_offset;
        stat34 = rng.get(first, (first + STAT_SUBBLOCKSIZE) - 1);
        offset = if generate_glow && grade >= 2 {
            (life_stone_level * STAT_SUBBLOCKSIZE)
                + ((temp - 2) * STAT_BLOCKSIZE)
                + (grade * (10 * STAT_SUBBLOCKSIZE))
                + 1
        } else {
            (life_stone_level * STAT_SUBBLOCKSIZE)
                + ((temp - 2) * STAT_BLOCKSIZE)
                + (rng.get(0, 1) * (10 * STAT_SUBBLOCKSIZE))
                + 1
        };
    } else if !generate_glow {
        offset = (life_stone_level * STAT_SUBBLOCKSIZE) + (rng.get(0, 1) * STAT_BLOCKSIZE) + 1;
    } else {
        offset = (life_stone_level * STAT_SUBBLOCKSIZE)
            + (rng.get(0, 1) * STAT_BLOCKSIZE)
            + (((grade + result_color) / 2) * (10 * STAT_SUBBLOCKSIZE))
            + 1;
    }

    let stat12 = rng.get(offset, (offset + STAT_SUBBLOCKSIZE) - 1);
    Some(stat34.wrapping_shl(16).wrapping_add(stat12))
}

fn generate_accessory(
    rng: &mut WorldRng,
    tables: &AugmentationData,
    settings: &AugmentSettings,
    level: i32,
    body_part: BodyPart,
    life_stone_id: i32,
    option_has_skill: &dyn Fn(i32) -> bool,
) -> i32 {
    if settings.retail_like_accessory {
        let mut selected12: Vec<&AugmentationChance> = Vec::new();
        let mut selected34: Vec<&AugmentationChance> = Vec::new();
        for ac in tables.accessory_chances() {
            if ac.weapon_type == "warrior" && ac.stone_id == life_stone_id {
                if ac.variation_id == 1 {
                    selected12.push(ac);
                } else {
                    selected34.push(ac);
                }
            }
        }
        let stat12 = pick_retail(rng, &selected12);
        let c = rng.below(100);
        let category = if c < 55 {
            55
        } else if c < 90 {
            35
        } else if c < 99 {
            9
        } else {
            1
        };
        let selected34_final: Vec<&AugmentationChance> = selected34
            .into_iter()
            .filter(|ac| ac.category_chance == category)
            .collect();
        let stat34 = pick_retail(rng, &selected34_final);
        return stat34.wrapping_shl(16).wrapping_add(stat12);
    }

    let life_stone_level = level.min(9);
    let (base, skills_length) = match body_part {
        BodyPart::BothFingers => (
            ACC_RING_START + (ACC_RING_BLOCKSIZE * life_stone_level),
            ACC_RING_SKILLS,
        ),
        BodyPart::BothEars => (
            ACC_EAR_START + (ACC_EAR_BLOCKSIZE * life_stone_level),
            ACC_EAR_SKILLS,
        ),
        // The caller only comes here for these three body parts; Java's `default` returns null.
        _ => (
            ACC_NECK_START + (ACC_NECK_BLOCKSIZE * life_stone_level),
            ACC_NECK_SKILLS,
        ),
    };

    let result_color = rng.get(0, 3);

    // first augmentation (stats only)
    let mut stat12 = rng.below(ACC_STAT_SUBBLOCKSIZE);
    let mut stat34 = 0;
    let mut has_skill = false;
    if rng.get(1, 100) <= settings.acc_skill_chance {
        // second augmentation (skill)
        stat34 = base + rng.below(skills_length);
        has_skill = option_has_skill(stat34);
    }

    if !has_skill {
        // second augmentation (stats)
        // calculating any different from stat12 value inside sub-block
        // starting from next and wrapping over using remainder
        stat34 = (stat12 + 1 + rng.below(ACC_STAT_SUBBLOCKSIZE - 1)) % ACC_STAT_SUBBLOCKSIZE;

        // this is a stats - skipping skills
        stat34 += base + skills_length + (ACC_STAT_SUBBLOCKSIZE * result_color);
    }

    // stat12 has stats only
    stat12 += base + skills_length + (ACC_STAT_SUBBLOCKSIZE * result_color);
    stat34.wrapping_shl(16).wrapping_add(stat12)
}

// ---------------------------------------------------------------------------------------------
// The packets
// ---------------------------------------------------------------------------------------------

/// Java `Augment.onCommand` (`bypass Augment 1` opens the augmentation window, `Augment 2` the
/// removal window).
pub fn augment_handler(world: &mut World, player: ObjectId, command: &str, target: Option<ObjectId>) -> bool {
    if !target.is_some_and(|t| world.npcs.contains_key(&t)) {
        return false;
    }
    match command_window(command) {
        Ok(Some(1)) => {
            world.send(player, &ExShowVariationMakeWindow);
            true
        }
        Ok(Some(2)) => {
            world.send(player, &ExShowVariationCancelWindow);
            true
        }
        Ok(_) => false,
        Err(()) => {
            warn!("Exception in Augment: bad command \"{command}\"");
            false
        }
    }
}

/// The number of `Integer.parseInt(command.substring(8, 9).trim())`: the ninth character of the
/// command. `Err` is where Java throws (a command shorter than nine characters, or a character
/// that is no number); a blank character parses as an empty string and throws as well.
fn command_window(command: &str) -> Result<Option<i32>, ()> {
    let ninth: String = command.chars().skip(8).take(1).collect();
    if ninth.is_empty() {
        return Err(());
    }
    ninth.trim().parse::<i32>().map(Some).map_err(|_| ())
}

/// Java `_lifeStones`: every life stone item with its grade and level.
pub fn life_stones() -> impl Iterator<Item = (i32, LifeStone)> {
    LIFE_STONES
        .iter()
        .map(|&(id, grade, level)| (id, LifeStone { grade, level }))
}

impl World {
    /// Installs `AugmentationData` (the skill map and the retail chance tables).
    pub fn set_augmentation_data(&mut self, data: Arc<AugmentationData>) {
        self.augment.data = Some(data);
    }

    /// A packet of the augmentation windows.
    pub(crate) fn on_augment_input(&mut self, id: ObjectId, input: AugmentInput) {
        match input {
            AugmentInput::ConfirmTargetItem { object_id } => {
                self.on_request_confirm_target_item(id, object_id)
            }
            AugmentInput::ConfirmRefinerItem {
                target_item_object_id,
                refiner_item_object_id,
            } => self.on_request_confirm_refiner_item(id, target_item_object_id, refiner_item_object_id),
            AugmentInput::ConfirmGemStone {
                target_item_object_id,
                refiner_item_object_id,
                gemstone_item_object_id,
                gemstone_count,
            } => self.on_request_confirm_gem_stone(
                id,
                (
                    target_item_object_id,
                    refiner_item_object_id,
                    gemstone_item_object_id,
                ),
                gemstone_count,
            ),
            AugmentInput::Refine {
                target_item_object_id,
                refiner_item_object_id,
                gemstone_item_object_id,
                gemstone_count,
            } => self.on_request_refine(
                id,
                (
                    target_item_object_id,
                    refiner_item_object_id,
                    gemstone_item_object_id,
                ),
                gemstone_count,
            ),
            AugmentInput::ConfirmCancelItem { object_id } => {
                self.on_request_confirm_cancel_item(id, object_id)
            }
            AugmentInput::RefineCancel {
                target_item_object_id,
            } => self.on_request_refine_cancel(id, target_item_object_id),
        }
    }

    // ------------------------------------------------------------------ AbstractRefinePacket.isValid

    /// Java `AbstractRefinePacket.isValid(player)`: the conditions of the player. Each refusal but
    /// the last two has a message.
    fn augment_player_valid(&mut self, id: ObjectId) -> bool {
        if self.is_in_store_mode(id) {
            self.send(
                id,
                &SystemMessage::new(
                    SystemMessageId::YouCannotAugmentItemsWhileAPrivateStoreOrPrivateWorkshopIsInOperation,
                ),
            );
            return false;
        }
        if self.players.get(&id).is_some_and(|p| p.trade.active.is_some()) {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouCannotAugmentItemsWhileEngagedInTradeActivities),
            );
            return false;
        }
        if self.is_dead(id) {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouCannotAugmentItemsWhileDead),
            );
            return false;
        }
        if self.is_paralyzed(id) {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouCannotAugmentItemsWhileParalyzed),
            );
            return false;
        }
        // `isFishing()` and `isCursedWeaponEquipped()`: neither exists yet.
        if self.is_sitting(id) {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouCannotAugmentItemsWhileSittingDown),
            );
            return false;
        }
        !(self.is_enchanting(id) || self.is_processing_transaction(id))
    }

    /// Java `AbstractRefinePacket.isValid(player, item)`: the player and the item to augment.
    fn augment_item_valid(&mut self, id: ObjectId, item: &ItemData, templates: &ItemTemplates) -> bool {
        if !self.augment_player_valid(id) {
            return false;
        }
        // The item comes from the player's inventory, so it is theirs.
        if item.is_augmented() {
            return false;
        }
        let Some(template) = templates.get(item.item_id) else {
            return false;
        };
        if template.is_hero_item() {
            return false;
        }
        // `isShadowItem()`: mana >= 0.
        if item.mana_left >= 0 {
            return false;
        }
        if template.is_common() {
            return false;
        }
        if template.is_etc_item() {
            return false;
        }
        // `isTimeLimitedItem()`: time > 0.
        if item.time > 0 {
            return false;
        }
        if template.is_pvp_item() && !self.settings.augment.alt_allow_augment_pvp_items {
            return false;
        }
        // `getCrystalType().isLesser(CrystalType.C)`.
        if template.crystal_type.level() < CrystalType::C.level() {
            return false;
        }
        // Source item can be equipped or in inventory: `inventory_item` finds nothing else.

        if let Some(weapon) = template.weapon() {
            if matches!(weapon.weapon_type, WeaponType::None | WeaponType::FishingRod) {
                return false;
            }
        } else if template.is_armor() {
            // Only accessories can be augmented.
            if !matches!(
                template.body_part,
                BodyPart::BothFingers | BodyPart::BothEars | BodyPart::Neck
            ) {
                return false;
            }
        } else {
            // neither weapon nor armor ?
            return false;
        }

        // blacklist check
        !self.settings.augment.blacklist.contains(&item.item_id)
    }

    /// Java `AbstractRefinePacket.isValid(player, item, refinerItem)`: and the life stone.
    fn augment_refiner_valid(
        &mut self,
        id: ObjectId,
        item: &ItemData,
        refiner: &ItemData,
        templates: &ItemTemplates,
    ) -> bool {
        if !self.augment_item_valid(id, item, templates) {
            return false;
        }
        // Lifestone must be located in inventory.
        if refiner.loc != LOC_INVENTORY {
            return false;
        }
        let Some(ls) = life_stone(refiner.item_id) else {
            return false;
        };
        let Some(template) = templates.get(item.item_id) else {
            return false;
        };
        // weapons can't be augmented with accessory ls
        if template.is_weapon() && ls.grade == GRADE_ACC {
            return false;
        }
        // and accessory can't be augmented with weapon ls
        if template.is_armor() && ls.grade != GRADE_ACC {
            return false;
        }
        // check for level of the lifestone
        let level = self.players.get(&id).map_or(0, |p| p.core.level);
        level >= ls.player_level()
    }

    /// Java `AbstractRefinePacket.isValid(player, item, refinerItem, gemStones)`: and the
    /// gemstones.
    fn augment_gemstone_valid(
        &mut self,
        id: ObjectId,
        item: &ItemData,
        refiner: &ItemData,
        gemstones: &ItemData,
        templates: &ItemTemplates,
    ) -> bool {
        if !self.augment_refiner_valid(id, item, refiner, templates) {
            return false;
        }
        // .. and located in inventory
        if gemstones.loc != LOC_INVENTORY {
            return false;
        }
        let Some(template) = templates.get(item.item_id) else {
            return false;
        };
        let grade = template.crystal_type;
        // Check for item id.
        if gemstone_id(grade) != gemstones.item_id {
            return false;
        }
        // Count must be greater or equal of required number.
        let Some(ls) = life_stone(refiner.item_id) else {
            return false;
        };
        i64::from(gemstone_count(grade, ls.grade)) <= gemstones.count
    }

    /// The three items of a refine packet, each looked up like
    /// `getInventory().getItemByObjectId(objectId)`.
    fn augment_items(
        &self,
        id: ObjectId,
        objects: (i32, i32, i32),
    ) -> Option<(ItemData, ItemData, ItemData)> {
        Some((
            self.inventory_item(id, ObjectId(objects.0))?.clone(),
            self.inventory_item(id, ObjectId(objects.1))?.clone(),
            self.inventory_item(id, ObjectId(objects.2))?.clone(),
        ))
    }

    // ------------------------------------------------------------------ the confirm packets

    /// Java `RequestConfirmTargetItem.runImpl`.
    fn on_request_confirm_target_item(&mut self, id: ObjectId, object_id: i32) {
        let Some(item) = self.inventory_item(id, ObjectId(object_id)).cloned() else {
            return;
        };
        let Some(templates) = self.item_templates() else {
            return;
        };
        if !self.augment_item_valid(id, &item, &templates) {
            // Different system message here.
            if item.is_augmented() {
                self.send(
                    id,
                    &SystemMessage::new(SystemMessageId::OnceAnItemIsAugmentedItCannotBeAugmentedAgain),
                );
                return;
            }
            self.send(id, &SystemMessage::new(SystemMessageId::ThisIsNotASuitableItem));
            return;
        }
        self.send(
            id,
            &ExPutItemResultForVariationMake {
                item_object_id: object_id,
                item_id: item.item_id,
            },
        );
    }

    /// Java `RequestConfirmRefinerItem.runImpl`.
    fn on_request_confirm_refiner_item(
        &mut self,
        id: ObjectId,
        target_object_id: i32,
        refiner_object_id: i32,
    ) {
        let Some(target) = self.inventory_item(id, ObjectId(target_object_id)).cloned() else {
            return;
        };
        let Some(refiner) = self.inventory_item(id, ObjectId(refiner_object_id)).cloned() else {
            return;
        };
        let Some(templates) = self.item_templates() else {
            return;
        };
        if !self.augment_refiner_valid(id, &target, &refiner, &templates) {
            self.send(id, &SystemMessage::new(SystemMessageId::ThisIsNotASuitableItem));
            return;
        }
        let (Some(template), Some(ls)) = (templates.get(target.item_id), life_stone(refiner.item_id)) else {
            return;
        };
        let grade = template.crystal_type;
        self.send(
            id,
            &ExPutIntensiveResultForVariationMake {
                refiner_item_object_id: refiner_object_id,
                life_stone_id: refiner.item_id,
                gemstone_item_id: gemstone_id(grade),
                gemstone_count: i64::from(gemstone_count(grade, ls.grade)),
            },
        );
    }

    /// Java `RequestConfirmGemStone.runImpl`.
    fn on_request_confirm_gem_stone(&mut self, id: ObjectId, objects: (i32, i32, i32), count: i64) {
        let Some((target, refiner, gemstone)) = self.augment_items(id, objects) else {
            return;
        };
        let Some(templates) = self.item_templates() else {
            return;
        };
        // Make sure the item is a gemstone
        if !self.augment_gemstone_valid(id, &target, &refiner, &gemstone, &templates) {
            self.send(id, &SystemMessage::new(SystemMessageId::ThisIsNotASuitableItem));
            return;
        }
        // Check for gemstone count
        let (Some(template), Some(ls)) = (templates.get(target.item_id), life_stone(refiner.item_id)) else {
            return;
        };
        if count != i64::from(gemstone_count(template.crystal_type, ls.grade)) {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::GemstoneQuantityIsIncorrect),
            );
            return;
        }
        self.send(
            id,
            &ExPutCommissionResultForVariationMake {
                gemstone_object_id: objects.2,
                item_id: gemstone.item_id,
                gemstone_count: count,
            },
        );
    }

    // ------------------------------------------------------------------ RequestRefine

    /// The failure of `RequestRefine`: `ExVariationResult(0, 0, 0)` and the message.
    fn refine_failed(&mut self, id: ObjectId) {
        self.send(
            id,
            &ExVariationResult {
                stat12: 0,
                stat34: 0,
                result: 0,
            },
        );
        self.send(
            id,
            &SystemMessage::new(SystemMessageId::AugmentationFailedDueToInappropriateConditions),
        );
    }

    /// Java `RequestRefine.runImpl`.
    fn on_request_refine(&mut self, id: ObjectId, objects: (i32, i32, i32), count: i64) {
        let Some((target, refiner, gemstone)) = self.augment_items(id, objects) else {
            return;
        };
        let Some(templates) = self.item_templates() else {
            return;
        };
        if !self.augment_gemstone_valid(id, &target, &refiner, &gemstone, &templates) {
            self.refine_failed(id);
            return;
        }
        let (Some(template), Some(ls)) = (templates.get(target.item_id), life_stone(refiner.item_id)) else {
            return;
        };
        if count != i64::from(gemstone_count(template.crystal_type, ls.grade)) {
            self.refine_failed(id);
            return;
        }

        // unequip item
        if target.loc == LOC_PAPERDOLL {
            let slot = usize::try_from(target.loc_data).unwrap_or(0);
            let changed = self.unequip_item_in_slot_and_record(id, slot);
            let mut entries = Vec::new();
            for object_id in changed {
                if let Some(entry) = self.modified_entry(id, object_id) {
                    entries.push(entry);
                }
            }
            // Sent inventory update for unequip instantly.
            self.send_inventory_update_now(id, entries);
            self.broadcast_user_info(id);
        }

        // consume the life stone
        if !self.destroy_item_by_object_id(id, refiner.object_id, 1, false) {
            return;
        }
        // consume the gemstones
        if !self.destroy_item_by_object_id(id, gemstone.object_id, count, false) {
            return;
        }

        let Some(tables) = self.augment.data.clone() else {
            return;
        };
        let settings = self.settings.augment.clone();
        let roll = RollInput {
            life_stone_level: ls.level,
            life_stone_grade: ls.grade,
            life_stone_id: refiner.item_id,
            body_part: template.body_part,
            magic_weapon: template.is_magic_weapon(),
        };
        let augmentation = self.with_rng(|world, rng| {
            let option_has_skill = |option_id: i32| {
                world.data.augmentation_option(option_id).is_some_and(|o| {
                    o.active_skill.is_some() || o.passive_skill.is_some() || !o.activation_skills.is_empty()
                })
            };
            generate_random_augmentation(rng, &tables, &settings, roll, &option_has_skill)
        });
        // NOTE(java-quirk): where Java's roll throws (an empty skill list of the skill map) the
        // life stone and the gemstones are already used up and the player hears nothing.
        let Some(augmentation) = augmentation else {
            warn!(
                "RequestRefine: no augmentation could be rolled for life stone {}",
                refiner.item_id
            );
            return;
        };
        self.set_item_augmentation(target.object_id, Some(augmentation));

        let stat12 = 0x0000_FFFF & augmentation;
        let stat34 = augmentation >> 16;
        self.send(
            id,
            &ExVariationResult {
                stat12,
                stat34,
                result: 1,
            },
        );

        // Sent inventory update for destruction instantly.
        if let Some(entry) = self.modified_entry(id, target.object_id) {
            self.send_inventory_update_now(id, vec![entry]);
        }
        self.send_cur_load(id);
    }

    // ------------------------------------------------------------------ removal

    /// Java `RequestConfirmCancelItem.runImpl`.
    fn on_request_confirm_cancel_item(&mut self, id: ObjectId, object_id: i32) {
        let Some(item) = self.inventory_item(id, ObjectId(object_id)).cloned() else {
            return;
        };
        // The item is the player's own: the owner check cannot fail.
        let Some(augmentation) = item.augmentation else {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::AugmentationRemovalCanOnlyBeDoneOnAnAugmentedItem),
            );
            return;
        };
        let Some(templates) = self.item_templates() else {
            return;
        };
        let Some(template) = templates.get(item.item_id) else {
            return;
        };
        if template.is_pvp_item() && !self.settings.augment.alt_allow_augment_pvp_items {
            self.send(id, &SystemMessage::new(SystemMessageId::ThisIsNotASuitableItem));
            return;
        }
        // Java: "TODO: S84 TOP price 3.2M". Any other item type is not augmentable.
        let Some(price) = removal_price(
            template.crystal_type,
            template.crystal_count_at(item.enchant_level),
        ) else {
            return;
        };
        self.send(
            id,
            &ExPutItemResultForVariationCancel {
                item_object_id: object_id,
                item_id: template.display_id,
                // NOTE(java-quirk): `(short) augmentationId` is sign extended.
                aug1: i32::from(augmentation as i16),
                aug2: augmentation >> 16,
                price,
            },
        );
    }

    /// Java `RequestRefineCancel.runImpl`.
    fn on_request_refine_cancel(&mut self, id: ObjectId, object_id: i32) {
        let Some(target) = self.inventory_item(id, ObjectId(object_id)).cloned() else {
            self.send(id, &ExVariationCancelResult { result: 0 });
            return;
        };
        // The item is the player's own: the owner check cannot fail.

        // Cannot remove augmentation from a not augmented item.
        if !target.is_augmented() {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::AugmentationRemovalCanOnlyBeDoneOnAnAugmentedItem),
            );
            self.send(id, &ExVariationCancelResult { result: 0 });
            return;
        }

        // get the price
        let templates = self.item_templates();
        let price = templates
            .as_ref()
            .and_then(|t| t.get(target.item_id))
            .and_then(|template| {
                removal_price(
                    template.crystal_type,
                    template.crystal_count_at(target.enchant_level),
                )
            });
        let Some(price) = price else {
            self.send(id, &ExVariationCancelResult { result: 0 });
            return;
        };

        // try to reduce the players adena
        // NOTE(java-quirk): `reduceAdena(.., true)` already says "not enough adena", and the packet
        // says it again.
        if !self.reduce_adena(id, price, true) {
            self.send(id, &ExVariationCancelResult { result: 0 });
            self.send(id, &SystemMessage::new(SystemMessageId::YouDoNotHaveEnoughAdena));
            return;
        }

        // unequip item
        let mut entries = Vec::new();
        if target.loc == LOC_PAPERDOLL {
            let slot = usize::try_from(target.loc_data).unwrap_or(0);
            for object_id in self.unequip_item_in_slot_and_record(id, slot) {
                if let Some(entry) = self.modified_entry(id, object_id) {
                    entries.push(entry);
                }
            }
        }

        // remove the augmentation
        self.set_item_augmentation(target.object_id, None);

        // send ExVariationCancelResult
        self.send(id, &ExVariationCancelResult { result: 1 });

        // send inventory update
        if let Some(entry) = self.modified_entry(id, target.object_id) {
            entries.push(entry);
        }
        for entry in entries {
            self.send_inventory_update(id, entry);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_augment_command_is_read_from_its_ninth_character() {
        assert_eq!(command_window("Augment 1"), Ok(Some(1)));
        assert_eq!(command_window("Augment 2"), Ok(Some(2)));
        // Anything after the ninth character is ignored.
        assert_eq!(command_window("Augment 2 foo"), Ok(Some(2)));
        assert_eq!(command_window("Augment 9"), Ok(Some(9)));
        // Too short, blank, no digit: Java throws and logs.
        assert_eq!(command_window("Augment"), Err(()));
        assert_eq!(command_window("Augment "), Err(()));
        assert_eq!(command_window("Augment  1"), Err(()));
        assert_eq!(command_window("Augment x"), Err(()));
    }

    #[test]
    fn there_are_87_life_stones_with_distinct_ids() {
        let mut ids: Vec<i32> = life_stones().map(|(id, _)| id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 87);
    }
}
