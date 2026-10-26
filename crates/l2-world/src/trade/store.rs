//! Private stores (sit-down sell, buy and package sell): the store state of a player, opening and
//! closing a store, the titles, the lists the owner sets, and the checks other systems make with
//! `isInStoreMode` (M6-STORE-01).
// Java: network/clientpackets/{RequestActionUse,RequestPrivateStoreManageSell,
//       RequestPrivateStoreManageBuy,RequestPrivateStoreQuitSell,RequestPrivateStoreQuitBuy,
//       SetPrivateStoreMsgSell,SetPrivateStoreMsgBuy,SetPrivateStoreWholeMsg,
//       SetPrivateStoreListSell,SetPrivateStoreListBuy}.java
// Java: entity/actor/Player.java (canOpenPrivateStore, tryOpenPrivateBuyStore,
//       tryOpenPrivateSellStore, setPrivateStoreType, getSellList, getBuyList, doInteract,
//       getMinShopDistance)
// Java: entity/actor/enums/player/PrivateStoreType.java
//
// # How it fits together
// - This file: the state, opening and closing a store, the titles and the two `SetPrivateStoreList*`
//   handlers (the owner confirms its list).
// - `store_lists.rs`: the packets of the lists (`PrivateStoreManageList*`, `PrivateStoreList*`),
//   `TradeList.adjustAvailableItem` / `updateItems`, `Player.doInteract`.
// - `store_deal.rs`: `TradeList.privateStoreBuy` / `privateStoreSell` and the two customer
//   packets (`RequestPrivateStoreBuy`, `RequestPrivateStoreSell`).
//
// A sell list and a buy list are [`TradeList`]s kept in [`TradeState`](super::TradeState), like
// Java's `_sellList` / `_buyList`. A sell list holds the real inventory items (object ids), a buy
// list holds item templates (object id 0) until a customer's inventory is matched against it.
//
// # Left out on purpose (each has a comment where it belongs)
// - Offline trade (`OfflineTradeEnable`, `OfflineTraderTable`, `setPrivateStoreType`'s
//   `OFFLINE_DISCONNECT_FINISHED` branch, the realtime `onTransaction` record): M6-STORE-01 leaves
//   it, there are no detached clients in the Rust server yet.
// - The cursed weapon check, `isSellingBuffs`
//   (`SellBuffsManager`), the olympiad (M8) and a mount.

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::client::{BuyListEntry, SellListEntry, StoreBuyRequestEntry, StoreSellRequestEntry};
use l2_protocol_game::server::{
    ActionFailed, ExPrivateStoreSetWholeMsg, PrivateStoreMsgBuy, PrivateStoreMsgSell, SystemMessage,
};
use l2_util::ObjectId;
use tracing::warn;

use super::{TradeItem, TradeList};
use crate::player::ItemData;
use crate::punishments::IllegalActionPunishment;
use crate::world::World;
use crate::zones::ZoneId;

/// Java `PrivateStoreType`: what a player offers (the id is what `UserInfo` / `CharInfo` write).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PrivateStoreType {
    #[default]
    None,
    Sell,
    SellManage,
    Buy,
    BuyManage,
    Manufacture,
    PackageSell,
    SellBuffs,
}

impl PrivateStoreType {
    /// Java `getId()`.
    pub fn id(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Sell => 1,
            Self::SellManage => 2,
            Self::Buy => 3,
            Self::BuyManage => 4,
            Self::Manufacture => 5,
            Self::PackageSell => 8,
            Self::SellBuffs => 9,
        }
    }
}

/// The values of the configuration the stores read. `Default` is the Java default of each key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreSettings {
    /// `PrivateStoreRangeConfig.SHOP_MIN_RANGE_FROM_PLAYER` (`ShopMinRangeFromPlayer`): a store may
    /// not be opened this close to a sitting player (Java: any player who sits).
    pub shop_min_range_from_player: i32,
    /// `PrivateStoreRangeConfig.SHOP_MIN_RANGE_FROM_NPC` (`ShopMinRangeFromNpc`).
    pub shop_min_range_from_npc: i32,
    /// `PlayerConfig.MAX_ITEM_IN_PACKET`: the largest inventory limit. A store packet with more
    /// items is a bad packet.
    pub max_item_in_packet: i32,
}

