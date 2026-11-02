//! The list, set and map fields of the game config that are generated instead of left as `()`.
//!
//! Java: the `split(..)` loops in `gameserver/config/**/*Config.java`. Java has no list getter, so
//! the generator cannot recognise these from the statement shape the way it does for `getInt`.
//! Instead this table names every field it may generate, with the kind of its Java type; the
//! generator then checks the table against the Java source (the declared type and the one
//! literal key and default string of the field's getter) and fails when they disagree. The
//! runtime side is `crates/l2-config/src/list_kinds.rs` (`IniFile::get_int_set` and friends).
//!
//! A field that is not in the table stays a `()` placeholder. A field must not be in both this
//! table and `ported.rs`.

/// How the value of a field is read; one `IniFile` getter per kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListKind {
    /// `Set<Integer>`, `split(",")`.
    IntSet,
    /// `List<Integer>`, `split(",")`.
    IntList,
    /// `Map<Integer, Integer>`, `split(";")` then `split(",")`.
    IntMap,
    /// `List<ItemHolder>`, `split(";")` then `split(",")`.
    ItemHolderList,
    /// `Map<String, Location>`, `split(";")` then `split(",")`.
    LocationMap,
    /// `List<String>` with the given one-character separator.
    StringList(char),
}

impl ListKind {
    /// The Java type of a field of this kind, without blanks.
    pub fn java_type(self) -> &'static str {
        match self {
            Self::IntSet => "Set<Integer>",
            Self::IntList => "List<Integer>",
            Self::IntMap => "Map<Integer,Integer>",
            Self::ItemHolderList => "List<ItemHolder>",
            Self::LocationMap => "Map<String,Location>",
            Self::StringList(_) => "List<String>",
        }
    }

    /// The Rust type of the generated field.
    pub fn rust_type(self) -> &'static str {
        match self {
            Self::IntSet => "std::collections::BTreeSet<i32>",
            Self::IntList => "Vec<i32>",
            Self::IntMap => "std::collections::HashMap<i32, i32>",
            Self::ItemHolderList => "Vec<crate::ItemHolder>",
            Self::LocationMap => "std::collections::BTreeMap<String, crate::game_manual::Location>",
            Self::StringList(_) => "Vec<String>",
        }
    }

    /// The `IniFile` method that reads it.
    pub fn getter(self) -> &'static str {
        match self {
            Self::IntSet => "get_int_set",
            Self::IntList => "get_int_list",
            Self::IntMap => "get_int_map",
            Self::ItemHolderList => "get_item_holder_list",
            Self::LocationMap => "get_location_map",
            Self::StringList(_) => "get_string_list",
        }
    }
}

use ListKind::{IntList, IntMap, IntSet, ItemHolderList, LocationMap, StringList};

/// `(Java class, Java field, kind)`.
pub const LIST_FIELDS: &[(&str, &str, ListKind)] = &[
    // AutoPlayConfig and AutoPotionsConfig are left out on purpose: their `Set<Integer>` fields are
    // read in file order by `game-server/src/custom_play.rs`, and a sorted `BTreeSet` would change
    // the order the potions are tried in.
    (
        "BossAnnouncementsConfig",
        "RAIDBOSSES_EXCLUDED_FROM_SPAWN_ANNOUNCEMENTS",
        IntSet,
    ),
    (
        "BossAnnouncementsConfig",
        "RAIDBOSSES_EXCLUDED_FROM_DEFEAT_ANNOUNCEMENTS",
        IntSet,
    ),
    ("ChampionMonstersConfig", "CHAMPION_REWARD_ITEMS", ItemHolderList),
    ("CommunityBoardConfig", "COMMUNITY_AVAILABLE_BUFFS", IntSet),
    (
        "CommunityBoardConfig",
        "COMMUNITY_AVAILABLE_TELEPORTS",
        LocationMap,
    ),
    ("NpcConfig", "LIST_PET_RENT_NPC", IntList),
    ("NpcConfig", "MINIONS_RESPAWN_TIME", IntMap),
    // NOTE(java-quirk): Java only reads the key when ENABLE_RANDOM_MONSTER_SPAWNS is on; the
    // value is read unconditionally here (nothing uses it while the feature is off).
    ("RandomSpawnsConfig", "MOBS_LIST_NOT_RANDOM", IntSet),
    ("RebirthConfig", "REBIRTH_SKILL_DELETE_WHITELIST", IntSet),
    ("RebirthConfig", "REBIRTH_MAGE_SKILLS", StringList(';')),
    ("RebirthConfig", "REBIRTH_FIGHTER_SKILLS", StringList(';')),
    ("TransmogConfig", "TRANSMOG_BANNED_ITEM_IDS", IntSet),
];

/// The kind of `class.field`, when the table lists it.
pub fn list_kind(class: &str, field: &str) -> Option<ListKind> {
    LIST_FIELDS
        .iter()
        .find(|(c, f, _)| *c == class && *f == field)
        .map(|(_, _, kind)| *kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup() {
        assert_eq!(list_kind("NpcConfig", "LIST_PET_RENT_NPC"), Some(IntList));
        assert_eq!(list_kind("NpcConfig", "NOPE"), None);
        assert_eq!(StringList(';').rust_type(), "Vec<String>");
        assert_eq!(IntMap.getter(), "get_int_map");
    }
}
