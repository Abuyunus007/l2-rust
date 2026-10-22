//! NPC shops: the buy lists of merchants, buying, selling and the refund list (M6-SHOP-01).
// Java: data/xml/BuyListData.java, mechanics/buylist/{BuyListHolder,Product}.java
// Java: taskmanagers/BuyListTaskManager.java
// Java: entity/actor/instance/Merchant.java (showBuyWindow)
// Java: network/clientpackets/{RequestBuyItem,RequestSellItem,RequestRefundItem,
//       RequestBuySellUIClose}.java
// Java: network/serverpackets/{BuyList,ExBuySellList}.java
// Java: entity/itemcontainer/PlayerRefund.java
// Java: dist/game/data/scripts/handlers/bypass/npc/Buy.java
//
// # Model
// The buy lists are definitions in `l2_data::BuyLists` (immutable, shared by `Arc`). What changes
// is the stock of the *limited* products (`count` in the XML) and their restock times: that is
// [`ShopWorld`], keyed by `(list id, item id)`, with Java's `buylists` table row written through
// the outbox ([`ProductRow`]) every time a stock changes. Java's `BuyListTaskManager` (a 60 s scan
// that queues the due products and a 50 ms task that restocks one of them) runs on the timer
// wheel ([`ShopTimer`]).
//
// The refund list of a player (`Player._refund`, a `PlayerRefund` item container) is
// [`ShopState::refund`], a list of the sold items in the order they were sold. Java keeps them as
// rows with `loc = REFUND`; here the row of a sold item is deleted when it is sold and written
// again (as `INVENTORY`) when it is refunded. The persistence task forgets what it wrote after
// every full store, so a `REFUND` row could never be deleted again, and Java never reads such a
// row back (`PlayerRefund.restore` does nothing, `deleteMe` destroys the items when the player
// leaves). The result is the same: a sold item is gone from the database as long as it is in the
// refund list, and gone for good when the player leaves.
//
// # What is left out, with the system that brings it
// - The castle tax. `MerchantPriceConfig.getTotalTaxRate()` adds the castle's `taxPercent` and the
//   treasury gets `subTotal * castleTaxRate`; castles are M7 (`// M7:` where it belongs). The
//   base tax of the town (`MerchantPriceConfig.xml`) is applied.
// - `ItemProcessType` logging (`LogItems`) and the GM audit.
// - `Wear` (`ShopPreviewList`) and `BuyShadowItem`: separate bypasses, not in this card.

mod buy;
mod packets;
mod refund;
mod sell;
mod stock;

use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;

use l2_data::{BuyLists, MerchantPriceConfigs};
use l2_protocol_game::client::{RequestBuyItem, RequestRefundItem, RequestSellItem};

pub use buy::buy_handler;
pub use stock::ProductRow;

use crate::player::ItemData;
use crate::timer::TimerId;
use crate::world::World;

/// `RequestBuyItem.CUSTOM_CB_SELL_LIST` (and the same constant of the other two packets): the list
/// id the community board uses; it needs no merchant.
pub(crate) const CUSTOM_CB_SELL_LIST: i32 = 423;

/// `PlayerRefund.addItem`: the refund list keeps 12 items.
pub(crate) const REFUND_LIST_SIZE: usize = 12;

/// `InventoryEnableTask`: the inventory comes back 1.5 s after a shop window opened.
pub(crate) const INVENTORY_BLOCK_MS: u64 = 1500;

/// Java `BuyListTaskManager`: the scan of the products waiting for a restock starts 1 s after the
/// manager and then runs every 60 s.
pub(crate) const RESTOCK_SCAN_FIRST_MS: u64 = 1000;
pub(crate) const RESTOCK_SCAN_PERIOD_MS: u64 = 60_000;

/// The values the shops read (Java `GeneralConfig`, `RatesConfig`, `MerchantZeroSellPriceConfig`).
/// `Default` is the Java default of each key.
#[derive(Debug, Clone, PartialEq)]
pub struct ShopSettings {
    /// `GeneralConfig.ALLOW_REFUND` (`AllowRefund`): sold items go to the refund list.
    pub allow_refund: bool,
    /// `GeneralConfig.ONLY_GM_ITEMS_FREE` (`OnlyGMItemsFree`): a buy list price of 0 is an
    /// exploit for everybody but a GM.
    pub only_gm_items_free: bool,
    /// `RatesConfig.RATE_SIEGE_GUARDS_PRICE` (`RateSiegeGuardsPrice`): the price of the siege
    /// guard items 3960 to 4026.
    pub rate_siege_guards_price: f32,
    /// `MerchantZeroSellPriceConfig.MERCHANT_ZERO_SELL_PRICE` (`MerchantZeroSellPrice`): merchants
    /// pay nothing for items.
    pub merchant_zero_sell_price: bool,
    /// Test hook: the clock (Unix ms) the restock times use instead of the real one.
    pub clock_override_unix_ms: Option<i64>,
}

