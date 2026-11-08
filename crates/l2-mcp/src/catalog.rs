//! The admin commands the assistant is taught: syntax, meaning and an example, taken from the
//! handlers in `crates/l2-world/src/admin` (a test checks that every name is registered there).
//!
//! Only commands that work for the virtual GM of the admin endpoint are listed: it has no
//! position and no inventory of its own, so `teleportto`, `move_to`, `goto`, `recall` and
//! `create_item` (into the GM's own bag) are left out. A command on "the selected target" gets the
//! character as `target` (`GameAction::CommandOn`).

/// One admin command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandDoc {
    /// The command word, without slashes and `admin_`.
    pub name: &'static str,
    /// `<required>` and `[optional]` arguments.
    pub syntax: &'static str,
    pub about: &'static str,
    /// It acts on the selected target: pass the character's name as `target`.
    pub needs_target: bool,
    pub example: &'static str,
}

const fn doc(
    name: &'static str,
    syntax: &'static str,
    about: &'static str,
    needs_target: bool,
    example: &'static str,
) -> CommandDoc {
    CommandDoc {
        name,
        syntax,
        about,
        needs_target,
        example,
    }
}

/// The catalog, grouped: character, items and skills, information, punishment, server.
pub const CATALOG: &[CommandDoc] = &[
    // ---- A character (target = the character's name) ----
    doc(
        "set_level",
        "set_level <level>",
        "Sets the exact level (exp adjusted, HP/MP/CP refilled).",
        true,
        "set_level 80",
    ),
    doc(
        "add_level",
        "add_level <levels>",
        "Adds (or with a minus removes) levels.",
        true,
        "add_level 5",
    ),
    doc(
        "add_exp_sp",
        "add_exp_sp <exp> <sp>",
        "Adds experience and skill points.",
        true,
        "add_exp_sp 1000000 50000",
    ),
    doc(
        "setclass",
        "setclass <class id>",
        "Changes the class (e.g. 88 Duelist, 93 Adventurer, 94 Archmage, 97 Cardinal).",
        true,
        "setclass 88",
    ),
    doc(
        "changename",
        "changename <new name>",
        "Renames the character.",
        true,
        "changename NewName",
    ),
    doc(
        "setnoble",
        "setnoble",
        "Switches noblesse status on or off.",
        true,
        "setnoble",
    ),
    doc(
        "sethero",
        "sethero",
        "Makes the character a hero.",
        true,
        "sethero",
    ),
    doc(
        "setkarma",
        "setkarma <value>",
        "Sets the karma (0 clears it; no negative values).",
        true,
        "setkarma 0",
    ),
    doc("setpk", "setpk <count>", "Sets the PK count.", true, "setpk 0"),
    doc(
        "setpvp",
        "setpvp <count>",
        "Sets the PvP count.",
        true,
        "setpvp 100",
    ),
    doc(
        "setinvul",
        "setinvul",
        "Switches the character's invulnerability on or off.",
        true,
        "setinvul",
    ),
    doc(
        "heal",
        "heal [character name]",
        "Fully restores HP, MP and CP.",
        false,
        "heal Amy",
    ),
    doc(
        "res",
        "res [character name]",
        "Resurrects a dead character.",
        false,
        "res Amy",
    ),
    doc(
        "sendhome",
        "sendhome [character name]",
        "Sends the character to the home town of its race.",
        false,
        "sendhome Amy",
    ),
    doc(
        "setcastle",
        "setcastle <castle name>",
        "Gives the castle to the clan of the character.",
        true,
        "setcastle Giran",
    ),
    doc(
        "spawn",
        "spawn <npc id or name> [count] [respawn seconds]",
        "Spawns NPCs or monsters next to the character; they respawn (default 1, 60 s).",
        true,
        "spawn 20101 3 120",
    ),
    doc(
        "spawn_once",
        "spawn_once <npc id or name> [count]",
        "Like spawn, but they do not respawn after death.",
        true,
        "spawn_once 20101 5",
    ),
    // ---- Items and skills ----
    doc(
        "give_item_target",
        "give_item_target <item id> [count]",
        "Gives the character an item (57 is Adena).",
        true,
        "give_item_target 57 1000000",
    ),
    doc(
        "give_item_to_all",
        "give_item_to_all <item id> [count]",
        "Gives every online character an item.",
        false,
        "give_item_to_all 57 1000",
    ),
    doc(
        "setew",
        "setew <enchant>",
        "Sets the enchant level of the weapon in the right hand.",
        true,
        "setew 16",
    ),
    doc(
        "add_skill",
        "add_skill <skill id> [level]",
        "Teaches a skill (level 1 if none given).",
        true,
        "add_skill 1204 2",
    ),
    doc(
        "give_all_skills",
        "give_all_skills",
        "Teaches every skill of the character's class and level.",
        true,
        "give_all_skills",
    ),
    doc(
        "buff",
        "buff <skill id> [level]",
        "Puts the effect of a skill on the character (e.g. 1204 Wind Walk).",
        true,
        "buff 1204 2",
    ),
    // ---- Information ----
    doc(
        "online",
        "online",
        "How many players are online, in peace zones, in instances, fighting.",
        false,
        "online",
    ),
    doc(
        "find_character",
        "find_character <part of a name>",
        "Lists online characters whose name contains the text.",
        false,
        "find_character Am",
    ),
    doc(
        "character_info",
        "character_info [character name]",
        "Level, class, place, account and more of a character.",
        false,
        "character_info Amy",
    ),
    doc(
        "clan_info",
        "clan_info [character name]",
        "The clan of a character: leader, level, castle, hall.",
        false,
        "clan_info Amy",
    ),
    // ---- Punishment (the administrator approves each) ----
    doc(
        "kick",
        "kick <character name>",
        "Disconnects the character.",
        false,
        "kick Spambot",
    ),
    doc(
        "kill",
        "kill [character name]",
        "Kills the character.",
        false,
        "kill Spambot",
    ),
    doc(
        "jail",
        "jail <character name>",
        "Puts the character in jail.",
        false,
        "jail Spambot",
    ),
    doc(
        "unjail",
        "unjail <character name>",
        "Lets the character out of jail.",
        false,
        "unjail Spambot",
    ),
    doc(
        "ban_chat",
        "ban_chat <character name>",
        "Forbids the character to chat.",
        false,
        "ban_chat Spambot",
    ),
    doc(
        "unban_chat",
        "unban_chat <character name>",
        "Allows the character to chat again.",
        false,
        "unban_chat Spambot",
    ),
    doc(
        "ban_char",
        "ban_char <character name>",
        "Bans the character.",
        false,
        "ban_char Spambot",
    ),
    doc(
        "unban_char",
        "unban_char <character name>",
        "Lifts the ban of the character.",
        false,
        "unban_char Spambot",
    ),
    doc(
        "ban_acc",
        "ban_acc <account name>",
        "Bans the account (all its characters).",
        false,
        "ban_acc someaccount",
    ),
    doc(
        "unban_acc",
        "unban_acc <account name>",
        "Lifts the ban of the account.",
        false,
        "unban_acc someaccount",
    ),
    // ---- Server ----
    doc(
        "announce_crit",
        "announce_crit <text>",
        "A critical (red) announcement to everybody.",
        false,
        "announce_crit Event starts now",
    ),
    doc(
        "server_shutdown",
        "server_shutdown <seconds>",
        "Shuts the server down after a countdown.",
        false,
        "server_shutdown 300",
    ),
    doc(
        "server_restart",
        "server_restart <seconds>",
        "Restarts the server after a countdown.",
        false,
        "server_restart 300",
    ),
    doc(
        "server_abort",
        "server_abort",
        "Cancels a running shutdown or restart countdown.",
        false,
        "server_abort",
    ),
    doc(
        "reload",
        "reload <html | quest <name> | l2r | fakeplayerchat>",
        "Reads data files again while the server runs.",
        false,
        "reload l2r",
    ),
    doc(
        "respawnall",
        "respawnall",
        "Reads the spawn files again and respawns every NPC.",
        false,
        "respawnall",
    ),
];

