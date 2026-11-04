//! Offline trade: a player who logs out with a private store open stays in the world without a
//! client, the store keeps selling, and the shops can be written to the database so the next start
//! brings them back (M6-FU-14).
// Java: data/sql/OfflineTraderTable.java (enteredOfflineMode, offlineMode, onTransaction,
//       removeTrader, storeOffliners, restoreOfflineTraders)
// Java: config/custom/OfflineTradeConfig.java
// Java: entity/actor/Player.java (setPrivateStoreType's OFFLINE_DISCONNECT_FINISHED branch)
// Java: entity/actor/status/PlayerStatus.java (OFFLINE_MODE_NO_DAMAGE)
// Java: network/clientpackets/{Logout,RequestRestart,EnterWorld,RequestPrivateStoreBuy,
//       RequestPrivateStoreSell,CharacterSelect}.java, Shutdown.java, GameServer.java
//
// # Model
// A player "without a client" (Java `getClient() == null || isDetached()`) is a player in
// `World::players` that has no entry in `World::sinks`; the rest of the world already reads it so
// (`siege/player.rs`, `pvp.rs`, `effects/buff_info.rs`). [`OfflineWorld::traders`] lists them with
// the time they went offline (`getOfflineStartTime`). The connection is closed with `ServerClose`
// like Java's `client.close(ServerClose.STATIC_PACKET)`. The login server hears `PlayerLogout` at once
// (Java `GameClient.onDisconnection` -> `sendLogout`; the account is free for a login while the trader
// stays). The character is stored with `online = 2` (`Player.isOnlineInt()` of a detached client).
//
// # The database rows
// `character_offline_trade` and `character_offline_trade_items` are written, removed and restored
// as Java's `OfflineTraderTable` does.
//
// # Left out
// - The buff shop (`isSellingBuffs`, type 9) is stored and restored like the other shops, under
//   as the others (M9-L01). The `.offline` command (`EnableOfflineCommand`) and
//   `OfflineAbnormalEffect` are in `custom/offline_trade.rs` (M9-C02);
//   `OfflineDisconnectSameAccount` is `release_offline_same_account` (`custom/offline_play.rs`).
// - `OfflineMaxDays` is checked only when the shops are restored, like Java: a running trader
//   never times out.

use std::collections::HashMap;

use l2_protocol_game::server::ServerClose;
use l2_util::ObjectId;
use tracing::{info, warn};

use super::PrivateStoreType;
use crate::command::LeaveReason;
use crate::objects::ObjectKind;
use crate::outbox::{LsEvent, PersistCommand};
use crate::player::{Player, PlayerData};
use crate::world::{World, unix_ms_now};
use crate::zones::ZoneId;

const MS_PER_DAY: i64 = 86_400_000;

/// `OfflineTrade.ini` (Java `OfflineTradeConfig`). `Default` is the Java default of each key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfflineSettings {
    /// `OFFLINE_TRADE_ENABLE`.
    pub offline_trade_enable: bool,
    /// `OFFLINE_CRAFT_ENABLE`.
    pub offline_craft_enable: bool,
    /// `OFFLINE_MODE_IN_PEACE_ZONE`.
    pub offline_mode_in_peace_zone: bool,
    /// `OFFLINE_MODE_NO_DAMAGE`.
    pub offline_mode_no_damage: bool,
    /// `OFFLINE_SET_NAME_COLOR`.
    pub offline_set_name_color: bool,
    /// `OFFLINE_NAME_COLOR` (`Integer.decode("0x" + OfflineNameColor)`, default 808080).
    pub offline_name_color: i32,
    /// `RESTORE_OFFLINERS`.
    pub restore_offliners: bool,
    /// `OFFLINE_MAX_DAYS`.
    pub offline_max_days: i32,
    /// `OFFLINE_DISCONNECT_FINISHED`.
    pub offline_disconnect_finished: bool,
    /// `STORE_OFFLINE_TRADE_IN_REALTIME`.
    pub store_offline_trade_in_realtime: bool,
}

