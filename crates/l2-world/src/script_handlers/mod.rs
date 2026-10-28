//! The script handlers S-HAND-02 ports (the ones S-HAND-01 left): newbie buffs, the voiced
//! `.expon`/`.expoff`, the escape and birthday user commands, and the item handlers.
// Java: dist/game/data/scripts/handlers/{bypass/npc,chat/commands,items}/*.java

pub(crate) mod charm;
pub(crate) mod dice;
pub(crate) mod event_item;
pub(crate) mod expoff;
pub(crate) mod npc;
pub(crate) mod find_pvp;
pub(crate) mod rent;
pub(crate) mod support;
pub mod unstuck;

use l2_protocol_game::server::ActionFailed;
use l2_util::ObjectId;

use crate::world::World;

impl World {
    /// Java `Npc.showChatWindow(player, filename)`: the page of the file (the "missing" page when
    /// it is not loaded) with `%objectId%` replaced, then `ActionFailed`.
    pub(crate) fn npc_show_chat_window_file(&mut self, npc: ObjectId, player: ObjectId, path: &str) {
        if let Some(message) = self.html_from_file(player, npc.0, path) {
            let html = message.html.replace("%objectId%", &npc.0.to_string());
            if let Some(message) = l2_protocol_game::server::NpcHtmlMessage::new(npc.0, &html) {
                self.send_html(player, &message);
            }
        }
        self.send(player, &ActionFailed);
    }
}