/// The commands whose name, syntax or text contains every word of `query` (any case). An empty
/// query gives all of them.
pub fn search(query: &str) -> Vec<&'static CommandDoc> {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    CATALOG
        .iter()
        .filter(|doc| {
            let text = format!("{} {} {}", doc.name, doc.syntax, doc.about).to_lowercase();
            words.iter().all(|word| text.contains(word.as_str()))
        })
        .collect()
}

/// One line per command, for the model.
pub fn describe(docs: &[&CommandDoc]) -> String {
    docs.iter()
        .map(|doc| {
            format!(
                "{} - {}{} Example: {}",
                doc.syntax,
                doc.about,
                if doc.needs_target {
                    " Needs target (the character's name)."
                } else {
                    ""
                },
                doc.example
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use std::collections::BTreeSet;
    use std::path::Path;

    use super::*;
    use crate::guard::is_destructive;

    #[test]
    fn every_command_is_registered_by_the_server() {
        // The handlers register `"admin_<name>"`; read them from the sources of the server.
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let mut sources = String::new();
        let mut dirs = vec![root.join("l2-world").join("src")];
        while let Some(dir) = dirs.pop() {
            for entry in std::fs::read_dir(dir).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    dirs.push(path);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    sources.push_str(&std::fs::read_to_string(path).unwrap());
                }
            }
        }
        for doc in CATALOG {
            assert!(
                sources.contains(&format!("\"admin_{}\"", doc.name)),
                "{} is not a registered admin command",
                doc.name
            );
            assert!(doc.syntax.starts_with(doc.name), "{}", doc.name);
            assert!(doc.example.starts_with(doc.name), "{}", doc.name);
        }
        let names: BTreeSet<&str> = CATALOG.iter().map(|d| d.name).collect();
        assert_eq!(names.len(), CATALOG.len(), "each command once");
    }

    #[test]
    fn punishing_and_stopping_commands_need_approval_and_the_rest_do_not() {
        let approval: BTreeSet<&str> = CATALOG
            .iter()
            .filter(|d| is_destructive(d.example))
            .map(|d| d.name)
            .collect();
        let expected: BTreeSet<&str> = [
            "kick",
            "kill",
            "jail",
            "ban_chat",
            "ban_char",
            "ban_acc",
            "server_shutdown",
            "server_restart",
            "respawnall",
            "setkarma",
        ]
        .into();
        assert_eq!(approval, expected);
    }

    #[test]
    fn the_search_finds_by_any_word() {
        let names = |q: &str| search(q).iter().map(|d| d.name).collect::<Vec<_>>();
        assert_eq!(names("set_level"), ["set_level"]);
        assert!(names("level").contains(&"add_level"));
        assert!(names("BAN").contains(&"ban_acc") && names("ban").contains(&"unban_char"));
        assert_eq!(names("ban account"), ["ban_acc", "unban_acc"]);
        assert!(names("nothing like this").is_empty());
        assert_eq!(search("").len(), CATALOG.len());
        let text = describe(&search("give_item_target"));
        assert!(
            text.contains("Needs target") && text.contains("Example: give_item_target 57"),
            "{text}"
        );
    }
}
