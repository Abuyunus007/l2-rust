//! Which topic of the config editor a key belongs to (GUI card G-07).
// Java: none (tooling). The topics are the owner's grouping of the `.ini` settings; the headers
// of the files (`# ---- Title ----`) are used because they already split the big files by theme.
//!
//! Every key of every `.ini` in `dist/` lands in exactly one [`Topic`]: [`topic_of`] walks
//! [`RULES`] from the top and takes the first rule that matches the file, the header group and
//! the key. `Custom/*.ini` files are not in the table: each one is a topic of its own, named
//! after the file.
//!
//! The test in `tests/config_hints.rs` fails when a key of `dist/` has no topic, or when a rule
//! is never used (a header that was renamed upstream would otherwise drop its keys into the
//! catch-all of the file without anybody noticing).
//!
//! # Adding a rule
//! Put a more specific rule above a more general one. `group` is the header title of the file
//! (compared without case and without a trailing `:`); empty = any. `key` is a key or a pattern
//! (`*` wildcards, see [`crate::ranges::key_matches`]); empty = any. Each file needs a rule with
//! both empty as its catch-all, which is also what makes a *new* file show up in the test.

use crate::defaults::Side;
use crate::ranges::key_matches;

/// The fixed topics, in the order of the left list.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StdTopic {
    ServerNetwork,
    Database,
    Rates,
    Player,
    CharacterCreation,
    PvpKarma,
    PartyClans,
    NpcsMonsters,
    GrandBosses,
    SiegesResidences,
    Olympiad,
    SevenSigns,
    Events,
    Geodata,
    ChatFlood,
    Admin,
}

impl StdTopic {
    pub const ALL: [StdTopic; 16] = [
        StdTopic::ServerNetwork,
        StdTopic::Database,
        StdTopic::Rates,
        StdTopic::Player,
        StdTopic::CharacterCreation,
        StdTopic::PvpKarma,
        StdTopic::PartyClans,
        StdTopic::NpcsMonsters,
        StdTopic::GrandBosses,
        StdTopic::SiegesResidences,
        StdTopic::Olympiad,
        StdTopic::SevenSigns,
        StdTopic::Events,
        StdTopic::Geodata,
        StdTopic::ChatFlood,
        StdTopic::Admin,
    ];

    pub fn label(self) -> &'static str {
        match self {
            StdTopic::ServerNetwork => "Server & network",
            StdTopic::Database => "Database",
            StdTopic::Rates => "Rates",
            StdTopic::Player => "Player",
            StdTopic::CharacterCreation => "Character creation",
            StdTopic::PvpKarma => "PvP & karma",
            StdTopic::PartyClans => "Party & clans",
            StdTopic::NpcsMonsters => "NPCs & monsters",
            StdTopic::GrandBosses => "Grand bosses",
            StdTopic::SiegesResidences => "Sieges & residences",
            StdTopic::Olympiad => "Olympiad",
            StdTopic::SevenSigns => "Seven Signs",
            StdTopic::Events => "Events",
            StdTopic::Geodata => "Geodata",
            StdTopic::ChatFlood => "Chat & flood",
            StdTopic::Admin => "Admin (Rust-only)",
        }
    }
}

/// A topic of the editor: one of the fixed ones, or a `Custom/*.ini` feature by its file name
/// without `.ini` (`Banking`). Sorts the fixed ones first, in list order, then the customs A-Z.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Topic {
    Std(StdTopic),
    Custom(String),
}

impl Topic {
    /// Text for the list and for the search (`Allowed Player Races`).
    pub fn label(&self) -> String {
        match self {
            Topic::Std(t) => t.label().to_owned(),
            Topic::Custom(stem) => humanize(stem),
        }
    }

    pub fn is_custom(&self) -> bool {
        matches!(self, Topic::Custom(_))
    }
}

/// `AllowedPlayerRaces` -> `Allowed Player Races`; a run of capitals stays one word and `Pvp`
/// is spelled `PvP` (`PvpAnnounce` -> `PvP Announce`, `FindPvP` -> `Find PvP`).
pub fn humanize(stem: &str) -> String {
    let stem = stem.replace("PvP", "Pvp");
    let chars: Vec<char> = stem.chars().collect();
    let mut out = String::new();
    for (i, &c) in chars.iter().enumerate() {
        let prev = i.checked_sub(1).and_then(|p| chars.get(p)).copied();
        let next = chars.get(i + 1).copied();
        let boundary = c.is_uppercase()
            && prev.is_some_and(|p| {
                p.is_lowercase()
                    || p.is_ascii_digit()
                    || (p.is_uppercase() && next.is_some_and(char::is_lowercase))
            });
        if boundary {
            out.push(' ');
        }
        out.push(c);
    }
    out.replace("Pvp", "PvP")
}

/// One line of the table. A field left empty matches anything; `side` `None` matches both.
#[derive(Clone, Copy, Debug)]
pub struct Rule {
    pub side: Option<Side>,
    pub file: &'static str,
    pub group: &'static str,
    pub key: &'static str,
    pub topic: StdTopic,
}

