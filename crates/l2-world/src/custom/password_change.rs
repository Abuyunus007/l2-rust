//! `config/Custom/PasswordChange.ini`: the voiced command `.changepassword`. The change itself
//! is the login server's: the game server sends `ChangePassword` and shows the answer.
// Java: dist/game/data/scripts/handlers/chat/commands/voiced/ChangePassword.java
// Java: gameserver/LoginServerThread.java (sendChangePassword)
// Java: gameserver/network/loginserverpackets/receive/ChangePasswordResponse.java

use l2_protocol_game::server::NpcHtmlMessage;
use l2_util::ObjectId;

use super::misc::tokens;
use crate::outbox::LsEvent;
use crate::world::World;

/// The page the command shows without parameters.
const PAGE: &str = "data/html/mods/ChangePassword.htm";

impl World {
    /// `ChangePassword.onCommand`. `params` is Java's `target`: null without parameters.
    pub(crate) fn change_password_voiced(&mut self, player: ObjectId, params: Option<&str>) -> bool {
        let Some(params) = params else {
            let html = self.get_htm(player, PAGE).unwrap_or_else(|| {
                "<html><body><br><br><center><font color=LEVEL>404:</font> File Not Found</center></body></html>"
                    .to_owned()
            });
            if let Some(message) = NpcHtmlMessage::new(0, &html) {
                self.send_html(player, &message);
            }
            return true;
        };

        let mut parts = tokens(params).into_iter();
        let (current, new, repeat) = (parts.next(), parts.next(), parts.next());
        let (Some(current), Some(new), Some(repeat)) = (current, new, repeat) else {
            self.send_message(player, "Invalid password data! You have to fill all boxes.");
            return false;
        };
        if new != repeat {
            self.send_message(player, "The new password doesn't match with the repeated one!");
            return false;
        }
        // `String.length()` counts UTF-16 units.
        let length = new.encode_utf16().count();
        if length < 3 {
            self.send_message(
                player,
                "The new password is shorter than 3 chars! Please try with a longer one.",
            );
            return false;
        }
        if length > 30 {
            self.send_message(
                player,
                "The new password is longer than 30 chars! Please try with a shorter one.",
            );
            return false;
        }

        let Some((account, name)) = self
            .players
            .get(&player)
            .map(|p| (p.data.account.clone(), p.data.name.clone()))
        else {
            return false;
        };
        self.outbox.ls_event(LsEvent::ChangePassword {
            account,
            character: name,
            old_pass: current.to_owned(),
            new_pass: new.to_owned(),
        });
        true
    }

    /// `ChangePasswordResponse`: the login server's answer goes to the character as a message.
    pub(crate) fn on_change_password_response(&mut self, character: &str, text: &str) {
        if let Some(id) = self.player_by_name(character) {
            self.send_message(id, text);
        }
    }
}
