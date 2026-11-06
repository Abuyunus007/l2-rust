//! The voiced command `.buffme`: the class buffer's page for a player anywhere.
//!
//! Own feature (D-60), not from the Java datapack. The world only routes the command: the pages,
//! the price and the buffs are the script `ClassBuffer`'s. What follows the word is the click on
//! the page (`.buffme show tank`, `.buffme buff tank`), so the buttons of the page can use the
//! `voice` bypass, which needs no NPC next to the player (a `Script` link does).

use l2_util::ObjectId;

use crate::world::World;

/// The script that answers `.buffme`.
const SCRIPT: &str = "ClassBuffer";

impl World {
    /// `.buffme [click]`: sends the script the event `p-<click>` (`p-` is the paid mode of the
    /// script; no click is the first page). `true`: the command is the script's, also when the
    /// script is not installed (a text with a dot must not become chat then, as with the other
    /// voiced commands of this server).
    pub(crate) fn buffme_voiced_command(&mut self, player: ObjectId, params: Option<&str>) -> bool {
        if let Some(script) = self.scripts.by_name(SCRIPT) {
            let click = params.map(str::trim).filter(|c| !c.is_empty()).unwrap_or("main");
            self.script_notify_event(script, &format!("p-{click}"), None, Some(player));
        }
        true
    }
}
