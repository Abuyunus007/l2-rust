//! The class categories of `categoryData.xml` the class change scripts ask about
//! (`player.isInCategory(type)`, `CategoryData.isInCategory(type, classId)`).
// Java: data/xml/CategoryData.java, entity/actor/Creature.java (isInCategory), Player.getId()
//
// `CategoryData` is not loaded as a whole yet: only the categories of player class ids that the
// village master, class master and certification scripts read are kept here, copied from the
// datapack (a test compares them with the reference file). Any other category answers `false`,
// like Java's `isInCategory` for a category the file does not list.

use l2_data::stats::CategoryType;
use l2_util::ObjectId;

use crate::world::World;

/// `FIRST_CLASS_GROUP`.
const FIRST_CLASS_GROUP: [u16; 11] = [0, 10, 18, 25, 31, 38, 44, 49, 53, 123, 124];
/// `SECOND_CLASS_GROUP`.
const SECOND_CLASS_GROUP: [u16; 20] = [
    1, 4, 7, 11, 15, 19, 22, 26, 29, 32, 35, 39, 42, 45, 47, 50, 54, 56, 125, 126,
];
/// `THIRD_CLASS_GROUP`.
const THIRD_CLASS_GROUP: [u16; 36] = [
    2, 3, 5, 6, 8, 9, 12, 13, 14, 16, 17, 20, 21, 23, 24, 27, 28, 30, 33, 34, 36, 37, 40, 41, 43, 46, 48, 51,
    52, 55, 57, 127, 128, 129, 130, 135,
];
/// `FOURTH_CLASS_GROUP`.
const FOURTH_CLASS_GROUP: [u16; 36] = [
    88, 89, 90, 91, 92, 93, 94, 95, 96, 97, 98, 99, 100, 101, 102, 103, 104, 105, 106, 107, 108, 109, 110,
    111, 112, 113, 114, 115, 116, 117, 118, 131, 132, 133, 134, 136,
];
/// `FIGHTER_GROUP`.
const FIGHTER_GROUP: [u16; 66] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 18, 19, 20, 21, 22, 23, 24, 31, 32, 33, 34, 35, 36, 37, 44, 45, 46, 47, 48,
    53, 54, 55, 56, 57, 88, 89, 90, 91, 92, 93, 99, 100, 101, 102, 106, 107, 108, 109, 113, 114, 117, 118,
    123, 124, 125, 126, 127, 128, 129, 130, 131, 132, 133, 134, 135, 136,
];
/// `MAGE_GROUP`.
const MAGE_GROUP: [u16; 37] = [
    10, 11, 12, 13, 14, 15, 16, 17, 25, 26, 27, 28, 29, 30, 38, 39, 40, 41, 42, 43, 49, 50, 51, 52, 94, 95,
    96, 97, 98, 103, 104, 105, 110, 111, 112, 115, 116,
];
/// `WIZARD_GROUP`.
const WIZARD_GROUP: [u16; 26] = [
    10, 11, 12, 13, 14, 25, 26, 27, 28, 38, 39, 40, 41, 94, 95, 96, 103, 104, 110, 111, 49, 50, 51, 52, 115,
    116,
];
/// `CLERIC_GROUP`.
const CLERIC_GROUP: [u16; 13] = [10, 15, 16, 17, 25, 29, 30, 38, 42, 43, 97, 98, 105];
/// `WARSMITH_GROUP`.
const WARSMITH_GROUP: [u16; 4] = [53, 56, 57, 118];
/// `BOUNTY_HUNTER_GROUP`.
const BOUNTY_HUNTER_GROUP: [u16; 4] = [53, 54, 55, 117];
/// `HUMAN_FALL_CLASS`.
const HUMAN_FALL_CLASS: [u16; 16] = [0, 1, 4, 7, 2, 3, 5, 6, 8, 9, 88, 89, 90, 91, 92, 93];
/// `HUMAN_MALL_CLASS` (the file lists 10 twice).
const HUMAN_MALL_CLASS: [u16; 14] = [10, 11, 12, 13, 14, 10, 15, 16, 17, 94, 95, 96, 97, 98];
/// `HUMAN_CALL_CLASS`.
const HUMAN_CALL_CLASS: [u16; 6] = [10, 15, 16, 17, 97, 98];
/// `ELF_FALL_CLASS`.
const ELF_FALL_CLASS: [u16; 11] = [18, 19, 22, 20, 21, 23, 24, 99, 100, 101, 102];
/// `ELF_MALL_CLASS` (the file lists 25 twice).
const ELF_MALL_CLASS: [u16; 10] = [25, 26, 27, 28, 25, 29, 30, 103, 104, 105];
/// `ELF_CALL_CLASS`.
const ELF_CALL_CLASS: [u16; 4] = [25, 29, 30, 105];
/// `ORC_FALL_CLASS`.
const ORC_FALL_CLASS: [u16; 7] = [44, 45, 47, 46, 48, 113, 114];
/// `ORC_MALL_CLASS`.
const ORC_MALL_CLASS: [u16; 6] = [49, 50, 51, 52, 115, 116];
/// `KAMAEL_FIRST_CLASS_GROUP`.
const KAMAEL_FIRST_CLASS_GROUP: [u16; 2] = [123, 124];
/// `KAMAEL_SECOND_CLASS_GROUP`.
const KAMAEL_SECOND_CLASS_GROUP: [u16; 2] = [125, 126];
/// `KAMAEL_THIRD_CLASS_GROUP`.
const KAMAEL_THIRD_CLASS_GROUP: [u16; 5] = [127, 128, 129, 130, 135];
/// `KAMAEL_FOURTH_CLASS_GROUP`.
const KAMAEL_FOURTH_CLASS_GROUP: [u16; 5] = [131, 132, 133, 134, 136];
/// `KAMAEL_MALE_MAIN_OCCUPATION`.
const KAMAEL_MALE_MAIN_OCCUPATION: [u16; 10] = [123, 125, 127, 128, 131, 132, 129, 133, 135, 136];
/// `KAMAEL_FEMALE_MAIN_OCCUPATION`.
const KAMAEL_FEMALE_MAIN_OCCUPATION: [u16; 10] = [124, 126, 129, 130, 133, 134, 128, 132, 135, 136];
/// `SUB_GROUP_WARRIOR`.
const SUB_GROUP_WARRIOR: [u16; 18] = [
    3, 127, 128, 129, 46, 55, 57, 48, 2, 89, 131, 113, 117, 118, 132, 133, 114, 88,
];
/// `SUB_GROUP_ROGUE`.
const SUB_GROUP_ROGUE: [u16; 14] = [8, 23, 36, 93, 101, 108, 9, 24, 37, 92, 102, 109, 130, 134];
/// `SUB_GROUP_KNIGHT`.
const SUB_GROUP_KNIGHT: [u16; 8] = [5, 6, 33, 20, 90, 91, 106, 99];
/// `SUB_GROUP_SUMMONER`.
const SUB_GROUP_SUMMONER: [u16; 6] = [14, 28, 41, 96, 104, 111];
/// `SUB_GROUP_WIZARD`.
const SUB_GROUP_WIZARD: [u16; 8] = [12, 27, 40, 13, 94, 103, 110, 95];
/// `SUB_GROUP_HEALER`.
const SUB_GROUP_HEALER: [u16; 6] = [16, 30, 43, 97, 105, 112];
/// `SUB_GROUP_ENCHANTER`.
const SUB_GROUP_ENCHANTER: [u16; 12] = [17, 51, 52, 21, 34, 135, 98, 115, 116, 100, 107, 136];