impl Default for StoreSettings {
    fn default() -> Self {
        Self {
            shop_min_range_from_player: 50,
            shop_min_range_from_npc: 100,
            // max(MaximumSlotsForNoDwarf 80, MaximumSlotsForDwarf 100, MaximumSlotsForGMPlayer 250)
            max_item_in_packet: 250,
        }
    }
}

/// The in-game packets of the private stores, as the session task decoded them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreInput {
    /// Java `RequestActionUse` for the three store actions (10 sell, 28 buy, 61 package sell). The
    /// packet is `RequestActionUse` in Java; here only the store part is routed to the world.
    ActionUse { action_id: i32 },
    /// Java `RequestPrivateStoreManageSell` (Java only checks that the player is alive).
    ManageSell,
    /// Java `RequestPrivateStoreManageBuy`.
    ManageBuy,
    /// Java `RequestPrivateStoreQuitSell`.
    QuitSell,
    /// Java `RequestPrivateStoreQuitBuy`.
    QuitBuy,
    /// Java `SetPrivateStoreMsgSell`.
    MsgSell { msg: String },
    /// Java `SetPrivateStoreMsgBuy`.
    MsgBuy { msg: String },
    /// Java `SetPrivateStoreWholeMsg` (the title of a package sale).
    WholeMsg { msg: String },
    /// Java `SetPrivateStoreListSell`; `items` is `None` for Java's bad list.
    ListSell {
        package_sale: bool,
        items: Option<Vec<SellListEntry>>,
    },
    /// Java `SetPrivateStoreListBuy`.
    ListBuy { items: Option<Vec<BuyListEntry>> },
    /// Java `RequestPrivateStoreBuy`: a customer buys from a sell store or package sale.
    Buy {
        store_player_id: i32,
        items: Option<Vec<StoreBuyRequestEntry>>,
    },
    /// Java `RequestPrivateStoreSell`: a customer sells to a buy store.
    Sell {
        store_player_id: i32,
        items: Option<Vec<StoreSellRequestEntry>>,
    },
}

/// `RequestActionUse` ids of the store actions.
const ACTION_PRIVATE_STORE_SELL: i32 = 10;
const ACTION_PRIVATE_STORE_BUY: i32 = 28;
const ACTION_PRIVATE_STORE_PACKAGE_SELL: i32 = 61;

/// The longest title the three `SetPrivateStore*Msg` packets accept (`MAX_MSG_LENGTH`).
const MAX_MSG_LENGTH: usize = 29;

/// Java `Math` on the price check of `addToTradeList`: `(MAX_ADENA / count) < price`.
fn price_overflows(max_adena: i64, count: i64, price: i64) -> bool {
    // `count >= 1` (the packet drops anything else).
    max_adena.checked_div(count).is_none_or(|limit| limit < price)
}

impl TradeList {
    /// Java `addItemByItemId(itemId, count, price)`: a template goes onto a buy list (object id 0).
    /// `None` (Java returns null) when the list is locked, the item is unknown, quest or not
    /// tradeable, or the price overflows.
    pub fn add_item_by_item_id(
        &mut self,
        template: Option<&l2_data::item::ItemTemplate>,
        owner_name: &str,
        item_id: i32,
        count: i64,
        price: i64,
        max_adena: i64,
    ) -> Option<TradeItem> {
        if self.locked {
            warn!("{owner_name}: Attempt to modify locked TradeList!");
            return None;
        }
        let Some(template) = template else {
            warn!("{owner_name}: Attempt to add invalid item to TradeList!");
            return None;
        };
        if !template.tradeable || template.quest_item {
            return None;
        }
        if !template.stackable && count > 1 {
            warn!("{owner_name}: Attempt to add non-stackable item to TradeList with count > 1!");
            return None;
        }
        if price_overflows(max_adena, count, price) {
            warn!("{owner_name}: Attempt to overflow adena !");
            return None;
        }
        // `new TradeItem(template, count, price)`: object id 0, nothing enchanted.
        let entry = TradeItem {
            object_id: ObjectId(0),
            item: ItemData {
                item_id,
                count,
                ..ItemData::default()
            },
            count,
            price,
        };
        self.items.push(entry.clone());
        // If Player has already confirmed this trade, invalidate the confirmation.
        self.invalidate_confirmation();
        Some(entry)
    }

