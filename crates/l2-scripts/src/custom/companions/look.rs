//! How a companion looks: a player of its class (the race and, for the Kamael, the sex come with
//! it), in armor and with a weapon that fit the class and its owner's level. The clients are told
//! it is a player (`CharInfo`), so it shows in the party window and the target window like one.

use l2_world::CompanionLook;

use super::classes::{CompanionClass, Sex};
use super::gear_data::{ARMOR, WEAPONS};

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

/// A random figure of `class` for an owner of `level`. `dice(n)` is a number in `[0, n)` (the
/// world's random numbers, so a seeded world makes the same figures).
pub fn roll(class: &CompanionClass, level: i32, dice: &mut dyn FnMut(i32) -> i32) -> CompanionLook {
    let female = match class.sex {
        Sex::Male => false,
        Sex::Female => true,
        Sex::Any => dice(2) == 0,
    };
    let mut look = CompanionLook {
        name: NAMES[dice(NAMES.len() as i32) as usize % NAMES.len()].to_owned(),
        title: class.role.title().to_owned(),
        race: class.race,
        female,
        class_id: class.id,
        hair: dice(5),
        hair_color: dice(4),
        face: dice(3),
        head: 0,
        right_hand: 0,
        left_hand: 0,
        gloves: 0,
        chest: 0,
        legs: 0,
        feet: 0,
    };
    dress(&mut look, class, level, dice);
    look
}

/// A figure saved at logout ([`Saved`]) as it comes back: its class, its face and name, and the
/// gear of `level`.
pub fn restore(
    saved: &Saved,
    class: &CompanionClass,
    level: i32,
    dice: &mut dyn FnMut(i32) -> i32,
) -> CompanionLook {
    let mut look = CompanionLook {
        name: saved.name.clone(),
        title: String::new(),
        race: class.race,
        female: saved.female,
        class_id: class.id,
        hair: saved.hair,
        hair_color: saved.hair_color,
        face: saved.face,
        head: 0,
        right_hand: 0,
        left_hand: 0,
        gloves: 0,
        chest: 0,
        legs: 0,
        feet: 0,
    };
    dress(&mut look, class, level, dice);
    look
}

/// What is kept of a companion over a logout: its class and its face. The gear follows the level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Saved {
    pub class_id: i32,
    pub name: String,
    pub female: bool,
    pub hair: i32,
    pub hair_color: i32,
    pub face: i32,
}

impl Saved {
    pub fn of(look: &CompanionLook) -> Self {
        Self {
            class_id: look.class_id,
            name: look.name.clone(),
            female: look.female,
            hair: look.hair,
            hair_color: look.hair_color,
            face: look.face,
        }
    }

    /// `class,name,sex,hair,hair color,face` (the names have no commas).
    pub fn to_text(&self) -> String {
        format!(
            "{},{},{},{},{},{}",
            self.class_id,
            self.name,
            u8::from(self.female),
            self.hair,
            self.hair_color,
            self.face
        )
    }

    /// The text of [`Saved::to_text`]; `None` for anything else.
    pub fn from_text(text: &str) -> Option<Self> {
        let parts: Vec<&str> = text.split(',').collect();
        let [class_id, name, female, hair, hair_color, face] = parts.as_slice() else {
            return None;
        };
        let number = |s: &str| s.trim().parse::<i32>().ok();
        let name = name.trim();
        if name.is_empty() {
            return None;
        }
        Some(Self {
            class_id: number(class_id)?,
            name: name.to_owned(),
            female: number(female)? != 0,
            hair: number(hair)?.clamp(0, 6),
            hair_color: number(hair_color)?.clamp(0, 3),
            face: number(face)?.clamp(0, 2),
        })
    }
}

