//! `config/Custom/WalkerBotProtection.ini`: a whisper that starts with a L2Walker script command
//! is an illegal action.

use l2_util::ObjectId;

use crate::world::World;

/// Java `Say2.WALKER_COMMAND_LIST`.
const WALKER_COMMAND_LIST: [&str; 35] = [
    "USESKILL",
    "USEITEM",
    "BUYITEM",
    "SELLITEM",
    "SAVEITEM",
    "LOADITEM",
    "MSG",
    "DELAY",
    "LABEL",
    "JMP",
    "CALL",
    "RETURN",
    "MOVETO",
    "NPCSEL",
    "NPCDLG",
    "DLGSEL",
    "CHARSTATUS",
    "POSOUTRANGE",
    "POSINRANGE",
    "GOHOME",
    "SAY",
    "EXIT",
    "PAUSE",
    "STRINDLG",
    "STRNOTINDLG",
    "CHANGEWAITTYPE",
    "FORCEATTACK",
    "ISMEMBER",
    "REQUESTJOINPARTY",
    "REQUESTOUTPARTY",
    "QUITPARTY",
    "MEMBERSTATUS",
    "CHARBUFFS",
    "ITEMCOUNT",
    "FOLLOWTELEPORT",
];

/// Java `Say2.checkBot(text)`: the text starts with a command of the list (case sensitive).
pub(crate) fn check_bot(text: &str) -> bool {
    WALKER_COMMAND_LIST
        .iter()
        .any(|command| text.starts_with(command))
}

impl World {
    /// `Say2.runImpl`: with `L2WalkerProtection` a whisper that starts with a walker command is
    /// an illegal action (`DEFAULT_PUNISH`) and is not sent. `true` when the text was refused.
    pub(crate) fn walker_protection_refuses(&mut self, id: ObjectId, whisper: bool, text: &str) -> bool {
        if !(self.settings.custom_misc.l2walker_protection && whisper && check_bot(text)) {
            return false;
        }
        let message = format!("Client Emulator Detect: {} using L2Walker.", self.player_text(id));
        let punishment = self.default_punish();
        self.handle_illegal_player_action(id, &message, punishment);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_command_at_the_start_is_a_bot() {
        assert!(check_bot("USESKILL 1234"));
        assert!(check_bot("MSG hello"));
        // `startsWith`, so a longer word that begins with a command counts too.
        assert!(check_bot("MSGSOMETHING"));
        assert!(check_bot("FOLLOWTELEPORT"));
    }

    #[test]
    fn plain_text_is_not() {
        assert!(!check_bot("hello"));
        assert!(!check_bot("msg hello"));
        assert!(!check_bot(" USESKILL"));
        assert!(!check_bot(""));
    }
}