impl Default for OfflineSettings {
    fn default() -> Self {
        Self {
            offline_trade_enable: false,
            offline_craft_enable: false,
            offline_mode_in_peace_zone: false,
            offline_mode_no_damage: false,
            offline_set_name_color: false,
            offline_name_color: 0x0080_8080,
            restore_offliners: false,
            offline_max_days: 10,
            offline_disconnect_finished: true,
            store_offline_trade_in_realtime: true,
        }
    }
}

impl OfflineSettings {
    /// `GameServer`: "(OFFLINE_TRADE_ENABLE || OFFLINE_CRAFT_ENABLE) && RESTORE_OFFLINERS" ->
    /// `restoreOfflineTraders()`: whether the rows are read at all (the caller asks before
    /// it touches the database).
    pub fn restore_wanted(&self) -> bool {
        (self.offline_trade_enable || self.offline_craft_enable) && self.restore_offliners
    }
}

/// One player in offline mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfflineTrader {
    /// `getOfflineStartTime()`: Unix milliseconds.
    pub start_time_ms: i64,
    /// `getAppearance().setNameColor(OFFLINE_NAME_COLOR)` with `OfflineSetNameColor`.
    pub name_color: Option<i32>,
}

/// What the world keeps for offline trade.
#[derive(Debug, Default)]
pub(crate) struct OfflineWorld {
    pub(crate) settings: OfflineSettings,
    /// The players without a client (`World.OFFLINE_TRADE_COUNT` is its length).
    pub(crate) traders: HashMap<ObjectId, OfflineTrader>,
}

/// One line of `character_offline_trade_items`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfflineTradeLine {
    /// The item id (buy store), the item's object id (sell store) or the recipe id (workshop).
    pub item: i32,
    pub count: i64,
    pub price: i64,
}

/// One row of `character_offline_trade`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfflineTradeStatus {
    /// `getOfflineStartTime()`.
    pub time: i64,
    /// The `PrivateStoreType` id.
    pub store_type: i32,
    pub title: Option<String>,
}

/// A shop: its row and its lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfflineTradeRow {
    pub char_id: ObjectId,
    pub status: OfflineTradeStatus,
    pub items: Vec<OfflineTradeLine>,
}

/// A write to the offline trade tables with Java's statements (`OfflineTraderTable`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OfflineTradeWrite {
    /// `storeOffliners`: both tables are emptied, then every shop is written (its lines, then its
    /// row).
    StoreAll(Vec<OfflineTradeRow>),
    /// `onTransaction(trader, finished, firstCall)`: the lines of the trader are deleted; when
    /// `finished` its row goes too, else the new lines are written and (`status` is `Some` on the
    /// first call) its row.
    Transaction {
        char_id: ObjectId,
        finished: bool,
        items: Vec<OfflineTradeLine>,
        status: Option<OfflineTradeStatus>,
    },
    /// `removeTrader`: the lines and the row of the trader are deleted.
    Remove { char_id: ObjectId },
}

/// The store type of a row `restoreOfflineTraders` brings back, or `None` for a row it skips: older
/// than `OfflineMaxDays` (`Calendar.add(DAY_OF_YEAR)` in the local zone, D-48), a type nobody
/// knows, `NONE`, or the buff shop (not ported).
pub fn restorable_store_type(
    settings: &OfflineSettings,
    time_ms: i64,
    type_id: i32,
    now_ms: i64,
) -> Option<PrivateStoreType> {
    if settings.offline_max_days > 0 && add_days_local(time_ms, settings.offline_max_days) <= now_ms {
        return None;
    }
    let kind = match type_id {
        0 => PrivateStoreType::None,
        1 => PrivateStoreType::Sell,
        2 => PrivateStoreType::SellManage,
        3 => PrivateStoreType::Buy,
        4 => PrivateStoreType::BuyManage,
        5 => PrivateStoreType::Manufacture,
        8 => PrivateStoreType::PackageSell,
        // `SELL_BUFFS`: the buff shop (`SellBuffsManager`) comes back as a package sale whose
        // lines are skills (`isSellBuff ? PACKAGE_SELL : findById(typeId)`).
        9 => PrivateStoreType::PackageSell,
        other => {
            warn!("OfflineTraderTable: PrivateStoreType with id {other} could not be found.");
            return None;
        }
    };
    (kind != PrivateStoreType::None).then_some(kind)
}