const fn any(file: &'static str, group: &'static str, key: &'static str, topic: StdTopic) -> Rule {
    Rule {
        side: None,
        file,
        group,
        key,
        topic,
    }
}

const fn login(file: &'static str, group: &'static str, key: &'static str, topic: StdTopic) -> Rule {
    Rule {
        side: Some(Side::Login),
        file,
        group,
        key,
        topic,
    }
}

const fn game(file: &'static str, group: &'static str, key: &'static str, topic: StdTopic) -> Rule {
    Rule {
        side: Some(Side::Game),
        file,
        group,
        key,
        topic,
    }
}

use StdTopic as T;

/// First match wins. Keys first, then header groups, then the catch-all of the file.
#[rustfmt::skip]
pub static RULES: &[Rule] = &[
    // ---- Both servers ----
    any("Admin.ini", "", "", T::Admin),
    any("Database.ini", "", "", T::Database),
    login("Server.ini", "", "", T::ServerNetwork),
    login("Network.ini", "", "", T::ServerNetwork),
    login("Threads.ini", "", "", T::ServerNetwork),
    login("Interface.ini", "", "", T::ServerNetwork),

    // ---- Game server: Server.ini ----
    game("Server.ini", "Misc Player Settings", "", T::CharacterCreation),
    game("Server.ini", "", "", T::ServerNetwork),
    game("Network.ini", "", "", T::ServerNetwork),
    game("Threads.ini", "", "", T::ServerNetwork),
    game("Interface.ini", "", "", T::ServerNetwork),
    game("Development.ini", "", "", T::ServerNetwork),
    game("IdManager.ini", "", "", T::Database),
    game("GeoEngine.ini", "", "", T::Geodata),
    game("GeoOverride.ini", "", "", T::Geodata),
    game("Rates.ini", "", "", T::Rates),

    // ---- Game server: Player.ini ----
    game("Player.ini", "", "PartyXpCutoff*", T::PartyClans),
    game("Player.ini", "", "ForbiddenNames", T::CharacterCreation),
    game("Player.ini", "", "DeleteCharAfterDays", T::CharacterCreation),
    game("Player.ini", "Karma", "", T::PvpKarma),
    game("Player.ini", "Clan", "", T::PartyClans),
    game("Player.ini", "Party", "", T::PartyClans),
    game("Player.ini", "Initial Settings", "", T::CharacterCreation),
    game("Player.ini", "", "", T::Player),

    // ---- Game server: General.ini ----
    game("General.ini", "", "GlobalChat", T::ChatFlood),
    game("General.ini", "", "TradeChat", T::ChatFlood),
    game("General.ini", "", "MinimumChatLevel", T::ChatFlood),
    game("General.ini", "", "UseChatFilter", T::ChatFlood),
    game("General.ini", "", "ChatFilterChars", T::ChatFlood),
    game("General.ini", "", "BanChatChannels", T::ChatFlood),
    game("General.ini", "", "LogChat", T::ChatFlood),
    game("General.ini", "", "NormalEnchantCostMultipiler", T::Player),
    game("General.ini", "", "SafeEnchantCostMultipiler", T::Player),
    game("General.ini", "", "OnlyGMItemsFree", T::Player),
    game("General.ini", "Falling Damage", "", T::Player),
    game("General.ini", "Skills & Effects", "", T::Player),
    game("General.ini", "Features", "", T::Player),
    game("General.ini", "Instances", "", T::Player),
    game("General.ini", "Misc Settings", "", T::Player),
    game("General.ini", "Teleport Bookmark Settings", "", T::Player),
    game("General.ini", "Bot Report Button settings", "", T::Player),
    game("General.ini", "Prime-Shop Settings", "", T::Player),
    game("General.ini", "Manor", "", T::SiegesResidences),
    game("General.ini", "Lottery", "", T::Events),
    game("General.ini", "Fishing Tournament", "", T::Events),
    game("General.ini", "Item Auction", "", T::Events),
    game("General.ini", "Dimension Rift", "", T::Events),
    game("General.ini", "Birthday Event Settings", "", T::Events),
    game("General.ini", "Handy's Block Checker Event Settings", "", T::Events),
    game("General.ini", "Hellbound Settings", "", T::NpcsMonsters),
    game("General.ini", "", "", T::ServerNetwork),

    // ---- Game server: Feature.ini ----
    game("Feature.ini", "Seven Signs", "", T::SevenSigns),
    game("Feature.ini", "Clan Reputation Points", "", T::PartyClans),
    game("Feature.ini", "", "", T::SiegesResidences),

    // ---- Game server: the rest ----
    game("FortSiege.ini", "", "", T::SiegesResidences),
    game("Siege.ini", "", "", T::SiegesResidences),
    game("TerritoryWar.ini", "", "", T::SiegesResidences),
    game("ConquerableHallSiege.ini", "", "", T::SiegesResidences),
    game("PVP.ini", "", "", T::PvpKarma),
    game("NPC.ini", "", "", T::NpcsMonsters),
    game("GrandBoss.ini", "", "", T::GrandBosses),
    game("GraciaSeeds.ini", "", "", T::GrandBosses),
    game("UndergroundColiseum.ini", "", "", T::Events),
    game("Olympiad.ini", "", "", T::Olympiad),
    game("FloodProtector.ini", "", "", T::ChatFlood),
];