/// The ids of a category this module keeps, `None` for any other category.
pub fn category_ids(category: CategoryType) -> Option<&'static [u16]> {
    Some(match category {
        CategoryType::FirstClassGroup => &FIRST_CLASS_GROUP,
        CategoryType::SecondClassGroup => &SECOND_CLASS_GROUP,
        CategoryType::ThirdClassGroup => &THIRD_CLASS_GROUP,
        CategoryType::FourthClassGroup => &FOURTH_CLASS_GROUP,
        CategoryType::FighterGroup => &FIGHTER_GROUP,
        CategoryType::MageGroup => &MAGE_GROUP,
        CategoryType::WizardGroup => &WIZARD_GROUP,
        CategoryType::ClericGroup => &CLERIC_GROUP,
        CategoryType::WarsmithGroup => &WARSMITH_GROUP,
        CategoryType::BountyHunterGroup => &BOUNTY_HUNTER_GROUP,
        CategoryType::HumanFallClass => &HUMAN_FALL_CLASS,
        CategoryType::HumanMallClass => &HUMAN_MALL_CLASS,
        CategoryType::HumanCallClass => &HUMAN_CALL_CLASS,
        CategoryType::ElfFallClass => &ELF_FALL_CLASS,
        CategoryType::ElfMallClass => &ELF_MALL_CLASS,
        CategoryType::ElfCallClass => &ELF_CALL_CLASS,
        CategoryType::OrcFallClass => &ORC_FALL_CLASS,
        CategoryType::OrcMallClass => &ORC_MALL_CLASS,
        CategoryType::KamaelFirstClassGroup => &KAMAEL_FIRST_CLASS_GROUP,
        CategoryType::KamaelSecondClassGroup => &KAMAEL_SECOND_CLASS_GROUP,
        CategoryType::KamaelThirdClassGroup => &KAMAEL_THIRD_CLASS_GROUP,
        CategoryType::KamaelFourthClassGroup => &KAMAEL_FOURTH_CLASS_GROUP,
        CategoryType::KamaelMaleMainOccupation => &KAMAEL_MALE_MAIN_OCCUPATION,
        CategoryType::KamaelFemaleMainOccupation => &KAMAEL_FEMALE_MAIN_OCCUPATION,
        CategoryType::SubGroupWarrior => &SUB_GROUP_WARRIOR,
        CategoryType::SubGroupRogue => &SUB_GROUP_ROGUE,
        CategoryType::SubGroupKnight => &SUB_GROUP_KNIGHT,
        CategoryType::SubGroupSummoner => &SUB_GROUP_SUMMONER,
        CategoryType::SubGroupWizard => &SUB_GROUP_WIZARD,
        CategoryType::SubGroupHealer => &SUB_GROUP_HEALER,
        CategoryType::SubGroupEnchanter => &SUB_GROUP_ENCHANTER,
        _ => return None,
    })
}

