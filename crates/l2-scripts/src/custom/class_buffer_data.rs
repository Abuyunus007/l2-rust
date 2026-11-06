//! The groups of the class buffer: which buffs, songs and dances a class gets.
//!
//! One group per way of playing: every class belongs to exactly one. The levels are the highest
//! the skill data has (a Might of level 1 would be a joke next to the one a Prophet casts). A group
//! never holds two skills of the same abnormal type, so each one stays on after the others landed,
//! and it fits the buff slots: at most `MaxBuffAmount` (20) buffs and `MaxDanceAmount` (12) songs
//! and dances. The test below the script checks all of that against the datapack.

use l2_world::script::class_is_mage;

/// A skill of a group: the id, the level, and the name the pages show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Buff {
    pub id: i32,
    pub level: i32,
    pub name: &'static str,
}

impl Buff {
    const fn new(id: i32, level: i32, name: &'static str) -> Self {
        Self { id, level, name }
    }
}

/// The buffs of one kind of class.
#[derive(Debug, Clone, Copy)]
pub struct Group {
    /// Used in the page links.
    pub key: &'static str,
    pub title: &'static str,
    pub about: &'static str,
    /// The player classes (`ClassId`) that belong here.
    pub classes: &'static [u16],
    pub buffs: &'static [Buff],
    /// Songs and dances: they have their own slots.
    pub songs: &'static [Buff],
}

impl Group {
    /// Every skill of the group, buffs first.
    pub fn all(&self) -> impl Iterator<Item = &'static Buff> {
        self.buffs.iter().chain(self.songs)
    }
}

/// Java `PlayerClass.isMage()` is the split the server already uses (the scheme buffer's auto buff
/// does too); a class this table does not know follows it.
pub fn group_of_class(class_id: i32) -> &'static Group {
    let fallback = if class_is_mage(class_id) {
        "mage"
    } else {
        "warrior"
    };
    u16::try_from(class_id)
        .ok()
        .and_then(|id| GROUPS.iter().find(|g| g.classes.contains(&id)))
        .or_else(|| GROUPS.iter().find(|g| g.key == fallback))
        .unwrap_or(&GROUPS[0])
}

/// A group by the key of a page link.
pub fn group_by_key(key: &str) -> Option<&'static Group> {
    GROUPS.iter().find(|g| g.key == key)
}

const WARRIOR_BUFFS: &[Buff] = &[
    Buff::new(1068, 3, "Might"),
    Buff::new(1086, 2, "Haste"),
    Buff::new(1077, 3, "Focus"),
    Buff::new(1242, 3, "Death Whisper"),
    Buff::new(1240, 3, "Guidance"),
    Buff::new(1062, 2, "Berserker Spirit"),
    Buff::new(1045, 6, "Bless the Body"),
    Buff::new(1040, 3, "Shield"),
    Buff::new(1036, 2, "Magic Barrier"),
    Buff::new(1035, 4, "Mental Shield"),
    Buff::new(1259, 4, "Resist Shock"),
    Buff::new(1204, 2, "Wind Walk"),
    Buff::new(1268, 4, "Vampiric Rage"),
    Buff::new(1044, 3, "Regeneration"),
    Buff::new(1243, 6, "Bless Shield"),
    Buff::new(1304, 3, "Advanced Block"),
    Buff::new(1356, 1, "Prophecy of Fire"),
];

const WARRIOR_SONGS: &[Buff] = &[
    Buff::new(271, 1, "Dance of the Warrior"),
    Buff::new(275, 1, "Dance of Fury"),
    Buff::new(274, 1, "Dance of Fire"),
    Buff::new(269, 1, "Song of Hunter"),
    Buff::new(364, 1, "Song of Champion"),
    Buff::new(264, 1, "Song of Earth"),
    Buff::new(267, 1, "Song of Warding"),
    Buff::new(304, 1, "Song of Vitality"),
    Buff::new(349, 1, "Song of Renewal"),
    Buff::new(310, 1, "Dance of the Vampire"),
    Buff::new(268, 1, "Song of Wind"),
    Buff::new(265, 1, "Song of Life"),
];