/// The topic of a key. `rel_file` is the path below the config folder (`Custom/Banking.ini`),
/// `group` the header title from [`crate::ini_edit::IniEntry::group`]. `None`: no rule knows the
/// file (a file somebody added); the editor lists those under "Other".
pub fn topic_of(side: Side, rel_file: &str, group: &str, key: &str) -> Option<Topic> {
    if side == Side::Game
        && let Some(stem) = rel_file
            .strip_prefix("Custom/")
            .and_then(|f| f.strip_suffix(".ini"))
        && !stem.contains('/')
    {
        return Some(Topic::Custom(stem.to_owned()));
    }
    rule_index(side, rel_file, group, key)
        .and_then(|i| RULES.get(i))
        .map(|r| Topic::Std(r.topic))
}

/// Index of the first rule that matches, for [`topic_of`] and for the "every rule is used" test.
pub fn rule_index(side: Side, rel_file: &str, group: &str, key: &str) -> Option<usize> {
    let group = norm_group(group);
    RULES.iter().position(|r| {
        r.side.is_none_or(|s| s == side)
            && r.file == rel_file
            && (r.group.is_empty() || norm_group(r.group) == group)
            && (r.key.is_empty() || key_matches(r.key, key))
    })
}

fn norm_group(g: &str) -> String {
    g.trim().trim_end_matches(':').trim().to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn customs_are_topics_of_their_own() {
        assert_eq!(
            topic_of(Side::Game, "Custom/Banking.ini", "", "BankingEnabled"),
            Some(Topic::Custom("Banking".into()))
        );
        // Only the game server has a Custom folder.
        assert_eq!(topic_of(Side::Login, "Custom/Banking.ini", "", "X"), None);
    }

    #[test]
    fn keys_beat_groups_beat_the_file() {
        // Party XP cut-off sits in Player.ini's "Other Settings" but is a party matter.
        assert_eq!(
            topic_of(Side::Game, "Player.ini", "Other Settings:", "PartyXpCutoffMethod"),
            Some(Topic::Std(StdTopic::PartyClans))
        );
        assert_eq!(
            topic_of(Side::Game, "Player.ini", "Karma", "AltKarmaPlayerCanShop"),
            Some(Topic::Std(StdTopic::PvpKarma))
        );
        // The group is compared without case and without the trailing colon.
        assert_eq!(
            topic_of(Side::Game, "Player.ini", "initial settings", "StartingLevel"),
            Some(Topic::Std(StdTopic::CharacterCreation))
        );
        assert_eq!(
            topic_of(Side::Game, "Player.ini", "Limits", "MaxRunSpeed"),
            Some(Topic::Std(StdTopic::Player))
        );
    }

    #[test]
    fn the_same_file_name_can_belong_elsewhere_per_server() {
        assert_eq!(
            topic_of(Side::Login, "Server.ini", "Networking", "LoginserverPort"),
            Some(Topic::Std(StdTopic::ServerNetwork))
        );
        assert_eq!(
            topic_of(Side::Game, "Server.ini", "Misc Player Settings", "CharMaxNumber"),
            Some(Topic::Std(StdTopic::CharacterCreation))
        );
        assert_eq!(
            topic_of(Side::Login, "Database.ini", "", "Password"),
            Some(Topic::Std(StdTopic::Database))
        );
        assert_eq!(topic_of(Side::Game, "Nope.ini", "", "X"), None);
    }

    #[test]
    fn topics_sort_fixed_first_then_customs() {
        let mut v = vec![
            Topic::Custom("Wedding".into()),
            Topic::Std(StdTopic::Admin),
            Topic::Custom("Banking".into()),
            Topic::Std(StdTopic::ServerNetwork),
        ];
        v.sort();
        assert_eq!(
            v,
            [
                Topic::Std(StdTopic::ServerNetwork),
                Topic::Std(StdTopic::Admin),
                Topic::Custom("Banking".into()),
                Topic::Custom("Wedding".into()),
            ]
        );
        assert_eq!(StdTopic::ALL.len(), 16);
        assert!(StdTopic::ALL.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn labels_are_readable() {
        assert_eq!(Topic::Std(StdTopic::PvpKarma).label(), "PvP & karma");
        assert_eq!(humanize("Banking"), "Banking");
        assert_eq!(humanize("AllowedPlayerRaces"), "Allowed Player Races");
        assert_eq!(humanize("NpcStatMultipliers"), "Npc Stat Multipliers");
        assert_eq!(humanize("FindPvP"), "Find PvP");
        assert_eq!(humanize("PvpAnnounce"), "PvP Announce");
        assert_eq!(humanize("L2WalkerProtection"), "L2 Walker Protection");
        assert!(Topic::Custom("X".into()).is_custom());
    }
}