    /// Java `removeItem(objectId, itemId, count)` of a list that has no partner (a store list):
    /// the first entry with that object id or that item id loses `count` (all of it for -1, or
    /// when `count` is not less than what it has). Returns the entry as it was.
    pub(crate) fn remove_item(&mut self, object_id: i32, item_id: i32, count: i64) -> Option<TradeItem> {
        if self.locked {
            warn!("Attempt to modify locked TradeList!");
            return None;
        }
        let index = self
            .items
            .iter()
            .position(|t| t.object_id.0 == object_id || t.item.item_id == item_id)?;
        // NOTE(java-quirk): Java also invalidates the partner's confirmation here; a store list has
        // no partner.
        let titem = self.items[index].clone();
        // Reduce item count or complete item.
        if count != -1 && titem.count > count {
            self.items[index].count = titem.count - count;
        } else {
            self.items.remove(index);
        }
        Some(titem)
    }
}

impl World {
    // ------------------------------------------------------------------ the store state

    /// Java `Player.getPrivateStoreType()`.
    pub fn private_store_type(&self, id: ObjectId) -> PrivateStoreType {
        self.players
            .get(&id)
            .map_or(PrivateStoreType::None, |p| p.trade.store_type)
    }

    /// Java `Player.isInStoreMode()`: any type but `NONE` (a store being set up counts).
    pub fn is_in_store_mode(&self, id: ObjectId) -> bool {
        self.private_store_type(id) != PrivateStoreType::None
    }

    /// Java `Player.setPrivateStoreType(type)`.
    // Not modelled: with `OfflineDisconnectFinished` a detached client whose store closes is
    // stored and removed (`OfflineTraderTable`); there are no offline traders (offline trade is a
    // later card).
    pub(crate) fn set_private_store_type(&mut self, id: ObjectId, kind: PrivateStoreType) {
        if let Some(p) = self.players.get_mut(&id) {
            p.trade.store_type = kind;
        }
    }

    /// Java `Player.getSellList()`: created on first use.
    pub(crate) fn sell_list_mut(&mut self, id: ObjectId) -> Option<&mut TradeList> {
        let p = self.players.get_mut(&id)?;
        Some(p.trade.sell_list.get_or_insert_with(|| TradeList::new(id)))
    }

    /// Java `Player.getBuyList()`: created on first use.
    pub(crate) fn buy_list_mut(&mut self, id: ObjectId) -> Option<&mut TradeList> {
        let p = self.players.get_mut(&id)?;
        Some(p.trade.buy_list.get_or_insert_with(|| TradeList::new(id)))
    }

    /// A copy of `getSellList()` (an empty list when none was made yet).
    pub(crate) fn sell_list_copy(&self, id: ObjectId) -> TradeList {
        self.players
            .get(&id)
            .and_then(|p| p.trade.sell_list.clone())
            .unwrap_or_else(|| TradeList::new(id))
    }

    /// A copy of `getBuyList()` (an empty list when none was made yet).
    pub(crate) fn buy_list_copy(&self, id: ObjectId) -> TradeList {
        self.players
            .get(&id)
            .and_then(|p| p.trade.buy_list.clone())
            .unwrap_or_else(|| TradeList::new(id))
    }

    // ------------------------------------------------------------------ limits

    /// Java `Player.getPrivateSellStoreLimit()`: `MaxPvtStoreSell*` plus `P_SELL_LIM`.
    pub(crate) fn private_sell_store_limit(&self, id: ObjectId) -> i64 {
        let Some(p) = self.players.get(&id) else {
            return 0;
        };
        let base = if p.data.race == crate::data::RACE_DWARF {
            self.settings.max_pvtstoresell_slots_dwarf
        } else {
            self.settings.max_pvtstoresell_slots_other
        };
        let bonus = self
            .stats(id)
            .map_or(0, |v| v.calc(l2_data::stats::Stat::PSellLim, 0.0) as i32);
        i64::from(base) + i64::from(bonus)
    }

