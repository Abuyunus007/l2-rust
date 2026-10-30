//! `NpcBuffersData.xml` (Java `NpcBuffersData`), copied into the code like `hellbound/data.rs`.
// Java: dist/game/data/scripts/ai/others/NpcBuffers/NpcBuffersData.xml

/// Java `AffectScope` (the two the file uses).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AffectScope {
    Party,
    Range,
}

/// Java `AffectObject` (the two the file uses).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AffectObject {
    Friend,
    NotFriend,
}

/// One `<skill>`: skill id and level, `delay` in seconds, scope and object.
// NOTE(java-quirk): Java reads the initial delay from `skillInitDelay`, which no `<skill>` has
// (the file says `initialDelay`), so the first run is always at once.
pub type BufferSkill = (i32, i32, i32, AffectScope, AffectObject);

/// `(npc id, skills)`.
pub const NPC_BUFFERS: &[(i32, &[BufferSkill])] = &[
    (106, &[(22250, 1, 60, AffectScope::Party, AffectObject::Friend)]),
    (139, &[(22035, 1, 60, AffectScope::Party, AffectObject::Friend)]),
    (143, &[(23308, 1, 5, AffectScope::Range, AffectObject::Friend)]),
    (144, &[(23309, 1, 5, AffectScope::Range, AffectObject::Friend)]),
    (145, &[(23310, 1, 5, AffectScope::Range, AffectObject::Friend)]),
    (146, &[(23311, 1, 5, AffectScope::Range, AffectObject::Friend)]),
    (13007, &[(2139, 1, 10, AffectScope::Range, AffectObject::Friend)]),
    (13018, &[(5123, 1, 5, AffectScope::Range, AffectObject::Friend)]),
    (
        13019,
        &[(5124, 1, 2, AffectScope::Range, AffectObject::NotFriend)],
    ),
    (13020, &[(5125, 1, 5, AffectScope::Range, AffectObject::Friend)]),
    (13021, &[(5126, 1, 5, AffectScope::Range, AffectObject::Friend)]),
    (13022, &[(5127, 1, 5, AffectScope::Range, AffectObject::Friend)]),
    (13023, &[(5128, 1, 5, AffectScope::Range, AffectObject::Friend)]),
    (13024, &[(5129, 1, 5, AffectScope::Range, AffectObject::Friend)]),
    (
        13028,
        &[(5145, 1, 2, AffectScope::Range, AffectObject::NotFriend)],
    ),
    (
        13030,
        &[(5134, 1, 2, AffectScope::Range, AffectObject::NotFriend)],
    ),
    (
        13071,
        &[(5272, 1, 5, AffectScope::Range, AffectObject::NotFriend)],
    ),
    (
        13072,
        &[(5272, 2, 5, AffectScope::Range, AffectObject::NotFriend)],
    ),
    (
        13073,
        &[(5272, 3, 5, AffectScope::Range, AffectObject::NotFriend)],
    ),
    (
        13074,
        &[(5272, 4, 5, AffectScope::Range, AffectObject::NotFriend)],
    ),
    (
        13075,
        &[(5272, 5, 5, AffectScope::Range, AffectObject::NotFriend)],
    ),
    (
        13076,
        &[(5272, 6, 5, AffectScope::Range, AffectObject::NotFriend)],
    ),
    (
        13257,
        &[(5272, 7, 5, AffectScope::Range, AffectObject::NotFriend)],
    ),
    (
        13258,
        &[(5272, 8, 5, AffectScope::Range, AffectObject::NotFriend)],
    ),
    (
        13259,
        &[(5272, 9, 5, AffectScope::Range, AffectObject::NotFriend)],
    ),
    (
        13260,
        &[(5272, 10, 5, AffectScope::Range, AffectObject::NotFriend)],
    ),
    (
        13261,
        &[(5272, 11, 5, AffectScope::Range, AffectObject::NotFriend)],
    ),
    (
        13262,
        &[(5272, 12, 5, AffectScope::Range, AffectObject::NotFriend)],
    ),
    (
        13263,
        &[(5272, 13, 5, AffectScope::Range, AffectObject::NotFriend)],
    ),
    (
        13264,
        &[(5272, 14, 5, AffectScope::Range, AffectObject::NotFriend)],
    ),
    (
        13265,
        &[(5272, 15, 5, AffectScope::Range, AffectObject::NotFriend)],
    ),
    (
        13266,
        &[(5272, 16, 5, AffectScope::Range, AffectObject::NotFriend)],
    ),
    (
        13267,
        &[(5272, 17, 5, AffectScope::Range, AffectObject::NotFriend)],
    ),
];
