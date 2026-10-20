//! The parts of Java's `PlayerClass` enum that skill learning reads: the constant name, the parent
//! (`getParent`), mage or fighter (`isMage`), the depth in the class tree (`level`) and
//! `equalsOrChildOf`.
//!
//! Java: `gameserver/entity/actor/enums/player/PlayerClass.java`.
//!
//! NOTE(java-quirk): the enum's parents are not always the ones `classList.xml` names (Bladedancer
//! and Elemental Master differ), and skill trees take their own parent chain from
//! `skillTrees/*.xml`. Skill learning (`NpcTemplate.canTeach`, the `SkillList` bypass) uses the
//! enum, so this table is a copy of the enum, not of the class list.

use crate::class_list::ClassId;

/// `(id, enum constant name, isMage, parent id)` of every `PlayerClass`, in id order.
const PLAYER_CLASSES: [(u16, &str, bool, Option<u16>); 103] = [
    (0, "FIGHTER", false, None),
    (1, "WARRIOR", false, Some(0)),
    (2, "GLADIATOR", false, Some(1)),
    (3, "WARLORD", false, Some(1)),
    (4, "KNIGHT", false, Some(0)),
    (5, "PALADIN", false, Some(4)),
    (6, "DARK_AVENGER", false, Some(4)),
    (7, "ROGUE", false, Some(0)),
    (8, "TREASURE_HUNTER", false, Some(7)),
    (9, "HAWKEYE", false, Some(7)),
    (10, "MAGE", true, None),
    (11, "WIZARD", true, Some(10)),
    (12, "SORCERER", true, Some(11)),
    (13, "NECROMANCER", true, Some(11)),
    (14, "WARLOCK", true, Some(11)),
    (15, "CLERIC", true, Some(10)),
    (16, "BISHOP", true, Some(15)),
    (17, "PROPHET", true, Some(15)),
    (18, "ELVEN_FIGHTER", false, None),
    (19, "ELVEN_KNIGHT", false, Some(18)),
    (20, "TEMPLE_KNIGHT", false, Some(19)),
    (21, "SWORDSINGER", false, Some(19)),
    (22, "ELVEN_SCOUT", false, Some(18)),
    (23, "PLAINS_WALKER", false, Some(22)),
    (24, "SILVER_RANGER", false, Some(22)),
    (25, "ELVEN_MAGE", true, None),
    (26, "ELVEN_WIZARD", true, Some(25)),
    (27, "SPELLSINGER", true, Some(26)),
    (28, "ELEMENTAL_SUMMONER", true, Some(26)),
    (29, "ORACLE", true, Some(25)),
    (30, "ELDER", true, Some(29)),
    (31, "DARK_FIGHTER", false, None),
    (32, "PALUS_KNIGHT", false, Some(31)),
    (33, "SHILLIEN_KNIGHT", false, Some(32)),
    (34, "BLADEDANCER", false, Some(32)),
    (35, "ASSASSIN", false, Some(31)),
    (36, "ABYSS_WALKER", false, Some(35)),
    (37, "PHANTOM_RANGER", false, Some(35)),
    (38, "DARK_MAGE", true, None),
    (39, "DARK_WIZARD", true, Some(38)),
    (40, "SPELLHOWLER", true, Some(39)),
    (41, "PHANTOM_SUMMONER", true, Some(39)),
    (42, "SHILLIEN_ORACLE", true, Some(38)),
    (43, "SHILLIEN_ELDER", true, Some(42)),
    (44, "ORC_FIGHTER", false, None),
    (45, "ORC_RAIDER", false, Some(44)),
    (46, "DESTROYER", false, Some(45)),
    (47, "ORC_MONK", false, Some(44)),
    (48, "TYRANT", false, Some(47)),
    (49, "ORC_MAGE", true, None),
    (50, "ORC_SHAMAN", true, Some(49)),
    (51, "OVERLORD", true, Some(50)),
    (52, "WARCRYER", true, Some(50)),
    (53, "DWARVEN_FIGHTER", false, None),
    (54, "SCAVENGER", false, Some(53)),
    (55, "BOUNTY_HUNTER", false, Some(54)),
    (56, "ARTISAN", false, Some(53)),
    (57, "WARSMITH", false, Some(56)),
    (88, "DUELIST", false, Some(2)),
    (89, "DREADNOUGHT", false, Some(3)),
    (90, "PHOENIX_KNIGHT", false, Some(5)),
    (91, "HELL_KNIGHT", false, Some(6)),
    (92, "SAGITTARIUS", false, Some(9)),
    (93, "ADVENTURER", false, Some(8)),
    (94, "ARCHMAGE", true, Some(12)),
    (95, "SOULTAKER", true, Some(13)),
    (96, "ARCANA_LORD", true, Some(14)),
    (97, "CARDINAL", true, Some(16)),
    (98, "HIEROPHANT", true, Some(17)),
    (99, "EVA_TEMPLAR", false, Some(20)),
    (100, "SWORD_MUSE", false, Some(21)),
    (101, "WIND_RIDER", false, Some(23)),
    (102, "MOONLIGHT_SENTINEL", false, Some(24)),
    (103, "MYSTIC_MUSE", true, Some(27)),
    (104, "ELEMENTAL_MASTER", true, Some(28)),
    (105, "EVA_SAINT", true, Some(30)),
    (106, "SHILLIEN_TEMPLAR", false, Some(33)),
    (107, "SPECTRAL_DANCER", false, Some(34)),
    (108, "GHOST_HUNTER", false, Some(36)),
    (109, "GHOST_SENTINEL", false, Some(37)),
    (110, "STORM_SCREAMER", true, Some(40)),
    (111, "SPECTRAL_MASTER", true, Some(41)),
    (112, "SHILLIEN_SAINT", true, Some(43)),
    (113, "TITAN", false, Some(46)),
    (114, "GRAND_KHAVATARI", false, Some(48)),
    (115, "DOMINATOR", true, Some(51)),
    (116, "DOOMCRYER", true, Some(52)),
    (117, "FORTUNE_SEEKER", false, Some(55)),
    (118, "MAESTRO", false, Some(57)),
    (123, "MALE_SOLDIER", false, None),
    (124, "FEMALE_SOLDIER", false, None),
    (125, "TROOPER", false, Some(123)),
    (126, "WARDER", false, Some(124)),
    (127, "BERSERKER", false, Some(125)),
    (128, "MALE_SOULBREAKER", false, Some(125)),
    (129, "FEMALE_SOULBREAKER", false, Some(126)),
    (130, "ARBALESTER", false, Some(126)),
    (131, "DOOMBRINGER", false, Some(127)),
    (132, "MALE_SOUL_HOUND", false, Some(128)),
    (133, "FEMALE_SOUL_HOUND", false, Some(129)),
    (134, "TRICKSTER", false, Some(130)),
    (135, "INSPECTOR", false, Some(126)),
    (136, "JUDICATOR", false, Some(135)),
];