    /// Java `Player.getPrivateBuyStoreLimit()`: `MaxPvtStoreBuy*` plus `P_BUY_LIM`.
    pub(crate) fn private_buy_store_limit(&self, id: ObjectId) -> i64 {
        let Some(p) = self.players.get(&id) else {
            return 0;
        };
        let base = if p.data.race == crate::data::RACE_DWARF {
            self.settings.max_pvtstorebuy_slots_dwarf
        } else {
            self.settings.max_pvtstorebuy_slots_other
        };
        let bonus = self
            .stats(id)
            .map_or(0, |v| v.calc(l2_data::stats::Stat::PBuyLim, 0.0) as i32);
        i64::from(base) + i64::from(bonus)
    }

    // ------------------------------------------------------------------ opening a store

    /// Java `Player.getMinShopDistance()` of any creature: a sitting player and an NPC keep
    /// stores away.
    fn min_shop_distance(&self, id: ObjectId) -> i32 {
        if self.players.contains_key(&id) {
            return if self.is_sitting(id) {
                self.settings.store.shop_min_range_from_player
            } else {
                0
            };
        }
        if self.npcs.contains_key(&id) {
            return self.settings.store.shop_min_range_from_npc;
        }
        0
    }

    /// Java `LocationUtil.checkIfInRange(range, this, creature, true)`: the 3D distance within
    /// `range` plus both collision radii, same instance.
    pub(crate) fn in_shop_range(&self, id: ObjectId, other: ObjectId, range: i32) -> bool {
        let instance = |o: ObjectId| self.objects.get(&o).map_or(0, |e| e.instance_id);
        if instance(id) != instance(other) {
            return false;
        }
        let combined = self.collision_radius_of(id) + self.collision_radius_of(other);
        self.distance_3d(id, other)
            .is_some_and(|d| d <= f64::from(range + combined))
    }

    /// Java `Player.canOpenPrivateStore()`.
    pub(crate) fn can_open_private_store(&self, id: ObjectId) -> bool {
        let store = &self.settings.store;
        if store.shop_min_range_from_npc > 0 || store.shop_min_range_from_player > 0 {
            // `World.getFirstVisibleObject(this, Creature.class, ...)`: any creature that is too
            // close, so the order of the search does not matter.
            let too_close = self.visible_objects(id).into_iter().any(|other| {
                self.objects.get(&other).is_some_and(|e| e.kind.is_creature()) && {
                    let min = self.min_shop_distance(other);
                    min > 0 && self.in_shop_range(id, other, min)
                }
            });
            if too_close {
                self.send(
                    id,
                    &SystemMessage::new(SystemMessageId::YouCannotOpenAPrivateStoreHere),
                );
                return false;
            }
        }
        // `!_inOlympiadMode` (M8) holds for everybody.
        !self.is_alike_dead(id)
            && !self.is_mounted(id)
            && !self.is_inside_zone(id, ZoneId::NoStore)
            && !self.is_casting_now(id)
    }

    /// Java `Player.tryOpenPrivateBuyStore()`.
    pub(crate) fn try_open_private_buy_store(&mut self, id: ObjectId) {
        // Player shouldn't be able to set stores if he/she is alike dead (dead or fake death).
        if self.can_open_private_store(id) {
            if matches!(
                self.private_store_type(id),
                PrivateStoreType::Buy | PrivateStoreType::BuyManage
            ) {
                self.set_private_store_type(id, PrivateStoreType::None);
            }
            if self.private_store_type(id) == PrivateStoreType::None {
                if self.is_sitting(id) {
                    self.stand_up(id);
                }
                self.set_private_store_type(id, PrivateStoreType::BuyManage);
                if let Some(packet) = self.private_store_manage_list_buy(id) {
                    self.send(id, &packet);
                }
            }
        } else {
            if self.is_inside_zone(id, ZoneId::NoStore) {
                self.send(
                    id,
                    &SystemMessage::new(SystemMessageId::YouCannotOpenAPrivateStoreHere),
                );
            }
            self.send(id, &ActionFailed);
        }
    }