/// `Calendar.add(Calendar.DAY_OF_YEAR, days)` in the machine's zone: the same wall-clock time
/// `days` days later.
fn add_days_local(time_ms: i64, days: i32) -> i64 {
    let later = time_ms.saturating_add(i64::from(days).saturating_mul(MS_PER_DAY));
    let before = l2_util::time::local_offset_millis(time_ms);
    let after = l2_util::time::local_offset_millis(later);
    later.saturating_add(before - after)
}

impl World {
    /// Installs `OfflineTrade.ini`. Call before players enter.
    pub fn set_offline_settings(&mut self, settings: OfflineSettings) {
        self.offline.settings = settings;
    }

    /// The offline trade settings.
    pub fn offline_settings(&self) -> &OfflineSettings {
        &self.offline.settings
    }

    /// Whether the player is in offline mode (a trader without a client).
    pub fn is_offline_trader(&self, id: ObjectId) -> bool {
        self.offline.traders.contains_key(&id)
    }

    /// `World.OFFLINE_TRADE_COUNT`.
    pub fn offline_trader_count(&self) -> usize {
        self.offline.traders.len()
    }

    /// The name colour `OfflineSetNameColor` gives a trader.
    pub(crate) fn offline_name_color(&self, id: ObjectId) -> Option<i32> {
        self.offline
            .traders
            .get(&id)
            .and_then(|t| t.name_color)
            // A character that plays offline has its own colour (`OfflinePlay.ini`, M9-C02).
            .or_else(|| self.offline_play_name_color(id))
    }

    /// `(OFFLINE_TRADE_ENABLE || OFFLINE_CRAFT_ENABLE)`.
    fn offline_enabled(&self) -> bool {
        let s = &self.offline.settings;
        s.offline_trade_enable || s.offline_craft_enable
    }

    // ------------------------------------------------------------------ entering

    /// Java `OfflineTraderTable.offlineMode(player)`: may the player stay as an offline shop?
    fn offline_mode_allowed(&self, id: ObjectId) -> bool {
        // `isRegisteredOnEvent()` (scripts) is not modelled; `isInOlympiadMode()`: M8-OLY-01.
        if !self.players.contains_key(&id)
            || self.is_in_olympiad_mode(id)
            || self.is_jailed(id)
            || self.is_in_vehicle(id)
        {
            return false;
        }
        let s = &self.offline.settings;
        let can_set_shop = match self.private_store_type(id) {
            PrivateStoreType::Sell | PrivateStoreType::PackageSell | PrivateStoreType::Buy => {
                s.offline_trade_enable
            }
            // NOTE(java-quirk): a workshop needs `OfflineTradeEnable`, not `OfflineCraftEnable`.
            PrivateStoreType::Manufacture => s.offline_trade_enable,
            _ => s.offline_craft_enable && self.is_crafting(id),
        };
        if s.offline_mode_in_peace_zone && !self.is_inside_zone(id, ZoneId::Peace) {
            return false;
        }
        // "Check whether client is null or player is already in offline mode."
        if !self.sinks.contains_key(&id) {
            return false;
        }
        can_set_shop
    }

