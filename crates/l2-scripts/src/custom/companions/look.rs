//! How a companion looks: a player of a random race and sex, in armor and with a weapon that fit
//! its owner's level and its role. The clients are told it is a player (`CharInfo`), so it shows
//! in the party window and the target window like one.

use l2_world::CompanionLook;

use super::gear_data::{ARMOR, WEAPONS};
use super::role::Role;

/// `(race, class id)` a role can be shown as. Races: 0 human, 1 elf, 2 dark elf, 3 orc, 4 dwarf.
/// The class is only what the party window and the target window write; the companion's skills
/// are the role's.
fn figures(role: Role) -> &'static [(i32, i32)] {
    match role {
        // Phoenix Knight, Eva's Templar, Shillien Templar, Titan, Fortune Seeker.
        Role::Tank => &[(0, 90), (1, 99), (2, 106), (3, 113), (4, 117)],
        // Cardinal, Eva's Saint, Shillien Saint: the races that have healers.
        Role::Healer => &[(0, 97), (1, 105), (2, 112)],
        // Duelist, Dreadnought, Sword Muse, Spectral Dancer, Titan, Fortune Seeker.
        Role::Warrior => &[(0, 88), (0, 89), (1, 100), (2, 107), (3, 113), (4, 117)],
        // Sagittarius, Moonlight Sentinel, Ghost Sentinel: the races that have archers.
        Role::Archer => &[(0, 92), (1, 102), (2, 109)],
    }
}

/// The names a companion is given.
const NAMES: [&str; 48] = [
    "Aldric", "Brenna", "Caelum", "Darian", "Elowen", "Faramir", "Gwyneth", "Halvard", "Isolde", "Jorund",
    "Kaelith", "Lorcan", "Mirabel", "Nerys", "Orlan", "Perrin", "Quillon", "Rowena", "Soren", "Talia",
    "Ulric", "Vesper", "Wynna", "Xander", "Ysolde", "Zephyr", "Arlen", "Briar", "Corwin", "Dara", "Eldon",
    "Fenna", "Garrick", "Hesper", "Ilyas", "Jessa", "Kellen", "Lyra", "Marek", "Neve", "Osric", "Petra",
    "Rurik", "Sable", "Tobin", "Una", "Varek", "Wren",
];

/// The grades in the order of `gear_data`: D, C, B, A, S, S80, S84.
const GRADE_FROM_LEVEL: [i32; 6] = [40, 52, 61, 76, 80, 84];

/// The index of the grade of gear a player of `level` wears (retail's level limits: D from 20,
/// C 40, B 52, A 61, S 76, S80 80, S84 84).
pub fn grade_for(level: i32) -> usize {
    GRADE_FROM_LEVEL.iter().take_while(|from| level >= **from).count()
}

/// The armor kind of a role. A warrior takes either heavy or light armor.
fn armor_kind(role: Role, dice: &mut dyn FnMut(i32) -> i32) -> &'static str {
    match role {
        Role::Tank => "HEAVY",
        Role::Healer => "MAGIC",
        Role::Archer => "LIGHT",
        Role::Warrior => {
            if dice(2) == 0 {
                "HEAVY"
            } else {
                "LIGHT"
            }
        }
    }
}

/// A random figure for a companion of `role` for an owner of `level`. `dice(n)` is a number in
/// `[0, n)` (the world's random numbers, so a seeded world makes the same figures).
pub fn roll(role: Role, level: i32, dice: &mut dyn FnMut(i32) -> i32) -> CompanionLook {
    let figures = figures(role);
    let (race, class_id) = figures[dice(figures.len() as i32) as usize % figures.len()];
    let grade = grade_for(level);

    let weapons: Vec<i32> = WEAPONS
        .iter()
        .filter(|(key, g, _)| *key == role.key() && *g == grade)
        .map(|(_, _, id)| *id)
        .collect();
    let right_hand = pick(&weapons, dice);

    let kind = armor_kind(role, dice);
    let sets: Vec<&[i32; 6]> = ARMOR
        .iter()
        .filter(|(g, k, _)| *g == grade && *k == kind)
        .map(|(_, _, parts)| parts)
        .collect();
    let [head, chest, legs, gloves, feet, set_shield] = pick(&sets, dice).copied().unwrap_or([0; 6]);
    // A set without a shield (some of the heavy ones have none) gets one of the grade.
    let shield = if set_shield != 0 {
        set_shield
    } else {
        let shields: Vec<i32> = ARMOR
            .iter()
            .filter(|(g, k, parts)| *g == grade && *k == "HEAVY" && parts[5] != 0)
            .map(|(_, _, parts)| parts[5])
            .collect();
        pick(&shields, dice).unwrap_or(0)
    };

    CompanionLook {
        name: NAMES[dice(NAMES.len() as i32) as usize % NAMES.len()].to_owned(),
        title: role.title().to_owned(),
        race,
        female: dice(2) == 0,
        class_id,
        hair: dice(5),
        hair_color: dice(4),
        face: dice(3),
        head,
        right_hand: right_hand.unwrap_or(0),
        // Only the tank carries a shield; a two-handed weapon or a bow leaves the hand empty.
        left_hand: if role == Role::Tank { shield } else { 0 },
        gloves,
        chest,
        legs,
        feet,
    }
}

