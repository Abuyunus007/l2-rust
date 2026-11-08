//! Which admin commands need a person's approval.

/// The beginnings of the parts of a command word (`ban_char` is `ban` and `char`) that make a
/// command destructive: it punishes a player, removes or resets something, or stops the server.
/// A false alarm costs one approval; a miss could cost a character, so the list is broad.
const DESTRUCTIVE_STEMS: &[&str] = &[
    "ban",
    "jail",
    "kick",
    "kill",
    "del",
    "destroy",
    "remove",
    "clear",
    "reset",
    "wipe",
    "erase",
    "shutdown",
    "restart",
    "punish",
    "take",
    "disband",
    "unspawn",
    "despawn",
    "abort",
    "logout",
    "disconnect",
    "setkarma",
    "res",
    "unequip",
    "drop",
];

/// The command word of an admin command, the way the server matches it: without slashes and the
/// `admin_` prefix, in lower case (`//Ban_Char Amy` is `ban_char`).
pub fn command_word(command: &str) -> String {
    let word = command
        .trim()
        .trim_start_matches('/')
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    word.strip_prefix("admin_").map_or(word.clone(), str::to_owned)
}

/// Does `command` need a person's approval?
pub fn is_destructive(command: &str) -> bool {
    let word = command_word(command);
    word.split('_')
        .any(|part| DESTRUCTIVE_STEMS.iter().any(|stem| part.starts_with(stem)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_word_is_the_one_the_server_matches() {
        assert_eq!(command_word("//Ban_Char Amy 0"), "ban_char");
        assert_eq!(command_word("admin_kick Bob"), "kick");
        assert_eq!(command_word("  serverinfo  "), "serverinfo");
        assert_eq!(command_word(""), "");
    }

    #[test]
    fn punishing_removing_and_stopping_need_approval() {
        for command in [
            "ban_char Amy",
            "kick Bob",
            "jail Bob 60",
            "kill",
            "killall",
            "delete",
            "delitem 57 1000",
            "server_shutdown 60",
            "server_restart 10",
            "server_abort",
            "punishment_add Amy CHARACTER BAN 0 spam",
            "unspawnall",
            "clear_inventory",
            "admin_ban_acc Amy",
            "//ban_chat Amy",
            "takeitem",
            "res",
            "setkarma 100",
        ] {
            assert!(is_destructive(command), "{command}");
        }
    }

    #[test]
    fn looking_and_giving_need_none() {
        for command in [
            "serverinfo",
            "announce The server restarts tonight",
            "find_character Amy",
            "create_item 57 1000",
            "setlevel 80",
            "teleportto Amy",
            "unban_char Amy",
            "help",
            "admin",
            "gmliston",
            "reload html",
        ] {
            assert!(!is_destructive(command), "{command}");
        }
    }
}