const TANK_BUFFS: &[Buff] = &[
    Buff::new(1040, 3, "Shield"),
    Buff::new(1036, 2, "Magic Barrier"),
    Buff::new(1045, 6, "Bless the Body"),
    Buff::new(1035, 4, "Mental Shield"),
    Buff::new(1259, 4, "Resist Shock"),
    Buff::new(1044, 3, "Regeneration"),
    Buff::new(1243, 6, "Bless Shield"),
    Buff::new(1304, 3, "Advanced Block"),
    Buff::new(1204, 2, "Wind Walk"),
    Buff::new(1068, 3, "Might"),
    Buff::new(1086, 2, "Haste"),
    Buff::new(1354, 1, "Arcane Protection"),
    Buff::new(1352, 1, "Elemental Protection"),
    Buff::new(1353, 1, "Divine Protection"),
    Buff::new(1284, 3, "Chant of Revenge"),
    Buff::new(1032, 3, "Invigor"),
    Buff::new(1033, 3, "Resist Poison"),
    Buff::new(1356, 1, "Prophecy of Fire"),
];

const TANK_SONGS: &[Buff] = &[
    Buff::new(264, 1, "Song of Earth"),
    Buff::new(267, 1, "Song of Warding"),
    Buff::new(304, 1, "Song of Vitality"),
    Buff::new(265, 1, "Song of Life"),
    Buff::new(349, 1, "Song of Renewal"),
    Buff::new(305, 1, "Song of Vengeance"),
    Buff::new(271, 1, "Dance of the Warrior"),
    Buff::new(529, 1, "Song of Elemental"),
    Buff::new(530, 1, "Dance of Alignment"),
    Buff::new(310, 1, "Dance of the Vampire"),
    Buff::new(268, 1, "Song of Wind"),
];

const DAGGER_BUFFS: &[Buff] = &[
    Buff::new(1068, 3, "Might"),
    Buff::new(1086, 2, "Haste"),
    Buff::new(1077, 3, "Focus"),
    Buff::new(1242, 3, "Death Whisper"),
    Buff::new(1240, 3, "Guidance"),
    Buff::new(1062, 2, "Berserker Spirit"),
    Buff::new(1087, 3, "Agility"),
    Buff::new(1204, 2, "Wind Walk"),
    Buff::new(1268, 4, "Vampiric Rage"),
    Buff::new(1045, 6, "Bless the Body"),
    Buff::new(1040, 3, "Shield"),
    Buff::new(1036, 2, "Magic Barrier"),
    Buff::new(1035, 4, "Mental Shield"),
    Buff::new(1259, 4, "Resist Shock"),
    Buff::new(1044, 3, "Regeneration"),
    Buff::new(1356, 1, "Prophecy of Fire"),
];

const DAGGER_SONGS: &[Buff] = &[
    Buff::new(271, 1, "Dance of the Warrior"),
    Buff::new(275, 1, "Dance of Fury"),
    Buff::new(274, 1, "Dance of Fire"),
    Buff::new(269, 1, "Song of Hunter"),
    Buff::new(364, 1, "Song of Champion"),
    Buff::new(310, 1, "Dance of the Vampire"),
    Buff::new(264, 1, "Song of Earth"),
    Buff::new(267, 1, "Song of Warding"),
    Buff::new(304, 1, "Song of Vitality"),
    Buff::new(349, 1, "Song of Renewal"),
    Buff::new(268, 1, "Song of Wind"),
    Buff::new(272, 1, "Dance of Inspiration"),
];