    /// Java `OfflineTraderTable.enteredOfflineMode(player)`: `true` when the player stays as an
    /// offline shop (the caller then neither stores nor removes it).
    pub(crate) fn entered_offline_mode(&mut self, id: ObjectId) -> bool {
        if !self.offline_mode_allowed(id) {
            return false;
        }
        // `OfflineTraderTable.enteredOfflineMode`: unless `DualboxCountOfflineTraders`, the trader
        // is not counted against the IP limits any more (M9-C06).
        self.dualbox_on_offline_mode(id);
        // `client.close(ServerClose.STATIC_PACKET)`, `client.setDetached(true)`: the connection
        // goes, the player stays.
        let account = self.players.get(&id).map(|p| p.data.account.clone());
        if let Some(sink) = self.sinks.remove(&id) {
            sink.send(l2_net::Encode::to_bytes(&ServerClose));
            sink.left_world(LeaveReason::Logout);
        }
        if let Some(session) = self.player_sessions.remove(&id) {
            self.sessions.remove(&session);
        }
        let name_color = self
            .offline
            .settings
            .offline_set_name_color
            .then_some(self.offline.settings.offline_name_color);
        // `if (getOfflineStartTime() == 0) setOfflineStartTime(now)`: a new trader.
        self.offline.traders.insert(
            id,
            OfflineTrader {
                start_time_ms: unix_ms_now(),
                name_color,
            },
        );
        self.leave_party(id);

        // "If the Player has Pet, unsummon it."
        if let Some(summon) = self.summon_of(id) {
            if let Some(state) = self.summon_mut(summon) {
                state.restore_summon = true;
            }
            self.unsummon(summon, Some(id));
            // "Dead pet wasn't unsummoned, broadcast npcinfo changes."
            if self.summon_of(id) == Some(summon) {
                self.broadcast_summon_npc_info(summon, 0);
            }
        }

        if name_color.is_some() {
            self.broadcast_user_info(id);
        }

        // "Store trade on exit, if realtime saving is enabled."
        if self.offline.settings.store_offline_trade_in_realtime {
            self.offline_on_transaction(id, false, true);
        }
        // `player.storeMe()`: with `online = 2`.
        if let Some(snapshot) = self.store_snapshot(id) {
            self.outbox
                .persist(PersistCommand::StorePlayer(Box::new(snapshot)));
        }
        // `GameClient.onDisconnection` -> `sendLogout(account)`.
        if let Some(account) = account {
            self.outbox.ls_event(LsEvent::PlayerLogout { account });
        }
        // `OfflineAbnormalEffect` (`OfflineTrade.ini`, M9-C02).
        self.offline_trade_abnormal_effect(id);
        info!("Entered offline mode, {id}.");
        true
    }

    // ------------------------------------------------------------------ the rows

    /// The shop of a trader as `storeOffliners` and `onTransaction` write it: the lines of the
    /// store type it has, and the title. `None` for a type the switch of `storeOffliners` skips
    /// (`continue`) because its enable key is off.
    fn offline_shop(
        &self,
        id: ObjectId,
        for_store_all: bool,
    ) -> Option<(Vec<OfflineTradeLine>, Option<String>)> {
        let s = &self.offline.settings;
        Some(match self.private_store_type(id) {
            PrivateStoreType::Buy => {
                if for_store_all && !s.offline_trade_enable {
                    return None;
                }
                let list = self.buy_list_copy(id);
                let items = list
                    .items
                    .iter()
                    .map(|i| OfflineTradeLine {
                        item: i.item.item_id,
                        count: i.count,
                        price: i.price,
                    })
                    .collect();
                (items, Some(list.title))
            }
            PrivateStoreType::Sell | PrivateStoreType::PackageSell => {
                if for_store_all && !s.offline_trade_enable {
                    return None;
                }
                let list = self.sell_list_copy(id);
                // `isSellingBuffs()`: the lines are the skills of the buff shop, with count 0.
                let items = if self.is_selling_buffs(id) {
                    self.selling_buffs(id)
                        .iter()
                        .map(|b| OfflineTradeLine {
                            item: b.skill_id,
                            count: 0,
                            price: b.price,
                        })
                        .collect()
                } else {
                    list.items
                        .iter()
                        .map(|i| OfflineTradeLine {
                            item: i.object_id.0,
                            count: i.count,
                            price: i.price,
                        })
                        .collect()
                };
                (items, Some(list.title))
            }
            PrivateStoreType::Manufacture => {
                if for_store_all && !s.offline_craft_enable {
                    return None;
                }
                let items = self
                    .manufacture_items(id)
                    .into_iter()
                    .map(|(recipe_id, cost)| OfflineTradeLine {
                        item: recipe_id,
                        count: 0,
                        price: cost,
                    })
                    .collect();
                (items, Some(self.recipe_store_name(id)))
            }
            // A store being set up: no lines, no title.
            _ => (Vec::new(), None),
        })
    }

    /// The `type` column of a trader: `isSellingBuffs() ? SELL_BUFFS : getPrivateStoreType()`.
    fn offline_store_type_id(&self, id: ObjectId) -> i32 {
        if self.is_selling_buffs(id) {
            return i32::from(PrivateStoreType::SellBuffs.id());
        }
        i32::from(self.private_store_type(id).id())
    }

