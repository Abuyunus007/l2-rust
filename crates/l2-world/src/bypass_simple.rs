//! The small NPC bypass handlers: `PlayerHelp`, `TutorialClose` and `QuestList`.
// Java: dist/game/data/scripts/handlers/bypass/npc/{PlayerHelp,TutorialClose,QuestList}.java

use l2_protocol_game::server::{ExShowQuestInfo, NpcHtmlMessage, TutorialCloseHtml};
use l2_util::ObjectId;
use tracing::warn;

use crate::bypass::BypassHandlers;
use crate::world::World;

/// Registers the handlers of this module (called by `BypassHandlers::default`).
pub(crate) fn register(handlers: &mut BypassHandlers) {
    // PlayerHelp.java
    handlers.register("player_help", player_help_handler);
    // TutorialClose.java
    handlers.register("tutorial_close", tutorial_close_handler);
    // QuestList.java
    handlers.register("questlist", quest_list_handler);
}

/// `new StringTokenizer(path).nextToken()`: the first run of characters that are not
/// `" \t\n\r\f"`.
fn first_token(text: &str) -> Option<&str> {
    text.split([' ', '\t', '\n', '\r', '\u{c}'])
        .find(|t| !t.is_empty())
}

/// What `PlayerHelp` shows for a command: the page under `data/html/help/` and the item id of the
/// `#<itemId>` suffix. `None` where Java returns without a window (`Err` where it throws).
fn player_help_target(command: &str) -> Result<Option<(&str, i32)>, String> {
    // `command.length() < 13`: the command is always ASCII, a bypass of `player_help <page>`.
    if command.chars().count() < 13 {
        return Ok(None);
    }
    let path: String = command.chars().skip(12).collect();
    if path.contains("..") {
        return Ok(None);
    }
    let start = command.len() - path.len();
    let path = &command[start..];
    let Some(token) = first_token(path) else {
        return Err("java.util.NoSuchElementException".to_owned());
    };
    // `token.split("#")`: Java drops trailing empty strings.
    let mut parts: Vec<&str> = token.split('#').collect();
    while parts.len() > 1 && parts.last().is_some_and(|p| p.is_empty()) {
        parts.pop();
    }
    let page = parts.first().copied().unwrap_or("");
    if parts.len() > 1 {
        let item = parts[1].parse::<i32>().map_err(|_| {
            format!(
                "java.lang.NumberFormatException: For input string: \"{}\"",
                parts[1]
            )
        })?;
        return Ok(Some((page, item)));
    }
    Ok(Some((page, 0)))
}

/// Java `PlayerHelp.onCommand`.
fn player_help_handler(
    world: &mut World,
    player: ObjectId,
    command: &str,
    _target: Option<ObjectId>,
) -> bool {
    match player_help_target(command) {
        Ok(None) => false,
        Ok(Some((page, item_id))) => {
            let path = format!("data/html/help/{page}");
            // `NpcHtmlMessage(0, itemId)` + `setFile`: a missing page is Java's "My text is
            // missing" page.
            let html = world.get_htm(player, &path).unwrap_or_else(|| {
                warn!("NpcHtmlMessage: Missing html page {path}");
                format!("<html><body>My Text is missing:<br>{path}</body></html>")
            });
            if let Some(message) = NpcHtmlMessage::new_item(0, item_id, &html) {
                world.send_html(player, &message);
            }
            true
        }
        Err(exception) => {
            warn!("Exception in PlayerHelp: {exception}");
            true
        }
    }
}

/// Java `TutorialClose.onCommand`.
fn tutorial_close_handler(
    world: &mut World,
    player: ObjectId,
    _command: &str,
    _target: Option<ObjectId>,
) -> bool {
    world.send(player, &TutorialCloseHtml);
    false
}

/// Java `QuestList.onCommand`: only an `Adventurer` NPC opens the quest window.
fn quest_list_handler(world: &mut World, player: ObjectId, _command: &str, target: Option<ObjectId>) -> bool {
    let is_adventurer = target
        .and_then(|t| world.npc_template(t))
        .is_some_and(|t| t.npc_type == "Adventurer");
    if !is_adventurer {
        return false;
    }
    world.send(player, &ExShowQuestInfo);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_help_reads_the_page_and_the_item() {
        assert_eq!(player_help_target("player_help"), Ok(None));
        assert_eq!(player_help_target("player_help a.htm"), Ok(Some(("a.htm", 0))));
        assert_eq!(
            player_help_target("player_help a.htm#57"),
            Ok(Some(("a.htm", 57)))
        );
        assert_eq!(
            player_help_target("player_help a.htm  more"),
            Ok(Some(("a.htm", 0)))
        );
        // A trailing `#` is dropped by `String.split`.
        assert_eq!(player_help_target("player_help a.htm#"), Ok(Some(("a.htm", 0))));
        assert_eq!(player_help_target("player_help ../x.htm"), Ok(None));
    }

    #[test]
    fn player_help_errors_are_java_exceptions() {
        assert_eq!(
            player_help_target("player_help a.htm#x"),
            Err("java.lang.NumberFormatException: For input string: \"x\"".to_owned())
        );
        assert!(player_help_target("player_help      ").is_err());
    }
}