/// Java `CategoryData.getInstance().isInCategory(type, id)` for a class id.
pub fn class_in_category(category: CategoryType, class_id: i32) -> bool {
    let Some(ids) = category_ids(category) else {
        return false;
    };
    u16::try_from(class_id).is_ok_and(|id| ids.contains(&id))
}

impl World {
    /// Java `player.isInCategory(type)`: the category of the active class (`Player.getId()` is
    /// `getPlayerClass().getId()`).
    pub fn is_in_category(&self, id: ObjectId, category: CategoryType) -> bool {
        self.players
            .get(&id)
            .is_some_and(|p| class_in_category(category, p.data.class_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every kept category against the reference datapack; skipped when it is not there.
    #[test]
    fn the_kept_categories_match_category_data() {
        let path = std::path::Path::new(r"C:\l2-rust\l2jmobius\dist\game\data\categoryData.xml");
        let Ok(text) = std::fs::read_to_string(path) else {
            eprintln!("skipped: {} does not exist", path.display());
            return;
        };
        let mut checked = 0;
        for &category in CategoryType::ALL {
            let Some(ids) = category_ids(category) else {
                continue;
            };
            let head = format!("<category name=\"{}\">", category.name());
            let start = text.find(&head).unwrap();
            let block = &text[start..start + text[start..].find("</category>").unwrap()];
            let listed: Vec<u16> = block
                .split("<id>")
                .skip(1)
                .map(|part| part.split("</id>").next().unwrap().trim().parse().unwrap())
                .collect();
            assert_eq!(ids, listed.as_slice(), "{}", category.name());
            checked += 1;
        }
        assert_eq!(checked, 31);
    }

    #[test]
    fn a_class_is_in_its_groups_only() {
        assert!(class_in_category(CategoryType::FirstClassGroup, 0));
        assert!(!class_in_category(CategoryType::SecondClassGroup, 0));
        assert!(class_in_category(CategoryType::ThirdClassGroup, 135));
        assert!(class_in_category(CategoryType::KamaelFemaleMainOccupation, 128));
        // A category this module does not keep.
        assert!(!class_in_category(CategoryType::AttackerGroup, 0));
        assert!(!class_in_category(CategoryType::FirstClassGroup, -1));
    }
}
