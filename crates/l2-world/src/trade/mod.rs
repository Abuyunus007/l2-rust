//! Private trade between two players: the request, the trade window, lock/confirm and the item
//! exchange (M6-TRADE-01).
// Java: network/clientpackets/{TradeRequest,AnswerTradeRequest,AddTradeItem,TradeDone}.java
// Java: network/holders/{TradeList,TradeItem}.java
// Java: network/serverpackets/{TradeStart,TradeOwnAdd,TradeOtherAdd,TradeUpdate,TradeOtherDone,
//       TradeDone,SendTradeRequest}.java
// Java: entity/actor/Player.java (the transaction state: getActiveRequester, isProcessingRequest,
//       isProcessingTransaction, onTransactionRequest, onTransactionResponse, startTrade,
//       onTradeStart, onTradeConfirm, onTradeCancel, onTradeFinish, cancelActiveTrade)
//
// # Model
// `Player._activeTradeList`, `_activeRequester`, `_requestExpireTime` and `_tradeRefusal` are one
// [`TradeState`] on the [`Player`](crate::Player). A [`TradeList`] holds ids only (the owner, the
// partner, the [`TradeItem`]s), so "is the partner still in the world" is a lookup, never a
// dangling reference (D-11). Everything that needs the inventory is a `World` method that gets the
// list's owner by id.
//
// - `request.rs`: `TradeRequest`, `AnswerTradeRequest` and the transaction state they use.
// - `window.rs`: `AddTradeItem`, `TradeDone`, `TradeList.addItem` / `confirm` / `validate`.
// - `exchange.rs`: `TradeList.doExchange`, `TransferItems`, weight and slot counting.
// - The item move itself is `World::transfer_item` (`inventory/transfer.rs`), which the private
//   stores call as well.
// - `store.rs`, `store_lists.rs`, `store_deal.rs`: the private stores (M6-STORE-01): opening and
//   closing, the lists the owner sets, the lists a customer sees and `TradeList.privateStoreBuy`
//   / `privateStoreSell`.
//
// # What Java does *not* do (so neither does this)
// Java cancels a trade only in `Player.deleteMe` (and then only `if (getActiveRequester() !=
// null)`), when a duel starts, and in `RequestExEnchantItemAttribute`. Moving away, dying,
// teleporting or attacking does **not** cancel it: `TradeDone` re-checks the 150 range when a
// player confirms, which is the only distance check after the request. See
// `World::cancel_active_trade`.
//
// # Left out on purpose (each has a comment where it belongs)
// - (the olympiad checks are in: `request.rs`, `store.rs`, M8-OLY-01)
//   a summoned pet's control item and a mounted strider's saddle (no summons or mounts yet), cursed weapons.
// - `ItemProcessType` logging (`LogItems`) and the GM audit.
// - `Player.setTradeRefusal` is only called by the script `AdminAdmin` (`//tradeoff`), a later card;
//   the flag is [`TradeState::refusal`].

mod exchange;
mod offline;
mod packets;
mod request;
mod store;
mod store_deal;
mod store_lists;
mod window;

pub(crate) use offline::OfflineWorld;
pub use offline::{
    OfflineSettings, OfflineTradeLine, OfflineTradeRow, OfflineTradeStatus, OfflineTradeWrite, OfflineTrader,
    restorable_store_type,
};
pub use store::{PrivateStoreType, StoreInput, StoreSettings};
pub use window::AddItemRules;

use l2_util::ObjectId;

use crate::player::ItemData;

