//! The packets of the trade window: `TradeStart` with the items a player may offer, and the
//! item layout of `TradeOwnAdd` / `TradeOtherAdd` / `TradeUpdate`.
// Java: network/serverpackets/{TradeStart,TradeOwnAdd,TradeOtherAdd,TradeUpdate,
//       AbstractItemPacket}.java
// Java: entity/itemcontainer/PlayerInventory.java (getAvailableItems)
// Java: entity/item/instance/Item.java (isAvailable)

use l2_data::item::ADENA_ID;
use l2_data::item::{ItemTemplate, TYPE1_SHIELD_ARMOR, TYPE2_MONEY, TYPE2_QUEST};
use l2_protocol_game::server::{ItemInfo, TradeItemInfo, TradeStart};
use l2_util::ObjectId;

use super::TradeItem;
use crate::data::ItemStatic;
use crate::enter::item_info;
use crate::inventory::{LOC_INVENTORY, LOC_PAPERDOLL};
use crate::player::ItemData;
use crate::world::{World, unix_ms_now};

impl World {
    /// Java `writeTradeItem(TradeItem)`'s fields.
    pub(crate) fn trade_item_info(&self, item: &TradeItem) -> TradeItemInfo {
        let stat = self
            .data
            .item(item.item.item_id)
            .unwrap_or_else(|| ItemStatic::unknown(item.item.item_id));
        let template = self.data.stat_data().and_then(|d| d.items.get(item.item.item_id));
        TradeItemInfo {
            // `getItem().getType1()`; an unknown item is Java's "other" kind.
            type1: template.map_or(4, |t| t.type1 as i16),
            object_id: item.object_id.0,
            item_id: stat.display_id,
            count: item.count,
            type2: stat.type2,
            // Java writes the low 16 bits of these ints.
            custom_type1: item.item.custom_type1 as i16,
            body_part: stat.body_part,
            enchant: item.item.enchant_level as i16,
            custom_type2: item.item.custom_type2 as i16,
            attack_element_type: item.item.attack_element_type(template) as i16,
            attack_element_power: item.item.attack_element_power(template) as i16,
            element_defense: std::array::from_fn(|element| {
                item.item.element_def_attr(element as i32, template) as i16
            }),
            // `Item.DEFAULT_ENCHANT_OPTIONS`: enchant options are not modelled.
            enchant_options: [0; 3],
        }
    }

    /// Java `Item.isAvailable(player, allowAdena, allowNonTradeable)` for an item of `player`.
    // Not modelled (nothing sets them yet): the control item of a summoned pet, the enchant scroll,
    // support item and attribute stone in use, the skill consumed by `getLastSimultaneousSkillCast`,
    // and `havePetInvItems` for a pet collar.
    fn is_item_available(
        &self,
        player: ObjectId,
        item: &ItemData,
        template: &ItemTemplate,
        allow_adena: bool,
        allow_non_tradeable: bool,
    ) -> bool {
        // Not equipped.
        if item.loc != LOC_INVENTORY {
            return false;
        }
        // Not a quest item, and not money that is a shield.
        if template.type2 == TYPE2_QUEST
            || (template.type2 == TYPE2_MONEY && template.type1 == TYPE1_SHIELD_ARMOR)
        {
            return false;
        }
        // Not adena.
        if !allow_adena && item.item_id == ADENA_ID {
            return false;
        }
        // Not the item the current skill consumes.
        if let Some(current) = self.current_skill(player)
            && let Some(table) = self.skill_table()
            && let Some(skill) = table.get(current.skill.id, current.skill.level)
            && skill.item_consume_id == item.item_id
        {
            return false;
        }
        // Tradeable (a pet collar is excluded only while the pet has items, not modelled).
        allow_non_tradeable || self.item_is_tradeable(item, template)
    }

    /// Java `new TradeStart(player)`: the packet for a player who has just begun to trade with
    /// `partner`. The item list is `getAvailableItems(true, isGM && GMTradeRestrictedItems,
    /// false)`.
    pub(crate) fn trade_start_packet(&self, player: ObjectId, partner: ObjectId) -> Option<TradeStart> {
        let p = self.players.get(&player)?;
        let allow_non_tradeable = self.is_gm(player) && self.settings.inventory.gm_trade_restricted_items;
        let templates = self.item_templates();
        let now_ms = unix_ms_now();
        let items: Vec<ItemInfo> = p
            .data
            .items
            .iter()
            .filter(|i| i.loc == LOC_INVENTORY || i.loc == LOC_PAPERDOLL)
            .filter(|i| {
                templates
                    .as_deref()
                    .and_then(|t| t.get(i.item_id))
                    .is_some_and(|t| self.is_item_available(player, i, t, true, allow_non_tradeable))
            })
            .map(|i| {
                let stat = self
                    .data
                    .item(i.item_id)
                    .unwrap_or_else(|| ItemStatic::unknown(i.item_id));
                item_info(
                    i,
                    &stat,
                    self.data.stat_data().and_then(|d| d.items.get(i.item_id)),
                    now_ms,
                )
            })
            .collect();
        Some(TradeStart {
            partner_object_id: partner.0,
            items,
        })
    }
}
