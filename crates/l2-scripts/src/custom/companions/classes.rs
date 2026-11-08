//! The classes a companion can be: every 1st, 2nd and 3rd class of High Five, with the role it
//! plays, the weapon it holds and the armor it wears. A player hires the classes of his own class
//! level ([`tier_for`]): from 20 the 1st classes, from 40 the 2nd, from 76 the 3rd. A companion
//! moves up with its owner ([`successor`]).
//!
//! Names, parents and the tiers are those of `data/stats/players/classList.xml` (a test checks).

use super::role::Role;

/// The weapon a class holds; the keys of `gear_data::WEAPONS`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Weapon {
    /// A one-handed sword and a shield.
    Shield,
    /// A two-handed sword.
    TwoHand,
    Dual,
    Pole,
    Fist,
    /// A blunt weapon for fighting (not a magic one).
    Blunt,
    Dagger,
    Bow,
    Crossbow,
    Ancient,
    Rapier,
    /// A staff or another magic weapon.
    Magic,
    /// A one-handed (magic) blunt weapon of a healer.
    Mace,
}

impl Weapon {
    /// The key in `gear_data::WEAPONS`.
    pub fn key(self) -> &'static str {
        match self {
            Weapon::Shield => "tank",
            Weapon::TwoHand => "warrior",
            Weapon::Dual => "dual",
            Weapon::Pole => "pole",
            Weapon::Fist => "fist",
            Weapon::Blunt => "blunt",
            Weapon::Dagger => "dagger",
            Weapon::Bow => "archer",
            Weapon::Crossbow => "crossbow",
            Weapon::Ancient => "ancient",
            Weapon::Rapier => "rapier",
            Weapon::Magic => "mage",
            Weapon::Mace => "healer",
        }
    }

    /// Does the class carry a shield in the other hand?
    pub fn with_shield(self) -> bool {
        self == Weapon::Shield
    }
}

/// Who may be the class (the Kamael classes are of one sex).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sex {
    Any,
    Male,
    Female,
}

/// One class a companion can be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompanionClass {
    pub id: i32,
    pub name: &'static str,
    /// 0 human, 1 elf, 2 dark elf, 3 orc, 4 dwarf, 5 kamael.
    pub race: i32,
    /// 1, 2 or 3.
    pub tier: u8,
    pub parent: i32,
    pub role: Role,
    pub weapon: Weapon,
    /// `HEAVY`, `LIGHT` or `MAGIC`.
    pub armor: &'static str,
    pub sex: Sex,
}

#[allow(clippy::too_many_arguments)]
const fn class(
    id: i32,
    name: &'static str,
    race: i32,
    tier: u8,
    parent: i32,
    role: Role,
    weapon: Weapon,
    armor: &'static str,
    sex: Sex,
) -> CompanionClass {
    CompanionClass {
        id,
        name,
        race,
        tier,
        parent,
        role,
        weapon,
        armor,
        sex,
    }
}

use Role::{Archer, Buffer, Dagger, Healer, Mage, Summoner, Tank, Warrior};
use Sex::{Any, Female, Male};
use Weapon::{
    Ancient, Blunt, Bow, Crossbow, Dagger as Knife, Dual, Fist, Mace, Magic, Pole, Rapier, Shield, TwoHand,
};

const H: &str = "HEAVY";
const L: &str = "LIGHT";
const M: &str = "MAGIC";