    /// Java `OfflineTraderTable.onTransaction(trader, finished, firstCall)`.
    pub(crate) fn offline_on_transaction(&mut self, id: ObjectId, finished: bool, first_call: bool) {
        let mut items = Vec::new();
        let mut status = None;
        // "Trade is done - clear info." Else the lines are written when the client is detached.
        if !finished && self.is_offline_trader(id) {
            let start = self.offline.traders.get(&id).map_or(0, |t| t.start_time_ms);
            let (lines, title) = self.offline_shop(id, false).unwrap_or_default();
            items = lines;
            if first_call {
                status = Some(OfflineTradeStatus {
                    time: start,
                    store_type: self.offline_store_type_id(id),
                    // Only the first call records the title.
                    title,
                });
            }
        }
        self.outbox
            .persist(PersistCommand::OfflineTrade(OfflineTradeWrite::Transaction {
                char_id: id,
                finished,
                items,
                status,
            }));
    }

    /// The part of `RequestPrivateStoreBuy` / `RequestPrivateStoreSell` after a deal: "Update
    /// offline trade record, if realtime saving is enabled." `remaining` is
    /// `storeList.getItemCount()`.
    pub(crate) fn offline_after_store_deal(&mut self, store: ObjectId, remaining: usize) {
        let s = &self.offline.settings;
        if s.offline_trade_enable && s.store_offline_trade_in_realtime && self.is_offline_trader(store) {
            self.offline_on_transaction(store, remaining == 0, false);
        }
    }

    /// Java `OfflineTraderTable.removeTrader(objectId)`.
    fn offline_remove_trader(&mut self, id: ObjectId) {
        self.outbox
            .persist(PersistCommand::OfflineTrade(OfflineTradeWrite::Remove {
                char_id: id,
            }));
    }

    /// `CharacterSelect`: "Disconnect offline trader." The account is in use by the new session,
    /// so the login server is not told.
    pub(crate) fn release_offline_trader_quietly(&mut self, id: ObjectId) -> bool {
        if !self.is_offline_trader(id) && !self.is_offline_play(id) {
            return false;
        }
        // `RESTORE_AUTO_PLAY_OFFLINERS && player.isAutoPlaying()` -> `removeOfflinePlay`, and what
        // the `OFFLINE_PLAY` limit of `CharacterSelect` needs from the player (M9-L01).
        self.offline_play_release_rows(id);
        self.offline_play_remember_release(id);
        self.leave(id, LeaveReason::Disconnected);
        true
    }

    /// `Player.setPrivateStoreType`'s `OFFLINE_DISCONNECT_FINISHED` branch: a trader whose store
    /// closed is removed from the table, stored and deleted (`Disconnection.of(this)
    /// .storeAndDelete()`).
    pub(crate) fn offline_store_closed(&mut self, id: ObjectId) {
        if self.offline.settings.offline_disconnect_finished && self.is_offline_trader(id) {
            self.offline_remove_trader(id);
            self.leave(id, LeaveReason::Disconnected);
        }
    }

    /// Java `EnterWorld`: "(OFFLINE_TRADE_ENABLE || OFFLINE_CRAFT_ENABLE) &&
    /// STORE_OFFLINE_TRADE_IN_REALTIME" -> `onTransaction(player, true, false)`: the character is
    /// back, its old shop rows go.
    pub(crate) fn offline_on_enter_world(&mut self, id: ObjectId) {
        if self.offline_enabled() && self.offline.settings.store_offline_trade_in_realtime {
            self.offline_on_transaction(id, true, false);
        }
    }

