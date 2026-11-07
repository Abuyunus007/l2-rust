//! The voiced command `.companion`: the combat mode of a player's companions.
//!
//! Own feature (D-61), not from the Java datapack. The world only routes the command: the
//! companions and their mode are the script `Companions`'s. `.companion aggressive` and
//! `.companion passive` switch it, `.companion` alone says which it is.

use l2_util::ObjectId;

use crate::world::World;

/// The script that answers `.companion`.
const SCRIPT: &str = "Companions";

impl World {
    /// `.companion [mode]`: sends the script the event `mode:<word>`. `true`: the command is the
    /// script's, also when the script is not installed (a text with a dot must not become chat
    /// then, as with the other voiced commands of this server).
    pub(crate) fn companion_voiced_command(&mut self, player: ObjectId, params: Option<&str>) -> bool {
        if let Some(script) = self.scripts.by_name(SCRIPT) {
            let word = params.map(str::trim).unwrap_or_default();
            self.script_notify_event(script, &format!("mode:{word}"), None, Some(player));
        }
        true
    }
}
