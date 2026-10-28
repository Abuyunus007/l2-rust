//! The small NPC bypasses `BuyShadowItem`, `Wear`, `ReceivePremium` and `voice`.
// Java: dist/game/data/scripts/handlers/bypass/npc/BuyShadowItem.java
// Java: dist/game/data/scripts/handlers/bypass/npc/Wear.java
// Java: dist/game/data/scripts/handlers/bypass/npc/ReceivePremium.java
// Java: dist/game/data/scripts/handlers/bypass/npc/VoiceCommand.java
// Java: network/serverpackets/ShopPreviewList.java

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{ActionFailed, ShopPreviewList, ShopPreviewProduct, SystemMessage};
use l2_util::ObjectId;
use tracing::warn;

use crate::bypass::BypassHandlers;
use crate::world::World;

/// `ItemTemplate.TYPE1_ITEM_QUESTITEM_ADENA`.
const TYPE1_ITEM_QUESTITEM_ADENA: i32 = 4;
/// The Expertise skill (`Player.getExpertiseLevel()` is its level).
const EXPERTISE_SKILL: i32 = 239;

/// Registers the handlers of this module (called by `BypassHandlers::default`).
pub(crate) fn register(handlers: &mut BypassHandlers) {
    handlers.register("BuyShadowItem", buy_shadow_item_handler);
    handlers.register("Wear", wear_handler);
    handlers.register("ReceivePremium", receive_premium_handler);
    handlers.register("voice", voice_handler);
}

/// Java `BuyShadowItem.onCommand`: the page of the shadow items the player's level may buy.
fn buy_shadow_item_handler(
    world: &mut World,
    player: ObjectId,
    _command: &str,
    target: Option<ObjectId>,
) -> bool {
    let Some(npc) = target.filter(|t| world.npc_is_merchant(*t)) else {
        return false;
    };
    let level = world.players.get(&player).map_or(1, |p| p.core.level);
    // NOTE(java-quirk): the four `if`s cover every level, so the page is always set.
    let file = if level < 40 {
        "data/html/common/shadow_item-lowlevel.htm"
    } else if level < 46 {
        "data/html/common/shadow_item_d.htm"
    } else if level < 52 {
        "data/html/common/shadow_item_c.htm"
    } else {
        "data/html/common/shadow_item_b.htm"
    };
    if let Some(message) = world.html_from_file(player, npc.0, file) {
        let html = message.html.replace("%objectId%", &npc.0.to_string());
        if let Some(message) = l2_protocol_game::server::NpcHtmlMessage::new(npc.0, &html) {
            world.send_html(player, &message);
        }
    }
    true
}

/// Java `Wear.onCommand`.
fn wear_handler(world: &mut World, player: ObjectId, command: &str, target: Option<ObjectId>) -> bool {
    if !target.is_some_and(|t| world.npcs.contains_key(&t)) {
        return false;
    }
    if !world.script_handlers.settings.allow_wear {
        return false;
    }
    // `new StringTokenizer(command, " ")`, `st.nextToken()`.
    let mut tokens = command.split(' ').filter(|t| !t.is_empty());
    tokens.next();
    let Some(value) = tokens.next() else {
        return false;
    };
    match value.parse::<i32>() {
        Ok(list_id) => {
            world.show_wear_window(player, list_id);
            true
        }
        Err(_) => {
            warn!("Exception in Wear: java.lang.NumberFormatException: For input string: \"{value}\"");
            false
        }
    }
}

impl World {
    /// Java `Wear.showWearWindow`.
    fn show_wear_window(&mut self, player: ObjectId, list_id: i32) {
        let Some(packet) = self.shop_preview_list(player, list_id) else {
            warn!("BuyList not found! BuyListId:{list_id}");
            self.send(player, &ActionFailed);
            return;
        };
        self.set_inventory_blocking_status(player);
        self.send(player, &packet);
    }

    /// Java `new ShopPreviewList(buyList, player.getAdena(), player.getExpertiseLevel())`.
    pub(crate) fn shop_preview_list(&self, player: ObjectId, list_id: i32) -> Option<ShopPreviewList> {
        let list = self.shop.lists.as_ref()?.get(list_id)?;
        let templates = self.item_templates()?;
        let expertise = self.stats(player).map_or(0, |v| v.skill_level(EXPERTISE_SKILL));
        let price = i64::from(self.settings.leftover.wear_price);
        let mut products = Vec::new();
        for def in list.products() {
            let Some(template) = templates.get(def.item_id) else {
                continue;
            };
            if template.crystal_type.level() <= expertise && template.is_equipable() {
                products.push(ShopPreviewProduct {
                    item_id: def.item_id,
                    type2: template.type2 as i16,
                    body_part: if template.type1 != TYPE1_ITEM_QUESTITEM_ADENA {
                        template.body_part.mask() as i16
                    } else {
                        0
                    },
                    price,
                });
            }
        }
        Some(ShopPreviewList {
            money: self.adena_of(player),
            list_id,
            products,
        })
    }
}

/// Java `ReceivePremium.onCommand`. The premium item list (`character_premium_items`, a premium
/// system feature) is not modelled, so the list is always empty and the answer is Java's for an
/// empty list.
fn receive_premium_handler(
    world: &mut World,
    player: ObjectId,
    _command: &str,
    target: Option<ObjectId>,
) -> bool {
    if !target.is_some_and(|t| world.npcs.contains_key(&t)) {
        return false;
    }
    world.send(
        player,
        &SystemMessage::new(SystemMessageId::ThereAreNoMoreDimensionalItemsToBeFound),
    );
    false
}

/// Java `VoiceCommand.onCommand`: `voice .<command> <params>` runs a voiced command from an NPC
/// page. Only the voiced commands this server has are known (`.expoff`, `.expon`); the handler
/// answers what the voiced handler answers.
fn voice_handler(world: &mut World, player: ObjectId, command: &str, _target: Option<ObjectId>) -> bool {
    // only voice commands allowed
    if command.len() > 7 && command.as_bytes().get(6) == Some(&b'.') {
        let rest = command.get(7..).unwrap_or("");
        // `command.indexOf(' ', 7)`: the first space after the dot.
        let vc = match rest.find(' ') {
            Some(end) => rest[..end].trim(),
            None => rest.trim(),
        };
        if !vc.is_empty() {
            return world.script_voiced_handler(player, vc);
        }
    }
    false
}
