//! The `Maps`, `Calculator`, `Bypass`, `SevenSignsRecord`, `SpecialXMas` and `TeleportBookmark`
//! handlers: a double click opens a client window (or, for the last one, adds teleport slots).
// Java: dist/game/data/scripts/handlers/items/Maps.java
// Java: dist/game/data/scripts/handlers/items/Calculator.java
// Java: dist/game/data/scripts/handlers/items/{Bypass,SevenSignsRecord,SpecialXMas,TeleportBookmark}.java
// Java: network/serverpackets/ShowMiniMap.java, ShowCalculator.java
//
// Both start with `if (!playable.isPlayer())` (a pet cannot carry the item). `Playable` is always
// a player here (summons are M6), so that branch is left out.

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{
    NpcHtmlMessage, ShowCalculator, ShowMiniMap, ShowXMasSeal, SystemMessage, SystemMessageParam,
};
use l2_util::ObjectId;

use crate::player::ItemData;
use crate::world::World;

impl World {
    /// Java `Bypass.onItemUse`: the page of the item (`data/html/item/<itemId>.htm`, with
    /// `%itemId%` the object id), or Java's "My Text is missing" page.
    pub(crate) fn bypass_item_on_use(&mut self, id: ObjectId, item: &ItemData) -> bool {
        let filename = format!("data/html/item/{}.htm", item.item_id);
        let html = match self.get_htm(id, &filename) {
            Some(content) => content.replace("%itemId%", &item.object_id.0.to_string()),
            None => format!("<html><body>My Text is missing:<br>{filename}</body></html>"),
        };
        if let Some(message) = NpcHtmlMessage::new_item(0, item.item_id, &html) {
            self.send_html(id, &message);
        }
        true
    }

    /// Java `SevenSignsRecord.onItemUse`: `SSQStatus(objectId, 1)`.
    pub(crate) fn seven_signs_record_on_use(&mut self, id: ObjectId) -> bool {
        self.on_request_ssq_status(id, 1);
        true
    }

    /// Java `SpecialXMas.onItemUse`: `ShowXMasSeal(item.getId())`.
    pub(crate) fn special_xmas_on_use(&mut self, id: ObjectId, item: &ItemData) -> bool {
        self.send(
            id,
            &ShowXMasSeal {
                item_id: item.item_id,
            },
        );
        true
    }

    /// Java `TeleportBookmark.onItemUse`: at most 9 slots; the scroll is used up and three slots
    /// are added.
    pub(crate) fn teleport_bookmark_item_on_use(&mut self, id: ObjectId, item: &ItemData) -> bool {
        let slots = self.players.get(&id).map_or(0, |p| p.data.profile.bookmark_slot);
        if slots >= 9 {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YourNumberOfMyTeleportsSlotsHasReachedItsMaximumLimit),
            );
            return false;
        }
        // `player.destroyItem(NONE, objectId, 1, null, false)`: its result is not checked.
        self.destroy_item_by_object_id(id, item.object_id, 1, false);
        if let Some(player) = self.players.get_mut(&id) {
            player.data.profile.bookmark_slot = slots + 3;
        }
        self.send(
            id,
            &SystemMessage::new(SystemMessageId::TheNumberOfMyTeleportsSlotsHasBeenIncreased),
        );
        self.send(
            id,
            &SystemMessage::new(SystemMessageId::S1HasDisappeared)
                .with(SystemMessageParam::ItemName(item.item_id)),
        );
        true
    }

    /// Java `Maps.onItemUse`: `playable.sendPacket(new ShowMiniMap(item.getId()))`.
    pub(crate) fn maps_on_use(&mut self, id: ObjectId, item: &ItemData) -> bool {
        self.send(
            id,
            &ShowMiniMap {
                map_id: item.item_id,
                // Java `SevenSigns.getInstance().getCurrentPeriod()` (M8-SS-01).
                seven_signs_period: self.ss_current_period() as u8,
            },
        );
        true
    }

    /// Java `Calculator.onItemUse`: `playable.sendPacket(new ShowCalculator(item.getId()))`.
    pub(crate) fn calculator_on_use(&mut self, id: ObjectId, item: &ItemData) -> bool {
        self.send(
            id,
            &ShowCalculator {
                calculator_id: item.item_id,
            },
        );
        true
    }
}