impl Default for ShopSettings {
    fn default() -> Self {
        Self {
            allow_refund: true,
            only_gm_items_free: true,
            rate_siege_guards_price: 1.0,
            merchant_zero_sell_price: false,
            clock_override_unix_ms: None,
        }
    }
}

/// What the shop keeps for one player: Java `Player._refund` and `_inventoryDisable`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ShopState {
    /// The items the player sold to merchants, oldest first (`loc` is `REFUND`).
    pub refund: Vec<ItemData>,
    /// Java `isInventoryDisabled()`: a shop window opened less than 1.5 s ago.
    pub inventory_disabled: bool,
}

/// The shop packets as the session hands them to the world.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShopInput {
    /// Java `RequestBuyItem`.
    Buy(RequestBuyItem),
    /// Java `RequestSellItem`.
    Sell(RequestSellItem),
    /// Java `RequestRefundItem`.
    Refund(RequestRefundItem),
    /// Java `RequestBuySellUIClose`.
    UiClose,
}

/// The shop timers (see the module notes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShopTimer {
    /// Java `BuyListProductTask`: queue the products whose restock time passed.
    RestockScan,
    /// Java `BuyListSaveTask`: restock the first queued product.
    RestockSave,
    /// Java `InventoryEnableTask`.
    InventoryEnable(l2_util::ObjectId),
}

/// The limited products' stock and the restock queue (Java `Product._count` and
/// `BuyListTaskManager`).
#[derive(Default)]
pub(crate) struct ShopWorld {
    pub lists: Option<Arc<BuyLists>>,
    pub price_configs: Option<Arc<MerchantPriceConfigs>>,
    /// `Product._count` of the limited products that were touched; a product that is not here has
    /// its full stock. May go below 0 (Java reads it clamped).
    pub counts: BTreeMap<(i32, i32), i64>,
    /// `BuyListTaskManager.PRODUCTS`: the time (ms) after which a product restocks. After a
    /// restart the value is not a time (see `restart_restock_task`).
    pub restock_at: BTreeMap<(i32, i32), i64>,
    /// `BuyListTaskManager.PENDING_UPDATES`.
    pub pending: VecDeque<(i32, i32)>,
    /// The task manager exists (Java creates it on first use).
    pub started: bool,
    /// The timer of the 50 ms save task while products wait.
    pub save_timer: Option<TimerId>,
}

impl World {
    /// Hands the world the buy lists and the merchant price configs (Java `BuyListData`,
    /// `MerchantPriceConfigTable`). Call before players enter.
    pub fn set_buy_lists(&mut self, lists: Arc<BuyLists>, price_configs: Arc<MerchantPriceConfigs>) {
        self.shop.lists = Some(lists);
        self.shop.price_configs = Some(price_configs);
    }

    /// `ClientInput::Shop`.
    pub(crate) fn on_shop_input(&mut self, id: l2_util::ObjectId, input: ShopInput) {
        match input {
            ShopInput::Buy(request) => self.on_request_buy_item(id, request),
            ShopInput::Sell(request) => self.on_request_sell_item(id, request),
            ShopInput::Refund(request) => self.on_request_refund_item(id, request),
            ShopInput::UiClose => self.on_request_buy_sell_ui_close(id),
        }
    }

    /// Java `Player.setInventoryBlockingStatus(true)`: the inventory is disabled for 1.5 s. Every
    /// call schedules its own `InventoryEnableTask`, as in Java.
    pub(crate) fn set_inventory_blocking_status(&mut self, id: l2_util::ObjectId) {
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        player.shop.inventory_disabled = true;
        let due = self.tick + crate::timer::TimerWheel::ms_to_ticks(INVENTORY_BLOCK_MS);
        self.timers
            .schedule_at(due, crate::timer::TimerKind::Shop(ShopTimer::InventoryEnable(id)));
    }

    /// Java `Player.isInventoryDisabled()`.
    pub(crate) fn is_inventory_disabled(&self, id: l2_util::ObjectId) -> bool {
        self.players.get(&id).is_some_and(|p| p.shop.inventory_disabled)
    }

    /// Java `RequestBuySellUIClose.runImpl`.
    fn on_request_buy_sell_ui_close(&mut self, id: l2_util::ObjectId) {
        if !self.players.contains_key(&id) || self.is_inventory_disabled(id) {
            return;
        }
        self.send_item_list(id, true);
    }

    /// The timers of `TimerKind::Shop`.
    pub(crate) fn on_shop_timer(&mut self, timer: ShopTimer) {
        match timer {
            ShopTimer::RestockScan => self.on_restock_scan(),
            ShopTimer::RestockSave => self.on_restock_save(),
            ShopTimer::InventoryEnable(id) => {
                if let Some(player) = self.players.get_mut(&id) {
                    player.shop.inventory_disabled = false;
                }
            }
        }
    }
}