/// Java `Player.REQUEST_TIMEOUT`: seconds a request stays open.
pub const REQUEST_TIMEOUT: i32 = 15;
/// Java `GameTimeTaskManager.TICKS_PER_SECOND`.
pub(crate) const GAME_TICKS_PER_SECOND: i32 = 10;
/// [`GAME_TICKS_PER_SECOND`] as a `long`.
pub(crate) const GAME_TICKS_PER_SECOND_I64: i64 = 10;
/// Java `BotReportTable.TRADE_ACTION_BLOCK_ID`.
pub(crate) const TRADE_ACTION_BLOCK_ID: i32 = -2;
/// `TradeRequest`: the farthest two players may stand apart (`calculateDistance3D(partner) > 150`)
/// to ask, and again when the trade is confirmed.
pub const TRADE_RANGE: f64 = 150.0;

/// One entry of a trade list (Java `TradeItem`): the item as it was when it was added, the count
/// that is offered and, for private stores, the price.
#[derive(Debug, Clone, PartialEq)]
pub struct TradeItem {
    pub object_id: ObjectId,
    /// The inventory row when the item was added (Java copies the type, enchant and element
    /// fields into the `TradeItem`). Its `count` is the inventory count then, not the offer.
    pub item: ItemData,
    /// Java `_count`: how many are offered.
    pub count: i64,
    /// Java `_price`: 0 in a private trade.
    pub price: i64,
}

/// Java `TradeList`: what one side of a trade (or a private store) offers.
#[derive(Debug, Clone, PartialEq)]
pub struct TradeList {
    pub owner: ObjectId,
    /// Java `_partner`.
    pub partner: Option<ObjectId>,
    /// Java keeps a `ConcurrentHashMap` key set of `TradeItem`s, whose order is the identity
    /// hash code, i.e. arbitrary. The insertion order is as good a choice.
    pub items: Vec<TradeItem>,
    /// Java `_title` (private stores).
    pub title: String,
    /// Java `_packaged` (private stores).
    pub packaged: bool,
    /// Java `_confirmed`.
    pub confirmed: bool,
    /// Java `_locked`.
    pub locked: bool,
}

impl TradeList {
    /// Java `new TradeList(owner)`.
    pub fn new(owner: ObjectId) -> Self {
        Self {
            owner,
            partner: None,
            items: Vec::new(),
            title: String::new(),
            packaged: false,
            confirmed: false,
            locked: false,
        }
    }

    /// Java `lock()`: no further changes.
    pub fn lock(&mut self) {
        self.locked = true;
    }

    /// Java `clear()`: the items go and the list is open again.
    pub fn clear(&mut self) {
        self.items.clear();
        self.locked = false;
    }

    /// Java `invalidateConfirmation()`.
    pub fn invalidate_confirmation(&mut self) {
        self.confirmed = false;
    }

    /// Java `getItemCount()`.
    pub fn item_count(&self) -> usize {
        self.items.len()
    }
}

/// The transaction fields of Java's `Player` that trade uses. A party invite, a friend invite or a
/// duel request use `active_requester` and the request time in the same way (Java shares them), so
/// the methods on [`World`](crate::World) in `request.rs` are `pub(crate)` for those cards.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TradeState {
    /// Java `_activeTradeList`.
    pub active: Option<TradeList>,
    /// Java `_activeRequester`: who asked this player.
    pub active_requester: Option<ObjectId>,
    /// The requester's `_requestExpireTime` when it asked. Java reads it from the requester's
    /// `Player` object, which stays reachable after that player left; here the requester's state
    /// is gone with it, so the value read then is kept.
    pub requester_expire_time: i64,
    /// Java `_requestExpireTime` (in `GameTimeTaskManager` ticks; 0 = no request).
    pub request_expire_time: i64,
    /// Java `_tradeRefusal`.
    pub refusal: bool,
    /// Java `_privateStoreType` (private stores, `store.rs`).
    pub store_type: PrivateStoreType,
    /// Java `_sellList`: what a sell store or package sale offers. Java creates it on the first
    /// `getSellList()`; `None` is "not created yet" (see `World::sell_list`).
    pub sell_list: Option<TradeList>,
    /// Java `_buyList`: what a buy store wants.
    pub buy_list: Option<TradeList>,
}