    /// Java `Player.tryOpenPrivateSellStore(isPackageSale)`.
    pub(crate) fn try_open_private_sell_store(&mut self, id: ObjectId, is_package_sale: bool) {
        // Player shouldn't be able to set stores if he/she is alike dead (dead or fake death).
        if self.can_open_private_store(id) {
            if matches!(
                self.private_store_type(id),
                PrivateStoreType::Sell | PrivateStoreType::SellManage | PrivateStoreType::PackageSell
            ) {
                self.set_private_store_type(id, PrivateStoreType::None);
            }
            if self.private_store_type(id) == PrivateStoreType::None {
                if self.is_sitting(id) {
                    self.stand_up(id);
                }
                self.set_private_store_type(id, PrivateStoreType::SellManage);
                if let Some(packet) = self.private_store_manage_list_sell(id, is_package_sale) {
                    self.send(id, &packet);
                }
            }
        } else {
            if self.private_store_type(id) != PrivateStoreType::None && !self.is_alike_dead(id) {
                self.set_private_store_type(id, PrivateStoreType::None);
                if self.is_sitting(id) {
                    self.stand_up(id);
                }
                return;
            }
            if self.is_inside_zone(id, ZoneId::NoStore) {
                self.send(
                    id,
                    &SystemMessage::new(SystemMessageId::YouCannotOpenAPrivateStoreHere),
                );
            }
            self.send(id, &ActionFailed);
        }
    }

    // ------------------------------------------------------------------ the dispatcher

    /// Routes one private store packet.
    pub(crate) fn on_store_input(&mut self, id: ObjectId, input: StoreInput) {
        if !self.players.contains_key(&id) {
            return;
        }
        match input {
            StoreInput::ActionUse { action_id } => self.on_store_action_use(id, action_id),
            StoreInput::ManageSell => self.on_request_private_store_manage_sell(id),
            StoreInput::ManageBuy => self.on_request_private_store_manage_buy(id),
            StoreInput::QuitSell | StoreInput::QuitBuy => self.on_request_private_store_quit(id),
            StoreInput::MsgSell { msg } => self.on_set_private_store_msg_sell(id, &msg),
            StoreInput::MsgBuy { msg } => self.on_set_private_store_msg_buy(id, &msg),
            StoreInput::WholeMsg { msg } => self.on_set_private_store_whole_msg(id, &msg),
            StoreInput::ListSell { package_sale, items } => {
                self.on_set_private_store_list_sell(id, package_sale, items);
            }
            StoreInput::ListBuy { items } => self.on_set_private_store_list_buy(id, items),
            StoreInput::Buy {
                store_player_id,
                items,
            } => self.on_request_private_store_buy(id, ObjectId(store_player_id), items),
            StoreInput::Sell {
                store_player_id,
                items,
            } => self.on_request_private_store_sell(id, ObjectId(store_player_id), items),
        }
    }

    /// The part of Java `RequestActionUse.runImpl` that comes before the `switch`, then the three
    /// store actions.
    fn on_store_action_use(&mut self, id: ObjectId, action_id: i32) {
        // Don't do anything if player is dead or confused.
        if (self.is_fake_death(id) && action_id != 0) || self.is_dead(id) || self.is_out_of_control(id) {
            self.send(id, &ActionFailed);
            return;
        }
        if !self.check_bot_penalty(id, action_id) {
            return;
        }
        // Don't allow to do some action if player is transformed.
        if !self.transformed_action_allowed(id, action_id) {
            self.send(id, &ActionFailed);
            warn!(
                "{} used action which he does not have! Id = {action_id} transform: {}",
                self.player_text(id),
                self.transformation_id(id)
            );
            return;
        }
        match action_id {
            // Private Store - Sell
            ACTION_PRIVATE_STORE_SELL => self.try_open_private_sell_store(id, false),
            // Private Store - Buy
            ACTION_PRIVATE_STORE_BUY => self.try_open_private_buy_store(id),
            // Private Store Package Sell
            ACTION_PRIVATE_STORE_PACKAGE_SELL => self.try_open_private_sell_store(id, true),
            _ => {}
        }
    }

    /// Java `RequestPrivateStoreManageSell.runImpl`.
    // NOTE(java-quirk): Java only refuses a dead player here ("TODO: implement me properly"); the
    // manage window itself comes from the `RequestActionUse` actions.
    fn on_request_private_store_manage_sell(&mut self, id: ObjectId) {
        // Player shouldn't be able to set stores if he/she is alike dead (dead or fake death), or
        // in olympiad (M8).
        if self.is_alike_dead(id) {
            self.send(id, &ActionFailed);
        }
    }