/// A random entry of a list.
fn pick<T: Copy>(list: &[T], dice: &mut dyn FnMut(i32) -> i32) -> Option<T> {
    if list.is_empty() {
        return None;
    }
    list.get(dice(list.len() as i32) as usize).copied()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::testing::datapack;

    /// A xorshift as dice: deterministic, and it spreads over the values.
    fn counter() -> impl FnMut(i32) -> i32 {
        let mut state: u32 = 0x2545_F491;
        move |bound| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            (state % bound.max(1) as u32) as i32
        }
    }

    #[test]
    fn the_grade_follows_the_levels_of_retail() {
        let grade = |level| grade_for(level);
        assert_eq!([grade(20), grade(39)], [0, 0], "D");
        assert_eq!([grade(40), grade(51)], [1, 1], "C");
        assert_eq!([grade(52), grade(60)], [2, 2], "B");
        assert_eq!([grade(61), grade(75)], [3, 3], "A");
        assert_eq!([grade(76), grade(79)], [4, 4], "S");
        assert_eq!(grade(80), 5, "S80");
        assert_eq!([grade(84), grade(99)], [6, 6], "S84");
    }

    #[test]
    fn every_role_has_a_weapon_and_armor_for_every_grade() {
        for role in Role::ALL {
            for grade in 0..=6 {
                assert!(
                    WEAPONS.iter().any(|(k, g, _)| *k == role.key() && *g == grade),
                    "{role:?} has no weapon of grade {grade}"
                );
            }
        }
        for grade in 0..=6 {
            for kind in ["HEAVY", "LIGHT", "MAGIC"] {
                assert!(
                    ARMOR.iter().any(|(g, k, _)| *g == grade && *k == kind),
                    "no {kind} armor of grade {grade}"
                );
            }
        }
    }

    #[test]
    fn a_figure_wears_what_fits_its_level_and_role() {
        for role in Role::ALL {
            for level in [20, 45, 55, 70, 78, 82, 85] {
                let mut dice = counter();
                for _ in 0..30 {
                    let look = roll(role, level, &mut dice);
                    let grade = grade_for(level);
                    assert!(
                        WEAPONS.contains(&(role.key(), grade, look.right_hand)),
                        "{role:?} {level}: weapon {} is not of grade {grade}",
                        look.right_hand
                    );
                    assert!(
                        ARMOR
                            .iter()
                            .any(|(g, _, parts)| *g == grade && parts[1] == look.chest),
                        "{role:?} {level}: chest {} is not of grade {grade}",
                        look.chest
                    );
                    assert!(look.chest != 0, "a companion is never naked");
                    assert_eq!(look.title, role.title());
                    assert!(
                        figures(role).contains(&(look.race, look.class_id)),
                        "{role:?} cannot be shown as race {} class {}",
                        look.race,
                        look.class_id
                    );
                    assert!((0..5).contains(&look.hair) && (0..4).contains(&look.hair_color));
                    assert!((0..3).contains(&look.face));
                    assert!(NAMES.contains(&look.name.as_str()));
                }
            }
        }
    }

    #[test]
    fn only_the_tank_carries_a_shield_and_the_bow_has_both_hands() {
        let mut dice = counter();
        for level in [30, 60, 85] {
            let tank = roll(Role::Tank, level, &mut dice);
            assert!(tank.left_hand != 0, "a tank holds a shield at level {level}");
            for role in [Role::Healer, Role::Warrior, Role::Archer] {
                assert_eq!(roll(role, level, &mut dice).left_hand, 0, "{role:?}");
            }
        }
    }

    #[test]
    fn the_races_and_sexes_vary_and_the_same_dice_make_the_same_figure() {
        let mut dice = counter();
        let looks: Vec<CompanionLook> = (0..60).map(|_| roll(Role::Warrior, 70, &mut dice)).collect();
        let races: BTreeSet<i32> = looks.iter().map(|l| l.race).collect();
        assert!(races.len() >= 4, "{races:?}");
        assert!(looks.iter().any(|l| l.female) && looks.iter().any(|l| !l.female));
        assert!(
            looks.iter().any(|l| l.right_hand != looks[0].right_hand),
            "the weapons vary"
        );

        let (mut a, mut b) = (counter(), counter());
        assert_eq!(roll(Role::Archer, 50, &mut a), roll(Role::Archer, 50, &mut b));
    }

    #[test]
    fn every_item_of_the_tables_exists_in_the_datapack() {
        let Some(root) = datapack() else {
            println!("skipped: no datapack");
            return;
        };
        let mut known: BTreeSet<i32> = BTreeSet::new();
        for entry in std::fs::read_dir(root.join("data/stats/items"))
            .unwrap()
            .flatten()
        {
            if entry.path().extension().is_none_or(|e| e != "xml") {
                continue;
            }
            let text = std::fs::read_to_string(entry.path()).unwrap();
            for part in text.split("<item id=\"").skip(1) {
                if let Some(id) = part.split('"').next().and_then(|s| s.parse().ok()) {
                    known.insert(id);
                }
            }
        }
        for (_, _, id) in WEAPONS {
            assert!(known.contains(id), "weapon {id} is not in the datapack");
        }
        for (_, _, parts) in ARMOR {
            for id in parts.iter().filter(|id| **id != 0) {
                assert!(known.contains(id), "armor piece {id} is not in the datapack");
            }
        }
    }
}