    /// `Shutdown.startShutdownActions`: "(trade || craft) && RESTORE_OFFLINERS &&
    /// !STORE_OFFLINE_TRADE_IN_REALTIME" -> `storeOffliners()`.
    pub(crate) fn offline_store_offliners(&mut self) {
        let s = &self.offline.settings;
        if !(self.offline_enabled() && s.restore_offliners && !s.store_offline_trade_in_realtime) {
            return;
        }
        let mut ids: Vec<ObjectId> = self
            .players
            .keys()
            .copied()
            .filter(|id| self.is_offline_trader(*id) && self.is_in_store_mode(*id))
            .collect();
        ids.sort();
        let mut rows = Vec::new();
        for id in ids {
            // The switch of `storeOffliners`: a type whose key is off is skipped.
            let Some((items, title)) = self.offline_shop(id, true) else {
                continue;
            };
            let start = self.offline.traders.get(&id).map_or(0, |t| t.start_time_ms);
            rows.push(OfflineTradeRow {
                char_id: id,
                status: OfflineTradeStatus {
                    time: start,
                    store_type: self.offline_store_type_id(id),
                    title,
                },
                items,
            });
        }
        info!("OfflineTraderTable: Offline traders stored ({}).", rows.len());
        self.outbox
            .persist(PersistCommand::OfflineTrade(OfflineTradeWrite::StoreAll(rows)));
    }

    // ------------------------------------------------------------------ damage

    /// `PlayerStatus.reduceHp`: "If OFFLINE_MODE_NO_DAMAGE is enabled and player is offline and he
    /// is in store/craft mode, no damage is taken."
    pub(crate) fn offline_takes_no_damage(&self, id: ObjectId) -> bool {
        let s = &self.offline.settings;
        if !s.offline_mode_no_damage || !self.is_offline_trader(id) {
            return false;
        }
        let store = self.private_store_type(id);
        // NOTE(java-quirk): a package sale is not in the list, a workshop needs `OfflineCraftEnable`
        // here (and `OfflineTradeEnable` to be allowed to stay).
        (s.offline_trade_enable && matches!(store, PrivateStoreType::Sell | PrivateStoreType::Buy))
            || (s.offline_craft_enable && (self.is_crafting(id) || store == PrivateStoreType::Manufacture))
    }

    // ------------------------------------------------------------------ restoring

    /// Java `OfflineTraderTable.restoreOfflineTraders`, once the characters are loaded: each
    /// player enters without a client, sits down with its shop open.
    pub fn restore_offline_traders(&mut self, traders: Vec<(PlayerData, OfflineTradeRow)>) {
        let mut restored = 0;
        for (data, row) in traders {
            let kind = match row.status.store_type {
                1 => PrivateStoreType::Sell,
                2 => PrivateStoreType::SellManage,
                3 => PrivateStoreType::Buy,
                4 => PrivateStoreType::BuyManage,
                5 => PrivateStoreType::Manufacture,
                8 => PrivateStoreType::PackageSell,
                // `SELL_BUFFS`: a package sale of the buff shop.
                9 => PrivateStoreType::PackageSell,
                _ => continue,
            };
            if self.restore_offline_trader(data, row, kind) {
                restored += 1;
            }
        }
        info!("OfflineTraderTable: Loaded {restored} offline traders.");
        // `if (!STORE_OFFLINE_TRADE_IN_REALTIME)` the tables are emptied: the shops live in memory
        // now and `storeOffliners` writes them at shutdown.
        if !self.offline.settings.store_offline_trade_in_realtime {
            self.outbox
                .persist(PersistCommand::OfflineTrade(OfflineTradeWrite::StoreAll(
                    Vec::new(),
                )));
        }
    }

    /// The state `World::enter` and `on_entered` build for a player, without the connection, the
    /// packets and the login server (a restored trader never told the login server it is in
    /// game: the account is free).
    pub(crate) fn enter_without_client(&mut self, data: PlayerData) {
        let id = data.object_id;
        self.forget_status(id);
        let mut player = Player::from_data(data, self.tick);
        player.entered_at_unix_ms = unix_ms_now();
        player.entered_world = true;
        self.players.insert(id, player);
        self.clean_loaded_warehouses(id);
        self.restore_craft(id);
        self.init_player_stats(id);
        self.restore_player_vitals(id);
        self.take_saved_summon_effects(id);
        self.load_social_lists(id);
        self.register_object(id, ObjectKind::Player);
        self.schedule_autosave(id);
        self.clan_restore_player(id);
        self.mail_remember_player(id);
        self.restore_item_reuse(id);
        // `player.spawnMe(x, y, z)`.
        if let Some((x, y, z)) = self.position_of(id) {
            self.spawn_at(id, x, y, z);
        }
    }