/// Every class, grouped by race and line.
#[rustfmt::skip]
pub const CLASSES: &[CompanionClass] = &[
    // ---- Human ----
    class(1, "Warrior", 0, 1, 0, Warrior, TwoHand, H, Any),
    class(2, "Gladiator", 0, 2, 1, Warrior, Dual, H, Any),
    class(88, "Duelist", 0, 3, 2, Warrior, Dual, H, Any),
    class(3, "Warlord", 0, 2, 1, Warrior, Pole, H, Any),
    class(89, "Dreadnought", 0, 3, 3, Warrior, Pole, H, Any),
    class(4, "Human Knight", 0, 1, 0, Tank, Shield, H, Any),
    class(5, "Paladin", 0, 2, 4, Tank, Shield, H, Any),
    class(90, "Phoenix Knight", 0, 3, 5, Tank, Shield, H, Any),
    class(6, "Dark Avenger", 0, 2, 4, Tank, Shield, H, Any),
    class(91, "Hell Knight", 0, 3, 6, Tank, Shield, H, Any),
    class(7, "Rogue", 0, 1, 0, Dagger, Knife, L, Any),
    class(8, "Treasure Hunter", 0, 2, 7, Dagger, Knife, L, Any),
    class(93, "Adventurer", 0, 3, 8, Dagger, Knife, L, Any),
    class(9, "Hawkeye", 0, 2, 7, Archer, Bow, L, Any),
    class(92, "Sagittarius", 0, 3, 9, Archer, Bow, L, Any),
    class(11, "Human Wizard", 0, 1, 10, Mage, Magic, M, Any),
    class(12, "Sorcerer", 0, 2, 11, Mage, Magic, M, Any),
    class(94, "Archmage", 0, 3, 12, Mage, Magic, M, Any),
    class(13, "Necromancer", 0, 2, 11, Mage, Magic, M, Any),
    class(95, "Soultaker", 0, 3, 13, Mage, Magic, M, Any),
    class(14, "Warlock", 0, 2, 11, Summoner, Magic, M, Any),
    class(96, "Arcana Lord", 0, 3, 14, Summoner, Magic, M, Any),
    class(15, "Cleric", 0, 1, 10, Healer, Mace, M, Any),
    class(16, "Bishop", 0, 2, 15, Healer, Mace, M, Any),
    class(97, "Cardinal", 0, 3, 16, Healer, Mace, M, Any),
    class(17, "Prophet", 0, 2, 15, Buffer, Mace, M, Any),
    class(98, "Hierophant", 0, 3, 17, Buffer, Mace, M, Any),
    // ---- Elf ----
    class(19, "Elven Knight", 1, 1, 18, Tank, Shield, H, Any),
    class(20, "Temple Knight", 1, 2, 19, Tank, Shield, H, Any),
    class(99, "Eva's Templar", 1, 3, 20, Tank, Shield, H, Any),
    class(21, "Sword Singer", 1, 2, 19, Buffer, TwoHand, H, Any),
    class(100, "Sword Muse", 1, 3, 21, Buffer, TwoHand, H, Any),
    class(22, "Elven Scout", 1, 1, 18, Dagger, Knife, L, Any),
    class(23, "Plains Walker", 1, 2, 22, Dagger, Knife, L, Any),
    class(101, "Wind Rider", 1, 3, 23, Dagger, Knife, L, Any),
    class(24, "Silver Ranger", 1, 2, 22, Archer, Bow, L, Any),
    class(102, "Moonlight Sentinel", 1, 3, 24, Archer, Bow, L, Any),
    class(26, "Elven Wizard", 1, 1, 25, Mage, Magic, M, Any),
    class(27, "Spellsinger", 1, 2, 26, Mage, Magic, M, Any),
    class(103, "Mystic Muse", 1, 3, 27, Mage, Magic, M, Any),
    class(28, "Elemental Summoner", 1, 2, 26, Summoner, Magic, M, Any),
    class(104, "Elemental Master", 1, 3, 28, Summoner, Magic, M, Any),
    class(29, "Elven Oracle", 1, 1, 25, Healer, Mace, M, Any),
    class(30, "Elven Elder", 1, 2, 29, Healer, Mace, M, Any),
    class(105, "Eva's Saint", 1, 3, 30, Healer, Mace, M, Any),
    // ---- Dark elf ----
    class(32, "Palus Knight", 2, 1, 31, Tank, Shield, H, Any),
    class(33, "Shillien Knight", 2, 2, 32, Tank, Shield, H, Any),
    class(106, "Shillien Templar", 2, 3, 33, Tank, Shield, H, Any),
    class(34, "Bladedancer", 2, 2, 32, Buffer, Dual, H, Any),
    class(107, "Spectral Dancer", 2, 3, 34, Buffer, Dual, H, Any),
    class(35, "Assassin", 2, 1, 31, Dagger, Knife, L, Any),
    class(36, "Abyss Walker", 2, 2, 35, Dagger, Knife, L, Any),
    class(108, "Ghost Hunter", 2, 3, 36, Dagger, Knife, L, Any),
    class(37, "Phantom Ranger", 2, 2, 35, Archer, Bow, L, Any),
    class(109, "Ghost Sentinel", 2, 3, 37, Archer, Bow, L, Any),
    class(39, "Dark Wizard", 2, 1, 38, Mage, Magic, M, Any),
    class(40, "Spellhowler", 2, 2, 39, Mage, Magic, M, Any),
    class(110, "Storm Screamer", 2, 3, 40, Mage, Magic, M, Any),
    class(41, "Phantom Summoner", 2, 2, 39, Summoner, Magic, M, Any),
    class(111, "Spectral Master", 2, 3, 41, Summoner, Magic, M, Any),
    class(42, "Shillien Oracle", 2, 1, 38, Healer, Mace, M, Any),
    class(43, "Shillien Elder", 2, 2, 42, Healer, Mace, M, Any),
    class(112, "Shillien Saint", 2, 3, 43, Healer, Mace, M, Any),
    // ---- Orc ----
    class(45, "Orc Raider", 3, 1, 44, Warrior, TwoHand, H, Any),
    class(46, "Destroyer", 3, 2, 45, Warrior, TwoHand, H, Any),
    class(113, "Titan", 3, 3, 46, Warrior, TwoHand, H, Any),
    class(47, "Monk", 3, 1, 44, Warrior, Fist, L, Any),
    class(48, "Tyrant", 3, 2, 47, Warrior, Fist, L, Any),
    class(114, "Grand Khavatari", 3, 3, 48, Warrior, Fist, L, Any),
    class(50, "Orc Shaman", 3, 1, 49, Buffer, Mace, M, Any),
    class(51, "Overlord", 3, 2, 50, Buffer, Mace, M, Any),
    class(115, "Dominator", 3, 3, 51, Buffer, Mace, M, Any),
    class(52, "Warcryer", 3, 2, 50, Buffer, Mace, M, Any),
    class(116, "Doom Cryer", 3, 3, 52, Buffer, Mace, M, Any),
    // ---- Dwarf ----
    class(54, "Scavenger", 4, 1, 53, Warrior, Blunt, H, Any),
    class(55, "Bounty Hunter", 4, 2, 54, Warrior, Blunt, H, Any),
    class(117, "Fortune Seeker", 4, 3, 55, Warrior, Blunt, H, Any),
    class(56, "Artisan", 4, 1, 53, Warrior, Blunt, H, Any),
    class(57, "Warsmith", 4, 2, 56, Warrior, Blunt, H, Any),
    class(118, "Maestro", 4, 3, 57, Warrior, Blunt, H, Any),
    // ---- Kamael ----
    class(125, "Trooper", 5, 1, 123, Warrior, Ancient, L, Male),
    class(127, "Berserker", 5, 2, 125, Warrior, Ancient, L, Male),
    class(131, "Doombringer", 5, 3, 127, Warrior, Ancient, L, Male),
    class(128, "Male Soul Breaker", 5, 2, 125, Mage, Rapier, L, Male),
    class(132, "Male Soul Hound", 5, 3, 128, Mage, Rapier, L, Male),
    class(126, "Warder", 5, 1, 124, Archer, Crossbow, L, Female),
    class(130, "Arbalester", 5, 2, 126, Archer, Crossbow, L, Female),
    class(134, "Trickster", 5, 3, 130, Archer, Crossbow, L, Female),
    class(129, "Female Soul Breaker", 5, 2, 126, Mage, Rapier, L, Female),
    class(133, "Female Soul Hound", 5, 3, 129, Mage, Rapier, L, Female),
    class(135, "Inspector", 5, 2, 126, Buffer, Rapier, L, Any),
    class(136, "Judicator", 5, 3, 135, Buffer, Rapier, L, Any),
];

