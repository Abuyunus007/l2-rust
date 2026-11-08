//! The voiced command `.companion`: the companions' page anywhere, and their combat mode.
//!
//! Own feature (D-61), not from the Java datapack. The world only routes the command: the
//! companions are the script `Companions`'s. `.companion` opens the hire page (its links are the
//! command again: `.companion hire 47`, `.companion dismiss 0`), `.companion aggressive` and
//! `.companion passive` switch the combat mode.

use l2_util::ObjectId;

use crate::world::World;

/// The script that answers `.companion`.
const SCRIPT: &str = "Companions";

impl World {
    /// `.companion [words]`: sends the script the event `p-<words>` (`p-main` without words).
    /// `true`: the command is the script's, also when the script is not installed (a text with a
    /// dot must not become chat then, as with the other voiced commands of this server).
    pub(crate) fn companion_voiced_command(&mut self, player: ObjectId, params: Option<&str>) -> bool {
        if let Some(script) = self.scripts.by_name(SCRIPT) {
            let words = params.map(str::trim).filter(|w| !w.is_empty()).unwrap_or("main");
            self.script_notify_event(script, &format!("p-{words}"), None, Some(player));
        }
        true
    }
}