    /// Java `RequestPrivateStoreManageBuy.runImpl`.
    fn on_request_private_store_manage_buy(&mut self, id: ObjectId) {
        // Player shouldn't be able to set stores if he/she is alike dead (dead or fake death), or
        // in olympiad (M8).
        if self.is_alike_dead(id) {
            return;
        }
        self.try_open_private_buy_store(id);
    }

    /// Java `RequestPrivateStoreQuitSell.runImpl` and `RequestPrivateStoreQuitBuy.runImpl` (the
    /// same code).
    fn on_request_private_store_quit(&mut self, id: ObjectId) {
        self.set_private_store_type(id, PrivateStoreType::None);
        self.stand_up(id);
        self.broadcast_user_info(id);
    }

    // ------------------------------------------------------------------ titles

    /// `PunishmentManager.handleIllegalPlayerAction(player, text, DEFAULT_PUNISH)`.
    pub(crate) fn store_illegal_action(&mut self, id: ObjectId, text: &str) {
        let punishment: IllegalActionPunishment = self.default_punish();
        self.handle_illegal_player_action(id, text, punishment);
    }

    /// `_storeMsg.length() > MAX_MSG_LENGTH` (Java counts UTF-16 units).
    fn title_too_long(msg: &str) -> bool {
        msg.encode_utf16().count() > MAX_MSG_LENGTH
    }

    /// Java `SetPrivateStoreMsgSell.runImpl`.
    fn on_set_private_store_msg_sell(&mut self, id: ObjectId, msg: &str) {
        if Self::title_too_long(msg) {
            let text = format!(
                "{} tried to overflow private store sell message",
                self.player_text(id)
            );
            self.store_illegal_action(id, &text);
            return;
        }
        if let Some(list) = self.sell_list_mut(id) {
            list.title = msg.to_string();
        }
        let title = self.sell_list_copy(id).title;
        self.send(
            id,
            &PrivateStoreMsgSell {
                object_id: id.0,
                message: title,
            },
        );
    }

    /// Java `SetPrivateStoreMsgBuy.runImpl`.
    fn on_set_private_store_msg_buy(&mut self, id: ObjectId, msg: &str) {
        if Self::title_too_long(msg) {
            let text = format!(
                "{} tried to overflow private store buy message",
                self.player_text(id)
            );
            self.store_illegal_action(id, &text);
            return;
        }
        if let Some(list) = self.buy_list_mut(id) {
            list.title = msg.to_string();
        }
        let title = self.buy_list_copy(id).title;
        self.send(
            id,
            &PrivateStoreMsgBuy {
                object_id: id.0,
                message: title,
            },
        );
    }

    /// Java `SetPrivateStoreWholeMsg.runImpl` (the title of a package sale is the sell list's).
    fn on_set_private_store_whole_msg(&mut self, id: ObjectId, msg: &str) {
        if Self::title_too_long(msg) {
            let text = format!(
                "{} tried to overflow private store whole message",
                self.player_text(id)
            );
            self.store_illegal_action(id, &text);
            return;
        }
        if let Some(list) = self.sell_list_mut(id) {
            list.title = msg.to_string();
        }
        // NOTE(java-quirk): `new ExPrivateStoreSetWholeMsg(player)` reads the title back from the
        // sell list, which is what was just set.
        let title = self.sell_list_copy(id).title;
        self.send(
            id,
            &ExPrivateStoreSetWholeMsg {
                object_id: id.0,
                message: title,
            },
        );
    }

    // ------------------------------------------------------------------ the owner's lists

    /// Java `TradeList.addItem(objectId, count, price)` on the sell list of `owner`: a refused
    /// item is `None` (Java returns null and `SetPrivateStoreListSell` ignores it).
    fn sell_list_add_item(
        &mut self,
        owner: ObjectId,
        object_id: i32,
        count: i64,
        price: i64,
    ) -> Option<TradeItem> {
        let object_id = ObjectId(object_id);
        let item = self.inventory_item(owner, object_id)?.clone();
        let templates = self.item_templates()?;
        let template = templates.get(item.item_id)?;
        let tradeable = self.item_is_tradeable(&item, template)
            || (self.is_gm(owner) && self.settings.inventory.gm_trade_restricted_items);
        // `_owner.getInventory().canManipulateWithItemId(item.getId())`.
        if !self.can_manipulate_with_item_id(owner, item.item_id) {
            return None;
        }
        let rules = super::AddItemRules {
            tradeable,
            max_adena: self.settings.ground.max_adena,
        };
        let list = self.sell_list_mut(owner)?;
        list.add_item(&item, template, count, price, rules)
    }

