//! The `Book` handler: the help page of an item.
// Java: dist/game/data/scripts/handlers/items/Book.java

use l2_protocol_game::server::{ActionFailed, NpcHtmlMessage};
use l2_util::ObjectId;

use crate::player::ItemData;
use crate::world::World;

impl World {
    /// Java `Book.onItemUse`: `data/html/help/<itemId>.htm`, or a "missing" page.
    pub(crate) fn book_on_use(&mut self, id: ObjectId, item: &ItemData) -> bool {
        // Pets cannot read: `Playable` is always a player here.
        let filename = format!("data/html/help/{}.htm", item.item_id);
        let content = self
            .get_htm(id, &filename)
            .unwrap_or_else(|| format!("<html><body>My Text is missing:<br>{filename}</body></html>"));
        if let Some(html) = NpcHtmlMessage::new_item(0, item.item_id, &content) {
            self.send_html(id, &html);
        }
        self.send(id, &ActionFailed);
        true
    }
}