/// Puts `class` and the gear of `level` on a figure; the face, the name and the sex stay (a
/// companion that moves up a class tier, or into a new grade).
pub fn dress(look: &mut CompanionLook, class: &CompanionClass, level: i32, dice: &mut dyn FnMut(i32) -> i32) {
    let grade = grade_for(level);
    let weapons: Vec<i32> = WEAPONS
        .iter()
        .filter(|(key, g, _)| *key == class.weapon.key() && *g == grade)
        .map(|(_, _, id)| *id)
        .collect();
    let sets: Vec<&[i32; 6]> = ARMOR
        .iter()
        .filter(|(g, k, _)| *g == grade && *k == class.armor)
        .map(|(_, _, parts)| parts)
        .collect();
    let [head, chest, legs, gloves, feet, set_shield] = pick(&sets, dice).copied().unwrap_or([0; 6]);
    // A set without a shield (most are) gets one of the grade.
    let shield = if set_shield != 0 {
        set_shield
    } else {
        let shields: Vec<i32> = ARMOR
            .iter()
            .filter(|(g, _, parts)| *g == grade && parts[5] != 0)
            .map(|(_, _, parts)| parts[5])
            .collect();
        pick(&shields, dice).unwrap_or(0)
    };
    look.class_id = class.id;
    look.race = class.race;
    look.title = class.role.title().to_owned();
    look.right_hand = pick(&weapons, dice).unwrap_or(0);
    // A shield for the knights; a two-handed weapon, a bow or a magic one leaves the hand empty.
    look.left_hand = if class.weapon.with_shield() { shield } else { 0 };
    look.head = head;
    look.chest = chest;
    look.legs = legs;
    look.gloves = gloves;
    look.feet = feet;
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
    use crate::custom::companions::classes::{CLASSES, by_id};
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
    fn a_figure_is_its_class_and_wears_what_fits_its_level() {
        let mut dice = counter();
        for class in CLASSES {
            for level in [20, 45, 55, 70, 78, 82, 85] {
                let look = roll(class, level, &mut dice);
                let grade = grade_for(level);
                assert_eq!(
                    (look.class_id, look.race),
                    (class.id, class.race),
                    "{}",
                    class.name
                );
                assert!(
                    WEAPONS.contains(&(class.weapon.key(), grade, look.right_hand)),
                    "{} {level}: weapon {} is not a {} of grade {grade}",
                    class.name,
                    look.right_hand,
                    class.weapon.key()
                );
                assert!(
                    ARMOR
                        .iter()
                        .any(|(g, k, parts)| *g == grade && *k == class.armor && parts[1] == look.chest),
                    "{} {level}: chest {} is not {} of grade {grade}",
                    class.name,
                    look.chest,
                    class.armor
                );
                assert_eq!(
                    look.left_hand != 0,
                    class.weapon.with_shield(),
                    "{}: the shield",
                    class.name
                );
                assert_eq!(look.title, class.role.title());
                match class.sex {
                    Sex::Male => assert!(!look.female, "{}", class.name),
                    Sex::Female => assert!(look.female, "{}", class.name),
                    Sex::Any => {}
                }
                assert!((0..5).contains(&look.hair) && (0..4).contains(&look.hair_color));
                assert!((0..3).contains(&look.face));
                assert!(NAMES.contains(&look.name.as_str()));
            }
        }
    }

    #[test]
    fn a_new_class_gets_new_gear_and_keeps_the_face() {
        let mut dice = counter();
        let monk = by_id(47).unwrap();
        let mut look = roll(monk, 30, &mut dice);
        let before = look.clone();
        dress(&mut look, by_id(48).unwrap(), 45, &mut dice);
        assert_eq!(look.class_id, 48);
        assert_eq!(
            (&look.name, look.female, look.hair, look.hair_color, look.face),
            (
                &before.name,
                before.female,
                before.hair,
                before.hair_color,
                before.face
            )
        );
        assert!(
            WEAPONS.contains(&("fist", 1, look.right_hand)),
            "a C grade fist: {}",
            look.right_hand
        );
    }

    #[test]
    fn a_saved_figure_comes_back_with_the_gear_of_the_new_level() {
        let mut dice = counter();
        let look = roll(by_id(48).unwrap(), 45, &mut dice);
        let saved = Saved::of(&look);
        let text = saved.to_text();
        assert_eq!(Saved::from_text(&text), Some(saved.clone()));
        let back = restore(&saved, by_id(48).unwrap(), 55, &mut dice);
        assert_eq!(
            (&back.name, back.female, back.hair, back.face, back.class_id),
            (&look.name, look.female, look.hair, look.face, 48)
        );
        assert!(WEAPONS.contains(&("fist", 2, back.right_hand)), "B grade at 55");
        for bad in ["", "48,Amy", "x,Amy,0,1,1,1", "48,,0,1,1,1", "48,Amy,0,1,1,1,9"] {
            assert_eq!(Saved::from_text(bad), None, "{bad}");
        }
    }

    #[test]
    fn the_sexes_and_weapons_vary_and_the_same_dice_make_the_same_figure() {
        let mut dice = counter();
        let gladiator = by_id(2).unwrap();
        let looks: Vec<CompanionLook> = (0..40).map(|_| roll(gladiator, 70, &mut dice)).collect();
        assert!(looks.iter().any(|l| l.female) && looks.iter().any(|l| !l.female));
        let weapons: BTreeSet<i32> = looks.iter().map(|l| l.right_hand).collect();
        assert!(weapons.len() > 1, "the weapons vary");
        let (mut a, mut b) = (counter(), counter());
        assert_eq!(roll(gladiator, 50, &mut a), roll(gladiator, 50, &mut b));
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