const ARCHER_BUFFS: &[Buff] = &[
    Buff::new(1068, 3, "Might"),
    Buff::new(1086, 2, "Haste"),
    Buff::new(1077, 3, "Focus"),
    Buff::new(1242, 3, "Death Whisper"),
    Buff::new(1240, 3, "Guidance"),
    Buff::new(1062, 2, "Berserker Spirit"),
    Buff::new(1204, 2, "Wind Walk"),
    Buff::new(1045, 6, "Bless the Body"),
    Buff::new(1040, 3, "Shield"),
    Buff::new(1036, 2, "Magic Barrier"),
    Buff::new(1035, 4, "Mental Shield"),
    Buff::new(1259, 4, "Resist Shock"),
    Buff::new(1044, 3, "Regeneration"),
    Buff::new(1356, 1, "Prophecy of Fire"),
];

const ARCHER_SONGS: &[Buff] = &[
    Buff::new(271, 1, "Dance of the Warrior"),
    Buff::new(275, 1, "Dance of Fury"),
    Buff::new(274, 1, "Dance of Fire"),
    Buff::new(272, 1, "Dance of Inspiration"),
    Buff::new(269, 1, "Song of Hunter"),
    Buff::new(364, 1, "Song of Champion"),
    Buff::new(264, 1, "Song of Earth"),
    Buff::new(267, 1, "Song of Warding"),
    Buff::new(304, 1, "Song of Vitality"),
    Buff::new(349, 1, "Song of Renewal"),
    Buff::new(268, 1, "Song of Wind"),
    Buff::new(265, 1, "Song of Life"),
];

const MAGE_BUFFS: &[Buff] = &[
    Buff::new(1059, 3, "Empower"),
    Buff::new(1085, 3, "Acumen"),
    Buff::new(1303, 2, "Wild Magic"),
    Buff::new(1078, 6, "Concentration"),
    Buff::new(1062, 2, "Berserker Spirit"),
    Buff::new(1045, 6, "Bless the Body"),
    Buff::new(1048, 6, "Bless the Soul"),
    Buff::new(1040, 3, "Shield"),
    Buff::new(1036, 2, "Magic Barrier"),
    Buff::new(1035, 4, "Mental Shield"),
    Buff::new(1259, 4, "Resist Shock"),
    Buff::new(1204, 2, "Wind Walk"),
    Buff::new(1397, 3, "Clarity"),
    Buff::new(1355, 1, "Prophecy of Water"),
];

const MAGE_SONGS: &[Buff] = &[
    Buff::new(273, 1, "Dance of the Mystic"),
    Buff::new(276, 1, "Dance of Concentration"),
    Buff::new(365, 1, "Dance of Siren"),
    Buff::new(264, 1, "Song of Earth"),
    Buff::new(267, 1, "Song of Warding"),
    Buff::new(268, 1, "Song of Wind"),
    Buff::new(304, 1, "Song of Vitality"),
    Buff::new(349, 1, "Song of Renewal"),
    Buff::new(363, 1, "Song of Meditation"),
    Buff::new(265, 1, "Song of Life"),
];

const HEALER_BUFFS: &[Buff] = &[
    Buff::new(1085, 3, "Acumen"),
    Buff::new(1078, 6, "Concentration"),
    Buff::new(1397, 3, "Clarity"),
    Buff::new(1048, 6, "Bless the Soul"),
    Buff::new(1045, 6, "Bless the Body"),
    Buff::new(1059, 3, "Empower"),
    Buff::new(1040, 3, "Shield"),
    Buff::new(1036, 2, "Magic Barrier"),
    Buff::new(1035, 4, "Mental Shield"),
    Buff::new(1259, 4, "Resist Shock"),
    Buff::new(1204, 2, "Wind Walk"),
    Buff::new(1354, 1, "Arcane Protection"),
    Buff::new(1413, 1, "Magnus' Chant"),
];

const HEALER_SONGS: &[Buff] = &[
    Buff::new(363, 1, "Song of Meditation"),
    Buff::new(276, 1, "Dance of Concentration"),
    Buff::new(267, 1, "Song of Warding"),
    Buff::new(264, 1, "Song of Earth"),
    Buff::new(304, 1, "Song of Vitality"),
    Buff::new(349, 1, "Song of Renewal"),
    Buff::new(268, 1, "Song of Wind"),
    Buff::new(265, 1, "Song of Life"),
    Buff::new(273, 1, "Dance of the Mystic"),
];