    fn restore_offline_trader(
        &mut self,
        data: PlayerData,
        row: OfflineTradeRow,
        kind: PrivateStoreType,
    ) -> bool {
        let id = data.object_id;
        if self.players.contains_key(&id) {
            warn!("OfflineTraderTable: {id} is already in the world; not restored.");
            return false;
        }
        let name = data.name.clone();
        self.enter_without_client(data);
        let max_adena = self.settings.ground.max_adena;
        match kind {
            PrivateStoreType::Buy => {
                let templates = self.item_templates();
                for line in &row.items {
                    let template = templates.as_ref().and_then(|t| t.get(line.item));
                    if let Some(list) = self.buy_list_mut(id) {
                        list.add_item_by_item_id(
                            template, &name, line.item, line.count, line.price, max_adena,
                        );
                    }
                }
                if let Some(list) = self.buy_list_mut(id) {
                    list.title = row.status.title.clone().unwrap_or_default();
                }
            }
            PrivateStoreType::Sell | PrivateStoreType::PackageSell => {
                if row.status.store_type == i32::from(PrivateStoreType::SellBuffs.id()) {
                    // The buff shop: the lines are skills and prices.
                    let holders = row
                        .items
                        .iter()
                        .map(|l| crate::custom::sell_buffs::SellBuffHolder {
                            skill_id: l.item,
                            price: l.price,
                        })
                        .collect();
                    self.sell_buffs_restore(id, holders);
                } else {
                    for line in &row.items {
                        self.sell_list_add_item(id, line.item, line.count, line.price);
                    }
                }
                if let Some(list) = self.sell_list_mut(id) {
                    list.title = row.status.title.clone().unwrap_or_default();
                    list.packaged = kind == PrivateStoreType::PackageSell;
                }
            }
            PrivateStoreType::Manufacture => {
                for line in &row.items {
                    self.put_manufacture_item(id, line.item, line.price);
                }
                if let Some(p) = self.players.get_mut(&id) {
                    p.craft.store_name = row.status.title.clone().unwrap_or_default();
                }
            }
            _ => {}
        }
        self.sit_down(id, true);
        let name_color = self
            .offline
            .settings
            .offline_set_name_color
            .then_some(self.offline.settings.offline_name_color);
        self.offline.traders.insert(
            id,
            OfflineTrader {
                start_time_ms: row.status.time,
                name_color,
            },
        );
        self.set_private_store_type(id, kind);
        self.restore_saved_effects(id);
        self.broadcast_user_info(id);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_javas() {
        let s = OfflineSettings::default();
        assert!(!s.offline_trade_enable && !s.offline_craft_enable);
        assert_eq!(s.offline_name_color, 0x808080);
        assert_eq!(s.offline_max_days, 10);
        assert!(s.offline_disconnect_finished && s.store_offline_trade_in_realtime);
    }

    #[test]
    fn a_row_is_restored_until_the_max_days_are_over() {
        let s = OfflineSettings::default();
        let now = 1_700_000_000_000;
        let day = MS_PER_DAY;
        // Nine days old: restored. Eleven days: skipped.
        assert_eq!(
            restorable_store_type(&s, now - 9 * day, 1, now),
            Some(PrivateStoreType::Sell)
        );
        assert_eq!(restorable_store_type(&s, now - 11 * day, 1, now), None);
        // `OfflineMaxDays = 0`: no limit.
        let unlimited = OfflineSettings {
            offline_max_days: 0,
            ..OfflineSettings::default()
        };
        assert_eq!(
            restorable_store_type(&unlimited, now - 400 * day, 3, now),
            Some(PrivateStoreType::Buy)
        );
    }

    #[test]
    fn the_store_types_of_the_rows() {
        let s = OfflineSettings::default();
        let now = 1_700_000_000_000;
        let kind = |id| restorable_store_type(&s, now, id, now);
        assert_eq!(kind(5), Some(PrivateStoreType::Manufacture));
        assert_eq!(kind(8), Some(PrivateStoreType::PackageSell));
        // The buff shop comes back as a package sale (M9-L01); NONE and unknown ids are skipped.
        assert_eq!(kind(9), Some(PrivateStoreType::PackageSell));
        assert_eq!(kind(0), None);
        assert_eq!(kind(77), None);
    }
}