impl ClassId {
    fn player_class_entry(self) -> Option<&'static (u16, &'static str, bool, Option<u16>)> {
        PLAYER_CLASSES
            .binary_search_by_key(&self.0, |entry| entry.0)
            .ok()
            .and_then(|index| PLAYER_CLASSES.get(index))
    }

    /// Java `PlayerClass.name()` (`toString()` is not overridden), e.g. `FIGHTER`. `None` for an id
    /// that is not a `PlayerClass`.
    pub fn enum_name(self) -> Option<&'static str> {
        self.player_class_entry().map(|entry| entry.1)
    }

    /// Java `PlayerClass.isMage()`. `false` for an id that is not a `PlayerClass`.
    pub fn is_mage(self) -> bool {
        self.player_class_entry().is_some_and(|entry| entry.2)
    }

    /// Java `PlayerClass.getParent()`: the class this one evolves from, `None` for a root class.
    pub fn enum_parent(self) -> Option<ClassId> {
        self.player_class_entry().and_then(|entry| entry.3).map(ClassId)
    }

    /// Java `PlayerClass.level()`: 0 for a root class, 1 for the first class change, and so on.
    pub fn class_level(self) -> i32 {
        let mut level = 0;
        let mut current = self.enum_parent();
        while let Some(parent) = current {
            level += 1;
            current = parent.enum_parent();
        }
        level
    }

    /// Java `PlayerClass.equalsOrChildOf(other)`.
    pub fn equals_or_child_of(self, other: ClassId) -> bool {
        let mut current = Some(self);
        while let Some(class) = current {
            if class == other {
                return true;
            }
            current = class.enum_parent();
        }
        false
    }

    /// True when the id is in Java's `PlayerClass` enum.
    pub fn is_player_class(self) -> bool {
        self.player_class_entry().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_entry_is_a_class_with_a_race_and_the_table_is_sorted() {
        for pair in PLAYER_CLASSES.windows(2) {
            assert!(pair[0].0 < pair[1].0);
        }
        for &(id, _, _, parent) in &PLAYER_CLASSES {
            assert!(ClassId(id).race().is_some(), "class {id} has no race");
            if let Some(parent) = parent {
                assert!(ClassId(parent).is_player_class(), "class {id}: parent {parent}");
            }
        }
    }

    #[test]
    fn levels_follow_the_parent_chain() {
        assert_eq!(ClassId(0).class_level(), 0); // Human Fighter
        assert_eq!(ClassId(1).class_level(), 1); // Warrior
        assert_eq!(ClassId(2).class_level(), 2); // Gladiator
        assert_eq!(ClassId(88).class_level(), 3); // Duelist
        assert_eq!(ClassId(88).enum_parent(), Some(ClassId(2)));
        assert_eq!(ClassId(123).class_level(), 0); // Male Soldier
    }

    #[test]
    fn mage_flag_and_names() {
        assert!(!ClassId(0).is_mage());
        assert!(ClassId(10).is_mage()); // Human Mystic
        assert!(ClassId(15).is_mage()); // Cleric
        assert_eq!(ClassId(0).enum_name(), Some("FIGHTER"));
        assert_eq!(ClassId(60).enum_name(), None);
    }

    #[test]
    fn equals_or_child_of() {
        let (fighter, warrior, gladiator, knight) = (ClassId(0), ClassId(1), ClassId(2), ClassId(4));
        assert!(gladiator.equals_or_child_of(gladiator));
        assert!(gladiator.equals_or_child_of(warrior));
        assert!(gladiator.equals_or_child_of(fighter));
        assert!(!gladiator.equals_or_child_of(knight));
        assert!(!fighter.equals_or_child_of(warrior));
    }

    #[test]
    fn the_enum_parent_differs_from_the_class_list_for_two_classes() {
        // Java's enum: Bladedancer (34) under 32, Elemental Master (104) under 28.
        assert_eq!(ClassId(34).enum_parent(), Some(ClassId(32)));
        assert_eq!(ClassId(104).enum_parent(), Some(ClassId(28)));
    }
}