/// What a player's summon or pet gets with the group: the fighting buffs, whatever the owner is.
pub const PET_BUFFS: &[Buff] = &[
    Buff::new(1068, 3, "Might"),
    Buff::new(1086, 2, "Haste"),
    Buff::new(1077, 3, "Focus"),
    Buff::new(1242, 3, "Death Whisper"),
    Buff::new(1240, 3, "Guidance"),
    Buff::new(1204, 2, "Wind Walk"),
    Buff::new(1045, 6, "Bless the Body"),
    Buff::new(1040, 3, "Shield"),
    Buff::new(1036, 2, "Magic Barrier"),
    Buff::new(1035, 4, "Mental Shield"),
];

pub const PET_SONGS: &[Buff] = &[
    Buff::new(271, 1, "Dance of the Warrior"),
    Buff::new(275, 1, "Dance of Fury"),
    Buff::new(274, 1, "Dance of Fire"),
    Buff::new(269, 1, "Song of Hunter"),
    Buff::new(264, 1, "Song of Earth"),
    Buff::new(267, 1, "Song of Warding"),
    Buff::new(304, 1, "Song of Vitality"),
    Buff::new(349, 1, "Song of Renewal"),
];

pub const GROUPS: [Group; 6] = [
    Group {
        key: "warrior",
        title: "Warrior",
        about: "Melee fighters: more damage, faster attacks, critical hits and enough HP to stand in front.",
        classes: &[
            0, 1, 2, 3, 18, 21, 31, 34, 44, 45, 46, 47, 48, 53, 54, 55, 56, 57, 88, 89, 100, 107, 113, 114,
            117, 118, 123, 124, 125, 127, 128, 129, 131, 132, 133, 135, 136,
        ],
        buffs: WARRIOR_BUFFS,
        songs: WARRIOR_SONGS,
    },
    Group {
        key: "tank",
        title: "Tank",
        about: "Knights: defence, HP, resistances and shield buffs, with enough damage to keep the enemy busy.",
        classes: &[4, 5, 6, 19, 20, 32, 33, 90, 91, 99, 106],
        buffs: TANK_BUFFS,
        songs: TANK_SONGS,
    },
    Group {
        key: "dagger",
        title: "Dagger",
        about: "Rogues and assassins: critical rate and damage, attack speed, evasion and life steal.",
        classes: &[7, 8, 22, 23, 35, 36, 93, 101, 108],
        buffs: DAGGER_BUFFS,
        songs: DAGGER_SONGS,
    },
    Group {
        key: "archer",
        title: "Archer",
        about: "Bows and crossbows: damage, accuracy, attack speed and critical hits from a distance.",
        classes: &[9, 24, 37, 92, 102, 109, 126, 130, 134],
        buffs: ARCHER_BUFFS,
        songs: ARCHER_SONGS,
    },
    Group {
        key: "mage",
        title: "Mage and Summoner",
        about: "Casters and summoners: magic damage, casting speed, magic crits, MP and protection while casting.",
        classes: &[
            10, 11, 12, 13, 14, 25, 26, 27, 28, 38, 39, 40, 41, 49, 50, 51, 52, 94, 95, 96, 103, 104, 110,
            111, 115, 116,
        ],
        buffs: MAGE_BUFFS,
        songs: MAGE_SONGS,
    },
    Group {
        key: "healer",
        title: "Healer and Support",
        about: "Clerics and oracles: stronger heals, faster casting, more MP and protection from interrupts.",
        classes: &[15, 16, 17, 29, 30, 42, 43, 97, 98, 105, 112],
        buffs: HEALER_BUFFS,
        songs: HEALER_SONGS,
    },
];
