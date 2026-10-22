//! The two shop windows: `BuyList` (the merchant's side) and `ExBuySellList` (the sell and refund
//! tabs), and the price config of a merchant.
// Java: network/serverpackets/{BuyList,ExBuySellList}.java
// Java: data/MerchantPriceConfigTable.java (getMerchantPriceConfig)
// Java: entity/itemcontainer/PlayerInventory.java (getAvailableItems)

use std::collections::HashSet;

use l2_data::MerchantPriceConfig;
use l2_protocol_game::server::{BuyList, BuyListProduct, ExBuySellList, ItemInfo, RefundEntry, SellEntry};
use l2_util::ObjectId;

use crate::data::ItemStatic;
use crate::enter::item_info;
use crate::inventory::LOC_INVENTORY;
use crate::world::{World, unix_ms_now};

/// The items `RatesConfig.RATE_SIEGE_GUARDS_PRICE` applies to.
pub(super) fn is_siege_guard_item(item_id: i32) -> bool {
    (3960..=4026).contains(&item_id)
}

impl World {
    /// Java `MerchantPriceConfigTable.getMerchantPriceConfig(npc)`: the first price config (lowest
    /// id) whose town zone is among the zones of the NPC's zone *region* (a zone counts when it
    /// touches the region, not only when the NPC stands inside it), else the default one.
    pub(crate) fn merchant_price_config(&self, npc: ObjectId) -> Option<MerchantPriceConfig> {
        let configs = self.shop.price_configs.as_ref()?;
        let zone_ids: HashSet<i32> = match (self.position_of(npc), self.zoning.zones.as_ref()) {
            (Some((x, y, _)), Some(zones)) => zones.region_zones(x, y).iter().map(|z| z.id).collect(),
            _ => HashSet::new(),
        };
        configs.for_zones(&zone_ids).cloned()
    }

    /// Java `MerchantPriceConfig.getTotalTaxRate()` of the merchant: `(castleTax + baseTax) / 100`.
    // M7: the castle's `taxPercent` (`hasCastle() ? _castle.getTaxPercent() + _baseTax : _baseTax`)
    // and, in `RequestBuyItem`, `castle.addToTreasury(subTotal * castleTaxRate)`.
    pub(crate) fn merchant_total_tax_rate(&self, npc: ObjectId) -> f64 {
        // Java would throw a NullPointerException for a merchant without a config.
        self.merchant_price_config(npc)
            .map_or(0.0, |config| f64::from(config.base_tax) / 100.0)
    }

    /// Java `new BuyList(buyList, player.getAdena(), taxRate)`.
    pub(crate) fn buy_list_packet(&self, player: ObjectId, list_id: i32, tax_rate: f64) -> Option<BuyList> {
        let list = self.shop.lists.as_ref()?.get(list_id)?;
        let templates = self.item_templates()?;
        let mut products = Vec::new();
        for def in list.products() {
            let count = self.product_count(list_id, def.item_id);
            // `(product.getCount() > 0) || !product.hasLimitedStock()`
            if count <= 0 && def.has_limited_stock() {
                continue;
            }
            let Some(template) = templates.get(def.item_id) else {
                continue;
            };
            let price = def.price_or(template.price);
            let shown = if is_siege_guard_item(def.item_id) {
                // `getPrice() * RATE_SIEGE_GUARDS_PRICE` is a float product.
                let guarded = (price as f32) * self.settings.shop.rate_siege_guards_price;
                (f64::from(guarded) * (1.0 + tax_rate)) as i64
            } else {
                (price as f64 * (1.0 + tax_rate)) as i64
            };
            products.push(BuyListProduct {
                item_id: def.item_id,
                count,
                type2: template.type2 as i16,
                type1: template.type1 as i16,
                body_part: template.body_part.mask(),
                enchant: template.default_enchant_level as i16,
                price: shown,
            });
        }
        let mut packet = BuyList::new(self.adena_of(player), list_id, products);
        // NOTE(java-quirk): the size counts the products that were left out.
        packet.size = u16::try_from(list.products().len()).unwrap_or(u16::MAX);
        Some(packet)
    }

    /// Java `new ExBuySellList(player, done)`: what the player can sell, and the refund list.
    pub(crate) fn ex_buy_sell_list_packet(&self, player: ObjectId, done: bool) -> Option<ExBuySellList> {
        let p = self.players.get(&player)?;
        let templates = self.item_templates();
        let zero = self.settings.shop.merchant_zero_sell_price;
        let now_ms = unix_ms_now();
        let info = |item: &crate::player::ItemData| -> ItemInfo {
            let stat = self
                .data
                .item(item.item_id)
                .unwrap_or_else(|| ItemStatic::unknown(item.item_id));
            item_info(
                item,
                &stat,
                self.data.stat_data().and_then(|d| d.items.get(item.item_id)),
                now_ms,
            )
        };

        let mut sell_list = Vec::new();
        for item in p.data.items.iter().filter(|i| i.loc == LOC_INVENTORY) {
            let Some(template) = templates.as_deref().and_then(|t| t.get(item.item_id)) else {
                continue;
            };
            // `getAvailableItems(false, false, false)`
            if !self.is_item_available(player, item, template, false, false)
                || !self.can_manipulate_with_item_id(player, item.item_id)
            {
                continue;
            }
            sell_list.push(SellEntry {
                item: info(item),
                price: if zero { 0 } else { i64::from(template.price / 2) },
            });
        }

        let mut refund_list = Vec::new();
        if self.has_refund(player) {
            for (index, item) in p.shop.refund.iter().enumerate() {
                let price = templates
                    .as_deref()
                    .and_then(|t| t.get(item.item_id))
                    .map_or(0, |t| i64::from(t.price / 2).wrapping_mul(item.count));
                refund_list.push(RefundEntry {
                    item: info(item),
                    index: i32::try_from(index).unwrap_or(i32::MAX),
                    price: if zero { 0 } else { price },
                });
            }
        }
        Some(ExBuySellList {
            sell_list,
            refund_list,
            done,
        })
    }

    /// Sends `new ExBuySellList(player, done)`.
    pub(crate) fn send_ex_buy_sell_list(&self, player: ObjectId, done: bool) {
        if let Some(packet) = self.ex_buy_sell_list_packet(player, done) {
            self.send(player, &packet);
        }
    }
}