/// The levels the class tiers start at: 1st class from 20, 2nd from 40, 3rd from 76.
const TIER_FROM: [i32; 3] = [20, 40, 76];

/// The class tier of a player of `level`; 0 below 20 (no class to hire).
pub fn tier_for(level: i32) -> u8 {
    TIER_FROM.iter().take_while(|from| level >= **from).count() as u8
}

/// The classes a player of `level` can hire, in the order of [`CLASSES`].
pub fn choices(level: i32) -> Vec<&'static CompanionClass> {
    let tier = tier_for(level);
    CLASSES.iter().filter(|c| c.tier == tier).collect()
}

pub fn by_id(id: i32) -> Option<&'static CompanionClass> {
    CLASSES.iter().find(|c| c.id == id)
}

/// The class `class` becomes at `tier`: itself when it is there already (or higher), else the
/// next class down its line, one tier at a time. Of two next classes, those of the same role
/// win; `dice(n)` (in `[0, n)`) picks among what is left. `None`: the line ends (never for the
/// classes of [`CLASSES`]).
pub fn successor(
    class: &'static CompanionClass,
    tier: u8,
    dice: &mut dyn FnMut(i32) -> i32,
) -> Option<&'static CompanionClass> {
    let mut current = class;
    while current.tier < tier {
        let children: Vec<&'static CompanionClass> =
            CLASSES.iter().filter(|c| c.parent == current.id).collect();
        let same_role: Vec<&'static CompanionClass> = children
            .iter()
            .copied()
            .filter(|c| c.role == current.role)
            .collect();
        let pool = if same_role.is_empty() { children } else { same_role };
        if pool.is_empty() {
            return None;
        }
        let pick = usize::try_from(dice(pool.len() as i32)).unwrap_or(0) % pool.len();
        current = pool[pick];
    }
    Some(current)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use std::collections::{BTreeMap, BTreeSet};

    use super::*;
    use crate::custom::companions::gear_data::{ARMOR, WEAPONS};
    use crate::testing::datapack;

    #[test]
    fn the_names_parents_and_tiers_are_those_of_the_class_list() {
        let Some(root) = datapack() else { return };
        let text = std::fs::read_to_string(root.join("data/stats/players/classList.xml")).unwrap();
        let mut list: BTreeMap<i32, (String, Option<i32>)> = BTreeMap::new();
        for line in text.lines().filter(|l| l.contains("<class ")) {
            let attr = |name: &str| {
                line.split(&format!("{name}=\""))
                    .nth(1)
                    .and_then(|r| r.split('"').next())
                    .map(str::to_owned)
            };
            let id: i32 = attr("classId").unwrap().parse().unwrap();
            list.insert(
                id,
                (
                    attr("name").unwrap(),
                    attr("parentClassId").map(|p| p.parse().unwrap()),
                ),
            );
        }
        let depth = |mut id: i32| {
            let mut tier = 0;
            while let Some((_, Some(parent))) = list.get(&id) {
                id = *parent;
                tier += 1;
            }
            tier
        };
        for class in CLASSES {
            let (name, parent) = list
                .get(&class.id)
                .unwrap_or_else(|| panic!("{} is not a class", class.id));
            assert_eq!(name, class.name, "{}", class.id);
            assert_eq!(*parent, Some(class.parent), "{}", class.name);
            assert_eq!(depth(class.id), class.tier, "{}", class.name);
        }
        // Every 1st, 2nd and 3rd class of High Five is there, once.
        let all: BTreeSet<i32> = list
            .keys()
            .copied()
            .filter(|id| (1..=3).contains(&depth(*id)))
            .collect();
        let ours: BTreeSet<i32> = CLASSES.iter().map(|c| c.id).collect();
        assert_eq!(ours.len(), CLASSES.len());
        assert_eq!(ours, all);
    }

    #[test]
    fn a_player_sees_the_classes_of_his_tier() {
        assert_eq!(
            [
                tier_for(19),
                tier_for(20),
                tier_for(39),
                tier_for(40),
                tier_for(75),
                tier_for(76),
                tier_for(85)
            ],
            [0, 1, 1, 2, 2, 3, 3]
        );
        assert!(choices(19).is_empty());
        let names = |level| choices(level).iter().map(|c| c.name).collect::<Vec<_>>();
        assert!(names(23).contains(&"Artisan") && names(23).contains(&"Monk"));
        assert!(!names(23).contains(&"Tyrant"));
        assert!(names(50).contains(&"Tyrant") && names(80).contains(&"Grand Khavatari"));
        for level in [20, 45, 80] {
            assert!(choices(level).iter().all(|c| c.tier == tier_for(level)));
        }
    }

    #[test]
    fn a_class_moves_up_its_own_line_and_keeps_its_role_where_it_can() {
        let mut first = |_: i32| 0;
        let monk = by_id(47).unwrap();
        assert_eq!(successor(monk, 1, &mut first).unwrap().name, "Monk");
        assert_eq!(successor(monk, 2, &mut first).unwrap().name, "Tyrant");
        assert_eq!(successor(monk, 3, &mut first).unwrap().name, "Grand Khavatari");
        // A Rogue (dagger) becomes a Treasure Hunter, never a Hawkeye (bow).
        assert_eq!(
            successor(by_id(7).unwrap(), 3, &mut |_| 1).unwrap().name,
            "Adventurer"
        );
        // A Human Wizard stays a mage: Sorcerer or Necromancer, by the dice; not a Warlock.
        let mut seen = BTreeSet::new();
        for roll in 0..4 {
            seen.insert(successor(by_id(11).unwrap(), 2, &mut |_| roll).unwrap().name);
        }
        assert_eq!(seen, BTreeSet::from(["Sorcerer", "Necromancer"]));
        // A 3rd class stays.
        assert_eq!(successor(by_id(114).unwrap(), 3, &mut first).unwrap().id, 114);
        // Every class reaches the 3rd tier.
        for class in CLASSES {
            let top = successor(class, 3, &mut first).unwrap();
            assert_eq!(top.tier, 3, "{}", class.name);
            assert_eq!(top.race, class.race);
        }
    }

    #[test]
    fn every_class_has_gear_of_every_grade() {
        for class in CLASSES {
            for grade in 0..=6 {
                assert!(
                    WEAPONS
                        .iter()
                        .any(|(k, g, _)| *k == class.weapon.key() && *g == grade),
                    "{}: no {} of grade {grade}",
                    class.name,
                    class.weapon.key()
                );
                assert!(
                    ARMOR.iter().any(|(g, k, _)| *g == grade && *k == class.armor),
                    "{}: no {} armor of grade {grade}",
                    class.name,
                    class.armor
                );
            }
        }
    }
}
