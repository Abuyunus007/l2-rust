//! The voiced commands `.expoff` and `.expon`, and the rule they switch.
// Java: dist/game/data/scripts/handlers/chat/commands/voiced/ExperienceGain.java
// Java: entity/actor/Player.java (disableExpGain, enableExpGain, onExperienceReceived)
// Java: entity/actor/stat/PlayableStat.java (addExp: the `ON_PLAYABLE_EXP_CHANGED` listener)
// Java: network/clientpackets/EnterWorld.java (the `EXPOFF` block)
// Java: dist/game/data/scripts/handlers/chat/channels/ChatGeneral.java (the voiced dispatch)
//
// Java adds a listener that terminates `addExp` while the player is alive. The listener exists
// exactly while the player variable `EXPOFF` is true (set by the command, restored at login), so
// the world reads the variable instead of keeping a listener.
//
// The other voiced commands (AutoPlay, Banking, ChatAdmin, Lang, ChangePassword, OfflinePlay,
// Offline, Online, Premium, AutoPotion) are registered by `MasterHandler` only when their config
// switch is on; with the default configs none is, so a `.word` is plain chat, as here.

use l2_util::ObjectId;

use crate::world::World;

/// The name of the player variable (`character_variables.var`).
pub(crate) const EXPOFF: &str = "EXPOFF";

/// `new StringTokenizer(text)`: whitespace set `" \t\n\r\f"`.
fn tokens(text: &str) -> Vec<&str> {
    text.split([' ', '\t', '\n', '\r', '\u{c}'])
        .filter(|t| !t.is_empty())
        .collect()
}

/// The `command` of `ChatGeneral.onChat` for a text that starts with `.`.
pub(crate) fn voiced_command_of(text: &str) -> &str {
    let tokens = tokens(text);
    if tokens.len() > 1 {
        tokens[0].get(1..).unwrap_or("")
    } else {
        text.get(1..).unwrap_or("")
    }
}

impl World {
    /// Java `getVariables().getBoolean("EXPOFF", false)`: `Boolean.parseBoolean` of the text.
    fn expoff_variable(&self, id: ObjectId) -> bool {
        self.players
            .get(&id)
            .and_then(|p| p.data.profile.variables.get(EXPOFF))
            .is_some_and(|v| v.eq_ignore_ascii_case("true"))
    }

    /// Whether the listener of `disableExpGain` stops `PlayableStat.addExp`: it does while the
    /// variable is on and the player is alive (`onExperienceReceived`).
    pub(crate) fn exp_gain_blocked(&self, id: ObjectId) -> bool {
        self.expoff_variable(id) && !self.is_dead(id)
    }

    /// The `ExperienceGain` handler of `ChatGeneral.onChat`'s voiced commands. `true` when a
    /// handler took the text (`vcdUsed`).
    pub(crate) fn script_voiced_command(&mut self, player: ObjectId, text: &str) -> bool {
        if !text.starts_with('.') {
            return false;
        }
        self.script_voiced_handler(player, voiced_command_of(text))
    }

    /// `VoicedCommandHandler.getHandler(command)` and `onCommand` of the voiced commands of this
    /// module (`ExperienceGain`, which always answers `true`). `false`: no handler.
    pub(crate) fn script_voiced_handler(&mut self, player: ObjectId, command: &str) -> bool {
        // `VoicedCommandHandler.getHandler`: the text up to the first space.
        let word = command.split(' ').next().unwrap_or(command);
        // `Wedding` registers `engage`, `divorce` and `gotolove` (with `AllowWedding`) and answers
        // with its own result.
        if let Some(result) = self.wedding_voiced_handler(player, word) {
            return result;
        }
        // `HellboundLoader` registers `hellbound` (S-API-05, `script/hellbound_api.rs`).
        if let Some(result) = self.hellbound_voiced_handler(player, word) {
            return result;
        }
        if !matches!(word, "expoff" | "expon") {
            return false;
        }
        // NOTE(java-quirk): the handler compares the whole command, so `.expoff ` (a trailing
        // space, one token) does nothing but still counts as used.
        if command == "expoff" {
            if !self.expoff_variable(player) {
                self.set_player_variable_text(player, EXPOFF, "true".to_owned());
                self.send_message(player, "Experience gain is disabled.");
            }
        } else if command == "expon" && self.expoff_variable(player) {
            self.set_player_variable_text(player, EXPOFF, "false".to_owned());
            self.send_message(player, "Experience gain is enabled.");
        }
        true
    }

    /// `EnterWorld`: "Check if expoff is enabled."
    pub(crate) fn exp_gain_on_enter(&mut self, id: ObjectId) {
        if self.expoff_variable(id) {
            self.send_message(id, "Experience gain is disabled.");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_command_of_a_text() {
        assert_eq!(voiced_command_of(".expoff"), "expoff");
        assert_eq!(voiced_command_of(".expoff now"), "expoff");
        // One token with a trailing space keeps the space.
        assert_eq!(voiced_command_of(".expoff "), "expoff ");
        assert_eq!(voiced_command_of("."), "");
    }
}