    /// Java `SetPrivateStoreListSell.runImpl`.
    fn on_set_private_store_list_sell(
        &mut self,
        id: ObjectId,
        package_sale: bool,
        items: Option<Vec<SellListEntry>>,
    ) {
        // Java drops a list with more than `MAX_ITEM_IN_PACKET` items already when it reads it.
        let max = usize::try_from(self.settings.store.max_item_in_packet).unwrap_or(0);
        let items = items.filter(|list| list.len() <= max);
        let Some(items) = items else {
            self.send(id, &SystemMessage::new(SystemMessageId::IncorrectItemCount));
            self.set_private_store_type(id, PrivateStoreType::None);
            self.broadcast_user_info(id);
            return;
        };

        if !self.allows_transaction(id) {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouAreNotAuthorizedToDoThat),
            );
            return;
        }

        if self.has_attack_stance_task(id) || self.is_in_duel(id) {
            self.send(
                id,
                &SystemMessage::new(
                    SystemMessageId::WhileYouAreEngagedInCombatYouCannotOperateAPrivateStoreOrPrivateWorkshop,
                ),
            );
            self.send_manage_list_sell(id, package_sale);
            self.send(id, &ActionFailed);
            return;
        }

        if self.is_inside_zone(id, ZoneId::NoStore) {
            self.send_manage_list_sell(id, package_sale);
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouCannotOpenAPrivateStoreHere),
            );
            self.send(id, &ActionFailed);
            return;
        }

        if !self.can_open_private_store(id) {
            self.send_manage_list_sell(id, package_sale);
            self.send(id, &ActionFailed);
            return;
        }

        // Check maximum number of allowed slots for pvt shops.
        if i64::try_from(items.len()).unwrap_or(i64::MAX) > self.private_sell_store_limit(id) {
            self.send_manage_list_sell(id, package_sale);
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouHaveExceededTheQuantityThatCanBeInputted),
            );
            return;
        }

        if let Some(list) = self.sell_list_mut(id) {
            list.clear();
            list.packaged = package_sale;
        }

        let max_adena = self.settings.ground.max_adena;
        let mut total_cost = self.adena_of(id);
        for entry in &items {
            // `Item.addToTradeList`: `(MAX_ADENA / _count) < _price` is checked before the item is
            // added.
            if price_overflows(max_adena, entry.count, entry.price) {
                self.send(
                    id,
                    &SystemMessage::new(SystemMessageId::YouHaveExceededYourOutOfPocketAdenaLimit),
                );
                return;
            }
            // NOTE(java-quirk): the result of `list.addItem` is ignored, so an item the list
            // refuses (not tradeable, not owned, a count that is too high...) is skipped quietly.
            self.sell_list_add_item(id, entry.item_id, entry.count, entry.price);

            // Java's `getPrice()` of the entry is `count * price`.
            total_cost = total_cost.wrapping_add(entry.count.wrapping_mul(entry.price));
            if total_cost > max_adena {
                self.send(
                    id,
                    &SystemMessage::new(SystemMessageId::YouHaveExceededYourOutOfPocketAdenaLimit),
                );
                return;
            }
        }

        self.sit_down(id, true);
        self.set_private_store_type(
            id,
            if package_sale {
                PrivateStoreType::PackageSell
            } else {
                PrivateStoreType::Sell
            },
        );
        self.broadcast_user_info(id);

        let title = self.sell_list_copy(id).title;
        if package_sale {
            self.broadcast_creature_packet(
                id,
                &ExPrivateStoreSetWholeMsg {
                    object_id: id.0,
                    message: title,
                },
            );
        } else {
            self.broadcast_creature_packet(
                id,
                &PrivateStoreMsgSell {
                    object_id: id.0,
                    message: title,
                },
            );
        }
    }

    /// Java `SetPrivateStoreListBuy.runImpl`.
    fn on_set_private_store_list_buy(&mut self, id: ObjectId, items: Option<Vec<BuyListEntry>>) {
        let max = usize::try_from(self.settings.store.max_item_in_packet).unwrap_or(0);
        let items = items.filter(|list| list.len() <= max);
        let Some(items) = items else {
            self.set_private_store_type(id, PrivateStoreType::None);
            self.broadcast_user_info(id);
            return;
        };

        if !self.allows_transaction(id) {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouAreNotAuthorizedToDoThat),
            );
            return;
        }

        if self.has_attack_stance_task(id) || self.is_in_duel(id) {
            self.send(
                id,
                &SystemMessage::new(
                    SystemMessageId::WhileYouAreEngagedInCombatYouCannotOperateAPrivateStoreOrPrivateWorkshop,
                ),
            );
            self.send_manage_list_buy(id);
            self.send(id, &ActionFailed);
            return;
        }

        if self.is_inside_zone(id, ZoneId::NoStore) {
            self.send_manage_list_buy(id);
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouCannotOpenAPrivateStoreHere),
            );
            self.send(id, &ActionFailed);
            return;
        }

        if !self.can_open_private_store(id) {
            self.send_manage_list_buy(id);
            self.send(id, &ActionFailed);
            return;
        }

        if let Some(list) = self.buy_list_mut(id) {
            list.clear();
        }

        // Check maximum number of allowed slots for pvt shops.
        if i64::try_from(items.len()).unwrap_or(i64::MAX) > self.private_buy_store_limit(id) {
            self.send_manage_list_buy(id);
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouHaveExceededTheQuantityThatCanBeInputted),
            );
            return;
        }

        let max_adena = self.settings.ground.max_adena;
        let name = self
            .players
            .get(&id)
            .map(|p| p.data.name.clone())
            .unwrap_or_default();
        let account = self.account_of(id);
        let templates = self.item_templates();
        let mut total_cost: i64 = 0;
        for entry in &items {
            // `Item.addToTradeList`.
            if price_overflows(max_adena, entry.count, entry.price) {
                let text = format!(
                    "Warning!! Character {name} of account {account} tried to set price more than {max_adena} adena in Private Store - Buy."
                );
                self.store_illegal_action(id, &text);
                return;
            }
            // NOTE(java-quirk): the result of `addItemByItemId` is ignored (see the sell list).
            let template = templates.as_ref().and_then(|t| t.get(entry.item_id));
            if let Some(list) = self.buy_list_mut(id) {
                list.add_item_by_item_id(
                    template,
                    &name,
                    entry.item_id,
                    entry.count,
                    entry.price,
                    max_adena,
                );
            }

            total_cost = total_cost.wrapping_add(entry.count.wrapping_mul(entry.price));
            if total_cost > max_adena {
                let text = format!(
                    "Warning!! Character {name} of account {account} tried to set total price more than {max_adena} adena in Private Store - Buy."
                );
                self.store_illegal_action(id, &text);
                return;
            }
        }

        // Check for available funds.
        if total_cost > self.adena_of(id) {
            self.send_manage_list_buy(id);
            self.send(
                id,
                &SystemMessage::new(
                    SystemMessageId::ThePurchasePriceIsHigherThanTheAmountOfMoneyThatYouHaveAndSoYouCannotOpenAPersonalStore,
                ),
            );
            return;
        }

        self.sit_down(id, true);
        self.set_private_store_type(id, PrivateStoreType::Buy);
        self.broadcast_user_info(id);
        let title = self.buy_list_copy(id).title;
        self.broadcast_creature_packet(
            id,
            &PrivateStoreMsgBuy {
                object_id: id.0,
                message: title,
            },
        );
    }

    /// `player.sendPacket(new PrivateStoreManageListSell(player, isPackageSale))`.
    fn send_manage_list_sell(&mut self, id: ObjectId, package_sale: bool) {
        if let Some(packet) = self.private_store_manage_list_sell(id, package_sale) {
            self.send(id, &packet);
        }
    }

    /// `player.sendPacket(new PrivateStoreManageListBuy(player))`.
    fn send_manage_list_buy(&mut self, id: ObjectId) {
        if let Some(packet) = self.private_store_manage_list_buy(id) {
            self.send(id, &packet);
        }
    }
}
